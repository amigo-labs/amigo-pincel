# Status

_Last updated: 2026-05-12_

**Branch:** `claude/continue-from-status-hrpIO` · M9.4 complete
(wasm-bindgen slice surface, Slices Panel sidebar, Slice toolbar
tool with marching-ants overlay, 9-patch + pivot editing handles).
M9 milestone done end-to-end.

## Next task

**M10** — PWA polish: service worker, IndexedDB autosave, recent
files registry, File System Access API + download fallback. See
CLAUDE.md §4 / spec §7. Slice user-data overlay color and per-tile
dirty events are still listed under "Open questions" and remain
deferred to the milestones that need them (User Data carrier
alongside §8.3 chunks; per-tile dirty rect alongside M12).

## Milestone status

| ID | Status | Scope |
|----|--------|-------|
| M1 | ✅ | `pincel-core` skeleton — Sprite / Layer / Frame / Cel / Palette types, SpriteBuilder |
| M2 | ✅ | Commands + linear undo bus (cap 100) — SetPixel, AddLayer, AddFrame |
| M3 | ✅ | `compose()` for image layers — RGBA, source-over, viewport+zoom |
| M4 | ✅ | `aseprite-loader` read adapter (RGBA only) |
| M5 | ✅ | `aseprite-writer` crate + write adapter (header / layer / palette / tags / cel) |
| M6 | ✅ | `pincel-wasm` + minimal Svelte UI (open / Pencil / save). M6.7 human cross-validation deferred. |
| M7 | ✅ | Tools — Eraser, Eyedropper, Line, Rect, Rect-Fill, Ellipse, Ellipse-Fill, Bucket, Move (pan + selection-content drag), Selection (Rect) + marching-ants overlay |
| M8.1–M8.6 | ✅ | Tilemap pipeline below the UI — core accessors, compose path (rotate→flip_x→flip_y), AddTileset / PlaceTile commands, aseprite_read + aseprite_write tileset+tilemap, wasm tileset surface |
| M8.7 | ✅ | UI: Tileset Panel + Tilemap Stamp tool + Tileset Editor sub-mode — split into M8.7a–d below |
| M9 | ✅ | Slice support — split into M9.1–M9.4 below |
| M10 | ⬜ | PWA polish |
| M11 | ⬜ | Tauri build |
| M12 | ⬜ | Performance pass |

### M8.7 sub-tasks

- [x] **M8.7a** — Tileset Panel + "Add Tileset" form. No new wasm.
- [x] **M8.7b** — Per-tile thumbnails. New wasm `tilePixels(tilesetId, tileId) -> Uint8Array` painted into small Canvas2D tiles via the `TileThumbnail` component.
- [x] **M8.7c** — `addTilemapLayer` + `placeTile` wired through a new `Stamp` toolbar tool. Topmost matching tilemap layer is auto-picked as the active target; grid + cell overlay paint on hover.
- [x] **M8.7d** — `setTilePixel` wasm + new `TileEditor` modal that double-click opens from a tile thumbnail. Direct pixel painting routes through the undo bus.

Auto-tile mode (paint-on-tilemap = auto reuse / create tiles) stays Phase 2 per spec §5.3 / §13.2.

### M9 sub-tasks

- [x] **M9.1** — `aseprite-writer` gains `SliceChunk` / `SliceKey` / `NinePatch` / `Pivot` types and a `0x2022` chunk encoder. Three new `WriteError` variants cover empty keys, non-monotonic frames, and per-key flag inconsistencies. Loader round-trip test covers plain + 9-patch-with-pivot slices, including negative pivot DWORD encoding.
- [x] **M9.2** — `pincel-core::codec` round-trips slices. `aseprite_write` translates `sprite.slices` → `SliceChunk`s (dropping editor-only `SliceId` + overlay color, which Aseprite stores out-of-band); `aseprite_read` re-uses the existing `parse_raw_file` pass — extended to recover both `Chunk::Tileset` and `Chunk::Slice` — and assigns sequential `SliceId`s by appearance order, defaulting colors to white. Integration test `slices_round_trip_plain_and_nine_patch_with_pivot` covers a plain slice and a 9-patch + pivot slice with a negative pivot key.
- [x] **M9.3** — `AddSlice` / `RemoveSlice` / `SetSliceKey` commands routed through the undo bus with apply / revert tests. Four new `CommandError` variants (`DuplicateSliceId`, `UnknownSlice`, `EmptySliceKeys`, `EmptySliceBounds`) cover the validation surface. `SetSliceKey` upserts into the sorted-by-frame keys vector, tracking "replaced" vs "inserted" so `revert` restores the prior key or removes the new slot. 19 unit tests cover happy path, error branches, and apply / revert / apply round-trips.
- [x] **M9.4** — wasm bindings + UI. New `pincel-wasm` surface: `addSlice` (auto-id like `addTileset`, single frame-0 key, `0xRRGGBBAA` overlay color, routes through `AddSlice`), `removeSlice`, `setSliceKey` (upsert at any frame, `Option<i32>` / `Option<u32>` for the center quartet and pivot pair, partial-quartet rejection), and 14 read getters covering slice enumeration, key enumeration, bounds, center, and pivot. New `SlicesPanel.svelte` sidebar mounted to the right of `TilesetPanel.svelte` reads the surface through a `rev` change counter, owns the "+ Add" form, renders per-slice color swatch + name + remove, exposes bounds inputs and toggleable 9-patch / pivot fieldsets. New `slice` toolbar tool reuses the press / drag / release shape pipeline: drag commits via `addSlice` (no active slice) or `setSliceKey` preserving center / pivot (active slice). Marching-ants overlay reused for the active slice's frame-0 bounds; 9-patch center rendered as a static accent rect in the slice's editor color; pivot rendered as a 3×3 black-on-white crosshair. `paintRectOutline` + `paintPivotCrosshair` are new exports in `lib/render/canvas2d.ts`. 16 new wasm tests cover the round-trip, undo, validation, and getter-defaults surface.

## Recent work

- **2026-05-12 — M9.4 (this branch).** New `pincel-wasm::Document` slice surface: `addSlice` / `removeSlice` / `setSliceKey` route through the M9.3 commands plus a `slice_key(&self, slice_id, key_index)` helper backing 14 read getters (`sliceCount`, `sliceIdAt`, `sliceName`, `sliceColor`, `sliceKeyCount`, and per-field key getters for bounds / center / pivot). `setSliceKey` accepts `Option<i32>` / `Option<u32>` for the center quartet and pivot pair, mapping `None` to "no center" / "no pivot" on the wasm side and rejecting partial sets so JS never silently drops fields. 16 new unit tests cover happy path, monotonic id assignment, undo round-trip, sorted-frame insertion, center / pivot round-trip, partial-quartet rejection, unknown-id rejection, empty-bounds rejection, and the unknown-id getter defaults. New `SlicesPanel.svelte` (~360 lines) mounts as a second sidebar; props match the M8.7a `TilesetPanel` shape (`doc`, `rev`, `activeSliceId`, `onChange`, `onActivate`). Each row owns a color swatch + name (both clickable to activate), an "×" remove button, X / Y / W / H number inputs, and toggleable 9-patch / pivot fieldsets with cX/cY/cW/cH and pX/pY inputs. `App.svelte` grows a `slice` `Tool` variant joined to the existing drag-shape pipeline; release commits via `addSlice` (no active slice) or `setSliceKey` preserving center / pivot (active slice). New `paintActiveSliceOverlay` paints the active slice's frame-0 marching ants, plus `paintRectOutline` for the 9-patch center (in the slice's editor color) and `paintPivotCrosshair` for the pivot. `reconcileActiveSlice` clears the local `activeSliceId` after an undo / redo strips the referenced slice. Marching-ants animation now also pulses while `activeSliceId !== null` so the overlay reads as live. UI gates green: `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`.
- **2026-05-12 — M9.3 (prior branch).** Three new commands in `pincel-core::command`: `AddSlice` appends a slice (rejecting duplicate ids, empty key vectors, and per-key empty bounds rects); `RemoveSlice` drops a slice by id and records its prior index so `revert` re-inserts at the same position; `SetSliceKey` upserts a key on a slice and uses `partition_point` to keep `Slice::keys` sorted by `frame` ascending. `SetSliceKey::revert` distinguishes the "replaced" and "inserted" cases via a private `PriorSlot` enum so it either restores the prior key or removes the new slot. Four new `CommandError` variants surface duplicate / unknown ids and the two emptiness rejections. `AnyCommand` and the `lib.rs` re-exports cover the new commands; the bus dispatch arms route them in the existing M8 pattern.
- **2026-05-12 — M9.2.** `pincel-core::aseprite_write` now translates `sprite.slices` → `aseprite_writer::SliceChunk`s; the per-slice `SliceId` and overlay `color` are editor-only and dropped on write (Aseprite stores neither in the slice chunk itself). `pincel-core::aseprite_read` extends the existing `parse_raw_file` pass (previously named `extract_tilesets`, now `extract_tilesets_and_slices`) to also collect `Chunk::Slice` entries, hydrating them into `Slice` with sequential `SliceId`s and white default color. New integration test `slices_round_trip_plain_and_nine_patch_with_pivot` round-trips a plain slice and a 9-patch + pivot slice (negative pivot included) end-to-end through the codec pair.
- **2026-05-12 — M9.1.** New `SliceChunk` / `SliceKey` / `NinePatch` / `Pivot` types in `aseprite-writer::file`. `AseFile` grows a mandatory `slices: Vec<SliceChunk>`. `write::write` emits a `0x2022` chunk per slice into frame 0 (after the existing layer / palette / tags / tileset chunks). `validate_slice` derives the chunk-level `NINE_PATCH` / `PIVOT` flag word from the keys and rejects empty key vecs, non-monotonic frame ordering, and keys that disagree about which optional fields they carry — three new `WriteError` variants cover those. Tests round-trip a plain slice and a 9-patch+pivot slice (including a negative pivot DWORD) through `aseprite-loader`'s raw chunk parser. `pincel-core::aseprite_write` carries a `slices: Vec::new()` stub for now; M9.2 wires the real slice translation.
- **2026-05-12 — M8.7c + M8.7d.** New `SetTilePixel` command in `pincel-core` writes a single RGBA pixel into `Tileset::tiles[tile_id].pixels` and joins the undo bus. Wasm surface gains `addTilemapLayer(name, tilesetId)` (creates the layer + tilemap cels sized to `ceil(canvas / tile_size)` for every existing frame), `setTilePixel(tilesetId, tileId, x, y, color)`, `addTile(tilesetId)`, and layer-enumeration getters (`layerIdAt`, `layerName`, `layerKind`, `layerTilesetId`). UI: TilesetPanel grows `+ Tile` / `+ Layer` buttons and clickable thumbnails (single click selects stamp target + auto-switches to the Stamp tool, double click opens the Tile Editor). App.svelte adds a `Stamp` toolbar tool with a grid + cell hover overlay drawn after the recompose blit. New `TileEditor.svelte` modal renders the active tile at 16× zoom and routes pointer paint through `setTilePixel`. 8 new wasm tests cover happy path + undo round-trip + error branches.
- **2026-05-12 — M8.7b.** New wasm method `Document::tile_pixels(tileset_id, tile_id) -> Vec<u8>` (JS `tilePixels`) returns non-premultiplied RGBA8 in row-major order. New `ui/src/lib/components/TileThumbnail.svelte` paints each tile to a Canvas2D with `image-rendering: pixelated` and a 2rem display size. `TilesetPanel` iterates `0..tileCount` for each tileset and propagates the existing `rev` change counter so undo / redo / open repaint the thumbnails. Errors when `tileset_id` is unknown, `tile_id` is past the stored tile range, or the tile is non-RGBA (indexed is Phase 2).
- **2026-05-12 — M8.7a.** `ui/src/lib/components/TilesetPanel.svelte` mounted as right-side sidebar in `App.svelte`. Reads via the M8.6 wasm surface; writes via `addTileset(name, tile_w, tile_h)`. Inline validation + wasm error surfacing. Reactivity over opaque wasm getters via a `tilesetRev` `$state` counter bumped on `newDoc` / `openFile` / `undo` / `redo` / `onChange`. Tile-size number inputs use `step="1"` + `inputmode="numeric"`. PR-27 Copilot review addressed in commit `4884f7a`.
- **2026-05-11 — M8.1–M8.6.** End-to-end tilemap pipeline below the UI. See commits `9c0a6cc` (wasm), `8f9f3ed` + `e4549ea` (write path), `c05a31b` + `d58197e` (read path), and the M8.1–M8.3 commits in `git log` for per-step detail.
- **Earlier 2026-05 — M7.1–M7.8c.** Tools expansion, end with the Selection (Rect) tool + marching-ants overlay. Move tool ships both viewport pan (M7.7a) and selection-content drag (M7.7b).
- **Earlier 2026-05 — M6.** wasm crate + Svelte 5 + Vite + open / paint / save MVP.
- **Earlier 2026-05 — M1–M5.** Core types, command bus, compose, codec read+write.

Full prose history for each milestone lives in `git log` (the prior 1647-line `STATUS.md` is preserved in the commits up to and including `4884f7a`).

## Build status

All gates green on this branch:

- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm install`, `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`

`cargo fmt --all --check` has pre-existing drift in `crates/pincel-wasm/src/lib.rs` — to clean up in a standalone fmt-only commit (out of scope for the current slice per CLAUDE.md §9).

## Website (Cloudflare Workers Builds)

Marketing site (`website/`) deploys via Cloudflare Workers Builds Git integration (project `amigo-pincel`). `wrangler.toml` at repo root runs `pnpm install --frozen-lockfile && pnpm build` inside `website/` and serves `website/build` with `not_found_handling = "404-page"`. SEO URLs derive from `website/src/lib/config.ts::siteUrl`.

Per-page compressed payload ~10 KB HTML + ~57 KB shared `_app` (well under the 200 KB spec §6.3 budget).

Human action still needed:

1. Confirm the Cloudflare `amigo-pincel` project's Workers Builds settings don't override `wrangler.toml` (or set: build command from `wrangler.toml`, root directory `/`).
2. Decide the production domain (spec §14 Q1) and update `website/src/lib/config.ts::siteUrl` if it differs from `https://pincel.app`.

## Open questions (still actionable)

- **Per-tile dirty events** — `setTilePixel` emits `dirty-canvas` today; a `dirty-tile-pixel` variant carrying `(tileset_id, tile_id, rect)` lands alongside the M12 dirty-rect refinement.
- **Explicit active layer** — Stamp tool auto-picks the topmost tilemap layer bound to the active tileset. A Layers panel + explicit active-layer selector lands when a reorder command needs it (M9 follow-up).
- **Tile Editor tool routing** — Only direct click-paint is wired. Routing Line / Rect / Bucket through the tile-pixel target needs a tile-pixel sister command per tool (Phase 2).
- **Auto-tile mode** — Painting on a tilemap that auto-creates / reuses tiles stays Phase 2 (spec §5.3 / §13.2).

- **M6.7** — Human-driven cross-validation: open hand-crafted fixture in Pincel, paint, save, reopen in upstream Aseprite. Programmatic round-trip is pinned by `crates/pincel-wasm/tests/paint_save_open_roundtrip.rs`.
- **Slice user-data round-trip** — `aseprite_read` now hydrates `0x2022` chunks into `Sprite.slices` (M9.2), but the per-slice overlay color lives in an adjacent User Data chunk (`0x2020`) that we still drop on both sides. Pincel reconstructs slices with `Rgba::WHITE`. Round-trip preservation of the color lands when the User Data carrier does.
- **Stable LayerIds** — IDs assigned by source-file position today. Stable for read-only sessions but conflicts with spec's "stable id" promise once a reorder command exists. Revisit when reorder lands.
- **Mid-list AddFrame** — Append-only today. Mid-list insertion needs a `FrameIndex` remap on cel map / `Tag` / `Slice` refs. Defer until a tool needs it.
- **Indexed-mode painting** — `SetPixel` is RGBA-only. Indexed needs either a payload enum or a separate command. Land when indexed `compose()` lands.
- **Auto-create cels on empty targets** — `MissingCel` today. Decide when Pencil hits the case in practice.
- **`compose()` allocation** — Allocates output buffer per call. Spec §4.1 wants pre-allocated scratch. Fold into M12.
- **`dirty_hint` not wired** — Accepted but ignored. Needs dirty-rect tracking (spec §4.3). Defer to M12.
- **`pincel-wasm` error type** — Returns `Result<_, String>` for host-target testability. Migrate to `JsError` once `wasm-pack test --node` lands.
- **`Document::undo` / `redo` dirty events** — Emit full-canvas `dirty-canvas` because commands don't carry their own dirty region. Per-command dirty-rect is M12.
- **`Document::new` 0-frame question** — `aseprite-writer` happily emits a 0-frame file that `aseprite-loader` then refuses to parse. Decide whether to enforce ≥1 frame in `SpriteBuilder::build` or leave as a "valid Pincel, invalid Aseprite" affordance.
- **Move/zoom ergonomics** — M7.7 lacks wheel/pinch zoom, auto-fit on open, and cursor-anchored zoom. Cosmetic; not blocking.
- **Selection in undo stack** — `selection` lives on `Sprite` directly, not through a command. Aseprite tracks selection in undo; Pincel does not. Revisit if "select → drag → undo" UX needs the marquee back.
- **`pincel-wasm` link order** — `link:` protocol needs `crates/pincel-wasm/pkg/` to exist before `pnpm install`. CI / contributor docs should encode the order.
- **`wasm-opt` dev profile disabled** — `pincel-wasm/Cargo.toml` `dev` profile disables `wasm-opt` because the bundled downloader fails in the dev env. `release` profile keeps it on. Pin a system `wasm-opt` and point `wasm-pack` at it via `WASM_OPT_PATH` in CI when the deploy story lands.

## Deferred Aseprite chunks

Beyond CLAUDE.md M5 scope but in spec §8.3: Color Profile (`0x2007`, sRGB), Old Palette (`0x0004`), External Files (`0x2008`), User Data (`0x2020`), Slice (`0x2022` — done in M9.2), Tileset (`0x2023` — done in M8.5). Land alongside the milestones that need them (M8 tilemaps ✅, M9 slices: chunk done, command + UI deferred to M9.3 / M9.4).
