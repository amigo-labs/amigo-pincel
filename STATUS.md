# Status

_Last updated: 2026-05-07_

## Completed

### M1 — `pincel-core` skeleton ✅

- Cargo workspace at repo root (edition 2024, shared lints, `unsafe_code = deny`)
- Crate `crates/pincel-core` with no platform dependencies
- Document model per `docs/specs/pincel.md` §3:
  - `Sprite`, `Metadata`, `ColorMode`
  - `Layer` (Image / Tilemap / Group), `LayerId`, `BlendMode` (Aseprite numeric values)
  - `Frame`, `FrameIndex`
  - `Cel`, `CelData` (Image / Tilemap / Linked), `PixelBuffer`, `TileRef`
  - `Palette`, `PaletteEntry`, `Rgba`
  - `Tileset`, `TilesetId`, `TileImage`, `PathRef`
  - `Slice`, `SliceId`, `SliceKey`
  - `Tag`, `TagDirection`
  - `geometry::{Point, Rect}`
  - `error::DocumentError` (`InvalidDimensions`, `DuplicateId`, `UnknownId`)
- Fluent `SpriteBuilder` that validates canvas dimensions and id uniqueness
- 20 unit tests cover construction, defaults, and validation

### M2 — `pincel-core` commands + undo ✅

- `document::CelMap` — `(LayerId, FrameIndex) → Cel` storage (BTreeMap-backed),
  used by commands and `compose()`
- `command::Command` trait with `apply` / `revert` / default `merge`
- `command::AnyCommand` enum dispatching to concrete commands without dyn
- `command::Bus` — linear undo / redo stack with configurable cap
  (default 100, see spec §6.2). `execute` clears redo, attempts a merge with
  the stack top, then pushes; oldest entries drop when the cap is exceeded
- `command::CommandError` (`MissingCel`, `NotAnImageCel`, `PixelOutOfBounds`,
  `UnsupportedColorMode`, `DuplicateLayerId`, `FrameIndexOutOfRange`)
- Three commands per spec §6 / CLAUDE.md M2:
  - `SetPixel` — RGBA-only single-pixel write, sprite-coord input,
    captures the prior color for revert
  - `AddLayer::on_top` / `AddLayer::at` — z-order insertion, rejects
    duplicate ids
  - `AddFrame::append` — append-only for M2 (mid-list insertion would
    require remapping cel `FrameIndex` keys; deferred)
- 17 unit tests in `command` plus a `tests/command_bus.rs` integration
  suite with 6 cases

### M3 — `pincel-core` `compose()` (image layers) ✅

- New `render` module with the spec §4.1 contract:
  - `ComposeRequest` (frame, viewport, zoom, onion-skin, layer filter,
    overlays, dirty hint) — with a `ComposeRequest::full` helper
  - `ComposeResult` (RGBA8 pixels, width, height, generation)
  - `LayerFilter` (`Visible` / `All` / `Only(Vec<LayerId>)`)
  - `Overlays`, `OnionSkin` (defaults match spec §4.2: 76 ≈ 0.3 × 255 alpha)
- `render::compose(&Sprite, &CelMap, &ComposeRequest) → Result<…, RenderError>`:
  - RGBA color mode only (indexed / grayscale return
    `RenderError::UnsupportedColorMode`)
  - Visible image layers in z-order; tilemap and group layers raise
    `UnsupportedLayerKind`
  - Source-over (`Normal`) blend with per-cel and per-layer opacity;
    other `BlendMode` variants raise `UnsupportedBlendMode`
  - Cels clipped to the viewport intersection (negative positions OK)
  - `LayerFilter::All` / `Only` honored; invisible layers skipped under
    the default `Visible` filter
  - Integer zoom 1..=64 via row-replicate + memcpy nearest-neighbor
    upscale (output is `viewport.w * zoom` × `viewport.h * zoom`)
  - `generation` is `0` (purely-functional `compose`); the UI layer is
    expected to maintain its own monotonic counter
- `RenderError` covers: `UnsupportedColorMode`, `InvalidZoom`,
  `EmptyViewport`, `UnknownFrame`, `UnsupportedLayerKind`,
  `UnsupportedBlendMode`
- 23 new unit tests (60 unit total) plus a `tests/render_compose.rs`
  integration suite with 3 cases (two-layer offset, viewport+zoom,
  per-frame cel selection)

### Build status

`cargo check`, `cargo test` (60 unit + 6 + 3 integration), `cargo clippy
--all-targets -- -D warnings`, and `cargo fmt --check` are all green on
the `claude/continue-from-status-lr9pG` branch.

## Next concrete task

**M3 follow-up — additional `BlendMode` variants in `compose()`**
(`docs/specs/pincel.md` §4.2)

Aseprite ships 19 blend modes (`Normal` … `Divide`). `compose()` currently
only implements `Normal`. The remaining 18 are needed for full read/write
parity with `.aseprite` files but are not blocking M4–M7 (loader/writer
tooling and Pencil-only UI both stay on `Normal`). Plan as one M-sized
task once a fixture surfaces the need:

1. Decide the canonical reference (Aseprite's `doc/blend_funcs.cpp` is
   the source of truth — link in module docs).
2. Implement per-channel blend functions, dispatched once per pixel
   instead of per blend mode.
3. Snapshot tests against fixtures created in Aseprite.

Until then, `RenderError::UnsupportedBlendMode` keeps non-`Normal`
modes loud rather than silently wrong.

**Recommended next milestone: M4 — `aseprite-loader` integration (read).**
Spec §7.1, CLAUDE.md §4 M4. Add the loader as a workspace dependency,
build an adapter from its output to `pincel-core::Sprite`, and round-trip
a hand-crafted fixture. Tilemap and slice chunks can be preserved opaquely
at first.

## Open questions

- `AddFrame` in M2 is append-only. Mid-list insertion needs a
  `FrameIndex` remap on the cel map (and on `Tag`/`Slice` references).
  Postpone until a tool actually needs it; revisit when the Pencil tool
  starts exercising frame navigation.
- `SetPixel` only supports RGBA color mode. Indexed-mode painting will
  need a separate command (or a payload enum) once an indexed `compose`
  path lands.
- Whether commands should auto-create cels when targeting an empty
  `(layer, frame)` slot. Current behavior: error out with `MissingCel`.
  Defer; the Pencil tool in M6 will be the first caller that has an
  opinion.
- `compose()` currently allocates the output buffer per call. Spec §4.1
  says "must not allocate per-pixel" and mentions pre-allocated scratch
  buffers stored on the document. This is a perf concern, not a
  correctness one — fold into M12.
- `dirty_hint` is accepted but ignored. Wiring it requires the dirty-rect
  tracking described in spec §4.3 (Phase 1.5). Defer to M12.
- Indexed-mode `compose` will need palette lookup; the palette type is
  already in the document model. Add when M3 image-only is no longer
  enough (likely alongside an indexed `SetPixel`).
