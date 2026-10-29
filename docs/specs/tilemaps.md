---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §3.2, pincel.md §3.4, pincel.md §5.2, pincel.md §5.3, pincel.md §9.2]
---

# Tilesets and tilemap layers

## Context

Game levels and repeating sprite parts are built from tiles: a tileset stores each tile image once, and a tilemap layer is a grid of references into it (`pincel.md` §3.2 `LayerKind::Tilemap`, `CelData::Tilemap`, §3.4 `Tileset`). Editing a tile changes every place that uses it. This spec records, as built, how the Pixel-mode user creates tilesets and tiles, adds tilemap layers, places tiles with the Tile Stamp tool (§5.2) and paints a tile's pixels in the Tile editor (§5.3).

Phase 1 is manual placement plus explicit tile editing; Aseprite's auto-tile mode is deferred to Phase 2 (§5.3, §13.2).

## Scope

### In scope

- Tilesets panel: add a tileset (name, tile size), add a tile, add a tilemap layer for a tileset, tile thumbnails, picking the stamp tile
- Tile Stamp tool: placing the picked tile into a grid cell of a tilemap layer, the tile-grid hover overlay, its refusals
- Tile editor: painting single pixels of one tile, undo of those edits
- Undo of every tileset / tile / tilemap edit

### Out of scope

- Auto-tile (painting at tile resolution that reuses or creates tiles) — because §5.3 defers it to Phase 2.
- Flipping or rotating tiles while stamping — because the Stamp tool always places tiles in canonical orientation; flip / rotate flags only arrive from opened files.
- Removing, renaming or reordering tilesets and tiles — because no command or UI for it exists.
- Line, Rectangle, Bucket and the other image tools inside the Tile editor — because the editor only paints single pixels today.
- Drawing tilemap cels (tile placement, flips, rotation, tile id 0, error cases) — because the `pixel-compose` spec owns `compose()`.
- Reading and writing tilesets and tilemap cels in `.aseprite`, and external tileset files — because the `aseprite-round-trip` spec owns the codec.
- Image-tool painting while a tilemap row is active — because the `pixel-layers-frames` spec owns the paint-target rule (tools fall back to the lowest image layer).

## Acceptance criteria

- [ ] AC-01 (event): When the user submits the Add tileset form with a name and a tile width and height, then an empty tileset (no tiles) with that name and tile size is added and listed with its id, tile size and tile count, and one undo removes it.
- [ ] AC-02 (always): The Add tileset form opens with the name `Tileset` and a 16 × 16 tile size.
- [ ] AC-03 (error): If the Add tileset form has an empty name, or a tile width or height that is not a positive integer, then the form shows an error and no tileset is added.
- [ ] AC-04 (event): When the user presses "+ Tile" on a tileset, then a fully transparent tile of the tileset's tile size is appended with the next tile id (ids count up from 0), and one undo removes it.
- [ ] AC-05 (event): When the user presses "+ Tile", then the new tile becomes the picked stamp tile and the Tile Stamp tool becomes the active tool.
- [ ] AC-06 (event): When the user presses "+ Layer" on a tileset, then a tilemap layer named `{tileset name} Layer`, bound to that tileset, is added on top of the stack with an empty grid of ⌈sprite width ∕ tile width⌉ × ⌈sprite height ∕ tile height⌉ cells on every existing frame, and one undo removes the layer together with its grids.
- [ ] AC-07 (always): Each tileset in the Tilesets panel lists its tiles in id order as thumbnails showing each tile's current pixels.
- [ ] AC-08 (event): When the user clicks a tile thumbnail, then that tile becomes the picked stamp tile (shown pressed), and the Tile Stamp tool becomes the active tool.
- [ ] AC-09 (state): While no stamp tile is picked, the Tile Stamp tool is disabled and its hint reads "Tile Stamp — pick a tile in the Tilesets panel first".
- [ ] AC-10 (event): When the user clicks the canvas with the Tile Stamp tool, then the picked tile is placed into the grid cell under the pointer (column ⌊x ∕ tile width⌋, row ⌊y ∕ tile height⌋) of the topmost tilemap layer bound to that tile's tileset, at the current frame; each click is one undo step and dragging places no further tiles.
- [ ] AC-11 (event): When a tile is placed into a cell, then only that cell of that layer and frame changes, and one undo restores the cell's previous tile.
- [ ] AC-12 (state): While the Tile Stamp tool is active and the pointer is over the canvas, a grid at the tileset's tile size and the cell under the pointer are drawn over the sprite.
- [ ] AC-13 (error): If no tilemap layer is bound to the picked tile's tileset, then a canvas click shows "add a tilemap layer first (+ Layer in the Tilesets panel)" in the status bar and nothing changes.
- [ ] AC-14 (error): If the clicked cell lies outside the tilemap's grid, then the placement is refused and the document is unchanged.
- [ ] AC-15 (error): If the target tilemap layer, or a group it is nested in, is locked, then the placement is refused and the document is unchanged.
- [ ] AC-16 (event): When the user double-clicks a tile thumbnail, then the Tile editor opens over the canvas showing that tile at 16 screen px per tile pixel with a heading naming the tileset, tile and size; clicking or dragging paints the pixels under the pointer in the foreground colour, and Done closes it.
- [ ] AC-17 (event): When a tile pixel is painted, then undo restores that pixel's previous colour.
- [ ] AC-18 (always): A tile is stored once: after its pixels are edited, every cell that references it, on every tilemap layer and frame, shows the edited pixels.
- [ ] AC-19 (error): If a tile pixel edit targets an unknown tileset, a tile id past the tileset's tiles, or a coordinate outside the tile, then it is refused and the tileset is unchanged.
- [ ] AC-20 (event): When undo removes the picked stamp tile, then the stamp pick is cleared and the Tile Stamp tool is disabled until another tile is picked.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

**Document model** (`crates/pincel-core/src/document/`): `Tileset { id: TilesetId, name, tile_size: (u32, u32), tiles: Vec<TileImage>, base_index: i32, external_file, … }`; `Tileset::new(id, name, tile_size)` starts with no tiles and `base_index = 1`. `TileRef { tile_id: u32, flip_x, flip_y, rotate_90 }`, `TileRef::EMPTY` has `tile_id: 0`. `LayerKind::Tilemap { tileset_id }`; `CelData::Tilemap { grid_w, grid_h, tiles: Vec<TileRef> }`.

**Commands** (`crates/pincel-core/src/command/`): `AddTileset` (append), `AddTile` (append a transparent tile; positional id = `tiles.len()`), `AddTilemapLayer` (layer + seeded per-frame cels as one undo unit), `PlaceTile` (one `TileRef` in one cel; no merge, so each placement is its own undo entry), `SetTilePixel` (one RGBA pixel of one tile; no merge, so each painted pixel is its own undo entry).

**wasm** (`crates/pincel-wasm/src/lib.rs`, "M8.6" / "M8.7" sections): `tilesetCount`, `tilesetIdAt(i)`, `tilesetName`, `tilesetTileWidth` / `tilesetTileHeight` / `tilesetTileCount` (0 / `""` for unknown ids), `tilePixels(tilesetId, tileId)` (RGBA bytes; errors for unknown tileset, tile past end, non-RGBA), `addTileset(name, tileW, tileH)` (id = max + 1; zero size rejected), `addTile(tilesetId)`, `addTilemapLayer(name, tilesetId)`, `layerKind` (`"tilemap"`), `layerTilesetId`, `placeTile(layer, frame, gridX, gridY, tileId)` (checks the lock, emits a `dirty-rect` for the one cell), `setTilePixel(tilesetId, tileId, x, y, 0xRRGGBBAA)` (emits `dirty-canvas`).

**UI**: `ui/src/modes/pixel/components/TilesetPanel.svelte` (form, "+ Tile", "+ Layer", thumbnails; click = pick, double-click = edit), `TileThumbnail.svelte`, `TileEditor.svelte` (modal-like overlay, `ZOOM = 16`, pencil-only), Stamp tool in `ui/src/modes/pixel/pointer.ts` (`activeTilemapLayerForTileset`, `commitStamp`), hover grid in `overlay-painter.ts` (`paintTileGrid`), stale-pick cleanup in `doc-ops.ts` (`reconcileActiveLayerAndStamp`), tool entry `tools.ts` (`'tilemap-stamp'`, disabled without a pick). Strings `tiles.*`, `tool.stamp*`, `px.needTilemap` in `ui/src/lib/i18n/en.ts`.

**e2e**: none for tilesets yet; `ui/e2e/helpers.ts` has `createDocument`, `drag`, `pixelCanvas`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Sprite size not a multiple of the tile size (e.g. 8×6 with 4×4 tiles) | The grid rounds up (2×2); the partial cells at the right / bottom edge can be stamped |
| Two tilemap layers bound to the same tileset | The Stamp tool targets the topmost of them, whichever row is active |
| Click left of or above the canvas with the Stamp tool | Ignored; nothing changes |
| Tile size larger than the sprite | The grid is 1 × 1 |
| Undo of "+ Layer" | The layer and every seeded grid leave together; no orphan cels remain |
| Tile editor drag leaving the tile area | Pointer positions outside the tile paint nothing |
| "+ Tile" while redo steps are pending | The pending redo steps are discarded, as for any edit |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

- Tile id 0 is both the first tile "+ Tile" creates and the "empty" id the renderer skips (`TileRef::EMPTY`, Aseprite convention): stamping the first tile of a fresh tileset leaves the cell blank on the canvas although its thumbnail shows pixels. Is this intended, or should a new tileset start with a stored empty tile 0 so user tiles count from 1 (as in Aseprite)? AC-04 / AC-10 / AC-18 depend on the answer.
- Frames added after a tilemap layer exists get no tilemap grid for that layer (`AddFrame` seeds no cels, and only image cels are auto-created on paint), so stamping on such a frame fails with a "missing cel" error in the status bar. Is that the accepted behavior, or should every frame of a tilemap layer have a grid? AC-10 depends on the answer.

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-wasm: tests::add_tileset_returns_zero_for_first_tileset_and_increments`, `tests::tileset_getters_round_trip_dimensions_and_name`, `tests::add_tileset_joins_undo_bus`; `pincel-core: command::add_tileset::tests::apply_appends_tileset`, `command::add_tileset::tests::revert_removes_inserted_tileset` | built | |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 | `pincel-wasm: tests::add_tile_appends_transparent_tile`, `tests::add_tile_joins_undo_bus`; `pincel-core: command::add_tile::tests::apply_appends_transparent_tile_sized_to_tileset`, `command::add_tile::tests::revert_pops_appended_tile` | built | |
| AC-05 |      | open   |        |
| AC-06 | `pincel-wasm: tests::add_tilemap_layer_creates_layer_and_seeds_cel_sized_to_canvas`, `tests::add_tilemap_layer_joins_undo_bus`; `pincel-core: command::add_tilemap_layer::tests::apply_inserts_layer_and_cels_together`, `command::add_tilemap_layer::tests::revert_removes_layer_and_seeded_cels` | built | |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 | `pincel-core: command::place_tile::tests::apply_writes_tile_and_records_previous`, `command::place_tile::tests::revert_restores_previous_tile`; `pincel-wasm: tests::place_tile_routes_through_command_bus_and_emits_dirty_rect`, `tests::place_tile_joins_undo_bus` | built | |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 | `pincel-wasm: tests::place_tile_rejects_out_of_grid_coords`; `pincel-core: command::place_tile::tests::out_of_bounds_coord_yields_error` | built | |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 | `pincel-wasm: tests::set_tile_pixel_joins_undo_bus`, `tests::set_tile_pixel_writes_and_emits_dirty_canvas`; `pincel-core: command::set_tile_pixel::tests::revert_restores_previous_pixel` | built | |
| AC-18 |      | open   |        |
| AC-19 | `pincel-core: command::set_tile_pixel::tests::out_of_bounds_pixel_yields_error`, `command::set_tile_pixel::tests::unknown_tile_yields_error`, `command::set_tile_pixel::tests::unknown_tileset_yields_error` | built | |
| AC-20 |      | open   |        |

## Decisions

