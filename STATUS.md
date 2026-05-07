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
- `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`
  are all green

## Next concrete task

**M2 — `pincel-core` commands + undo** (`docs/specs/pincel.md` §6, CLAUDE.md §4 M2)

- Add `command::Command` trait (`apply` / `revert` / optional merge logic)
- Implement a small `command::Bus` that holds the undo and redo stacks
- Implement three commands: `SetPixel`, `AddLayer`, `AddFrame`
- Tests: apply / revert round-trip preserves state; redo after undo replays

Estimated size: M (3–6 files, ≤400 lines, multiple commits). Plan as:

1. `command` module skeleton (Command trait, Bus, error variants)
2. `SetPixel` command + tests
3. `AddLayer` and `AddFrame` commands + tests

## Open questions

- Whether `Command::apply` should mutate `Sprite` directly or return a new
  `Sprite` (copy-on-write). The spec says "mutate the document," so plan to
  use `&mut Sprite` for apply/revert. Confirm before locking in.
- Cel storage: spec §3.2 says cels are keyed by `(LayerId, FrameIndex)`. Not
  yet on `Sprite`; defer until commands need it (M2 `SetPixel` will force it).
- Whether to allocate ids inside the document (auto-incrementing counter) or
  to require callers to supply them. Currently callers supply; revisit when
  `AddLayer` lands.
