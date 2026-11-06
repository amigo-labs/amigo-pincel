---
status: todo
created: 2026-11-05
size: L
mode: Pixel
design: [mcp.md, pincel.md §3.2, pincel.md §3.4, pincel.md §3.5, pincel.md §5.3, pincel.md §7.1, pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-structure]
---

# MCP parity: Pixel assets (slices, tilesets, grid)

## Context

`mcp-headless-pixel` gives agents a headless `pincel-mcp` server. Its tools cannot reach three kinds of Pixel document data the editor edits: slices (`slices.md`), tilesets with tilemap layers (`tilemaps.md`), and the document grid (`pixel-grid.md`, the cell size and offset stored in the `.aseprite` header). `layer.list` already reports `kind: "tilemap"`, but an agent cannot learn which tileset a tilemap layer uses, cannot create one, and cannot place or paint tiles. The `draw.*` tools refuse a tilemap layer named by `layer` ("is not an image layer").

This spec is the Pixel-assets part of the five `mcp-parity-*` specs (Decision Log 2026-11-05 "MCP tool parity"). Every capability already exists in `pincel-core` as an undoable command and is used by the editor. The tools expose these capabilities; they do not add new editing behavior.

**"The editor" in this spec** means the result the Pixel editor's UI produces for the same action, which is the result of the core command the UI issues for it (through `pincel-wasm`). A test compares the tool's result with that command applied directly to an equal document, and checks the concrete values this spec states.

**Conventions** inherited from `mcp-headless-pixel` and `mcp-parity-structure`: frame indexes are 0-based; layers are addressed by the ids `layer.list` reports; slices and tilesets by the ids `slice.list` and `tileset.list` report; tiles by their tile id within the tileset (tile 0 is the tileset's empty tile). Color arguments follow `mcp-headless-pixel` AC-21, and colors in results use the form of `palette.get` (`#RRGGBBAA`). Rectangles are `{x, y, width, height}` and points `{x, y}`, as in `doc.read_pixels`' `region`. A slice key's 9-patch center and pivot are relative to the key's bounds origin, as in `slices.md` and the Aseprite format.

All tools of this spec are Pixel-only: the Image editor has no slices, tilesets or document grid. On an Image document they fail with the mode error of `mcp-headless-image` AC-05.

## Scope

### In scope

- Slices: `slice.list`, `slice.add`, `slice.remove`, `slice.set_key` (per-frame key with bounds, 9-patch center and pivot)
- Tilesets and tiles: `tileset.list`, `tileset.add`, `tile.add`, `tile.read_pixels`, `tile.set_pixels`
- Tilemap layers: `layer.add` with `kind: "tilemap"` and `tileset`; `layer.list` reports the tileset of each tilemap layer
- Tilemap cels: `tilemap.get` (read the cells of a tilemap layer in a frame), `tilemap.place` (a batch of tile placements)
- The document grid: `grid.set`; `doc.info` / `doc.list` report the grid
- `doc.info` / `doc.list` report the slice count and the tileset count of a Pixel document
- What `draw.*`, `effect.apply` and `doc.read_pixels` do with a tilemap layer
- `pincel://guide` documents the new tools

### Out of scope

- Renaming a slice or changing its color after creation, removing a single slice key — because no command and no editor UI exists for them (`slices.md` Out of scope); parity means reaching what the editor can do
- Slice user data — because the editor neither shows nor edits it; the codec keeps it unchanged
- Removing, renaming or reordering tilesets and tiles, and external tileset files — because no command or UI exists for them (`tilemaps.md` Out of scope)
- Placing tiles flipped or rotated — because the editor's Tile Stamp always places tiles in canonical orientation and `pincel-wasm` exposes no flags; `tilemap.get` still reports the flags a file brought in
- Auto-tile (drawing on a tilemap layer at pixel resolution, creating or reusing tiles) — because `pincel.md` §5.3 defers it to Phase 2
- An image preview of a single tile or tileset — because `tile.read_pixels` returns a tile's exact values and `doc.preview` shows placed tiles; a tile preview is a view feature, not a document change
- Grid display state (show grid, show pixel grid, line color, opacity) — because it is a per-user view preference, not document state (`pixel-grid.md` Decisions), and the parity Decision Log entry excludes view state
- Tilesets, slices and the grid in Image documents — because the Image editor offers none of them
- Selection tools — because `mcp-parity-selection-transform` owns them

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains `slice.list`, `slice.add`, `slice.remove`, `slice.set_key`, `tileset.list`, `tileset.add`, `tile.add`, `tile.read_pixels`, `tile.set_pixels`, `tilemap.get`, `tilemap.place` and `grid.set`, each meeting `mcp-headless-pixel` AC-02.
- [ ] AC-02 (error): If a tool of this spec, or `layer.add` with `kind: "tilemap"`, is called on an Image document, then the call fails with `isError: true`, the message names the document's mode, and the document is unchanged.
- [ ] AC-03 (error): If a tool of this spec names in a top-level argument a slice, tileset, tile, layer or frame that does not exist, then the call fails with `isError: true` naming the argument and its value, and the document is unchanged. Entries of a batch are covered by AC-18, AC-24 and AC-35 instead.
- [ ] AC-04 (event): When the client calls `slice.list`, then each slice is reported in document order with `id`, `name`, `color` and `keys` in frame order, each key with `frame`, `bounds`, `center` (or `null`) and `pivot` (or `null`).
- [ ] AC-05 (event): When the client calls `slice.add` with a `name` and optional `bounds` and `color`, then a slice with that name (leading and trailing whitespace removed, as the editor's form does), one key at frame 0 with those bounds and neither center nor pivot, and that color is appended to `slice.list`, and the result carries its id; without `bounds` it covers the whole canvas and without `color` it is `#3B82F6FF`, as the editor's Add slice form.
- [ ] AC-06 (error): If `slice.add` gets an empty or whitespace-only name, `bounds` with a width or height below 1, or a malformed `color` (`mcp-headless-pixel` AC-21), then the call fails with `isError: true` naming the argument, and no slice is added.
- [ ] AC-07 (event): When the client calls `slice.remove` with a slice, then `slice.list` no longer reports it, and one `doc.undo` restores it at its previous position in the list.
- [ ] AC-08 (event): When the client calls `slice.set_key` with a slice, a `frame` (default 0) and at least one of `bounds`, `center` and `pivot`, then the slice has exactly one key at that frame, inserted in frame order or replacing the key that frame had, and every other key is unchanged.
- [ ] AC-09 (event): When `slice.set_key` omits `bounds`, `center` or `pivot`, then the new key takes that field from the key in effect at that frame (the key at that frame, else the nearest key at an earlier frame); `center: null` or `pivot: null` stores the key without it.
- [ ] AC-10 (error): If `slice.set_key` gets none of `bounds`, `center` and `pivot`, a `bounds` or `center` with a width or height below 1, a `center` or `pivot` missing one of its fields, or omits `bounds` at a frame before the slice's first key, then the call fails with `isError: true` naming the argument, and the slice is unchanged.
- [ ] AC-11 (event): When the client calls `tileset.list`, then each tileset is reported in document order with `id`, `name`, `tile_width`, `tile_height` and `tile_count`, the count including tile 0.
- [ ] AC-12 (event): When the client calls `tileset.add` with a `name`, `tile_width` and `tile_height`, then `tileset.list` reports a new tileset with that name (leading and trailing whitespace removed, as the editor's form does) and tile size holding one fully transparent tile 0, and the result carries its id.
- [ ] AC-13 (error): If `tileset.add` gets an empty or whitespace-only name, or a tile width or height outside 1–4096, then the call fails with `isError: true` naming the argument, and no tileset is added.
- [ ] AC-14 (event): When the client calls `tile.add` with a tileset, then a fully transparent tile of the tileset's tile size is appended, the result carries its tile id (the previous tile count), and `tileset.list` reports the count one higher.
- [ ] AC-15 (event): When the client calls `tile.read_pixels` with a tileset, a tile and `format` (`hex` or `index`), then the result has one text row per pixel row of the tile (or of the given `region` within the tile) with one color per pixel, in the forms of `mcp-headless-pixel` AC-13 and AC-44.
- [ ] AC-16 (error): If `tile.read_pixels` gets a `region` outside the tile, or the region (the whole tile without `region`) is larger than 64×64 in either dimension, then the call fails with `isError: true` stating the tile size or the 64×64 limit and suggesting `region`.
- [ ] AC-17 (event): When the client calls `tile.set_pixels` with a tileset, a tile and a batch of `{x, y, color}` in tile coordinates, then each listed pixel of that tile has its color and no other pixel of any tile changed, and every cel that references the tile, on every tilemap layer and frame, composes with the new pixels.
- [ ] AC-18 (error): If any pixel of a `tile.set_pixels` batch lies outside the tile, or its color is malformed, then the call fails with `isError: true` naming the first offending pixel and the tile size (or the color argument), and the tileset is unchanged.
- [ ] AC-19 (event): When the client calls `layer.add` with `kind: "tilemap"` and a `tileset`, then a tilemap layer bound to that tileset, named `<tileset name> Layer` unless `name` is given, is added at the position the other `layer.add` arguments give (`mcp-parity-structure` AC-03, AC-04, `parent`), with an empty grid of ⌈canvas width ∕ tile width⌉ × ⌈canvas height ∕ tile height⌉ cells on every frame; it becomes the active layer and the result carries its id.
- [ ] AC-20 (error): If `layer.add` gets `tileset` without `kind: "tilemap"`, or `kind: "tilemap"` without `tileset`, then the call fails with `isError: true` naming the argument, and no layer is added.
- [ ] AC-21 (always): `layer.list` reports for each layer a `tileset`: the tileset id of a tilemap layer, `null` for every other layer.
- [ ] AC-22 (event): When the client calls `tilemap.get` with a tilemap `layer` and optional `frame` (default the active frame), then the result carries `grid_width`, `grid_height`, `rows` (one string per grid row, top to bottom, holding each cell's tile id in decimal, separated by single spaces) and `oriented` (one `{col, row, flip_x, flip_y, rotate_90}` per cell whose tile has any of these flags set, empty otherwise); a frame where the layer has no grid yet reports the empty grid of AC-19's size, all tile ids 0.
- [ ] AC-23 (event): When the client calls `tilemap.place` with a tilemap `layer`, optional `frame` (default the active frame) and a batch of `{col, row, tile}`, then each listed cell holds that tile in canonical orientation (`tile: 0` empties the cell) and no other cell changed; on a frame where the layer has no grid yet, an empty grid is created first, as the editor's Tile Stamp does, unless every entry places tile 0 (the missing grid already reads as empty, so nothing changes and no grid is created).
- [ ] AC-24 (error): If `tilemap.place` names a layer that is not a tilemap layer, a cell outside the grid of that layer's cel in that frame (AC-19's size when the frame has no grid yet; an opened file's cel may differ), or a tile id that is not below its tileset's tile count, then the call fails with `isError: true`; for a batch entry the message names the entry's position in the batch, the offending field and its value, and the grid size or the tile count. The document is unchanged.
- [ ] AC-25 (error): If `tilemap.place` targets a locked tilemap layer, or a layer inside a locked group, then the call fails as in `mcp-headless-pixel` AC-28; `tile.set_pixels` and `tile.add` work whatever the lock state of the layers that use the tileset, as the editor's Tile editor does.
- [ ] AC-26 (error): If `draw.*` or `effect.apply` names a tilemap layer as `layer`, then the call fails with `isError: true` saying the layer is a tilemap layer and naming `tile.set_pixels` and `tilemap.place`, and the document is unchanged.
- [ ] AC-27 (event): When the client calls `doc.read_pixels` with a tilemap `layer`, then each pixel, in canvas coordinates, equals what the editor's compose draws for that layer alone in the given frame at full opacity with normal blending (tiles drawn with their flip and rotate flags; transparent where a cell holds tile 0 or the layer has no grid), not an error.
- [ ] AC-28 (event): When the client calls `grid.set` with `x`, `y`, `width` and `height` (all four required), then `doc.info` reports that grid, and after `doc.save` and `doc.open` of the file the grid is the same.
- [ ] AC-29 (error): If `grid.set` gets a `width` or `height` outside 1–65535, or an `x` or `y` outside −32768–32767, then the call fails with `isError: true` naming the argument and its range, and the grid is unchanged; a missing argument fails as a schema mismatch (`mcp-headless-pixel` AC-37).
- [ ] AC-30 (event): When the client calls `doc.info` or `doc.list`, then each Pixel document reports `slices` (the slice count), `tilesets` (the tileset count) and `grid` as `{x, y, width, height}`, which is `{0, 0, 16, 16}` for a new document and the stored header grid, a zero size included, for an opened file; Image documents report none of these fields.
- [ ] AC-31 (always): Every successful call of a tool of this spec that changes the document is exactly one undo step, as in `mcp-headless-pixel` AC-09, including a whole `tile.set_pixels` or `tilemap.place` batch; a successful call that leaves the document unchanged (an empty batch, `grid.set` with the current grid, placing the tile a cell already holds) adds no undo step.
- [ ] AC-32 (always): The `pincel://guide` resource contains the name of every tool of this spec and states (the test checks the terms in quotes): that they work on "Pixel" documents only; that "tile 0" is the empty tile, `tilemap.place` with `tile: 0` clears a cell, and painting tile 0 with `tile.set_pixels` shows in every empty cell; that slice centers and pivots are "relative" to the key's bounds; and that `draw.*` cannot paint on "tilemap" layers.
- [ ] AC-33 (error): If `tilemap.get` names a layer that is not a tilemap layer, or a grid more than 64 cells wide or more than 64 cells high, then the call fails with `isError: true` naming the layer's kind, or the grid size and the 64 × 64 limit.
- [ ] AC-34 (state): While the active layer is a tilemap layer, `draw.*` and `effect.apply` without `layer` act on the lowest image layer, as the editor's paint-target rule does (`mcp-headless-pixel` AC-43), and the result's `layer` names it; if the document has no image layer, the call fails with `isError: true` saying so, and the document is unchanged.
- [ ] AC-35 (error): If an entry of a `tile.set_pixels` or `tilemap.place` batch lacks a field, or a coordinate or tile id is not a non-negative integer, then the call fails with `isError: true` naming the entry's position and the field (`mcp-headless-pixel` AC-37), and the document is unchanged.

## Technical notes

Every capability exists already. Grep for these names, because line numbers drift.

| Tool | `pincel-core` command / data | `pincel-wasm` counterpart the editor uses |
| ---- | ---------------------------- | ----------------------------------------- |
| `slice.list` | `Sprite::slices`, `Slice`, `SliceKey` | `sliceCount`, `sliceIdAt`, `sliceName`, `sliceColor`, `sliceKey*` |
| `slice.add` | `AddSlice` (id = max + 1) | `addSlice` (name check is in `SlicesPanel.svelte`) |
| `slice.remove` | `RemoveSlice` | `removeSlice` |
| `slice.set_key` | `SetSliceKey` (upsert by frame; checks bounds, not the frame index) | `setSliceKey` |
| `tileset.list` / `tileset.add` | `Sprite::tilesets`, `AddTileset` with a stored transparent tile 0 | `tileset*` getters, `addTileset` (name check in `TilesetPanel.svelte`) |
| `tile.add` | `AddTile` (id = `tiles.len()`) | `addTile` |
| `tile.read_pixels` | `Tileset::tile` | `tilePixels` |
| `tile.set_pixels` | `SetTilePixel` per pixel, one `CommandBatch` | `setTilePixel` |
| `layer.add` tilemap | `AddTilemapLayer` (on top, seeded grids), then `MoveLayerTo` in the same batch | `addTilemapLayer`, `layerTilesetId` |
| `tilemap.get` | `CelData::Tilemap`, `TileRef` | — (the editor draws the cels) |
| `tilemap.place` | `PlaceTile` per cell (creates a missing grid), one `CommandBatch` | `placeTile` (lock check in wasm) |
| `grid.set` | `SetGrid`, `Metadata::grid_or_default` | `setGrid`, `gridX` / `gridY` / `gridWidth` / `gridHeight` |

Checks that live outside the core command and that the tools must make themselves: empty names (UI forms), the frame index of `slice.set_key`, the lock check of `tilemap.place` (wasm), and the tile id range of `tilemap.place` (`PlaceTile` does not check it; the editor only offers existing tiles).

Today `Doc::paint_target` refuses an explicit tilemap layer with "is not an image layer; pixel tools need an image layer" and `layer_pixels` in `doc.read_pixels` refuses a tilemap layer; AC-26 rewords the first, AC-27 replaces the second with the composed tiles (`render/tilemap_layer.rs`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `slice.add` with `bounds` partly or wholly outside the canvas | Allowed, as in the Aseprite format and the Slices panel's X / Y fields |
| `slice.set_key` at a frame between two keys | A key is inserted in frame order; it applies until the next key's frame |
| `slice.set_key` with a `center` outside the new bounds | Accepted; the center keeps its relative coordinates (`slices.md` Edge cases) |
| Two slices with the same name | Allowed, as in the Aseprite format; they keep distinct ids |
| `frame.remove` after `slice.set_key` on later frames | Keys follow the frame edit as in `slices.md` AC-18; `slice.list` shows the result |
| `tileset.add` with a tile size larger than the canvas | Allowed; a tilemap layer for it has a 1 × 1 grid |
| Canvas size not a multiple of the tile size | The grid rounds up; edge cells can be placed (`tilemaps.md` Edge cases) |
| `tilemap.place` on a frame added after the tilemap layer | The grid is created and the tiles placed, one undo step |
| `tilemap.place` with `tile: 0` on a cell that is already empty | Succeeds; document unchanged, no undo step (AC-31) |
| `tilemap.place` batch with the same cell twice | The later entry wins, as two placements in a row would |
| `tile.set_pixels` on tile 0 | Allowed, as the core allows it; every empty cell then shows those pixels, which the guide warns about |
| `tile.read_pixels` of a 128 × 128 tile without `region` | Fails as in AC-16, suggesting `region` |
| `tilemap.get` on a 4096 × 4096 canvas with 1 × 1 tiles | Fails as in AC-33, naming the 4096 × 4096 grid and the 64 × 64 limit; nothing is returned (see Assumptions) |
| `tilemap.get` on a 64 × 65 grid | Fails as in AC-33: the limit applies to each dimension |
| `tileset.add` with a name another tileset already has | Allowed, as in the editor's form and the Aseprite format; the tilesets keep distinct ids |
| `slice.add` with `name: "  hitbox "` | The slice is named `hitbox` |
| `tilemap.place` with `tile: 0` on a frame without a grid | Succeeds; no grid is created, document unchanged, no undo step (AC-23, AC-31) |
| `layer.remove` of a tilemap layer, `layer.duplicate` of one | Behave as `mcp-parity-structure` specifies for any layer; the tileset stays |
| `grid.set` with an offset larger than the cell size | Stored as given, as in `pixel-grid.md` |
| `doc.open` of a file whose header grid is 0 wide | `doc.info` reports width 0; `grid.set` can replace it with a valid grid |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `slice.add` requires `name`; the Slice tool's automatic `Slice {n}` is not reproduced | The Add slice form, whose defaults AC-05 follows, takes a name; an agent always has one to give | Low |
| `slice.set_key` keeps omitted fields from the key in effect at that frame | The Slices panel edits one field at a time and keeps the others; agents should not have to restate a whole key to move a pivot | Low |
| Tile size is limited to 1–4096 per side in `tileset.add` | The editor's form only checks for positive integers, but the `.aseprite` writer refuses sizes above 65535 and a single tile larger than the largest canvas (4096, `mcp-headless-pixel` AC-46) is never fully shown; the cap bounds one tile to 64 MiB | Low |
| `tile.read_pixels` reads at most 64 × 64 pixels per call (AC-16) | The same limit as `doc.read_pixels` (`mcp-headless-pixel` AC-14), so a text result never exceeds 4096 pixels | Low |
| `tilemap.get` returns at most 64 × 64 cells per call and offers no region | Keeps the text result as small as `doc.read_pixels` (64 × 64); a 4096-px canvas with 64-px tiles is exactly 64 × 64 cells | Medium: a larger grid needs a `region` argument later |
| `doc.read_pixels` on a tilemap layer composes only that layer's tiles, without its opacity and blend mode, in canvas coordinates | `doc.read_pixels` with `layer` returns the cel's own pixels in canvas coordinates for image layers; the tile pixels are a tilemap cel's own pixels | Low |
| Tile editing ignores layer locks | The Tile editor (`SetTilePixel`) has no lock check, and a tile is not a layer | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built  |        |
| AC-02 | `asset_tools_on_an_image_document_fail_naming_the_mode` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-03 | `naming_a_missing_asset_fails_with_argument_and_value` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-04 | `slice_add_list_and_remove_like_the_editor` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-05 | `slice_add_list_and_remove_like_the_editor` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-06 | `slice_add_with_bad_arguments_adds_nothing` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-07 | `slice_add_list_and_remove_like_the_editor` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-08 | `slice_set_key_upserts_by_frame_and_inherits_fields` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-09 | `slice_set_key_upserts_by_frame_and_inherits_fields` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-10 | `slice_set_key_with_bad_arguments_leaves_the_slice` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-11 | `tileset_tools_list_add_and_grow` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-12 | `tileset_tools_list_add_and_grow` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-13 | `tileset_add_with_bad_arguments_adds_nothing` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-14 | `tileset_tools_list_add_and_grow` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-15 | `tile_read_and_set_pixels_reach_every_cel_that_shows_the_tile` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-16 | `tile_read_pixels_refuses_outside_or_large_regions` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-17 | `tile_read_and_set_pixels_reach_every_cel_that_shows_the_tile` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-18 | `tile_set_pixels_bad_entries_leave_the_tileset` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-19 | `layer_add_tilemap_binds_the_tileset_with_an_empty_grid` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-20 | `layer_add_tileset_and_kind_tilemap_go_together` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-21 | `layer_add_tilemap_binds_the_tileset_with_an_empty_grid` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-22 | `tilemap_get_and_place_read_and_write_cells` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-23 | `tilemap_get_and_place_read_and_write_cells` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-24 | `tilemap_place_bad_entries_name_position_field_and_limit` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-25 | `tilemap_place_respects_locks_while_tile_edits_do_not` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-26 | `drawing_on_a_tilemap_layer_points_to_the_tile_tools` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-27 | `read_pixels_of_a_tilemap_layer_shows_its_tiles` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-28 | `grid_set_is_reported_and_saved` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-29 | `grid_set_out_of_range_leaves_the_grid` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-30 | `doc_info_reports_slice_and_tileset_counts_and_grid`, `grid_set_is_reported_and_saved`, `an_opened_files_zero_size_header_grid_is_reported_and_replaceable` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-31 | `asset_edits_record_one_undo_step_and_none_without_a_change` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-32 | `read_resource_guide_covers_workflow_and_both_modes` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-33 | `tilemap_get_refuses_other_layers_and_large_grids` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-34 | `a_tilemap_active_layer_leaves_drawing_on_the_lowest_image_layer` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |
| AC-35 | `tile_set_pixels_bad_entries_leave_the_tileset`, `tilemap_place_bad_entries_name_position_field_and_limit` (`crates/pincel-agent/src/tests/assets.rs`) | built  |        |

## Decisions

- 2026-11-05: Tilemap layers are added through `layer.add` with `kind: "tilemap"` and `tileset`, not a separate `tilemap.add_layer` — the parity rule gives one tool per capability, and `mcp-parity-structure` already adds groups through `kind: "group"`; the placement arguments then work for every layer kind.
- 2026-11-05: `draw.*` and `effect.apply` refuse an explicit tilemap layer and keep the editor's paint-target rule otherwise (AC-26) — the editor's image tools never paint on a tilemap layer (they fall back to the lowest image layer, `pixel-layers-frames`), auto-tile is Phase 2, and a precise refusal that names the tile tools lets the model correct itself.
- 2026-11-05: `tile.set_pixels` and `tilemap.place` take batches and are one undo step per call — the editor issues one command per pixel or click, but an agent call is one unit (`mcp-headless-pixel` AC-09), and batching mirrors `draw.pixels`; the result per pixel or cell equals the editor's.
- 2026-11-05: The grid gets its own `grid.set` tool and is read through `doc.info` — the grid is one small document property, and `doc.info` is where agents look first; a `grid.get` would duplicate it.
- 2026-11-05: `tilemap.place` with `tile: 0` clears the cell (AC-23), although the editor offers no tile eraser — owner's choice, because agents must be able to correct a single cell without undoing every later step, the core `PlaceTile` already accepts the empty tile so no core work is needed, and Aseprite's eraser does the same on tilemap layers.
- 2026-11-05: `slice.set_key` defaults to frame 0, `tilemap.get` and `tilemap.place` to the active frame — each follows what the editor edits: the Slices panel and the Slice tool edit only the first-frame key (`slices.md`), while the Tile Stamp places on the current frame (`tilemaps.md` AC-10), as `draw.*` does (`mcp-headless-pixel` AC-43).
- 2026-11-05: Names given to `slice.add` and `tileset.add` are trimmed — the editor's forms trim them (`formName.trim()`), and an agent should get the same name a user typing it would.
- 2026-11-05: `grid.set` requires all four values — the editor's Grid Settings dialog commits cell size and offset together, and the current grid is in `doc.info`, so a partial update would only add ambiguity.
- 2026-11-05: `tilemap.get` reports tile ids as space-separated decimal rows and lists only the flipped or rotated cells separately — the rows stay as compact as `doc.read_pixels`' text rows, and flags are rare (only opened files carry them).
- 2026-11-05 (clarification pass): a `tilemap.place` call that only places tile 0 on a frame without a grid creates no grid and records no step (AC-23) — a missing grid already reads as empty (AC-22), so creating it would change nothing visible and break "no change, no undo step" (AC-31).
- 2026-11-05: Tool areas `slice.*`, `tileset.*`, `tile.*`, `tilemap.*` and `grid.*` — each names the object the verb acts on, as `layer.*` and `tag.*` do.
- 2026-11-05 (spec-build): A tilemap layer added by `layer.add` becomes the agent's active layer, while `doc.info`'s `active_layer` keeps reporting the paint target (the editor's paint-target rule, AC-34), so drawing tools still land on an image layer.
- 2026-11-05 (spec-build): `tile.set_pixels` and `tilemap.place` batches are checked entry by entry before anything runs and committed as one `CommandBatch` through the dry-run rule of `mcp-parity-structure`, so an unchanged batch records no step (AC-31).
- 2026-11-06 (spec-build): `an_opened_files_zero_size_header_grid_is_reported_and_replaceable` (AC-30, an opened file's header grid of width 0) was green at once, because the behavior existed; it went red against a temporary break (`Doc::info` replacing a zero-size grid with the 16×16 default: "left: {…width: 16…} right: {…width: 0…}"), which was reverted.
