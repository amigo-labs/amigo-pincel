//! `slice.*`, `tileset.*`, `tile.*`, `tilemap.*` and `grid.set`: the Pixel
//! editor's slices, tilesets / tilemaps and document grid. See
//! docs/specs/mcp-parity-pixel-assets.md.

use pincel_core::AddTileset;
use pincel_core::{
    AddSlice, AddTile, AnyCommand, CelData, ColorMode, FrameIndex, LayerId, LayerKind, PixelBuffer,
    PlaceTile, Rect, RemoveSlice, Rgba, SetGrid, SetSliceKey, SetTilePixel, Slice, SliceId,
    SliceKey, TileImage, TileRef, Tileset, TilesetId,
};
use serde_json::{Value, json};

use super::doc::format_rows;
use super::{READ_MAX, frame_prop, tool_schema};
use crate::args::{Args, color_schema, hex, parse_color, parse_region, region_schema};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Every tool of this module, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    SLICE_LIST,
    SLICE_ADD,
    SLICE_REMOVE,
    SLICE_SET_KEY,
    TILESET_LIST,
    TILESET_ADD,
    TILE_ADD,
    TILE_READ_PIXELS,
    TILE_SET_PIXELS,
    TILEMAP_GET,
    TILEMAP_PLACE,
    GRID_SET,
];

/// The editor's Add slice color.
const SLICE_COLOR: u32 = 0x3B82_F6FF;
/// The largest grid `tilemap.get` returns.
const GRID_MAX: u32 = 64;

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "width": r.width, "height": r.height })
}

fn slice_json(s: &Slice) -> Value {
    let keys: Vec<Value> = s
        .keys
        .iter()
        .map(|k| {
            json!({
                "frame": k.frame.0,
                "bounds": rect_json(k.bounds),
                "center": k.center.map(rect_json),
                "pivot": k.pivot.map(|(x, y)| json!({ "x": x, "y": y })),
            })
        })
        .collect();
    json!({ "id": s.id.0, "name": s.name, "color": hex(s.color), "keys": keys })
}

fn slices_out(id: &str, doc: &Doc) -> Value {
    let slices: Vec<Value> = doc.sprite.slices.iter().map(slice_json).collect();
    json!({ "doc": id, "slices": slices })
}

fn tileset_json(t: &Tileset) -> Value {
    json!({
        "id": t.id.0,
        "name": t.name,
        "tile_width": t.tile_size.0,
        "tile_height": t.tile_size.1,
        "tile_count": t.tile_count(),
    })
}

/// A trimmed, non-empty `name` (the editor's forms trim and refuse empty
/// names).
fn name_arg(args: &Args) -> Result<String, AgentError> {
    let name = args.req_str("name")?.trim();
    if name.is_empty() {
        return Err(AgentError::tool("argument `name` must not be empty"));
    }
    Ok(name.to_owned())
}

fn slice_arg(doc: &Doc, args: &Args) -> Result<SliceId, AgentError> {
    let id = args.req_u32("slice")?;
    doc.sprite
        .slices
        .iter()
        .find(|s| s.id.0 == id)
        .map(|s| s.id)
        .ok_or_else(|| {
            AgentError::tool(format!(
                "argument `slice`: no slice with id {id}; slice.list shows the ids"
            ))
        })
}

fn tileset_arg<'a>(doc: &'a Doc, args: &Args) -> Result<&'a Tileset, AgentError> {
    let id = args.req_u32("tileset")?;
    doc.sprite.tileset(TilesetId::new(id)).ok_or_else(|| {
        AgentError::tool(format!(
            "argument `tileset`: no tileset with id {id}; tileset.list shows the ids"
        ))
    })
}

fn tile_arg(ts: &Tileset, args: &Args) -> Result<u32, AgentError> {
    let tile = args.req_u32("tile")?;
    if (tile as usize) < ts.tile_count() {
        Ok(tile)
    } else {
        Err(AgentError::tool(format!(
            "argument `tile`: no tile {tile} in tileset {} ({} tiles, ids 0–{})",
            ts.id.0,
            ts.tile_count(),
            ts.tile_count().saturating_sub(1)
        )))
    }
}

pub(super) const SLICE_LIST: Tool = Tool {
    name: "slice.list",
    description: "Pixel documents: list the slices with id, name, color and keys (frame, bounds, 9-patch center and pivot, both relative to the key's bounds).",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(ToolOutput::json(slices_out(&id, doc)))
    },
};

pub(super) const SLICE_ADD: Tool = Tool {
    name: "slice.add",
    description: "Pixel documents: add a slice named `name` with one key at frame 0 over `bounds` (default the whole canvas) and `color` (default #3B82F6FF), like the editor's Add slice form. Returns its id.",
    schema: || {
        tool_schema(
            json!({
                "name": { "type": "string", "description": "Slice name" },
                "bounds": region_schema("Slice rectangle in canvas px; default the whole canvas"),
                "color": color_schema("Overlay color; default #3B82F6FF"),
            }),
            &["name"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let name = name_arg(args)?;
        let bounds = match args.get("bounds") {
            Some(b) => parse_region(b)?,
            None => Rect::new(0, 0, doc.sprite.width, doc.sprite.height),
        };
        let color = match args.get("color") {
            Some(c) => parse_color(c, "color", &doc.sprite.palette)?,
            None => Rgba::from_u32(SLICE_COLOR),
        };
        let new_id = match doc.sprite.slices.iter().map(|s| s.id.0).max() {
            None => 0,
            Some(u32::MAX) => return Err(AgentError::tool("slice id space exhausted")),
            Some(m) => m + 1,
        };
        let slice = Slice {
            id: SliceId::new(new_id),
            name,
            color,
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds,
                center: None,
                pivot: None,
            }],
            user_data: None,
        };
        doc.commit(vec![AddSlice::new(slice).into()])?;
        let mut out = slices_out(&id, doc);
        out["slice"] = json!(new_id);
        Ok(ToolOutput::json(out))
    },
};

pub(super) const SLICE_REMOVE: Tool = Tool {
    name: "slice.remove",
    description: "Pixel documents: remove a slice with all its keys.",
    schema: || {
        tool_schema(
            json!({ "slice": { "type": "integer", "minimum": 0, "description": "Slice id from slice.list" } }),
            &["slice"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let slice = slice_arg(doc, args)?;
        doc.commit(vec![RemoveSlice::new(slice).into()])?;
        Ok(ToolOutput::json(slices_out(&id, doc)))
    },
};

pub(super) const SLICE_SET_KEY: Tool = Tool {
    name: "slice.set_key",
    description: "Pixel documents: set a slice's key at `frame` (default 0), inserting it or replacing that frame's key. Fields not given come from the key in effect at that frame; `center: null` / `pivot: null` remove them. `center` (9-patch inner rectangle) and `pivot` are relative to the key's bounds.",
    schema: || {
        tool_schema(
            json!({
                "slice": { "type": "integer", "minimum": 0, "description": "Slice id from slice.list" },
                "frame": { "type": "integer", "minimum": 0, "description": "Frame index; default 0" },
                "bounds": region_schema("Slice rectangle in canvas px"),
                "center": {
                    "anyOf": [region_schema("9-patch center, relative to the bounds"), { "type": "null" }],
                    "description": "9-patch center {x, y, width, height} relative to the bounds, or null to remove it",
                },
                "pivot": {
                    "anyOf": [
                        {
                            "type": "object",
                            "properties": { "x": { "type": "integer" }, "y": { "type": "integer" } },
                            "required": ["x", "y"],
                            "additionalProperties": false,
                        },
                        { "type": "null" },
                    ],
                    "description": "Pivot {x, y} relative to the bounds, or null to remove it",
                },
            }),
            &["slice"],
        )
    },
    run: slice_set_key,
};

fn slice_set_key(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let slice_id = slice_arg(doc, args)?;
    let frame = doc.check_frame(args.u32("frame")?.unwrap_or(0))?;
    let given = |k: &str| args.get(k).is_some() || args.is_null(k);
    if !given("bounds") && !given("center") && !given("pivot") {
        return Err(AgentError::tool(
            "slice.set_key needs at least one of `bounds`, `center` and `pivot`",
        ));
    }
    let slice = doc
        .sprite
        .slices
        .iter()
        .find(|s| s.id == slice_id)
        .ok_or_else(|| AgentError::tool("the slice vanished"))?;
    let current = slice.keys.iter().rev().find(|k| k.frame <= frame).copied();
    let bounds = match args.get("bounds") {
        Some(b) => parse_region(b)?,
        None => current.map(|k| k.bounds).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `bounds` is needed: the slice has no key at or before frame {}",
                frame.0
            ))
        })?,
    };
    let center = match args.get("center") {
        Some(c) => {
            Some(parse_region(c).map_err(|e| AgentError::tool(format!("argument `center`: {e}")))?)
        }
        None if args.is_null("center") => None,
        None => current.and_then(|k| k.center),
    };
    let pivot = match args.get("pivot") {
        Some(p) => Some((
            crate::args::int_i32(p.get("x"), "pivot.x")?,
            crate::args::int_i32(p.get("y"), "pivot.y")?,
        )),
        None if args.is_null("pivot") => None,
        None => current.and_then(|k| k.pivot),
    };
    let key = SliceKey {
        frame,
        bounds,
        center,
        pivot,
    };
    doc.commit_if_changed(|| vec![SetSliceKey::new(slice_id, key).into()])?;
    Ok(ToolOutput::json(slices_out(&id, doc)))
}

pub(super) const TILESET_LIST: Tool = Tool {
    name: "tileset.list",
    description: "Pixel documents: list the tilesets with id, name, tile size and tile count (tile 0, the empty tile, included).",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        let sets: Vec<Value> = doc.sprite.tilesets.iter().map(tileset_json).collect();
        Ok(ToolOutput::json(json!({ "doc": id, "tilesets": sets })))
    },
};

pub(super) const TILESET_ADD: Tool = Tool {
    name: "tileset.add",
    description: "Pixel documents: add a tileset named `name` with tiles of tile_width × tile_height px (1–4096) holding the empty tile 0, like the editor's New tileset form. Returns its id.",
    schema: || {
        tool_schema(
            json!({
                "name": { "type": "string", "description": "Tileset name" },
                "tile_width": { "type": "integer", "minimum": 1, "maximum": 4096, "description": "Tile width in px" },
                "tile_height": { "type": "integer", "minimum": 1, "maximum": 4096, "description": "Tile height in px" },
            }),
            &["name", "tile_width", "tile_height"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let name = name_arg(args)?;
        let (w, h) = (args.req_u32("tile_width")?, args.req_u32("tile_height")?);
        let new_id = match doc.sprite.tilesets.iter().map(|t| t.id.0).max() {
            None => 0,
            Some(u32::MAX) => return Err(AgentError::tool("tileset id space exhausted")),
            Some(m) => m + 1,
        };
        let mut tileset = Tileset::new(TilesetId::new(new_id), name, (w, h));
        tileset.tiles.push(TileImage {
            pixels: PixelBuffer::empty(w, h, ColorMode::Rgba),
        });
        doc.commit(vec![AddTileset::new(tileset).into()])?;
        let sets: Vec<Value> = doc.sprite.tilesets.iter().map(tileset_json).collect();
        Ok(ToolOutput::json(
            json!({ "doc": id, "tileset": new_id, "tilesets": sets }),
        ))
    },
};

fn tileset_prop() -> Value {
    json!({ "type": "integer", "minimum": 0, "description": "Tileset id from tileset.list" })
}

fn tile_prop() -> Value {
    json!({ "type": "integer", "minimum": 0, "description": "Tile id within the tileset (0 is the empty tile)" })
}

pub(super) const TILE_ADD: Tool = Tool {
    name: "tile.add",
    description: "Pixel documents: append a transparent tile to a tileset; returns its tile id.",
    schema: || tool_schema(json!({ "tileset": tileset_prop() }), &["tileset"]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let ts = tileset_arg(doc, args)?;
        let (ts_id, tile) = (ts.id, ts.tile_count());
        doc.commit(vec![AddTile::new(ts_id).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "tileset": ts_id.0, "tile": tile }),
        ))
    },
};

pub(super) const TILE_READ_PIXELS: Tool = Tool {
    name: "tile.read_pixels",
    description: "Pixel documents: read a tile's colors as text rows (at most 64×64; `region` in tile coordinates), format hex (#RRGGBBAA) or index (first palette index with that color, or -).",
    schema: || {
        tool_schema(
            json!({
                "tileset": tileset_prop(),
                "tile": tile_prop(),
                "region": region_schema("Rectangle within the tile, at most 64×64; default the whole tile"),
                "format": { "type": "string", "enum": ["hex", "index"], "description": "Pixel format; default hex" },
            }),
            &["tileset", "tile"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        let ts = tileset_arg(doc, args)?;
        let tile = tile_arg(ts, args)?;
        let (tw, th) = ts.tile_size;
        let r = match args.get("region") {
            Some(v) => parse_region(v)?,
            None => Rect::new(0, 0, tw, th),
        };
        if !crate::args::inside_canvas(r, tw, th) {
            return Err(AgentError::tool(format!(
                "region ({}, {}, {}×{}) lies outside the {tw}×{th} tile",
                r.x, r.y, r.width, r.height
            )));
        }
        if r.width > READ_MAX || r.height > READ_MAX {
            return Err(AgentError::tool(format!(
                "{}×{} px is larger than the {READ_MAX}×{READ_MAX} limit; read the {tw}×{th} tile in parts with `region`",
                r.width, r.height
            )));
        }
        let buf = &ts.tiles[tile as usize].pixels;
        let mut px = Vec::with_capacity(r.width as usize * r.height as usize * 4);
        for y in r.y..r.y + r.height as i32 {
            for x in r.x..r.x + r.width as i32 {
                let i = ((y as u32 * buf.width + x as u32) * 4) as usize;
                px.extend_from_slice(buf.data.get(i..i + 4).unwrap_or(&[0; 4]));
            }
        }
        let index = args.str("format") == Some("index");
        Ok(ToolOutput::json(json!({
            "doc": id,
            "tileset": ts.id.0,
            "tile": tile,
            "region": rect_json(r),
            "format": if index { "index" } else { "hex" },
            "rows": format_rows(&px, r.width, index, &doc.sprite.palette),
        })))
    },
};

pub(super) const TILE_SET_PIXELS: Tool = Tool {
    name: "tile.set_pixels",
    description: "Pixel documents: set pixels of a tile ({x, y, color} in tile coordinates), like the editor's Tile editor; every tilemap cell that shows the tile changes with it. One undo step for the whole batch. Painting tile 0 shows in every empty cell.",
    schema: || {
        tool_schema(
            json!({
                "tileset": tileset_prop(),
                "tile": tile_prop(),
                "pixels": {
                    "type": "array",
                    "items": crate::schema::object(
                        json!({
                            "x": { "type": "integer", "minimum": 0 },
                            "y": { "type": "integer", "minimum": 0 },
                            "color": color_schema("Pixel color"),
                        }),
                        &["x", "y", "color"],
                    ),
                    "description": "Pixels to set, in tile coordinates",
                },
            }),
            &["tileset", "tile", "pixels"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let ts = tileset_arg(doc, args)?;
        let tile = tile_arg(ts, args)?;
        let (tw, th) = ts.tile_size;
        let ts_id = ts.id;
        let entries = args
            .get("pixels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut sets = Vec::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            let x = e.get("x").and_then(Value::as_u64).unwrap_or(u64::MAX);
            let y = e.get("y").and_then(Value::as_u64).unwrap_or(u64::MAX);
            if x >= u64::from(tw) || y >= u64::from(th) {
                return Err(AgentError::tool(format!(
                    "pixels[{i}] ({x}, {y}) lies outside the {tw}×{th} tile"
                )));
            }
            let color = parse_color(
                e.get("color").unwrap_or(&Value::Null),
                &format!("pixels[{i}].color"),
                &doc.sprite.palette,
            )?;
            sets.push((x as u32, y as u32, color));
        }
        let changed = doc.commit_if_changed(|| {
            sets.iter()
                .map(|&(x, y, c)| SetTilePixel::new(ts_id, tile, x, y, c).into())
                .collect()
        })?;
        Ok(ToolOutput::json(json!({ "doc": id, "changed": changed })))
    },
};

/// The tilemap layer named by `layer`, with its tileset (AC-24, AC-33).
fn tilemap_arg(doc: &Doc, args: &Args) -> Result<(LayerId, TilesetId), AgentError> {
    let lid = super::layer_id_arg(doc, args, "layer")?;
    let layer = doc
        .sprite
        .layer(lid)
        .ok_or_else(|| AgentError::tool("the layer vanished"))?;
    match layer.kind {
        LayerKind::Tilemap { tileset_id } => Ok((lid, tileset_id)),
        LayerKind::Image => Err(AgentError::tool(format!(
            "argument `layer`: layer {} is an image layer, not a tilemap layer",
            lid.0
        ))),
        LayerKind::Group => Err(AgentError::tool(format!(
            "argument `layer`: layer {} is a group, not a tilemap layer",
            lid.0
        ))),
    }
}

/// The grid of the tilemap cel at `frame`: its size and cells, or the
/// empty grid a new layer gets (AC-19, AC-22).
fn grid_of(
    doc: &Doc,
    layer: LayerId,
    ts: TilesetId,
    frame: FrameIndex,
) -> (u32, u32, Vec<TileRef>, bool) {
    if let Some(cel) = doc.cels.get(layer, frame)
        && let CelData::Tilemap {
            grid_w,
            grid_h,
            tiles,
        } = &cel.data
    {
        return (*grid_w, *grid_h, tiles.clone(), true);
    }
    let (tw, th) = doc.sprite.tileset(ts).map_or((1, 1), |t| t.tile_size);
    let (w, h) = (
        doc.sprite.width.div_ceil(tw.max(1)),
        doc.sprite.height.div_ceil(th.max(1)),
    );
    (w, h, vec![TileRef::EMPTY; (w * h) as usize], false)
}

fn tilemap_target_schema(extra: Value) -> Value {
    let mut props = json!({
        "layer": { "type": "integer", "minimum": 0, "description": "Id of a tilemap layer" },
        "frame": frame_prop(),
    });
    if let (Some(p), Value::Object(e)) = (props.as_object_mut(), extra) {
        p.extend(e);
    }
    let required: &[&str] = if props.get("cells").is_some() {
        &["layer", "cells"]
    } else {
        &["layer"]
    };
    tool_schema(props, required)
}

pub(super) const TILEMAP_GET: Tool = Tool {
    name: "tilemap.get",
    description: "Pixel documents: read a tilemap layer's grid at `frame` (default the active frame): `rows` of tile ids (0 = the empty tile) and the cells whose tile is flipped or rotated. At most 64×64 cells.",
    schema: || tilemap_target_schema(json!({})),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        let (layer, ts) = tilemap_arg(doc, args)?;
        let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
        let (w, h, tiles, _) = grid_of(doc, layer, ts, frame);
        if w > GRID_MAX || h > GRID_MAX {
            return Err(AgentError::tool(format!(
                "the grid is {w}×{h} cells, above the {GRID_MAX}×{GRID_MAX} limit of tilemap.get"
            )));
        }
        let rows: Vec<String> = tiles
            .chunks(w.max(1) as usize)
            .map(|row| {
                row.iter()
                    .map(|t| t.tile_id.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        let oriented: Vec<Value> = tiles
            .iter()
            .enumerate()
            .filter(|(_, t)| t.flip_x || t.flip_y || t.rotate_90)
            .map(|(i, t)| {
                json!({
                    "col": i as u32 % w, "row": i as u32 / w,
                    "flip_x": t.flip_x, "flip_y": t.flip_y, "rotate_90": t.rotate_90,
                })
            })
            .collect();
        Ok(ToolOutput::json(json!({
            "doc": id,
            "layer": layer.0,
            "frame": frame.0,
            "grid_width": w,
            "grid_height": h,
            "rows": rows,
            "oriented": oriented,
        })))
    },
};

pub(super) const TILEMAP_PLACE: Tool = Tool {
    name: "tilemap.place",
    description: "Pixel documents: put tiles into a tilemap layer's grid at `frame` (default the active frame), like the editor's Tile Stamp: `cells` of {col, row, tile}; `tile: 0` clears a cell. One undo step for the whole batch.",
    schema: || {
        tilemap_target_schema(json!({
            "cells": {
                "type": "array",
                "items": crate::schema::object(
                    json!({
                        "col": { "type": "integer", "minimum": 0 },
                        "row": { "type": "integer", "minimum": 0 },
                        "tile": { "type": "integer", "minimum": 0 },
                    }),
                    &["col", "row", "tile"],
                ),
                "description": "Cells to set",
            },
        }))
    },
    run: tilemap_place,
};

fn tilemap_place(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let (layer, ts) = tilemap_arg(doc, args)?;
    doc.check_unlocked(layer)?;
    let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
    let (w, h, _, has_grid) = grid_of(doc, layer, ts, frame);
    let count = doc.sprite.tileset(ts).map_or(0, Tileset::tile_count) as u64;
    let entries = args
        .get("cells")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut cells = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let get = |k: &str| e.get(k).and_then(Value::as_u64).unwrap_or(u64::MAX);
        let (col, row, tile) = (get("col"), get("row"), get("tile"));
        if col >= u64::from(w) {
            return Err(AgentError::tool(format!(
                "cells[{i}].col {col} lies outside the {w}×{h} grid"
            )));
        }
        if row >= u64::from(h) {
            return Err(AgentError::tool(format!(
                "cells[{i}].row {row} lies outside the {w}×{h} grid"
            )));
        }
        if tile >= count {
            return Err(AgentError::tool(format!(
                "cells[{i}].tile {tile} is not in tileset {} ({count} tiles)",
                ts.0
            )));
        }
        cells.push((col as u32, row as u32, tile as u32));
    }
    if !has_grid && cells.iter().all(|c| c.2 == 0) {
        // A missing grid already reads as empty.
        return Ok(ToolOutput::json(json!({ "doc": id, "changed": false })));
    }
    let changed = doc.commit_if_changed(|| {
        cells
            .iter()
            .map(|&(c, r, t)| -> AnyCommand {
                PlaceTile::new(
                    layer,
                    frame,
                    c,
                    r,
                    TileRef {
                        tile_id: t,
                        ..TileRef::EMPTY
                    },
                )
                .into()
            })
            .collect()
    })?;
    Ok(ToolOutput::json(json!({ "doc": id, "changed": changed })))
}

pub(super) const GRID_SET: Tool = Tool {
    name: "grid.set",
    description: "Pixel documents: set the document grid (stored in the .aseprite header): origin x, y and cell width × height.",
    schema: || {
        tool_schema(
            json!({
                "x": { "type": "integer", "minimum": -32768, "maximum": 32767, "description": "Grid origin x" },
                "y": { "type": "integer", "minimum": -32768, "maximum": 32767, "description": "Grid origin y" },
                "width": { "type": "integer", "minimum": 1, "maximum": 65535, "description": "Cell width in px" },
                "height": { "type": "integer", "minimum": 1, "maximum": 65535, "description": "Cell height in px" },
            }),
            &["x", "y", "width", "height"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let grid = Rect::new(
            args.req_i32("x")?,
            args.req_i32("y")?,
            args.req_u32("width")?,
            args.req_u32("height")?,
        );
        let changed = doc.commit_if_changed(|| vec![SetGrid::new(grid).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "grid": rect_json(grid), "changed": changed }),
        ))
    },
};
