# Changelog

All notable changes to Pincel will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

The initial 1.0.0 release ships the full Phase 1 scope per
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

[Unreleased]: https://github.com/amigo-labs/amigo-pincel/compare/v0.0.0...HEAD
