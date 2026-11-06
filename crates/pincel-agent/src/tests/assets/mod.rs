//! Slices, tilesets, tilemaps and the grid (mcp-parity-pixel-assets).

mod grid;
mod slices;
mod tilemaps;
mod tiles;

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
