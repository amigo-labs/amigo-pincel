---
status: implemented
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

- [x] AC-01 (event): When the user submits the Add tileset form with a name and a tile width and height, then a tileset with that name and tile size holding one empty tile (id 0, fully transparent) is added and listed with its id, tile size and tile count, and one undo removes it.
- [x] AC-02 (always): The Add tileset form opens with the name `Tileset` and a 16 × 16 tile size.
- [x] AC-03 (error): If the Add tileset form has an empty name, or a tile width or height that is not a positive integer, then the form shows an error and no tileset is added.
- [x] AC-04 (event): When the user presses "+ Tile" on a tileset, then a fully transparent tile of the tileset's tile size is appended with the next tile id (ids count up from 1; tile 0 is the empty tile), and one undo removes it.
- [x] AC-05 (event): When the user presses "+ Tile", then the new tile becomes the picked stamp tile and the Tile Stamp tool becomes the active tool.
- [x] AC-06 (event): When the user presses "+ Layer" on a tileset, then a tilemap layer named `{tileset name} Layer`, bound to that tileset, is added on top of the stack with an empty grid of ⌈sprite width ∕ tile width⌉ × ⌈sprite height ∕ tile height⌉ cells on every existing frame, and one undo removes the layer together with its grids.
- [x] AC-07 (always): Each tileset in the Tilesets panel lists its tiles from id 1 in id order as thumbnails showing each tile's current pixels; the empty tile 0 is not shown and cannot be picked.
- [x] AC-08 (event): When the user clicks a tile thumbnail, then that tile becomes the picked stamp tile (shown pressed), and the Tile Stamp tool becomes the active tool.
- [x] AC-09 (state): While no stamp tile is picked, the Tile Stamp tool is disabled and its hint reads "Tile Stamp — pick a tile in the Tilesets panel first".
- [x] AC-10 (event): When the user clicks the canvas with the Tile Stamp tool, then the picked tile is placed into the grid cell under the pointer (column ⌊x ∕ tile width⌋, row ⌊y ∕ tile height⌋) of the topmost tilemap layer bound to that tile's tileset, at the current frame; each click is one undo step and dragging places no further tiles.
- [x] AC-11 (event): When a tile is placed into a cell, then only that cell of that layer and frame changes, and one undo restores the cell's previous tile.
- [x] AC-12 (state): While the Tile Stamp tool is active and the pointer is over the canvas, a grid at the tileset's tile size and the cell under the pointer are drawn over the sprite.
- [x] AC-13 (error): If no tilemap layer is bound to the picked tile's tileset, then a canvas click shows "add a tilemap layer first (+ Layer in the Tilesets panel)" in the status bar and nothing changes.
- [x] AC-14 (error): If the clicked cell lies outside the tilemap's grid, then the placement is refused and the document is unchanged.
- [x] AC-15 (error): If the target tilemap layer, or a group it is nested in, is locked, then the placement is refused and the document is unchanged.
- [x] AC-16 (event): When the user double-clicks a tile thumbnail, then the Tile editor opens over the canvas showing that tile at 16 screen px per tile pixel with a heading naming the tileset, tile and size; clicking or dragging paints the pixels under the pointer in the foreground colour, and Done closes it.
- [x] AC-17 (event): When a tile pixel is painted, then undo restores that pixel's previous colour.
- [x] AC-18 (always): A tile is stored once: after its pixels are edited, every cell that references it, on every tilemap layer and frame, shows the edited pixels.
- [x] AC-19 (error): If a tile pixel edit targets an unknown tileset, a tile id past the tileset's tiles, or a coordinate outside the tile, then it is refused and the tileset is unchanged.
- [x] AC-20 (event): When undo removes the picked stamp tile, then the stamp pick is cleared and the Tile Stamp tool is disabled until another tile is picked.
- [x] AC-21 (event): When the user stamps a tile on a frame where the target tilemap layer has no grid yet (a frame added after the layer), then an empty grid of the layer's size (as in AC-06) is created for that layer and frame and the tile is placed into it, as one undo step; one undo removes both the tile and the grid.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

**Document model** (`crates/pincel-core/src/document/`): `Tileset { id: TilesetId, name, tile_size: (u32, u32), tiles: Vec<TileImage>, base_index: i32, external_file, … }`; `Tileset::new(id, name, tile_size)` starts with no tiles and `base_index = 1` and stays that way for the reader, which takes tile 0 from the file; a tileset created in the UI (`AddTileset`, wasm `addTileset`) starts with a stored transparent tile 0, as Aseprite does, so the first user tile gets id 1 and no user tile is written as Aseprite's empty tile. `TileRef { tile_id: u32, flip_x, flip_y, rotate_90 }`, `TileRef::EMPTY` has `tile_id: 0`. `LayerKind::Tilemap { tileset_id }`; `CelData::Tilemap { grid_w, grid_h, tiles: Vec<TileRef> }`.

**Commands** (`crates/pincel-core/src/command/`): `AddTileset` (append), `AddTile` (append a transparent tile; positional id = `tiles.len()`), `AddTilemapLayer` (layer + seeded per-frame cels as one undo unit), `PlaceTile` (one `TileRef` in one cel; no merge, so each placement is its own undo entry; creates a missing empty grid for that layer and frame in the same undo step, like image cels are auto-created on paint — `AddFrame` stays unchanged, so no empty cels are written to files), `SetTilePixel` (one RGBA pixel of one tile; no merge, so each painted pixel is its own undo entry).

**wasm** (`crates/pincel-wasm/src/lib.rs`, "M8.6" / "M8.7" sections): `tilesetCount`, `tilesetIdAt(i)`, `tilesetName`, `tilesetTileWidth` / `tilesetTileHeight` / `tilesetTileCount` (0 / `""` for unknown ids), `tilePixels(tilesetId, tileId)` (RGBA bytes; errors for unknown tileset, tile past end, non-RGBA), `addTileset(name, tileW, tileH)` (id = max + 1; zero size rejected), `addTile(tilesetId)`, `addTilemapLayer(name, tilesetId)`, `layerKind` (`"tilemap"`), `layerTilesetId`, `placeTile(layer, frame, gridX, gridY, tileId)` (checks the lock, emits a `dirty-rect` for the one cell), `setTilePixel(tilesetId, tileId, x, y, 0xRRGGBBAA)` (emits `dirty-canvas`).

**UI**: `ui/src/modes/pixel/components/TilesetPanel.svelte` (form, "+ Tile", "+ Layer", thumbnails; click = pick, double-click = edit), `TileThumbnail.svelte`, `TileEditor.svelte` (modal-like overlay, `ZOOM = 16`, pencil-only), Stamp tool in `ui/src/modes/pixel/pointer.ts` (`activeTilemapLayerForTileset`, `commitStamp`), hover grid in `overlay-painter.ts` (`paintTileGrid`), stale-pick cleanup in `doc-ops.ts` (`reconcileActiveLayerAndStamp`), tool entry `tools.ts` (`'tilemap-stamp'`, disabled without a pick). Strings `tiles.*`, `tool.stamp*`, `px.needTilemap` in `ui/src/lib/i18n/en.ts`.

**e2e**: `ui/e2e/tilemaps.spec.ts` (panel, Stamp tool, hover grid, Tile editor; composed pixels via the Eyedropper, tile pixels and the overlay sampled test-side).

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
| Stamping on a frame added after the tilemap layer | The missing grid is created and the tile placed, one undo step (AC-21); no "missing cel" error |
| Adding frames to a sprite with a tilemap layer, then saving without stamping | No empty tilemap cels are written for the new frames |
| Tileset opened from an `.aseprite` file | Its tile 0 comes from the file; the panel hides it like a UI-created tileset's tile 0 |

## Summary

> Completed: 2026-11-03

- Tilesets with a transparent tile 0, new tiles from id 1, thumbnails without tile 0, and an Add tileset form that opens with its defaults and validates its sizes itself
- The Tile Stamp tool places the picked tile, draws the tile grid and the hovered cell, creates a frame's grid when it has none, and refuses locked layers or a missing tilemap layer
- The Tile editor paints single tile pixels, each one undoable, and an edited tile shows in every cell that uses it; every tileset edit refreshes the undo state, and the panel's texts are translated

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-wasm: tests::add_tileset_stores_transparent_tile_zero`, `tests::add_tileset_returns_zero_for_first_tileset_and_increments`, `tests::tileset_getters_round_trip_dimensions_and_name`, `tests::add_tileset_joins_undo_bus` (crates/pincel-wasm/src/lib.rs); `pincel-core: command::add_tileset::tests::apply_appends_tileset`, `command::add_tileset::tests::revert_removes_inserted_tileset`; e2e `the Add tileset form adds a tileset with an empty tile 0 and refuses bad input (AC-01, AC-02, AC-03)` (ui/e2e/tilemaps.spec.ts) | verified | ba53299 |
| AC-02 | e2e `the Add tileset form adds a tileset with an empty tile 0 and refuses bad input (AC-01, AC-02, AC-03)` (ui/e2e/tilemaps.spec.ts) | verified | ba53299 |
| AC-03 | e2e `the Add tileset form adds a tileset with an empty tile 0 and refuses bad input (AC-01, AC-02, AC-03)` (ui/e2e/tilemaps.spec.ts) | verified | 6aed618 |
| AC-04 | `pincel-wasm: tests::add_tile_appends_transparent_tile`, `tests::add_tile_joins_undo_bus`; `pincel-core: command::add_tile::tests::apply_appends_transparent_tile_sized_to_tileset`, `command::add_tile::tests::revert_pops_appended_tile`; e2e `+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)` (ui/e2e/tilemaps.spec.ts) | verified | ba53299 |
| AC-05 | e2e `+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)` (ui/e2e/tilemaps.spec.ts) | verified | 35f631c |
| AC-06 | `pincel-wasm: tests::add_tilemap_layer_creates_layer_and_seeds_cel_sized_to_canvas`, `tests::add_tilemap_layer_joins_undo_bus`; `pincel-core: command::add_tilemap_layer::tests::apply_inserts_layer_and_cels_together`, `command::add_tilemap_layer::tests::revert_removes_layer_and_seeded_cels` | verified | 6aed618 |
| AC-07 | e2e `+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)` (ui/e2e/tilemaps.spec.ts) | verified | 35f631c |
| AC-08 | e2e `+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)` (ui/e2e/tilemaps.spec.ts) | verified | 9af6b69 |
| AC-09 | e2e `+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)`, `undoing the picked tile clears the stamp pick (AC-20)` (ui/e2e/tilemaps.spec.ts) | verified | 9a7f4f0 |
| AC-10 | e2e `the Tile Stamp tool places the picked tile into the cell under the pointer (AC-10, AC-11)` (ui/e2e/tilemaps.spec.ts) | verified | c43a845 |
| AC-11 | `pincel-core: command::place_tile::tests::apply_writes_tile_and_records_previous`, `command::place_tile::tests::revert_restores_previous_tile`; `pincel-wasm: tests::place_tile_routes_through_command_bus_and_emits_dirty_rect`, `tests::place_tile_joins_undo_bus` | verified | 5dd2afe |
| AC-12 | e2e `the Tile Stamp tool draws the tile grid and the hovered cell (AC-12)` (ui/e2e/tilemaps.spec.ts) | verified | 22f5a52 |
| AC-13 | e2e `stamping without a tilemap layer asks for one and changes nothing (AC-13)` (ui/e2e/tilemaps.spec.ts) | verified | 3678aba |
| AC-14 | `pincel-wasm: tests::place_tile_rejects_out_of_grid_coords`; `pincel-core: command::place_tile::tests::out_of_bounds_coord_yields_error` | verified | 48b5d53 |
| AC-15 | `pincel-wasm: tests::place_tile_refuses_a_locked_layer_or_one_in_a_locked_group` (crates/pincel-wasm/src/lib.rs) | verified | 6aed618 |
| AC-16 | e2e `the Tile editor paints the pixels of one tile, one undo step each (AC-16, AC-17)` (ui/e2e/tilemaps.spec.ts) | verified | 35f631c |
| AC-17 | `pincel-wasm: tests::set_tile_pixel_joins_undo_bus`, `tests::set_tile_pixel_writes_and_emits_dirty_canvas`; `pincel-core: command::set_tile_pixel::tests::revert_restores_previous_pixel` | verified | b5e6482 |
| AC-18 | `pincel-wasm: tests::an_edited_tile_shows_in_every_cell_that_references_it` (crates/pincel-wasm/src/lib.rs) | verified | 6aed618 |
| AC-19 | `pincel-core: command::set_tile_pixel::tests::out_of_bounds_pixel_yields_error`, `command::set_tile_pixel::tests::unknown_tile_yields_error`, `command::set_tile_pixel::tests::unknown_tileset_yields_error` | verified | 6aed618 |
| AC-20 | e2e `undoing the picked tile clears the stamp pick (AC-20)` (ui/e2e/tilemaps.spec.ts) | verified | 6aed618 |
| AC-21 | `pincel-core: command::place_tile::tests::apply_on_a_frame_without_a_grid_creates_the_empty_grid_and_places`, `command::place_tile::tests::revert_removes_the_grid_apply_created`, `command::place_tile::tests::refused_placement_on_a_frame_without_a_grid_creates_nothing`, `command::place_tile::tests::frame_past_the_end_still_yields_missing_cel`; e2e `stamping on a frame added after the tilemap layer creates its grid in one undo step (AC-21)` (ui/e2e/tilemaps.spec.ts) | verified | dff3361 |

## Decisions

- 2026-10-29: A tileset created in the UI starts with a stored transparent tile 0, "+ Tile" ids start at 1, and tile 0 is not shown as a pickable thumbnail; `Tileset::new` stays as is for the reader (AC-01, AC-04, AC-07 changed) — today the first user tile renders blank and is written to `.aseprite` as Aseprite's empty tile, which loses data (owner accepted the recommendation).
- 2026-10-29: Stamping on a frame without a grid creates the empty grid for that layer and frame in the same undo step, while `AddFrame` stays unchanged (AC-21) — it matches Aseprite, where empty cels exist implicitly, and fixes the "missing cel" error at the point of use without writing empty cels to files (owner accepted the recommendation).
- 2026-11-03: Test gaps closed (AC-01–05, 07–10, 12, 13, 15, 16, 18, 20, 21). Red runs: the wasm tile-0 test and the `place_tile` grid tests failed against the code as found; the e2e tests failed against the code as found for AC-01 (Undo stayed disabled after a first tileset), AC-02, AC-03, AC-07 and the German strings; every other new test was shown red against a temporary break that was then reverted — a Stamp pick not switching the tool (AC-05), a thumbnail click not picking (AC-08), the Stamp tool always enabled (AC-09), stamps always on frame 0 (AC-10, the AC-21 e2e), no hovered-cell outline (AC-12), no status message (AC-13), a lock check that ignores groups (AC-15), Tile editor zoom 8 (AC-16), compose reading the wrong tile (AC-18), no stamp-pick cleanup on undo (AC-20).
- 2026-11-03: Bugs the tests found, fixed in their own commits: `addTileset` stored no tile 0 (AC-01, AC-04) and the panel showed tile 0 as a thumbnail (AC-07) — decided on 2026-10-29 but not built yet; `PlaceTile` refused a frame without a grid (AC-21); tileset edits did not refresh the undo depth, so Undo stayed disabled after a first tileset (AC-01); the tile size fields tripped the browser's own min / step validation, which blocked the submit and left a stale error standing (AC-03); the form kept earlier values on reopening (AC-02); "+ Tile", "+ Layer", the summary line and the thumbnail label were hard-coded English (now `tiles.plusTile`, `tiles.plusLayer`, `tiles.summary` / `tiles.summaryOne`, `tiles.thumb`; e2e `the Tilesets panel speaks the UI language`).
- 2026-11-03: Read as built: the listed tile count is the stored count, tile 0 included (a new tileset reads "1 tile", matching AC-01's "holding one empty tile"); the Tile editor heading names the tileset by its id ("Editing tileset 0 · tile 1 · 4×3"), as the Stamp tool hint does (AC-16); every pointer event of a Tile editor drag is its own undo entry, so a pixel a drag passes twice takes two undos (AC-17).
