---
status: todo
created: 2026-10-29
size: S
mode: Pixel
design: [pincel.md §9.3, pincel.md §17.5]
---

# Pixel wasm npm API

## Context

`crates/pincel-wasm` compiles `pincel-core` to WebAssembly. It is published to npm as `@amigo-labs/pincel-wasm` (wasm-pack, `--target web`). It is the only way the Pixel editor reaches the core (UI imports go through `ui/src/lib/core/`). It is also the surface embedders get (pincel.md §17.5).

The contract in §9.3:

- State stays in Rust. JS holds an opaque `Document` handle.
- JS asks for rendered frames and byte buffers.
- JS pulls change notifications with `drainEvents()` once per animation frame.

This spec records the cross-cutting contract of that package as built on 2026-10-29: document lifecycle, compose and export calls, the event queue, and how errors cross the boundary. Feature-specific methods (tools, layers, palette, timeline, tilemaps, slices, selection, effects, text, transforms, import) belong to their feature specs.

## Scope

### In scope

- Creating a document, opening it from and saving it to `.aseprite` bytes
- `compose`, `composeDirty` and the `ComposeFrame` result; `exportPng` as the single-frame byte export
- The event queue: `drainEvents`, event kinds and their fields, bounding
- Undo and redo entry points (`undo`, `redo`, `endStroke`, `undoDepth`, `redoDepth`) as API calls
- Error behavior: thrown exceptions, lookups by unknown id or out-of-range index, no side effects when a call fails
- The package's name, module format and top-level exports

### Out of scope

- The high-level `@amigo-labs/pincel` package (`Pincel.create`, `mount`, `on('change' | 'save')`) from §17.5 — because it does not exist yet
- What individual edit methods do (`applyTool`, `addLayer`, `setPaletteColor`, `addSlice`, …) — because the drawing-tools, layers-frames, palette, tilemaps, slices and selection specs own them
- `exportAtlas` and the sheet JSON — because the `pixel-export` spec owns them
- `pincel-image-wasm` (Image mode) — because it is a separate, handle-based package with its own generated types
- Which edits merge into one undo entry — because `pixel-undo-history` owns it

## Acceptance criteria

- [ ] AC-01 (event): When `new Document(width, height)` is called with non-zero sizes, it returns a document that is ready to paint:
  - the given canvas size
  - one layer and one frame
  - a fully transparent composed frame
  - the 32-color DB32 palette
  - an empty undo and redo history
  - no queued events
- [ ] AC-02 (error): If `new Document` is called with a width or height of 0, it throws.
- [ ] AC-03 (event): When `Document.openAseprite(bytes)` is called with a valid file, it returns a new document with the file's canvas size, layers and frames, and an empty undo and redo history.
- [ ] AC-04 (error): If `Document.openAseprite` is called with bytes that are not a supported Aseprite file, it throws with a non-empty message.
- [ ] AC-05 (event): When `saveAseprite()` is called, it returns the bytes of an Aseprite file that `Document.openAseprite` reads back with the same pixels.
- [ ] AC-06 (event): When `compose(frame, zoom)` is called, it returns a frame whose width and height are the canvas size times `zoom` and whose `pixels` hold 4 bytes (RGBA) per output pixel.
- [ ] AC-07 (error): If `compose` is called with a frame index that does not exist, or with a zoom of 0 or above 64, it throws.
- [ ] AC-08 (event): When `composeDirty(frame, zoom, x, y, width, height)` is called, it returns only the part of that rect that lies on the canvas, with `dirtyX` / `dirtyY` giving its top-left corner in sprite coordinates. A rect that covers the whole canvas gives the same pixels as `compose`. A rect fully off the canvas gives an empty frame.
- [ ] AC-09 (always): Every read of `ComposeFrame.pixels`, of the bytes from `saveAseprite` / `exportPng`, and of `AtlasExport.png` returns a new copy. Changing that array in JS does not change the document or later reads.
- [ ] AC-10 (event): When `exportPng(frame)` is called, it returns PNG-encoded bytes of that frame at 1:1 scale. An unknown frame index throws.
- [ ] AC-11 (event): When `drainEvents()` is called, it returns every queued event in the order the events were queued, and the next call returns only events queued after that.
- [ ] AC-12 (state): While nobody calls `drainEvents()`, the queue stays bounded: once it is full, each new event drops the oldest one.
- [ ] AC-13 (event): When a pixel edit succeeds, a `dirty-rect` event is queued. It carries the layer id, the frame index and the bounding rect of the change in sprite coordinates.
- [ ] AC-14 (event): When an edit to the document's structure succeeds (for example moving a layer, adding a slice, editing a tile's pixels or switching the current frame), a `dirty-canvas` event is queued. Its numeric fields carry no meaning.
- [ ] AC-15 (event): When `undo()` or `redo()` reverts or re-applies a pixel edit, it returns `true`, updates `undoDepth` / `redoDepth`, and queues one `dirty-rect` for that edit's region. When there is nothing to undo or redo, it returns `false` and queues nothing.
- [ ] AC-16 (event): When several `applyTool` calls are made between two `endStroke()` calls, they form one undo step.
- [ ] AC-17 (event): When the selection is set or cleared, a `selection-changed` event carrying the new rect is queued, with all four values 0 for an empty selection. The event is queued even when the selection did not change.
- [ ] AC-18 (error): If a call fails, it throws a JS exception with a message, queues no event, and leaves the document unchanged.
- [ ] AC-19 (always): Getters that take a layer, tileset or slice id return a neutral value (empty string, 0 or false) for an unknown id instead of throwing. Getters that take a position in a list (layer, palette entry, slice key) throw when the position is out of range.
- [ ] AC-20 (always): The package is published as `@amigo-labs/pincel-wasm` as an ES module. Its default export initializes the wasm module. It exports the classes `Document`, `ComposeFrame`, `AtlasExport` and `Event`, and the functions `palettePresetNames`, `blendModeNames`, `effectNames` and `registerFont`.

## Technical notes

Hints that drift; check the code before relying on them.

- **Usage:**

  ```javascript
  import init, { Document } from '@amigo-labs/pincel-wasm';
  await init();
  const doc = new Document(64, 64);          // or Document.openAseprite(bytes)
  doc.applyTool('pencil', x, y, 0xff0000ff); // 0xRRGGBBAA
  doc.endStroke();                           // on pointer-up
  const frame = doc.compose(0, 1);           // frame index, zoom 1..=64
  const bytes = doc.saveAseprite();
  doc.free();
  ```

- **`Document`** (`crates/pincel-wasm/src/lib.rs`) owns `sprite: Sprite`, `cels: CelMap`, `bus: Bus`, `events: EventQueue`, `active_layer`, `current_frame` and `onion_skin`. It is split across `lib.rs` and the submodules `layer_ops`, `timeline`, `selection_ops`, `shapes`, `transforms`, `effects`, `text` and `import`.
  - `new Document(w, h)` seeds layer `"Layer 1"` (id 0), one 100 ms frame, a transparent full-canvas cel and `Palette::db32()`.
  - `openAseprite` and `new` both start with `current_frame = 0` and an empty `Bus`.
- **Compose:** `compose(frame: u32, zoom: u32) -> Result<ComposeFrame, String>` and `composeDirty(frame, zoom, dx: i32, dy: i32, dw: u32, dh: u32)` call `pincel_core::compose` with `ComposeRequest::full`. That means the full canvas, the `Visible` layer filter, no overlays, and the onion skin set with `setOnionSkin`.
  - `ComposeFrame` exposes `width`, `height`, `dirtyX`, `dirtyY` and `pixels` (row-major, non-premultiplied RGBA8, a fresh `Uint8Array` copy on each read).
- **Export:** `exportPng(frame) -> Box<[u8]>`. `exportAtlas(columns, tag?, imageName?) -> AtlasExport` with the getters `png`, `manifest` (JSON) and `data` (Aseprite `--data` JSON).
- **Events** (`src/events.rs`): `Event` has the getters `kind`, `layer`, `frame`, `x`, `y`, `width` and `height`. Fields that do not apply to a kind are 0.
  - The kinds are `"dirty-rect"`, `"dirty-canvas"` and `"selection-changed"`. New kinds are only ever appended.
  - The queue holds `DEFAULT_EVENT_CAP = 1024` events and drops the oldest when full.
  - Undo and redo turn the command's `DirtyRegion` into an event through `Event::from_dirty`.
  - Palette edits queue no event; the UI re-reads the palette after each edit.
- **Errors:** every fallible method returns `Result<_, String>`, which wasm-bindgen turns into a thrown JS exception. The message text comes from `pincel-core` `Display` impls with a prefix such as `failed to open Aseprite:`, and the code says it is not part of the public contract. `JsError` is avoided so the API can be tested on the host target.
- **Free functions:** `palettePresetNames()`, `blendModeNames()`, `effectNames()` and `registerFont(data) -> u32`.
- **Build:** `wasm-pack build --target web --release --scope amigo-labs` (the `pnpm wasm:build` script in `ui/`). The output goes to `crates/pincel-wasm/pkg/`, which is gitignored. The crate type is `["cdylib", "rlib"]`; the `rlib` is what lets host-side `cargo test` run.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `composeDirty` rect that sticks out past the canvas | Clipped to the canvas; `dirtyX` / `dirtyY` are the clipped corner |
| `composeDirty` rect entirely off the canvas | Empty frame: width 0, height 0, empty `pixels` |
| `clearSelection()` with no selection | Still queues `selection-changed` with all zeros |
| `undo()` on an empty history | Returns `false`, queues no event |
| Painting while the active layer is a group or tilemap, or is unknown | The paint goes to the lowest image layer; with no image layer the call throws |
| Over 1024 undrained events (for example in a background tab) | Only the newest 1024 are kept |
| `layerName(id)` / `layerKind(id)` for an unknown id | Returns `""` instead of throwing |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel_wasm::tests::new_bootstraps_one_layer_one_frame`, `…::fresh_document_seeds_the_db32_palette`, `…::compose_fresh_document_yields_transparent_canvas`, `…::drain_events_is_empty_on_a_fresh_document` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-02 | `pincel_wasm::tests::new_rejects_zero_width`, `…::new_rejects_zero_height` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-03 | `pincel_wasm::tests::save_then_open_roundtrips_a_fresh_document` (crates/pincel-wasm/src/lib.rs), `paint_save_open_roundtrip::paint_save_open_roundtrip_preserves_undo_target_state` (crates/pincel-wasm/tests/paint_save_open_roundtrip.rs), `pincel_wasm::tests::set_current_frame_routes_painting_to_that_frame` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-04 | `pincel_wasm::tests::open_aseprite_rejects_garbage_bytes` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-05 | `paint_save_open_roundtrip::paint_save_open_roundtrip_preserves_pixels` (crates/pincel-wasm/tests/paint_save_open_roundtrip.rs) | built |        |
| AC-06 | `pincel_wasm::tests::compose_honors_integer_zoom`, `…::compose_fresh_document_yields_transparent_canvas` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-07 | `pincel_wasm::tests::compose_rejects_unknown_frame`, `…::compose_rejects_zoom_zero`, `…::compose_rejects_zoom_above_max` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-08 | `pincel_wasm::tests::compose_dirty_returns_sub_rect_at_sprite_coords`, `…::compose_dirty_disjoint_from_viewport_returns_empty_frame`, `…::compose_dirty_full_viewport_matches_compose` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-09 | `buffer_copies::compose_frame_pixels_mutating_a_read_leaves_later_reads_unchanged`, `…::save_aseprite_mutating_the_bytes_leaves_the_document_unchanged`, `…::export_png_mutating_the_bytes_leaves_the_document_unchanged`, `…::atlas_export_png_mutating_a_read_leaves_later_reads_unchanged` (crates/pincel-wasm/tests/buffer_copies.rs), `@amigo-labs/pincel-wasm package › byte reads are fresh copies that JS can change without effect` (ui/e2e/wasm-npm-api.spec.ts) | built |        |
| AC-10 | `pincel_wasm::tests::export_png_returns_bytes_with_png_signature`, `…::export_png_matches_core_export`, `…::export_png_rejects_unknown_frame` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-11 | `pincel_wasm::events::tests::queue_drains_in_fifo_order` (crates/pincel-wasm/src/events.rs) | built |        |
| AC-12 | `pincel_wasm::events::tests::queue_drops_oldest_when_capped` (crates/pincel-wasm/src/events.rs) | built |        |
| AC-13 | `pincel_wasm::tests::apply_tool_pencil_emits_dirty_rect_for_painted_pixel`, `…::apply_line_emits_bounding_box_dirty_rect` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-14 | `pincel_wasm::tests::move_layer_up_reorders_and_emits_event`, `…::add_slice_emits_dirty_canvas_event`, `…::set_tile_pixel_writes_and_emits_dirty_canvas` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-15 | `pincel_wasm::tests::undo_redo_emit_dirty_rect_for_paint_command_and_track_depth`, `…::undo_on_empty_stack_returns_false_and_emits_nothing`, `…::redo_on_empty_stack_returns_false_and_emits_nothing` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-16 | `pincel_wasm::tests::apply_tool_drag_then_end_stroke_is_single_undo_entry`, `…::two_strokes_are_two_undo_entries` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-17 | `pincel_wasm::tests::set_selection_emits_selection_changed_with_new_bounds`, `…::set_selection_with_empty_rect_clears_and_emits_zeros`, `…::clear_selection_emits_event_even_when_no_prior_selection` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-18 | `pincel_wasm::tests::apply_tool_failure_does_not_emit_an_event`, `…::move_layer_at_edge_errors`, `…::set_current_frame_rejects_out_of_range` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-19 | `pincel_wasm::tests::layer_kind_distinguishes_image_tilemap_unknown`, `…::tileset_getters_return_defaults_for_unknown_id`, `…::slice_getters_return_defaults_for_unknown_ids`, `…::palette_getters_error_out_of_range`, `…::layer_id_at_out_of_range_index_returns_err` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-20 | `@amigo-labs/pincel-wasm package › is published under the scoped name as an ES module`, `…› default export initializes the wasm module`, `…› exports the documented classes and functions` (ui/e2e/wasm-npm-api.spec.ts) | built |        |

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering). The spec records the copy semantics actually built (AC-09), not the zero-copy view that §9.3 and the package README describe.
- 2026-10-29: The AC-09 and AC-20 tests were green on their first run because the behavior already exists. To show they can fail, the AC-09 JS test was run against a generated `pixels` getter patched to return one shared buffer, and it failed (`expect(received).not.toBe(expected)`).
- 2026-10-29: AC-09 and AC-20 are tested from Node in a Playwright spec (`ui/e2e/wasm-npm-api.spec.ts`), with no browser page. The spec imports the package by its name through the `ui/` link and passes the wasm bytes to the default export. Playwright is the only JS test runner in the gates, and adding Vitest would mean a new dependency.
- 2026-10-29: The package README now says `ComposeFrame.pixels` is a fresh copy, instead of the zero-copy view it described before. The README ships in the npm package, and AC-09 records the copy semantics.
