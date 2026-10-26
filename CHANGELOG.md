# Changelog

## [1.1.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.0.1...v1.1.0) (2026-10-26)


### Features

* desktop downloads on the website, New cards open in place on the start screen ([8d3e411](https://github.com/amigo-labs/amigo-pincel/commit/8d3e411e34403c37b54c7858046caab5763a8d93))
* **ui:** New cards on the start screen open in place ([f3eb37a](https://github.com/amigo-labs/amigo-pincel/commit/f3eb37a29c5696f5c3462a952f646efc6502dacd))

## [1.0.1](https://github.com/amigo-labs/amigo-pincel/compare/v1.0.0...v1.0.1) (2026-10-26)


### Bug Fixes

* **tauri:** Windows app icon as RGBA so the Windows build embeds it ([8fad095](https://github.com/amigo-labs/amigo-pincel/commit/8fad095293c8d6548ccbbf6c8c773faf1a11aa18))
* **tauri:** Windows app icon as RGBA so the Windows build embeds it ([fc84db9](https://github.com/amigo-labs/amigo-pincel/commit/fc84db951221f09abe759a1e3d55f18ff2ca8de1))

## [1.0.0](https://github.com/amigo-labs/amigo-pincel/compare/v0.1.9...v1.0.0) (2026-10-26)


### Features

* **release:** release-please, self-updater, setup and start-up as in amigo-blinzel ([2695564](https://github.com/amigo-labs/amigo-pincel/commit/269556452826ee235680fdae5adc13d594ef15ee))
* **release:** web and desktop installers in every release, published as one ([f24056e](https://github.com/amigo-labs/amigo-pincel/commit/f24056e0687e86ce7903c43a38cc451a5652a35b))
* **tauri:** setup and start-up behaviour as in amigo-blinzel ([50083e6](https://github.com/amigo-labs/amigo-pincel/commit/50083e65f40fce3da0790d94005c0f323582e992))
* **tauri:** silent self-updater, as in amigo-blinzel ([50d963a](https://github.com/amigo-labs/amigo-pincel/commit/50d963a7e08caa729cb71b8b02b9af04c7742b58))


### Bug Fixes

* **release:** point the Tauri CLI at src-tauri ([add5c95](https://github.com/amigo-labs/amigo-pincel/commit/add5c95d6c00a3d52328f619f3ee4b539217ffd1))
* **tauri:** keep the setup's Capabilities under the installer's own key ([ef95e22](https://github.com/amigo-labs/amigo-pincel/commit/ef95e22d4df63d1704e2bafb36bbd6ed759aff40))
* **tauri:** offer Default apps once, also across interactive upgrades ([10f70d8](https://github.com/amigo-labs/amigo-pincel/commit/10f70d814d108022dc4902573f99219b7d4bb0af))
* **tauri:** updater settings written whole and one at a time ([69cf888](https://github.com/amigo-labs/amigo-pincel/commit/69cf888a9ca0cba0b9b5df09c6230cd167cebbf1))

## Changelog

## Before 1.0.0 (v0.1.x pre-releases)

The v0.1.x pre-releases (one per push to `main`) built up the full
Phase 1 scope per
[`docs/specs/pincel.md`](docs/specs/pincel.md) §16.

### Added

- **Core document model** (`pincel-core`): Sprite, Layer, Frame, Cel,
  Palette, Tileset, Slice, Tag types with a `SpriteBuilder` API. Pure
  logic, no I/O, no platform dependencies.
- **Command bus + undo/redo** with a 100-deep linear history. Commands:
  `SetPixel`, `AddLayer`, `AddFrame`, `DrawLine`, `DrawRectangle`,
  `DrawEllipse`, `FillRegion`, `MoveSelectionContent`, `AddTileset`,
  `PlaceTile`, `SetTilePixel`, `AddSlice`, `RemoveSlice`, `SetSliceKey`.
- **`compose()`** for RGBA image and tilemap layers — viewport-windowed,
  zoom-aware, caller-owned scratch buffer, `dirty_hint` honored,
  per-command dirty-region reporting routed end-to-end into a Canvas2D
  sub-rect blit.
- **Aseprite codec.** Read (image layers + tilemaps + slices, via
  `aseprite-loader`) and write (header, layer, cel, palette, tags,
  tileset, tilemap cel, slice chunks). Round-trip tests cover image,
  tilemap, and 9-patch+pivot slice fixtures.
- **`aseprite-writer` crate** as a standalone, publishable Rust library
  for the v1.3 `.aseprite` format, dual-licensed MIT or Apache-2.0.
- **`pincel-wasm`** wasm-bindgen layer with the public API surface
  described in spec §17.5 — opens, paints, exports, saves
  `.aseprite` files from any JS host. Builds to `@amigo-labs/pincel`
  for npm.
- **UI tool set** (Svelte 5 + Tailwind 4): Pencil, Eraser, Eyedropper,
  Line, Rectangle (outline + fill), Ellipse (outline + fill), Bucket,
  Move (viewport pan + selection-content drag), Selection (Rect) with
  marching-ants overlay, Tilemap Stamp with grid + cell hover overlay,
  Slice tool with 9-patch and pivot editing.
- **Tileset Panel + Tile Editor modal** — add tilesets, add tiles,
  edit tile pixels with the full undo stack.
- **Slices Panel** — add / remove slices, edit bounds, toggle
  9-patch center, toggle pivot.
- **PWA** — service worker via `vite-plugin-pwa` `injectManifest`,
  manifest (standalone display, theme color, maskable icon).
- **File I/O** — File System Access API with in-place save-through and
  download fallback, recents (IndexedDB, 8-entry LRU), 30-second
  autosave snapshots with boot-time Recovery Dialog.
- **Tauri 2 desktop shell** — native open/save dialogs, native menu
  bar (File / Edit / View / Help) with platform accelerators, file
  associations for `.aseprite` / `.ase`, single-instance forwarding,
  macOS `RunEvent::Opened` handling, first-launch file-association
  advisory dialog.
- **Performance** — Criterion baselines for `compose()`, per-command
  `DirtyRegion`, Canvas2D sub-rect blit driven by a union-bounding-box
  dirty aggregator.
