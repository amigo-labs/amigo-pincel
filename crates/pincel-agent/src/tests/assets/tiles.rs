//! `tileset.*` and `tile.*` (AC-11 … AC-18).

use super::*;

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
