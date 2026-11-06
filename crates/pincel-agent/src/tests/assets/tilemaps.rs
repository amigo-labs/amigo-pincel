//! Tilemap layers and cels, and the tools around them (AC-19 … AC-27, AC-33, AC-34).

use super::*;

#[test]
fn layer_add_tilemap_binds_the_tileset_with_an_empty_grid() {
    let mut s = S::new();
    let id = tiled(&mut s);
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    assert_eq!(list[1]["kind"], "tilemap");
    assert_eq!(list[1]["name"], "ground Layer");
    assert_eq!(list[1]["tileset"], 0);
    assert_eq!(list[0]["tileset"], Value::Null);
    assert_eq!(
        s.ok("doc.info", json!({}))["active_layer"],
        0,
        "paint target stays the image layer"
    );
    let d = s.doc(&id);
    assert_eq!(d.active_layer, Some(LayerId::new(1)));
    for f in 0..2 {
        let cel = d
            .cels
            .get(LayerId::new(1), FrameIndex::new(f))
            .expect("seeded cel");
        let pincel_core::CelData::Tilemap { grid_w, grid_h, .. } = &cel.data else {
            panic!("tilemap cel");
        };
        assert_eq!((*grid_w, *grid_h), (5, 3));
    }
    // Every cell of the new grid is empty.
    for f in 0..2 {
        let cel = d
            .cels
            .get(LayerId::new(1), FrameIndex::new(f))
            .expect("seeded cel");
        let pincel_core::CelData::Tilemap { tiles, .. } = &cel.data else {
            panic!("tilemap cel");
        };
        assert!(tiles.iter().all(|t| *t == pincel_core::TileRef::EMPTY));
    }
    // Placement and a given name, as other layers; the result carries the
    // new layer's id and the new layer becomes active.
    let out = s.ok(
        "layer.add",
        json!({ "kind": "tilemap", "tileset": 0, "name": "Back", "target": 0, "place": "below" }),
    );
    assert_eq!(out["layer"], 2);
    let list = out["layers"].clone();
    assert_eq!(list[0]["name"], "Back");
    assert_eq!(list[0]["id"], 2);
    assert_eq!(list[0]["tileset"], 0);
    assert_eq!(s.doc(&id).active_layer, Some(LayerId::new(2)));
    // Inside a group, by `parent`.
    let group = s.ok("layer.add", json!({ "kind": "group" }))["layer"].clone();
    let out = s.ok(
        "layer.add",
        json!({ "kind": "tilemap", "tileset": 0, "parent": group }),
    );
    let new = out["layer"].clone();
    let entry = out["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .find(|l| l["id"] == new)
        .cloned()
        .expect("listed");
    assert_eq!(entry["parent"], group);
    assert_eq!(entry["kind"], "tilemap");
}

#[test]
fn tilemap_grid_rounds_up_to_cover_the_canvas() {
    for ((w, h), (tw, th), grid) in [((10, 7), (4, 4), (3, 2)), ((10, 7), (16, 16), (1, 1))] {
        let mut s = S::new();
        let id = s.new_doc(w, h);
        s.ok(
            "tileset.add",
            json!({ "name": "t", "tile_width": tw, "tile_height": th }),
        );
        let out = s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
        assert_eq!(out["layer"], 1);
        let cel = s
            .doc(&id)
            .cels
            .get(LayerId::new(1), FrameIndex::new(0))
            .expect("seeded cel");
        let pincel_core::CelData::Tilemap { grid_w, grid_h, .. } = &cel.data else {
            panic!("tilemap cel");
        };
        assert_eq!((*grid_w, *grid_h), grid, "{w}x{h} in {tw}x{th} tiles");
        let got = s.ok("tilemap.get", json!({ "layer": 1 }));
        assert_eq!(got["grid_width"], json!(grid.0));
        assert_eq!(got["grid_height"], json!(grid.1));
        // The last, partly covered cell takes a tile.
        s.ok(
            "tilemap.place",
            json!({ "layer": 1, "cells": [{ "col": grid.0 - 1, "row": grid.1 - 1, "tile": 0 }] }),
        );
    }
}

#[test]
fn layer_add_tileset_and_kind_tilemap_go_together() {
    let mut s = S::new();
    let id = tiled(&mut s);
    for (args, arg) in [
        (json!({ "tileset": 0 }), "tileset"),
        (json!({ "kind": "image", "tileset": 0 }), "tileset"),
        (json!({ "kind": "group", "tileset": 0 }), "tileset"),
        (json!({ "kind": "tilemap" }), "tileset"),
    ] {
        let before = s.state(&id);
        let e = s.err("layer.add", args.clone());
        assert!(e.contains(&format!("`{arg}`")), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn tilemap_get_and_place_read_and_write_cells() {
    let mut s = S::new();
    let id = tiled(&mut s);
    s.ok("tilemap.place", json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }, { "col": 4, "row": 2, "tile": 1 }] }));
    let out = s.ok("tilemap.get", json!({ "layer": 1 }));
    assert_eq!(
        (out["grid_width"].clone(), out["grid_height"].clone()),
        (json!(5), json!(3))
    );
    assert_eq!(out["rows"], json!(["1 0 0 0 0", "0 0 0 0 0", "0 0 0 0 1"]));
    assert_eq!(out["oriented"], json!([]));
    // Oriented cells are reported.
    {
        let d = s.ws.get_mut(&id).expect("doc");
        let f = d.active_frame;
        let cel = d.cels.get_mut(LayerId::new(1), f).expect("cel");
        if let pincel_core::CelData::Tilemap { tiles, .. } = &mut cel.data {
            tiles[0].flip_x = true;
        }
    }
    let out = s.ok("tilemap.get", json!({ "layer": 1 }));
    assert_eq!(
        out["oriented"],
        json!([{ "col": 0, "row": 0, "flip_x": true, "flip_y": false, "rotate_90": false }])
    );
    // Placing resets the orientation; tile 0 empties.
    s.ok("tilemap.place", json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }, { "col": 4, "row": 2, "tile": 0 }] }));
    let out = s.ok("tilemap.get", json!({ "layer": 1 }));
    assert_eq!(out["oriented"], json!([]));
    assert_eq!(out["rows"][2], "0 0 0 0 0");
    // A frame without a grid reads as the empty grid; placing creates it.
    s.ws.get_mut(&id)
        .expect("doc")
        .cels
        .remove(LayerId::new(1), FrameIndex::new(0));
    let out = s.ok("tilemap.get", json!({ "layer": 1, "frame": 0 }));
    assert_eq!(out["rows"], json!(vec!["0 0 0 0 0"; 3]));
    let depth = undo_depth(&mut s);
    assert_eq!(
        s.ok(
            "tilemap.place",
            json!({ "layer": 1, "frame": 0, "cells": [{ "col": 1, "row": 1, "tile": 0 }] })
        )["changed"],
        false
    );
    assert_eq!(undo_depth(&mut s), depth);
    assert!(
        s.doc(&id)
            .cels
            .get(LayerId::new(1), FrameIndex::new(0))
            .is_none()
    );
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "frame": 0, "cells": [{ "col": 1, "row": 1, "tile": 1 }] }),
    );
    assert_eq!(
        s.ok("tilemap.get", json!({ "layer": 1, "frame": 0 }))["rows"][1],
        "0 1 0 0 0"
    );
}

#[test]
fn tilemap_place_bad_entries_name_position_field_and_limit() {
    let mut s = S::new();
    let id = tiled(&mut s);
    for (args, needle) in [
        (json!({ "layer": 0, "cells": [] }), "image layer"),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }, { "col": 5, "row": 0, "tile": 1 }] }),
            "cells[1].col 5 lies outside the 5×3 grid",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 3, "tile": 1 }] }),
            "cells[0].row 3 lies outside the 5×3 grid",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }, { "col": 0, "row": 0, "tile": 2 }] }),
            "cells[1].tile 2 is not in tileset 0 (2 tiles)",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "tile": 1 }] }),
            "cells[0].row",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": -1 }] }),
            "cells[0].tile",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("tilemap.place", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn tilemap_place_respects_locks_while_tile_edits_do_not() {
    let mut s = S::new();
    let id = tiled(&mut s);
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    let before = s.state(&id);
    let e = s.err(
        "tilemap.place",
        json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }] }),
    );
    assert!(e.contains("locked"), "{e}");
    assert_eq!(s.state(&id), before);
    let out = s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 1, "pixels": [{ "x": 0, "y": 0, "color": "#00ff00" }] }),
    );
    assert_eq!(out["changed"], true);
    let out = s.ok("tile.add", json!({ "tileset": 0 }));
    assert_eq!(out["tile"], 2);
    assert_eq!(
        s.ok("tileset.list", json!({}))["tilesets"][0]["tile_count"],
        3
    );
}

#[test]
fn tilemap_place_in_a_locked_group_fails_naming_the_layer() {
    let mut s = S::new();
    let id = tiled(&mut s);
    let group = s.ok("layer.add", json!({ "kind": "group" }))["layer"].clone();
    let layer = s.ok(
        "layer.add",
        json!({ "kind": "tilemap", "tileset": 0, "parent": group }),
    )["layer"]
        .clone();
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    let before = s.state(&id);
    let e = s.err(
        "tilemap.place",
        json!({ "layer": layer, "cells": [{ "col": 0, "row": 0, "tile": 1 }] }),
    );
    assert!(
        e.contains(&format!("layer {layer} (\"ground Layer\") is locked")),
        "{e}"
    );
    assert_eq!(s.state(&id), before);
    // The Tile editor works whatever the lock state of the layers.
    let out = s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 1, "pixels": [{ "x": 1, "y": 1, "color": "#0000ff" }] }),
    );
    assert_eq!(out["changed"], true);
    assert_eq!(s.ok("tile.add", json!({ "tileset": 0 }))["tile"], 2);
}

#[test]
fn drawing_on_a_tilemap_layer_points_to_the_tile_tools() {
    let mut s = S::new();
    let id = tiled(&mut s);
    for (tool, args) in [
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }], "layer": 1 }),
        ),
        ("draw.stroke", json!({ "points": [[0, 0]], "layer": 1 })),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#00ff00", "layer": 1 }),
        ),
        (
            "draw.rect",
            json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#00ff00", "filled": true, "layer": 1 }),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#00ff00", "filled": true, "layer": 1 }),
        ),
        (
            "draw.fill",
            json!({ "x": 1, "y": 1, "color": "#0000ff", "layer": 1 }),
        ),
        ("draw.clear", json!({ "layer": 1 })),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "layer": 1 }),
        ),
        (
            "draw.text",
            json!({ "text": "A", "x": 0, "y": 0, "layer": 1 }),
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "layer": 1 }),
        ),
        ("effect.apply", json!({ "id": "invert", "layer": 1 })),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args);
        assert!(
            e.contains("tilemap layer")
                && e.contains("tile.set_pixels")
                && e.contains("tilemap.place"),
            "{tool}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn read_pixels_of_a_tilemap_layer_shows_its_tiles() {
    let mut s = S::new();
    tiled(&mut s);
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "cells": [{ "col": 1, "row": 0, "tile": 1 }] }),
    );
    s.ok(
        "layer.set",
        json!({ "layer": 1, "opacity": 40, "visible": false }),
    );
    let out = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 3, "y": 0, "width": 3, "height": 1 }, "layer": 1 }),
    );
    assert_eq!(out["rows"], json!(["#00000000 #FF0000FF #FF0000FF"]));
}

#[test]
fn read_pixels_of_a_tilemap_layer_honors_tile_flags() {
    let mut s = S::new();
    let id = tiled(&mut s);
    s.ok("tile.add", json!({ "tileset": 0 }));
    s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 2, "pixels": [{ "x": 1, "y": 0, "color": "#ff0000" }] }),
    );
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "cells": [
            { "col": 0, "row": 0, "tile": 2 },
            { "col": 1, "row": 0, "tile": 2 },
            { "col": 2, "row": 0, "tile": 2 },
        ] }),
    );
    let frame = s.doc(&id).active_frame;
    {
        let doc = s.ws.get_mut(&id).expect("doc");
        let cel = doc
            .cels
            .get_mut(LayerId::new(1), frame)
            .expect("tilemap cel");
        let pincel_core::CelData::Tilemap { tiles, .. } = &mut cel.data else {
            panic!("tilemap cel");
        };
        tiles[0].flip_x = true;
        tiles[1].flip_y = true;
        tiles[2].rotate_90 = true;
    }
    let out = s.ok(
        "doc.read_pixels",
        json!({ "layer": 1, "region": { "x": 0, "y": 0, "width": 12, "height": 4 } }),
    );
    let red = [(2, 0), (5, 3), (11, 1)];
    let want: Vec<String> = (0..4)
        .map(|y| {
            (0..12)
                .map(|x| {
                    if red.contains(&(x, y)) {
                        "#FF0000FF"
                    } else {
                        "#00000000"
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(out["rows"], json!(want));
    // tilemap.get reports each flag.
    let got = s.ok("tilemap.get", json!({ "layer": 1 }));
    assert_eq!(
        got["oriented"],
        json!([
            { "col": 0, "row": 0, "flip_x": true, "flip_y": false, "rotate_90": false },
            { "col": 1, "row": 0, "flip_x": false, "flip_y": true, "rotate_90": false },
            { "col": 2, "row": 0, "flip_x": false, "flip_y": false, "rotate_90": true },
        ])
    );
}

#[test]
fn tilemap_get_refuses_other_layers_and_large_grids() {
    let mut s = S::new();
    tiled(&mut s);
    let e = s.err("tilemap.get", json!({ "layer": 0 }));
    assert!(e.contains("image layer"), "{e}");
    let mut s = S::new();
    s.new_doc(300, 8);
    s.ok(
        "tileset.add",
        json!({ "name": "t", "tile_width": 2, "tile_height": 2 }),
    );
    s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
    let e = s.err("tilemap.get", json!({ "layer": 1 }));
    assert!(e.contains("150×4") && e.contains("64×64"), "{e}");
    // Too high is refused as too wide is; 64 × 64 itself is not.
    for ((w, h), refused) in [((8, 300), Some("4×150")), ((64, 64), None)] {
        let mut s = S::new();
        s.new_doc(w, h);
        let tile = if refused.is_some() { 2 } else { 1 };
        s.ok(
            "tileset.add",
            json!({ "name": "t", "tile_width": tile, "tile_height": tile }),
        );
        s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
        match refused {
            Some(size) => {
                let e = s.err("tilemap.get", json!({ "layer": 1 }));
                assert!(e.contains(size) && e.contains("64×64"), "{e}");
            }
            None => {
                let got = s.ok("tilemap.get", json!({ "layer": 1 }));
                assert_eq!(got["grid_height"], 64);
            }
        }
    }
}

#[test]
fn a_tilemap_active_layer_leaves_drawing_on_the_lowest_image_layer() {
    let mut s = S::new();
    let id = tiled(&mut s);
    let out = s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    assert_eq!(out["layer"], 0);
    // effect.apply without `layer` acts on the same layer and names it;
    // the tilemap cel stays as it was.
    let tilemap = s
        .doc(&id)
        .cels
        .get(LayerId::new(1), FrameIndex::new(1))
        .cloned();
    let out = s.ok("effect.apply", json!({ "id": "invert" }));
    assert_eq!(out["layer"], 0);
    assert_eq!(out["changed"], true);
    let read = s.ok(
        "doc.read_pixels",
        json!({ "layer": 0, "region": { "x": 0, "y": 0, "width": 1, "height": 1 } }),
    );
    assert_eq!(read["rows"], json!(["#00FFFFFF"]));
    assert_eq!(
        s.doc(&id)
            .cels
            .get(LayerId::new(1), FrameIndex::new(1))
            .cloned(),
        tilemap
    );
    let mut t = S::new();
    let tid = t.new_doc(8, 8);
    t.ok(
        "tileset.add",
        json!({ "name": "t", "tile_width": 4, "tile_height": 4 }),
    );
    t.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
    t.ok("layer.remove", json!({ "layer": 0 }));
    for (tool, args) in [
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
        ),
        ("effect.apply", json!({ "id": "invert" })),
    ] {
        let before = t.state(&tid);
        let e = t.err(tool, args);
        assert!(e.contains("no image layer"), "{tool}: {e}");
        assert_eq!(t.state(&tid), before, "{tool}");
    }
}

#[test]
fn layer_list_reports_each_layers_tileset() {
    let mut s = S::new();
    tiled(&mut s);
    s.ok(
        "tileset.add",
        json!({ "name": "wall", "tile_width": 2, "tile_height": 2 }),
    );
    let wall = s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 1 }))["layer"].clone();
    let group = s.ok("layer.add", json!({ "kind": "group" }))["layer"].clone();
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    let layers = list.as_array().expect("layers");
    assert_eq!(layers.len(), 4);
    for l in layers {
        assert!(
            l.as_object().is_some_and(|o| o.contains_key("tileset")),
            "every layer reports `tileset`: {l}"
        );
        let want = match l["id"].as_u64() {
            Some(1) => json!(0),
            _ if l["id"] == wall => json!(1),
            _ => Value::Null,
        };
        assert_eq!(l["tileset"], want, "{l}");
    }
    let kinds: Vec<(Value, Value)> = layers
        .iter()
        .map(|l| (l["id"].clone(), l["kind"].clone()))
        .collect();
    assert!(kinds.contains(&(group, json!("group"))), "{kinds:?}");
    assert!(kinds.contains(&(json!(0), json!("image"))), "{kinds:?}");
}

#[test]
fn tilemap_place_equals_the_editors_tile_stamp_and_a_later_entry_wins() {
    let mut s = S::new();
    let id = tiled(&mut s);
    s.ok("tile.add", json!({ "tileset": 0 }));
    let frame = s.doc(&id).active_frame;
    let want = {
        let (mut sprite, mut cels) = s.state(&id);
        let place = |c: u32, r: u32, t: u32| -> pincel_core::AnyCommand {
            pincel_core::PlaceTile::new(
                LayerId::new(1),
                frame,
                c,
                r,
                pincel_core::TileRef {
                    tile_id: t,
                    ..pincel_core::TileRef::EMPTY
                },
            )
            .into()
        };
        pincel_core::Bus::new()
            .execute(
                pincel_core::CommandBatch::new(vec![
                    place(1, 1, 1),
                    place(3, 2, 2),
                    place(1, 1, 2),
                ])
                .into(),
                &mut sprite,
                &mut cels,
            )
            .expect("editor");
        (sprite, cels)
    };
    let depth = undo_depth(&mut s);
    let out = s.ok(
        "tilemap.place",
        json!({ "layer": 1, "cells": [
            { "col": 1, "row": 1, "tile": 1 },
            { "col": 3, "row": 2, "tile": 2 },
            { "col": 1, "row": 1, "tile": 2 },
        ] }),
    );
    assert_eq!(out["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(s.state(&id), want);
    assert_eq!(
        s.ok("tilemap.get", json!({ "layer": 1 }))["rows"],
        json!(["0 0 0 0 0", "0 2 0 0 0", "0 0 0 2 0"]),
        "the later entry for cell (1, 1) wins"
    );
    // Placing a tile and clearing the cell again in one batch leaves the
    // document as it was: no undo step.
    let before = s.state(&id);
    let out = s.ok(
        "tilemap.place",
        json!({ "layer": 1, "cells": [
            { "col": 0, "row": 0, "tile": 1 },
            { "col": 0, "row": 0, "tile": 0 },
        ] }),
    );
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(s.state(&id), before);
}

#[test]
fn tilemap_place_checks_cells_against_the_cels_own_grid() {
    // An opened file's cel may have another grid than a new layer gets.
    let mut s = S::new();
    let id = tiled(&mut s);
    {
        let d = s.ws.get_mut(&id).expect("doc");
        let cel = d
            .cels
            .get_mut(LayerId::new(1), FrameIndex::new(0))
            .expect("cel");
        cel.data = pincel_core::CelData::Tilemap {
            grid_w: 2,
            grid_h: 2,
            tiles: vec![pincel_core::TileRef::EMPTY; 4],
        };
        d.cels.remove(LayerId::new(1), FrameIndex::new(1));
    }
    for (frame, cell, needle) in [
        (
            0,
            json!({ "col": 2, "row": 0, "tile": 1 }),
            "cells[0].col 2 lies outside the 2×2 grid",
        ),
        (
            0,
            json!({ "col": 0, "row": 2, "tile": 1 }),
            "cells[0].row 2 lies outside the 2×2 grid",
        ),
        (
            1,
            json!({ "col": 5, "row": 0, "tile": 1 }),
            "cells[0].col 5 lies outside the 5×3 grid",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(
            "tilemap.place",
            json!({ "layer": 1, "frame": frame, "cells": [cell] }),
        );
        assert!(e.contains(needle), "{e}");
        assert_eq!(s.state(&id), before);
    }
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "frame": 0, "cells": [{ "col": 1, "row": 1, "tile": 1 }] }),
    );
    assert_eq!(
        s.ok("tilemap.get", json!({ "layer": 1, "frame": 0 }))["rows"],
        json!(["0 0", "0 1"])
    );
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "frame": 1, "cells": [{ "col": 4, "row": 2, "tile": 1 }] }),
    );
    assert_eq!(
        s.ok("tilemap.get", json!({ "layer": 1, "frame": 1 }))["rows"],
        json!(["0 0 0 0 0", "0 0 0 0 0", "0 0 0 0 1"])
    );
}
