# Status

_Last updated: 2026-06-10_

**Branch:** `claude/missing-items-E5TJi` · M12.2 done — `compose()` now
takes a caller-owned `&mut Vec<u8>` for the output (no per-call
allocation in the steady state) and honors `dirty_hint`, shrinking the
returned buffer to the intersection of viewport and hint (spec §4.3).
`ComposeResult` gains a `dirty_rect: Rect` field reporting the rendered
sprite-coord region; `width`/`height` collapse to `dirty.width * zoom`
and `dirty.height * zoom`. Empty intersections short-circuit with an
empty `out` so callers can skip the upload.

## M13 — Layers panel + reorder (complete)

Post-M12 feature (spec §3.2, panel layout §6). Decision: reorder is a
**sibling block-swap, group-atomic** (Decision Log §15) — moving a group
carries its children; no layer crosses a group boundary. Cross-group
drag is deferred. Task breakdown:

- [x] **M13.1** — core `MoveLayer { Up | Down }` command: swaps a
  layer's contiguous subtree with the adjacent sibling's subtree via a
  range rotation; `revert` rotates back. New `CommandError::UnknownLayer`
  / `LayerAtEdge`. Wired into `AnyCommand`. 10 unit tests (siblings,
  group-atomic moves, leaf-over-group, intra-group, all four edge
  cases).
- [x] **M13.2** — wasm surface: `moveLayerUp(id)` / `moveLayerDown(id)`
  route a `MoveLayer` through the bus (group-atomic, undo-routed, emit
  `dirty-canvas`); `From<MoveLayer>` for `AnyCommand`; new `layerVisible`
  / `layerOpacity` read getters for the panel. 6 host tests (reorder both
  directions, undo round-trip, edge + unknown errors, getter defaults).
  The explicit **active layer** is UI state and lands with the panel
  (M13.3).
- [x] **M13.3** — `LayersPanel.svelte` mounted as the first right-side
  panel: z-ordered list (top-most first), active-row highlight (click →
  `onActivate` sets `activeLayerId`), ▲/▼ reorder via `moveLayerUp` /
  `moveLayerDown` (disabled at the flat list ends; a true mid-group edge
  is a caught `LayerAtEdge` no-op), visibility indicator (●/○ + dimmed
  name, read-only). Reorder bumps the shared `rev` so the list
  re-derives; the canvas recomposes off the `dirty-canvas` event. UI
  gates green (`pnpm check` / `lint` / `build`).
- [x] **M13.3b** — active layer wired into the paint surface. wasm
  `setActiveLayer(id)` stores the target; a `paint_target_layer()` helper
  resolves it (active iff it exists and is an image layer, else the
  lowest-z image layer) and now backs all six pixel paths (pencil/eraser,
  line, rect, ellipse, bucket, move-selection). `paintTargetLayer()`
  getter exposes the resolved id. `LayersPanel` row-click calls
  `setActiveLayer`. 3 host tests (resolution, group/unknown fallback,
  pencil lands on the active cel while the default stays clear).
- [x] **M13.3c** — layer visibility toggle, end to end: core
  `SetLayerVisible { layer, visible }` (prior value captured for revert,
  `dirty_region = Canvas`), wasm `setLayerVisible(id, visible)` through
  the bus, and a clickable ●/○ eye button in `LayersPanel` wired to it.
  Undo-routed. Core + 2 host tests.
- [x] **M13.3d** — inline rename: core `SetLayerName` (`dirty_region =
  None`), wasm `renameLayer(id, name)`, panel double-click-to-edit
  (Enter / blur commits, Escape cancels, empty discarded). Core + 2 host
  tests.
- [x] **M13.4** — reorder survives the aseprite round-trip. No code
  change needed: `build_layer_index_map` already maps each `LayerId` to
  its current Vec position, so a reorder (which permutes positions but
  keeps ids) writes every cel at the right `layer_index`. On reload ids
  are renumbered by position — `LayerId` is positional by the format's
  design (it stores no editor id), which is consistent and closes the
  "Stable LayerIds" open question. Locked in by a `pincel-core`
  integration test (reorder → write → read; asserts z-order + per-layer
  cel content by name + pixels).
- [ ] Later — opacity / blend-mode controls, lock toggle, cross-group
  drag-and-drop reparenting (all outside M13).

## Next task

**M12.6** — Verify the spec exit criterion: 256×256 sprite at zoom
32 maintains 60 fps on M1 / mid-tier Windows.

**Done:** the UI-driven frame-time probe has landed. Press **F2** in the
editor to toggle it; the footer then shows effective fps (EMA of
inter-tick spacing) plus average and worst compose cost over a rolling
60-frame window. The probe wraps the `recompose` / `recomposeDirty`
calls in `App.svelte::tick` with `performance.now()` and is fully gated
behind the toggle, so normal use pays only a single boolean test per
frame. The Criterion bench suite already covers `compose()` in
isolation.

**Verification method (manual):** `pnpm dev`, `New`, resize/create a
256×256 document, zoom to 32, press F2, then drag the Pencil rapidly
across the canvas. Read fps / compose-ms off the footer. The exit
criterion holds when fps stays at/near 60 and compose stays well under
the ~16.6 ms frame budget. Record measured numbers below once taken on
target hardware.

**Measured:** _pending — capture on M1 / mid-tier Windows._

**M12.5** (WebGPU adapter, spec §4.4 / §17.2) — **done.** Split into
M12.5a (render-adapter seam + stacked overlay canvas) and M12.5b
(WebGPU backend + capability detection). The base layer now blits
through a `CanvasRenderer`; WebGPU is preferred with automatic Canvas2D
fallback, and `?renderer=canvas2d` forces the fallback for A/B testing.
The active backend shows in the footer. Verify on real hardware via the
Cloudflare branch-preview URL in a WebGPU-capable browser (this sandbox
is headless — GPU rendering not exercisable here).

## M12 baselines (criterion, 2026-05-24)

Recorded on the M12.1 commit. `cargo bench -p pincel-core --bench compose`.
Numbers are sandbox-host medians; relative comparison is what matters for
subsequent slices.

| Bench | Median |
|-------|--------|
| `compose_256_single_layer_full` | 129.49 µs |
| `compose_256_four_layers_full`  | 1.3730 ms |
| `compose_256_dirty_hint_4x4`    | 131.03 µs (≈ full path — `dirty_hint` ignored pre-M12.2) |
| `compose_64_tilemap_full`       | 6.3359 µs |
| `compose_zoom_32_upscale_8x8_to_256x256` | 23.914 µs (8×8 viewport, zoom 32 → 256×256 output) |

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
| M10 | ✅ | PWA polish — split into M10.1–M10.4 below |
| M11.1 | ✅ | Tauri 2 scaffold — `src-tauri/` crate, workspace member, CLI wiring, `isTauri()` helper |
| M11.2 | ✅ | Native FS commands (`read_file_bytes` / `write_file_bytes`) + `tauri-plugin-dialog` + `ui/src/lib/fs/index.ts` Tauri branch |
| M11.3 | ✅ | Native menu bar (File / Edit / View / Help) + Recents submenu wired via `set_recent_menu` |
| M11.4 | ✅ | `bundle.fileAssociations` for `.aseprite` / `.ase`, single-instance forward, macOS `RunEvent::Opened`, first-launch advisory dialog |
| M12.1 | ✅ | Profiling baseline — `criterion` workspace dev-dep, `crates/pincel-core/benches/compose.rs` with five scenarios (single-layer / four-layer / dirty-hint / tilemap / zoom-32). Numbers pinned above. |
| M12.2 | ✅ | `compose()` takes `out: &mut Vec<u8>` (scratch reuse); honors `dirty_hint` via `Rect::intersect`; `ComposeResult` drops `pixels`, gains `dirty_rect`. |
| M12.3 | ✅ | Per-command `DirtyRegion` complete on the paint surface: type + trait method + `Bus::last_dirty_region()` + `Document::undo`/`redo` + bucket / move event paths all emit precise `dirty-rect` events. SetPixel / DrawLine / DrawRectangle / DrawEllipse / FillRegion / MoveSelectionContent all report sprite-coord rects; structural / tilemap / slice commands keep the safe-but-coarse `Canvas` default. |
| M12.4 | ✅ | Canvas2D sub-rect blit. `ComposeFrame` exposes `dirtyX` / `dirtyY`; new `Document::composeDirty(...)` + `blitDirtyFrame(...)`. `App.svelte::tick` aggregates `dirty-rect` events into a union bbox and routes through the sub-rect path when no overlays are live (selection / drag / stamp / active slice all force the full path). |
| M12.5 | ✅ | WebGPU render adapter (spec §4.4). `CanvasRenderer` seam + stacked overlay canvas (M12.5a), `WebGPURenderer` + capability detection + `?renderer=canvas2d` force toggle (M12.5b). |
| M12.6 | ⬜ | 60 fps verification on M1 / mid-tier Windows (manual, target hardware). |

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

### M10 sub-tasks

- [x] **M10.1** — `ui/src/lib/fs/index.ts` adapter. `hasFsAccess()` feature-flags the UI. `pickAndOpen()` returns `{name, bytes, handle}`; the FSA path keeps the returned `FileSystemFileHandle` so subsequent saves can write in place, the fallback path spawns a one-shot `<input type="file">`. `saveBytes(bytes, target, opts)` resolves in order: write-through-existing-handle / FSA save picker / Blob+anchor download; `forceAs: true` skips the in-place arm so the explicit `Save As…` button always re-prompts. `Save` / `Save As (download)` toolbar label switches per `hasFsAccess()`; `Save As…` button shows on FSA browsers only. `App.svelte` `saveTarget` `$state` carries `{name, handle}` across the session, reset on `newDoc` and refreshed on every open / save-as. `ensureReadWritePermission` exported for reuse from M10.2's `openRecent`.
- [x] **M10.2** — New `ui/src/lib/idb/` module group: `db.ts` opens the `pincel` IDB database (version 1; stores `prefs`, `recent_files`, `autosave_snapshots`; idempotent `openDb()` with cached open promise; promise-wrapped `idbRequest` + `transactionDone` helpers; `isIdbAvailable()` SSR guard); `recent-files.ts` exposes `upsertRecent` / `listRecents` / `removeRecent` / `clearRecents` with a `MAX_RECENTS = 8` cap and openedAt-indexed LRU eviction inside the insert transaction; `prefs.ts` exposes `getPref` / `setPref` / `removePref` as primitive k/v over the `prefs` store. `autosave_snapshots` is schema-only in M10.2 (composite `[docId, ts]` keyPath); the timer + recovery dialog land in M10.3. App.svelte gains a `docId = $state<string>(crypto.randomUUID())` per-document identity (refreshed on `New` / `Open`, preserved on `Open Recent`), `recordRecent()` upserts after every successful open / save / save-as when both `recentsAvailable` and `saveTarget.handle` are set, and a `Recent…` toolbar dropdown (FSA + IDB-capable browsers only) shows the eight most-recent FSA-handle-bearing files; clicking re-opens via `ensureReadWritePermission` + `handle.getFile()`.
- [x] **M10.3** — `ui/src/lib/idb/autosave.ts` (`writeSnapshot` / `latestSnapshot` / `listLatestSnapshots` / `removeSnapshots`) keeps at most one row per `docId` in the `autosave_snapshots` store. App.svelte arms a 30 s `setInterval` after `loadCore` resolves; each tick short-circuits unless `doc.undoDepth` has advanced past `lastWriteUndoDepth`, then writes the encoded `.aseprite` bytes. Successful `save` / `openDoc` / `openRecent` / `applyRecovery` all clear the snapshot for the current `docId` and re-baseline `lastWriteUndoDepth` so the next dirty edit re-arms the timer. New `RecoveryDialog.svelte` modal mounts on app start when `listLatestSnapshots()` returns ≥ 1 entry; each row offers `Recover` (loads the snapshot, re-binds `docId` to the snapshot's id, clears the row) and `Discard` (drops the row). `Not now` dismisses without touching the store so the snapshots survive to the next boot.
- [x] **M10.4** — `vite-plugin-pwa@^1.3.0` + `workbox-precaching@^7.4.1` devDependencies (spec §10.1 mandates `injectManifest` so this counts as spec-approved). `vite.config.ts` registers `VitePWA` with `strategies: 'injectManifest'`, `srcDir: 'src'`, `filename: 'sw.ts'`, `registerType: 'autoUpdate'`, and an explicit `injectManifest.globPatterns` widened to cover `.wasm` (the wasm-pack output goes into `dist/assets/`). Custom `src/sw.ts` (~30 lines) routes the manifest through `precacheAndRoute(self.__WB_MANIFEST)` and calls `skipWaiting` / `clients.claim` so a fresh deploy activates without a tab close. Built SW precaches 7 unique URLs totalling ~1.9 MiB (WASM is the dominant entry). `manifest.webmanifest` carries `Pincel` name / short name / description, `display: standalone`, `#0a0a0a` background + theme colors, and a single SVG icon at `purpose: "any maskable"` reused from the website favicon. `index.html` gains `<meta name="theme-color">`, description, and the SVG favicon link; the registration script is injected automatically.

## Recent work

- **2026-06-10 — Deep-fixup session (this branch).** Repo-wide
  analyze → plan → fix pass; task record in `PLAN.md`. Fixed: tag
  colors were bleached to white on open (`aseprite_read::map_tag` now
  reads the in-chunk RGB our writer emits; round-trip test asserts
  non-white colors survive); async file ops (open / save / recent /
  recovery) gained a `fileOpBusy` re-entrancy guard so a double-clicked
  Save can't run two concurrent FSA writes and a second Open can't
  dispose a `Document` mid-use. Simplified: the 10-line post-replace
  reset block (×5 in App.svelte) is now `resetDocViewState()`. UX:
  Rect Fill / Ellipse / Ellipse Fill gained tooltips and `U` now
  cycles the shape group (Aseprite pattern). DX/docs: root `README.md`
  (clone → run incl. the wasm-pkg-before-install order), LICENSE-MIT /
  LICENSE-APACHE added (root + `aseprite-writer`), the two
  `too_many_arguments` allows documented per CLAUDE.md §9, CLAUDE.md
  §10/§14 synced to the real scripts and layout, spec §11.4 `isTauri`
  snippet fixed for Tauri 2. All gates green (454 Rust tests;
  `pnpm lint`/`check`/`build`).

- **2026-05-31 — M13.3d rename + M13.4 reorder round-trip (this branch);
  M13 complete.** Rename: core `SetLayerName` (captures the prior name
  for `revert`, `dirty_region = None` since it changes no pixels), wasm
  `renameLayer(id, name)`, and double-click-to-edit in `LayersPanel`
  (Enter / blur commits, Escape cancels, blank discarded). M13.4: a new
  `pincel-core` integration test reorders layers, round-trips through
  `write_aseprite` / `read_aseprite`, and asserts z-order plus per-layer
  cel content survive — confirming `build_layer_index_map`'s
  position-mapping already makes reorder lossless; `LayerId` is positional
  by the format's design. Core + 2 host rename tests; 131 wasm tests; all
  `cargo` + `pnpm` gates green. M13 (Layers panel: list · reorder · active
  selection driving paint · visibility toggle · rename · lossless
  round-trip) is done.

- **2026-05-31 — M13.3c layer visibility toggle (this branch).** Core
  `SetLayerVisible { layer, visible }` flips `Layer::visible` (prior value
  captured for `revert`, `dirty_region = Canvas` since hidden layers drop
  from the composite); wasm `setLayerVisible(id, visible)` routes it
  through the undo bus and emits `dirty-canvas`; the `LayersPanel`
  visibility indicator is now a clickable ●/○ button wired to it (bumps
  `rev`). Core unit tests + 2 host tests (toggle + undo, unknown-id
  error); 129 wasm tests green. `cargo`/`pnpm` gates green. Rename is
  M13.3d.

- **2026-05-31 — M13.3b active-layer paint targeting (this branch).**
  The Layers panel selection is now functional: `Document` gains an
  `active_layer: Option<LayerId>` with `setActiveLayer(id)` and a private
  `paint_target_layer()` that prefers the active layer when it exists and
  is an image layer, else falls back to the lowest-z image layer (the
  prior behavior). All six pixel paths (apply_tool, line, rect, ellipse,
  bucket, move-selection) route through it — six identical inline picks
  collapsed to one helper. `paintTargetLayer()` getter surfaces the
  resolved id; `LayersPanel` row-click calls `setActiveLayer`. A fresh
  `Document` resets the target to `None`, so New / Open keep the
  fallback. 3 host tests (resolution, group / unknown fallback, pencil
  lands on the active cel while the default stays clear); 127 wasm tests
  green. `pnpm check`/`lint`/`build` green. Visibility toggle + rename
  are M13.3c.

- **2026-05-31 — M13.3 LayersPanel (this branch).** New
  `ui/src/lib/components/LayersPanel.svelte`, mounted as the first
  right-side panel. Lists layers top-most-first (reversed z-order) via
  the `layerCount` / `layerIdAt` / `layerName` / `layerKind` /
  `layerVisible` getters behind the shared `rev` counter; a row click
  sets the parent's `activeLayerId` (highlight only for now); ▲/▼ call
  `moveLayerUp` / `moveLayerDown` and bump `rev` to re-derive, with the
  canvas recomposing off the emitted `dirty-canvas`. Move buttons are
  disabled at the flat list ends and a true mid-group edge is swallowed
  as a `LayerAtEdge` no-op. Visibility is shown read-only (●/○ + dimmed
  name). Painting still auto-picks the image layer — active-layer paint
  targeting + visibility toggle + rename are M13.3b. `pnpm check` (0
  errors) / `lint` / `build` green.

- **2026-05-31 — M13.1 core MoveLayer command (this branch).** First
  slice of the Layers-panel feature. `pincel-core::MoveLayer { Up |
  Down }` reorders a layer among its siblings by swapping its whole
  contiguous subtree with the adjacent sibling's subtree (a single range
  `rotate_left`; `revert` is the inverse `rotate_right`), so groups move
  atomically and the flat-Vec contiguity invariant survives. New
  `CommandError::UnknownLayer` / `LayerAtEdge`; `MoveDirection` +
  `MoveLayer` exported and wired into `AnyCommand` (apply / revert /
  dirty_region = Canvas). 10 unit tests cover sibling swaps, group-atomic
  moves, a leaf jumping over a sibling group, intra-group moves, and the
  four edge/unknown cases. `cargo test`/`clippy -D warnings`/`fmt` green.
  wasm + UI panel are M13.2–M13.3.

- **2026-05-31 — Reject zero-frame writes (this branch).** Closes the
  0-frame footgun: a frameless `Sprite` (which `SpriteBuilder::build`
  still allows as a valid in-memory object) used to write an Aseprite
  header claiming 0 frames that `aseprite-loader` then refused to parse.
  `aseprite_writer::write` now returns the new `WriteError::NoFrames`
  before emitting anything, propagating through `pincel-core` as
  `CodecError::Write`. Decision: enforce the invariant at the format
  boundary, not in the builder, so the permissive minimal-sprite
  affordance stays. Writer + `pincel-core` tests cover the rejection.

- **2026-05-31 — Slice overlay color round-trip via User Data (this
  branch).** Closes the long-standing fidelity gap where Pincel dropped a
  slice's editor color on save and reconstructed every slice as white.
  `aseprite-writer`: `SliceChunk` gains `user_data: Option<UserData>`
  (text + RGBA); `write()` emits a `0x2020` User Data chunk right after
  each slice (Aseprite attaches user data to the preceding chunk). New
  `UserData` re-export; writer round-trip test asserts the loader reads
  the color back. `pincel-core`: `map_slice` writes the color into that
  chunk and `extract_tilesets_and_slices` tracks the preceding slice to
  apply a trailing `UserData` color (white fallback). The slices
  integration test now round-trips two distinct colors incl. a
  non-opaque alpha. Gates green: `cargo test`/`clippy`/`fmt` for
  `aseprite-writer` + `pincel-core`, plus `pincel-wasm` host tests
  (118 pass). Note text + property maps stay dropped (no per-slice note
  field yet).

- **2026-05-31 — M12.5 WebGPU render adapter (this branch).** Spec §4.4.
  **M12.5a** introduces the render-adapter seam: `ui/src/lib/render/
  types.ts::CanvasRenderer` (`draw` / `drawDirty` / `destroy` + `backend`
  label) and `Canvas2DRenderer` wrapping the existing blit helpers. The
  single render canvas splits into a base layer (driven by the renderer)
  and a transparent, `pointer-events-none` Canvas2D **overlay** stacked
  exactly on top via a sizing/transform wrapper; all transient furniture
  (drag previews, marquee, tile grid, slice accents) moves to the overlay
  through the new `paintOverlays()`, and the dirty fast path wipes it via
  `clearOverlay()`. Behaviour stayed identical (still Canvas2D).
  **M12.5b** adds `WebGPURenderer` — a full-screen-triangle blit of the
  sprite texture (`writeTexture` upload, nearest sampling, premultiplied
  output to match the Canvas2D compositing of non-premultiplied pixels),
  with `drawDirty` doing a sub-rect `writeTexture` + full redraw.
  `WebGPURenderer.create()` resolves to `null` (never throws) on any
  failure, and only claims the canvas's `'webgpu'` context as its last
  step so the Canvas2D fallback stays viable. `App.svelte::createRenderer`
  prefers WebGPU, falls back automatically, and honours
  `?renderer=canvas2d`; the footer shows the active backend. New
  devDependency `@webgpu/types` (type-only; Decision Log §15) — TS's
  bundled `lib.dom` still omits WebGPU types. UI gates green: `pnpm
  check` (0 errors), `pnpm lint`, `pnpm build`. GPU rendering itself is
  not exercisable in this headless sandbox — verify on the Cloudflare
  branch-preview URL in a WebGPU browser.

- **2026-05-31 — Move/zoom ergonomics: auto-fit + keyboard zoom (this
  branch).** Continues the cursor-anchored-wheel-zoom thread. New pure
  helper `ui/src/lib/view/fit.ts::fitZoom(viewportW, viewportH, spriteW,
  spriteH, min, max, margin)` returns the largest integer display-zoom
  that shows the whole sprite with a margin, clamped to `[1, 64]` and
  falling back to `min` for degenerate / unmeasured inputs. `App.svelte`
  binds the flex-centered stage wrapper's `clientWidth` / `clientHeight`
  (`stageW` / `stageH`) and gains `fitView()`, which picks that zoom
  (24 px margin) and re-centers (pan 0), falling back to the historical
  8× when the stage hasn't been measured yet. `fitView()` now runs on
  every document replacement (`newDoc`, `openDoc`, `applyRecovery`,
  `openRecent`, `openRecentById`, and the `onMount` initial doc) so a
  freshly-loaded sprite always lands fully in view regardless of its
  dimensions. `resetView()` (the "Reset" toolbar button + `View ▸ Reset
  Zoom` menu item) now delegates to `fitView()` instead of the fixed 8×,
  which was off-screen for large sprites. New bare-key zoom shortcuts in
  `onKeyDown` (inside the existing no-modifier / not-editable guard, so
  Ctrl/Cmd +/- stays the browser's page zoom): `+`/`=` zoom in, `-`/`_`
  zoom out, `0` fits. UI gates green: `pnpm check` (0 errors), `pnpm
  lint`, `pnpm build`. Touch pinch-zoom is the remaining ergonomics gap
  (can't be exercised headless; deferred).

- **2026-05-24 — M11.2 + M11.3 + M11.4 (this branch).** Native desktop
  shell closes out: M11.2 adds two `#[tauri::command] async fn`
  wrappers around `std::fs` (`read_file_bytes`, `write_file_bytes`)
  plus `tauri-plugin-dialog` for native open / save pickers, and
  `ui/src/lib/fs/index.ts` `pickAndOpen` / `saveBytes` branch vornean
  on `isTauri()`. `OpenedFile` and `SaveTarget` grow a `path: string |
  null` field; `RecentFile` schema does too (legacy rows treated as
  `path: null`). `@tauri-apps/api@^2` + `@tauri-apps/plugin-dialog@^2`
  added as runtime deps. App.svelte's `openRecent` dispatches on
  `tauriHost && r.path` vs the existing FSA-handle branch; Save / Save
  As… buttons unlock on Tauri too. M11.3 adds a four-submenu menu
  (File / Edit / View / Help) via the Tauri 2 `tauri::menu` API, with
  standard accelerators (Cmd/Ctrl+N/O/S/Z, …) and predefined Cut /
  Copy / Paste / About items. Menu events emit on a `"menu"` window
  event with the item id as payload; `set_recent_menu` lets the
  renderer rebuild the Open Recent submenu from the IDB-backed
  recents list (empty list yields a disabled "(no recent files)"
  placeholder). New `ui/src/lib/menu/index.ts` (~60 lines) exposes
  `wireNativeMenu` + `syncRecentMenu`; a `$effect` in App.svelte syncs
  the submenu on every recents change. M11.4 adds
  `bundle.fileAssociations` for `.aseprite` / `.ase` (MIME
  `application/x-aseprite`), `tauri-plugin-single-instance` so a
  second double-click forwards to the running instance, and CLI-arg
  parsing (`first_file_arg` — skips binary + `-flag` args) so
  file-association launches hand the file to the renderer via an
  `open-file` event. macOS `RunEvent::Opened` is wired behind
  `#[cfg(target_os = "macos")]` for Finder + `open -a` flows. New
  `FileAssocDialog.svelte` (~80 lines) is a one-shot Tauri-only
  advisory that walks the user through per-OS steps to register Pincel
  as the default handler; "Don't show again" persists
  `fileAssocPromptShown` in the M10.2 prefs store. All gates green:
  `cargo fmt --all --check`, `cargo check --workspace`, `cargo test
  --workspace` (419 tests, 0 failures), `cargo clippy --workspace
  --all-targets -- -D warnings`, `pnpm check / lint / build /
  wasm:build`. Runtime `pnpm tauri:dev` not exercised in this session
  (needs a graphical display); CI confirms Rust + UI build.

- **2026-05-24 — Workspace fmt drift (this branch).** Pre-existing
  `cargo fmt` drift in `crates/aseprite-writer/{error.rs,write.rs}`
  and four `crates/pincel-core/src/command/*.rs` files cleaned up in
  a fmt-only commit before the M11 slices. `cargo fmt --all --check`
  now clean.

- **2026-05-13 — M11.1 (prior branch).** New `src-tauri/` crate (`pincel-tauri`) with `[[bin]] name = "pincel"`, `tauri = "2"` + `tauri-build = "2"` as runtime / build deps, and `pincel-core = { workspace = true }` per CLAUDE.md §5.5 (declared now, exercised in M11.2). `src/main.rs` is the minimal Tauri 2 entry — `Builder::default().run(generate_context!())` gated on the `windows_subsystem = "windows"` cfg for release. `tauri.conf.json` v2 schema points `beforeDevCommand` / `beforeBuildCommand` at `pnpm --dir ../ui dev|build` so the CLI drives the existing UI workflow; `devUrl: http://localhost:5173` matches Vite's default and `frontendDist: ../ui/dist` matches `pnpm build` output. `capabilities/default.json` grants only `core:default` to the `main` window — FS / dialog / event permissions land with M11.2. Raster icons (32×32, 128×128, 128×128@2x, 512×512 `icon.png`, multi-size `icon.ico`) generated from `ui/public/favicon.svg` via `rsvg-convert` + ImageMagick, re-encoded RGBA after Tauri's codegen rejected the RGB output. macOS `.icns` is the one missing platform asset (`tauri icon` upstream can synthesize it; tracked under open questions). Workspace `Cargo.toml` grows the `src-tauri` member entry. `ui/package.json` adds `@tauri-apps/cli@^2` (devDep) and `tauri` / `tauri:dev` / `tauri:build` scripts; `@tauri-apps/api` is deferred to M11.2 where the JS-side `invoke()` lands. `ui/src/lib/platform/index.ts` exposes `isTauri()` probing both Tauri 2 (`__TAURI_INTERNALS__`) and v1 (`__TAURI__`) globals; spec §11.4 says `__TAURI__` but Tauri 2 ships `__TAURI_INTERNALS__`, so the helper covers both with a comment. `vite.config.ts` gains `clearScreen: false`, `envPrefix: ['VITE_', 'TAURI_ENV_*']`, and `server.strictPort: true` to keep the Vite dev server's port pinned at 5173 (so Tauri's `devUrl` never resolves to a stale process). All gates green: `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt -p pincel-tauri --check`, `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`, `pnpm exec tauri info` reports environment ✔. Runtime `pnpm tauri:dev` not exercised in CI (needs a graphical display); the spec gate is the Rust + UI builds passing.
- **2026-05-13 — M10.3 + M10.4 (prior branch).** Autosave + recovery + service worker land together as the M10 closer. `ui/src/lib/idb/autosave.ts` keeps at most one snapshot per `docId` in the M10.2 `autosave_snapshots` store via an `IDBKeyRange.bound([docId, -∞], [docId, +∞])` cursor that delete-walks prior rows inside the same readwrite transaction before the `put`. `listLatestSnapshots()` defensively dedupes by `docId` in case a partial write left an older row behind. App.svelte arms a 30 s `setInterval` from `onMount` once `autosaveAvailable` (== `isIdbAvailable()`) is true; the tick body is no-op unless `doc.undoDepth` has advanced past `lastWriteUndoDepth`. `lastWriteUndoDepth` is re-baselined on `newDoc` / `openDoc` / `openRecent` / `save` / `applyRecovery`, and `clearAutosave()` runs after each successful save / open so the recovery probe never surfaces a snapshot that matches on-disk state. New `RecoveryDialog.svelte` (~80 lines) mounts on boot when `listLatestSnapshots()` returns ≥ 1 row; each row exposes `Recover` (loads the snapshot via `Document.openAseprite`, re-binds `docId` to the snapshot's id, clears the row), `Discard` (drops the row), and a global `Not now` dismiss that keeps the snapshots for the next boot. `vite-plugin-pwa@^1.3.0` + `workbox-precaching@^7.4.1` devDependencies (spec §10.1 mandates `injectManifest` — counts as spec-approved). `vite.config.ts` adds `VitePWA({ strategies: 'injectManifest', srcDir: 'src', filename: 'sw.ts', registerType: 'autoUpdate', manifest: {…}, injectManifest: { globPatterns: '**/*.{js,css,html,wasm,svg,webmanifest}' } })`. Custom `src/sw.ts` imports `precacheAndRoute` from `workbox-precaching`, hands it `self.__WB_MANIFEST`, and skip-waits / claims clients on install / activate. `public/favicon.svg` (copied from `website/static/favicon.svg`), `index.html` gains `<meta name="theme-color" content="#0a0a0a">` + description + SVG favicon link. `tsconfig.json` `types` adds `vite-plugin-pwa/client` so `self.__WB_MANIFEST` types resolve. Built SW precaches 7 unique URLs (wasm + JS + CSS + HTML + manifest + favicon + registerSW) totalling ~1.9 MiB. All gates green: `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`.
- **2026-05-12 — M10.2 (prior branch).** New `ui/src/lib/idb/` module group lays the IndexedDB substrate that M10.3 (autosave + recovery) and the recents UX in this commit both depend on. `db.ts` opens `pincel` v1 with three stores: `prefs` (keyPath `key`, simple k/v), `recent_files` (keyPath `id`, index `by_openedAt`), `autosave_snapshots` (composite keyPath `[docId, ts]`; schema-only in M10.2). The shared `openDb()` caches its open promise so concurrent first-touches collapse to one IDB open request, and clears the cache on rejection so a subsequent call retries. Helpers `idbRequest` + `transactionDone` wrap the request / transaction lifecycles. `recent-files.ts` upserts with prior-`addedAt` preservation, then evicts past `MAX_RECENTS = 8` inside the same readwrite transaction by walking the `by_openedAt` index in ascending order and dropping the overflow tail. `prefs.ts` is the minimal `getPref` / `setPref` / `removePref` k/v surface — included now since CLAUDE.md §9 bans `localStorage`. App.svelte adds `docId = $state<string>(crypto.randomUUID())` (refreshed on `New` / `Open`, preserved on `Open Recent` so re-opens count as the same doc and M10.3 snapshots survive page reloads), `recordRecent()` upserts after each successful open / save / save-as when both `recentsAvailable` and `saveTarget.handle` are set, and a `Recent…` toolbar dropdown (FSA + IDB-capable browsers only) lists the eight most-recent FSA-handle-bearing files; clicking re-opens via `ensureReadWritePermission` + `handle.getFile()`. UI gates green: `pnpm check`, `pnpm lint`, `pnpm build`.
- **2026-05-12 — M10.1 (this branch).** New `ui/src/lib/fs/index.ts` (~210 lines) encapsulates file open / save behind one surface. `hasFsAccess()` probes `window.showOpenFilePicker`; `pickAndOpen()` runs the FSA picker when available (retaining the returned `FileSystemFileHandle`) and otherwise spawns a hidden `<input type="file">` that cleans itself up on `change` / `cancel`. `saveBytes(bytes, target, opts)` writes through `target.handle` in place when present and writable (gating on `queryPermission` / `requestPermission({ mode: 'readwrite' })`), prompts `showSaveFilePicker` when FSA is available but no handle is bound, and falls back to a Blob + anchor download otherwise; user-cancelled save-as returns the original target unchanged. The signatures pin `Uint8Array<ArrayBuffer>` (not `ArrayBufferLike`) so callers copy through `new Uint8Array(doc.saveAseprite())` before invoking — required by lib.dom's typing of FSA `write()` and `Blob`. `App.svelte` drops the previous hidden `<input>` element, replaces the inline `openFile` / `save` with `openDoc` / `save({ forceAs? })`, and adds a `saveTarget = $state<SaveTarget>` carrying `{name, handle}` across the session (reset on `newDoc`, refreshed on every successful open / save-as). Toolbar label switches `Save` ↔ `Save As (download)` per `hasFsAccess()`; an extra `Save As…` button is rendered only on FSA-capable browsers. UI gates green: `pnpm check`, `pnpm lint`, `pnpm build`.
- **2026-05-12 — M9.4 (prior branch).** New `pincel-wasm::Document` slice surface: `addSlice` / `removeSlice` / `setSliceKey` route through the M9.3 commands plus a `slice_key(&self, slice_id, key_index)` helper backing 14 read getters (`sliceCount`, `sliceIdAt`, `sliceName`, `sliceColor`, `sliceKeyCount`, and per-field key getters for bounds / center / pivot). `setSliceKey` accepts `Option<i32>` / `Option<u32>` for the center quartet and pivot pair, mapping `None` to "no center" / "no pivot" on the wasm side and rejecting partial sets so JS never silently drops fields. 16 new unit tests cover happy path, monotonic id assignment, undo round-trip, sorted-frame insertion, center / pivot round-trip, partial-quartet rejection, unknown-id rejection, empty-bounds rejection, and the unknown-id getter defaults. New `SlicesPanel.svelte` (~360 lines) mounts as a second sidebar; props match the M8.7a `TilesetPanel` shape (`doc`, `rev`, `activeSliceId`, `onChange`, `onActivate`). Each row owns a color swatch + name (both clickable to activate), an "×" remove button, X / Y / W / H number inputs, and toggleable 9-patch / pivot fieldsets with cX/cY/cW/cH and pX/pY inputs. `App.svelte` grows a `slice` `Tool` variant joined to the existing drag-shape pipeline; release commits via `addSlice` (no active slice) or `setSliceKey` preserving center / pivot (active slice). New `paintActiveSliceOverlay` paints the active slice's frame-0 marching ants, plus `paintRectOutline` for the 9-patch center (in the slice's editor color) and `paintPivotCrosshair` for the pivot. `reconcileActiveSlice` clears the local `activeSliceId` after an undo / redo strips the referenced slice. Marching-ants animation now also pulses while `activeSliceId !== null` so the overlay reads as live. UI gates green: `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`.
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

- `cargo fmt --all --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm install`, `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`

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

- **macOS `icon.icns`** — M11.1 ships PNG + ICO icons generated from `ui/public/favicon.svg`. The macOS bundle target needs `icon.icns`; `pnpm exec tauri icon ui/public/favicon.svg` regenerates the full platform set in one shot. Land alongside the first macOS build attempt (M11.4 or release prep).
- **Spec §11.4 `isTauri` global** — _resolved 2026-06-10._ Spec snippet now probes both `__TAURI_INTERNALS__` (Tauri 2) and `__TAURI__` (v1 / `withGlobalTauri`), matching `ui/src/lib/platform/isTauri()`.

- **M6.7** — Human-driven cross-validation: open hand-crafted fixture in Pincel, paint, save, reopen in upstream Aseprite. Programmatic round-trip is pinned by `crates/pincel-wasm/tests/paint_save_open_roundtrip.rs`.
- **Slice user-data round-trip** — _resolved 2026-05-31._ `aseprite-writer` now emits a `0x2020` User Data chunk after each slice and `pincel-core` writes / recovers the overlay color through it. Note text + property maps (`0x2020` flags `0x1` / `0x4`) are still dropped — Pincel has no per-slice note field yet; revisit if one is added.
- **Stable LayerIds** — IDs assigned by source-file position today. Stable for read-only sessions but conflicts with spec's "stable id" promise once a reorder command exists. Revisit when reorder lands.
- **Mid-list AddFrame** — Append-only today. Mid-list insertion needs a `FrameIndex` remap on cel map / `Tag` / `Slice` refs. Defer until a tool needs it.
- **Indexed-mode painting** — `SetPixel` is RGBA-only. Indexed needs either a payload enum or a separate command. Land when indexed `compose()` lands.
- **Auto-create cels on empty targets** — `MissingCel` today. Decide when Pencil hits the case in practice.
- **`compose()` allocation** — Allocates output buffer per call. Spec §4.1 wants pre-allocated scratch. Fold into M12.
- **`dirty_hint` not wired** — Accepted but ignored. Needs dirty-rect tracking (spec §4.3). Defer to M12.
- **`pincel-wasm` error type** — Returns `Result<_, String>` for host-target testability. Migrate to `JsError` once `wasm-pack test --node` lands.
- **`Document::undo` / `redo` dirty events** — Emit full-canvas `dirty-canvas` because commands don't carry their own dirty region. Per-command dirty-rect is M12.
- **`Document::new` 0-frame question** — _resolved 2026-05-31._ Decided to keep `SpriteBuilder::build` permissive (a frameless sprite is a valid in-memory Pincel object) and enforce the format invariant at the write boundary: `aseprite_writer::write` now returns `WriteError::NoFrames` instead of emitting a header readers reject. Covered by writer + `pincel-core` tests.
- **Move/zoom ergonomics** — cursor-anchored mouse-wheel zoom
  (`App.svelte::onWheel`, non-passive listener), **auto-fit on open**
  (`fitView` + `ui/src/lib/view/fit.ts::fitZoom`, runs on every doc
  replacement), and **keyboard zoom shortcuts** (`+`/`=`, `-`/`_`, `0`
  → fit) all landed. The "Reset" control + `View ▸ Reset Zoom` now
  fit-to-viewport instead of the old fixed 8×. Still missing: touch
  pinch-zoom. Cosmetic; not blocking.
- **Selection in undo stack** — `selection` lives on `Sprite` directly, not through a command. Aseprite tracks selection in undo; Pincel does not. Revisit if "select → drag → undo" UX needs the marquee back.
- **`pincel-wasm` link order** — _resolved 2026-06-10._ The root `README.md` and CLAUDE.md §10 document that `pnpm wasm:build` must run before `pnpm install`; CI already encoded the order.
- **`wasm-opt` dev profile disabled** — `pincel-wasm/Cargo.toml` `dev` profile disables `wasm-opt` because the bundled downloader fails in the dev env. `release` profile keeps it on. Pin a system `wasm-opt` and point `wasm-pack` at it via `WASM_OPT_PATH` in CI when the deploy story lands.

## Deferred Aseprite chunks

Beyond CLAUDE.md M5 scope but in spec §8.3: Color Profile (`0x2007`, sRGB), Old Palette (`0x0004`), External Files (`0x2008`), User Data (`0x2020`), Slice (`0x2022` — done in M9.2), Tileset (`0x2023` — done in M8.5). Land alongside the milestones that need them (M8 tilemaps ✅, M9 slices: chunk done, command + UI deferred to M9.3 / M9.4).
