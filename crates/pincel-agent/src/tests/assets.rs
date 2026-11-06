//! Slices, tilesets, tilemaps and the grid (mcp-parity-pixel-assets).

use pincel_core::{FrameIndex, LayerId, read_aseprite};
use serde_json::{Value, json};

use super::S;

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// A 20×12 Pixel document with two frames, a tileset (id 0, 4×4 tiles,
/// tile 1 red) and a tilemap layer (id 1) over it.
fn tiled(s: &mut S) -> String {
    let id = s.new_doc(20, 12);
    s.ok("frame.add", json!({}));
    s.ok(
        "tileset.add",
        json!({ "name": " ground ", "tile_width": 4, "tile_height": 4 }),
    );
    s.ok("tile.add", json!({ "tileset": 0 }));
    let px: Vec<Value> = (0..4)
        .flat_map(|y| (0..4).map(move |x| json!({ "x": x, "y": y, "color": "#ff0000" })))
        .collect();
    s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 1, "pixels": px }),
    );
    s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
    id
}

#[test]
fn asset_tools_on_an_image_document_fail_naming_the_mode() {
    let mut s = S::new();
    let id = s.new_image(8, 8);
    for (tool, args) in [
        ("slice.list", json!({})),
        ("slice.add", json!({ "name": "a" })),
        ("slice.remove", json!({ "slice": 0 })),
        (
            "slice.set_key",
            json!({ "slice": 0, "pivot": { "x": 0, "y": 0 } }),
        ),
        ("tileset.list", json!({})),
        (
            "tileset.add",
            json!({ "name": "t", "tile_width": 4, "tile_height": 4 }),
        ),
        ("tile.add", json!({ "tileset": 0 })),
        ("tile.read_pixels", json!({ "tileset": 0, "tile": 0 })),
        (
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 0, "pixels": [] }),
        ),
        ("tilemap.get", json!({ "layer": 0 })),
        ("tilemap.place", json!({ "layer": 0, "cells": [] })),
        (
            "grid.set",
            json!({ "x": 0, "y": 0, "width": 8, "height": 8 }),
        ),
        ("layer.add", json!({ "kind": "tilemap", "tileset": 0 })),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args);
        assert!(e.contains("Image document"), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn naming_a_missing_asset_fails_with_argument_and_value() {
    let mut s = S::new();
    let id = tiled(&mut s);
    for (tool, args, arg) in [
        ("slice.remove", json!({ "slice": 9 }), "slice"),
        (
            "slice.set_key",
            json!({ "slice": 9, "pivot": { "x": 0, "y": 0 } }),
            "slice",
        ),
        ("tile.add", json!({ "tileset": 9 }), "tileset"),
        (
            "tile.read_pixels",
            json!({ "tileset": 0, "tile": 9 }),
            "tile",
        ),
        (
            "tile.set_pixels",
            json!({ "tileset": 9, "tile": 0, "pixels": [] }),
            "tileset",
        ),
        ("tilemap.get", json!({ "layer": 9 }), "layer"),
        (
            "tilemap.place",
            json!({ "layer": 1, "frame": 9, "cells": [] }),
            "frame",
        ),
        (
            "layer.add",
            json!({ "kind": "tilemap", "tileset": 9 }),
            "tileset",
        ),
        (
            "slice.set_key",
            json!({ "slice": 0, "frame": 9, "pivot": { "x": 0, "y": 0 } }),
            "frame",
        ),
        ("tilemap.get", json!({ "layer": 1, "frame": 9 }), "frame"),
        ("tilemap.place", json!({ "layer": 9, "cells": [] }), "layer"),
        (
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 9, "pixels": [] }),
            "tile",
        ),
        (
            "tile.read_pixels",
            json!({ "tileset": 9, "tile": 0 }),
            "tileset",
        ),
    ] {
        if tool == "slice.set_key"
            && s.ok("slice.list", json!({}))["slices"]
                .as_array()
                .is_none_or(|a| a.is_empty())
        {
            s.ok("slice.add", json!({ "name": "a" }));
        }
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(arg) && e.contains('9'), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn slice_add_list_and_remove_like_the_editor() {
    let mut s = S::new();
    s.new_doc(16, 8);
    let out = s.ok("slice.add", json!({ "name": "  body ", "bounds": { "x": 1, "y": 2, "width": 3, "height": 4 }, "color": "#00ff0080" }));
    assert_eq!(out["slice"], 0);
    let out = s.ok("slice.add", json!({ "name": "all" }));
    assert_eq!(out["slice"], 1);
    let list = s.ok("slice.list", json!({}))["slices"].clone();
    assert_eq!(
        list,
        json!([
            { "id": 0, "name": "body", "color": "#00FF0080", "keys": [{ "frame": 0, "bounds": { "x": 1, "y": 2, "width": 3, "height": 4 }, "center": null, "pivot": null }] },
            { "id": 1, "name": "all", "color": "#3B82F6FF", "keys": [{ "frame": 0, "bounds": { "x": 0, "y": 0, "width": 16, "height": 8 }, "center": null, "pivot": null }] },
        ])
    );
    s.ok("slice.remove", json!({ "slice": 0 }));
    assert_eq!(
        s.ok("slice.list", json!({}))["slices"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    s.ok("doc.undo", json!({}));
    assert_eq!(
        s.ok("slice.list", json!({}))["slices"],
        list,
        "undo restores the position"
    );
}

#[test]
fn slice_add_with_bad_arguments_adds_nothing() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (args, arg) in [
        (json!({ "name": "  " }), "name"),
        (
            json!({ "name": "a", "bounds": { "x": 0, "y": 0, "width": 0, "height": 2 } }),
            "width",
        ),
        (json!({ "name": "a", "color": "blue" }), "color"),
    ] {
        let before = s.state(&id);
        let e = s.err("slice.add", args.clone());
        assert!(e.contains(arg), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn slice_set_key_upserts_by_frame_and_inherits_fields() {
    let mut s = S::new();
    s.new_doc(16, 8);
    for _ in 0..3 {
        s.ok("frame.add", json!({}));
    }
    s.ok(
        "slice.add",
        json!({ "name": "a", "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 } }),
    );
    s.ok("slice.set_key", json!({ "slice": 0, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } }));
    // Frame 2 inherits bounds, center and pivot from frame 0's key.
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 2, "bounds": { "x": 2, "y": 0, "width": 8, "height": 8 } }),
    );
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 1, "pivot": null }),
    );
    let keys = s.ok("slice.list", json!({}))["slices"][0]["keys"].clone();
    assert_eq!(
        keys,
        json!([
            { "frame": 0, "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } },
            { "frame": 1, "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": null },
            { "frame": 2, "bounds": { "x": 2, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } },
        ])
    );
}

#[test]
fn slice_set_key_with_bad_arguments_leaves_the_slice() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok("frame.add", json!({}));
    s.ok("slice.add", json!({ "name": "a" }));
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 1, "pivot": { "x": 1, "y": 1 } }),
    );
    s.ok("slice.remove", json!({ "slice": 0 }));
    s.ok("doc.undo", json!({}));
    for (args, arg) in [
        (json!({ "slice": 0 }), "bounds"),
        (
            json!({ "slice": 0, "bounds": { "x": 0, "y": 0, "width": 0, "height": 1 } }),
            "bounds.width",
        ),
        (
            json!({ "slice": 0, "center": { "x": 0, "y": 0, "width": 1, "height": 0 } }),
            "center",
        ),
        (json!({ "slice": 0, "pivot": { "x": 1 } }), "pivot"),
    ] {
        let before = s.state(&id);
        let e = s.err("slice.set_key", args.clone());
        assert!(e.contains(arg), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    // No key before frame 1 but a first key at 0: a key before the first
    // one needs bounds. Build a slice whose first key is at frame 1.
    {
        let d = s.ws.get_mut(&id).expect("doc");
        d.sprite.slices[0].keys.remove(0);
    }
    let e = s.err(
        "slice.set_key",
        json!({ "slice": 0, "frame": 0, "pivot": { "x": 0, "y": 0 } }),
    );
    assert!(e.contains("`bounds`"), "{e}");
}

#[test]
fn tileset_tools_list_add_and_grow() {
    let mut s = S::new();
    s.new_doc(8, 8);
    let out = s.ok(
        "tileset.add",
        json!({ "name": " ground ", "tile_width": 4, "tile_height": 2 }),
    );
    assert_eq!(out["tileset"], 0);
    let list = s.ok("tileset.list", json!({}))["tilesets"].clone();
    assert_eq!(
        list,
        json!([{ "id": 0, "name": "ground", "tile_width": 4, "tile_height": 2, "tile_count": 1 }])
    );
    assert_eq!(
        s.ok("tile.read_pixels", json!({ "tileset": 0, "tile": 0 }))["rows"],
        json!(vec!["#00000000 #00000000 #00000000 #00000000"; 2])
    );
    assert_eq!(s.ok("tile.add", json!({ "tileset": 0 }))["tile"], 1);
    assert_eq!(
        s.ok("tileset.list", json!({}))["tilesets"][0]["tile_count"],
        2
    );
}

#[test]
fn tileset_add_with_bad_arguments_adds_nothing() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (args, arg) in [
        (
            json!({ "name": "", "tile_width": 4, "tile_height": 4 }),
            "name",
        ),
        (
            json!({ "name": "t", "tile_width": 0, "tile_height": 4 }),
            "tile_width",
        ),
        (
            json!({ "name": "t", "tile_width": 4, "tile_height": 4097 }),
            "tile_height",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("tileset.add", args.clone());
        assert!(e.contains(arg), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn tile_read_and_set_pixels_reach_every_cel_that_shows_the_tile() {
    let mut s = S::new();
    s.new_doc(20, 12);
    s.ok("frame.add", json!({}));
    s.ok(
        "tileset.add",
        json!({ "name": "g", "tile_width": 4, "tile_height": 4 }),
    );
    s.ok("tile.add", json!({ "tileset": 0 }));
    s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }));
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "cells": [{ "col": 1, "row": 0, "tile": 1 }] }),
    );
    s.ok(
        "tilemap.place",
        json!({ "layer": 1, "frame": 0, "cells": [{ "col": 2, "row": 2, "tile": 1 }] }),
    );
    s.ok("tile.set_pixels", json!({ "tileset": 0, "tile": 1, "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }, { "x": 3, "y": 1, "color": { "index": 21 } }] }));
    let rows = s.ok(
        "tile.read_pixels",
        json!({ "tileset": 0, "tile": 1, "region": { "x": 0, "y": 0, "width": 4, "height": 2 } }),
    )["rows"]
        .clone();
    assert_eq!(rows[0], "#FF0000FF #00000000 #00000000 #00000000");
    assert_eq!(rows[1], "#00000000 #00000000 #00000000 #FFFFFFFF");
    let idx = s.ok("tile.read_pixels", json!({ "tileset": 0, "tile": 1, "region": { "x": 3, "y": 1, "width": 1, "height": 1 }, "format": "index" }))["rows"].clone();
    assert_eq!(idx, json!(["21"]));
    // Frame 1 shows the tile at cell (1, 0); frame 0 at (2, 2).
    let read = |s: &mut S, frame: u32, x: i32, y: i32| -> Value {
        s.ok(
            "doc.read_pixels",
            json!({ "region": { "x": x, "y": y, "width": 1, "height": 1 }, "frame": frame }),
        )["rows"][0]
            .clone()
    };
    assert_eq!(read(&mut s, 1, 4, 0), "#FF0000FF");
    assert_eq!(read(&mut s, 0, 8, 8), "#FF0000FF");
    assert_eq!(read(&mut s, 0, 4, 0), "#00000000");
}

#[test]
fn tile_read_pixels_refuses_outside_or_large_regions() {
    let mut s = S::new();
    s.new_doc(8, 8);
    s.ok(
        "tileset.add",
        json!({ "name": "big", "tile_width": 80, "tile_height": 8 }),
    );
    let e = s.err("tile.read_pixels", json!({ "tileset": 0, "tile": 0 }));
    assert!(e.contains("64×64") && e.contains("region"), "{e}");
    let e = s.err(
        "tile.read_pixels",
        json!({ "tileset": 0, "tile": 0, "region": { "x": 78, "y": 0, "width": 4, "height": 1 } }),
    );
    assert!(e.contains("80×8"), "{e}");
}

#[test]
fn tile_set_pixels_bad_entries_leave_the_tileset() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok(
        "tileset.add",
        json!({ "name": "g", "tile_width": 4, "tile_height": 4 }),
    );
    for (pixels, needle) in [
        (
            json!([{ "x": 0, "y": 0, "color": "#ff0000" }, { "x": 4, "y": 0, "color": "#ff0000" }]),
            "pixels[1]",
        ),
        (
            json!([{ "x": 0, "y": 0, "color": "nope" }]),
            "pixels[0].color",
        ),
        (json!([{ "x": 0, "color": "#ff0000" }]), "pixels[0].y"),
        (
            json!([{ "x": -1, "y": 0, "color": "#ff0000" }]),
            "pixels[0].x",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 0, "pixels": pixels }),
        );
        assert!(e.contains(needle), "{e}");
        assert_eq!(s.state(&id), before);
    }
    let e = s.err(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 0, "pixels": [{ "x": 9, "y": 0, "color": "#ff0000" }] }),
    );
    assert!(e.contains("4×4"), "{e}");
}

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
            "cells[1].col 5",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 3, "tile": 1 }] }),
            "5×3",
        ),
        (
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 2 }] }),
            "2 tiles",
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
fn grid_set_is_reported_and_saved() {
    let mut s = S::new();
    s.new_doc(8, 8);
    assert_eq!(
        s.ok("doc.info", json!({}))["grid"],
        json!({ "x": 0, "y": 0, "width": 16, "height": 16 })
    );
    s.ok(
        "grid.set",
        json!({ "x": -2, "y": 3, "width": 8, "height": 4 }),
    );
    let grid = json!({ "x": -2, "y": 3, "width": 8, "height": 4 });
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
    s.ok("doc.save", json!({ "path": "g.aseprite" }));
    let out = read_aseprite(&s.host.files["g.aseprite"]).expect("read");
    assert_eq!(
        out.sprite.metadata.grid_or_default(),
        pincel_core::Rect::new(-2, 3, 8, 4)
    );
    s.ok("doc.open", json!({ "path": "g.aseprite" }));
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
}

#[test]
fn an_opened_files_zero_size_header_grid_is_reported_and_replaceable() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let (mut sprite, cels) = s.state(&id);
    sprite.metadata.grid = Some(pincel_core::Rect::new(1, -2, 0, 5));
    let mut bytes = Vec::new();
    pincel_core::write_aseprite(&sprite, &cels, &mut bytes).expect("write");
    let header = read_aseprite(&bytes).expect("read").sprite.metadata.grid;
    assert_eq!(header, Some(pincel_core::Rect::new(1, -2, 0, 5)));
    s.host.files.insert("zero.aseprite".into(), bytes);
    s.ok("doc.open", json!({ "path": "zero.aseprite" }));
    let stored = json!({ "x": 1, "y": -2, "width": 0, "height": 5 });
    assert_eq!(s.ok("doc.info", json!({}))["grid"], stored);
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(list[1]["grid"], stored);
    let grid = json!({ "x": 0, "y": 0, "width": 4, "height": 4 });
    assert_eq!(s.ok("grid.set", grid.clone())["changed"], true);
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
}

#[test]
fn grid_set_out_of_range_leaves_the_grid() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    // The argument and its range, from the schema check.
    for (args, message) in [
        (
            json!({ "x": 0, "y": 0, "width": 0, "height": 4 }),
            "width must be 1–65535",
        ),
        (
            json!({ "x": 0, "y": 0, "width": 4, "height": 65536 }),
            "height must be 1–65535",
        ),
        (
            json!({ "x": 40000, "y": 0, "width": 4, "height": 4 }),
            "x must be -32768–32767",
        ),
        (
            json!({ "x": 0, "y": -40000, "width": 4, "height": 4 }),
            "y must be -32768–32767",
        ),
    ] {
        let before = s.state(&id);
        let e = s.call("grid.set", args.clone()).unwrap_err();
        assert_eq!(
            e,
            crate::AgentError::InvalidArgs {
                tool: "grid.set".to_owned(),
                message: message.to_owned(),
            },
            "{args}"
        );
        assert_eq!(s.state(&id), before);
    }
    // A missing argument is a schema mismatch.
    let before = s.state(&id);
    let e = s
        .call("grid.set", json!({ "x": 0, "y": 0, "width": 4 }))
        .unwrap_err();
    assert!(
        matches!(&e, crate::AgentError::InvalidArgs { message, .. } if message.starts_with("missing required argument `height`")),
        "{e:?}"
    );
    assert_eq!(s.state(&id), before);
}

#[test]
fn doc_info_reports_slice_and_tileset_counts_and_grid() {
    let mut s = S::new();
    tiled(&mut s);
    s.ok("slice.add", json!({ "name": "a" }));
    let info = s.ok("doc.info", json!({}));
    assert_eq!(
        (info["slices"].clone(), info["tilesets"].clone()),
        (json!(1), json!(1))
    );
    s.new_image(4, 4);
    let info = s.ok("doc.info", json!({}));
    for key in ["slices", "tilesets", "grid"] {
        assert!(info.get(key).is_none(), "{key}");
    }
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(list[0]["grid"]["width"], 16);
}

#[test]
fn asset_edits_record_one_undo_step_and_none_without_a_change() {
    let mut s = S::new();
    tiled(&mut s);
    for (tool, args) in [
        ("slice.add", json!({ "name": "a" })),
        (
            "slice.set_key",
            json!({ "slice": 0, "pivot": { "x": 1, "y": 1 } }),
        ),
        (
            "tileset.add",
            json!({ "name": "b", "tile_width": 2, "tile_height": 2 }),
        ),
        ("tile.add", json!({ "tileset": 0 })),
        (
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 2, "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }, { "x": 1, "y": 0, "color": "#ff0000" }] }),
        ),
        (
            "tilemap.place",
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }, { "col": 1, "row": 0, "tile": 2 }] }),
        ),
        (
            "grid.set",
            json!({ "x": 0, "y": 0, "width": 8, "height": 8 }),
        ),
        ("slice.remove", json!({ "slice": 0 })),
    ] {
        let depth = undo_depth(&mut s);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool}");
    }
    for (tool, args) in [
        (
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 1, "pixels": [] }),
        ),
        ("tilemap.place", json!({ "layer": 1, "cells": [] })),
        (
            "tilemap.place",
            json!({ "layer": 1, "cells": [{ "col": 0, "row": 0, "tile": 1 }] }),
        ),
        (
            "grid.set",
            json!({ "x": 0, "y": 0, "width": 8, "height": 8 }),
        ),
    ] {
        let depth = undo_depth(&mut s);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth, "{tool} {args}");
    }
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
