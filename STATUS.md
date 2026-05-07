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
  used by commands and (later) by `compose()`
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
- 17 new unit tests (now 37 unit total) plus a `tests/command_bus.rs`
  integration suite with 6 cases: undo restores state, redo replays,
  execute clears redo, add-layer / add-frame round trips, and history-cap
  trimming

### Build status

`cargo check`, `cargo test` (37 unit + 6 integration), `cargo clippy
--all-targets -- -D warnings`, and `cargo fmt --check` are all green on
the `claude/continue-from-status-lRO2f` branch.

## Next concrete task

**M3 — `pincel-core` `compose()`, image layers only**
(`docs/specs/pincel.md` §4, CLAUDE.md §4 M3)

- `render` module with `compose(&Sprite, &CelMap, &ComposeRequest) -> ComposeResult`
- RGBA-only path (palette lookup deferred until indexed mode lands)
- No tilemaps, no slices, no overlays — those arrive with M8 / M9
- Snapshot test: a hand-built sprite produces the expected RGBA bytes

Estimated size: M (3–5 files, ≤400 lines, multiple commits). Plan as:

1. `render` module skeleton: `ComposeRequest`, `ComposeResult`,
   `LayerFilter`, `Overlays`, `OnionSkin` stubs
2. `compose()` pass over visible image cels with `Normal` blend, no zoom,
   no overlays — get a known fixture green
3. Apply per-cel opacity, layer opacity, and the remaining `BlendMode`
   variants required by Aseprite parity
4. Integer zoom (nearest-neighbor upscale)

## Open questions

- `AddFrame` in M2 is append-only. Mid-list insertion needs a
  `FrameIndex` remap on the cel map (and on `Tag`/`Slice` references).
  Postpone until a tool actually needs it; revisit when `compose()` and
  the Pencil tool start exercising frame navigation.
- `SetPixel` only supports RGBA color mode. Indexed-mode painting will
  need a separate command (or a payload enum) once the Indexed compose
  path lands in M3.
- Whether commands should auto-create cels when targeting an empty
  `(layer, frame)` slot. Current behavior: error out with `MissingCel`.
  Defer; the Pencil tool in M6 will be the first caller that has an
  opinion.
