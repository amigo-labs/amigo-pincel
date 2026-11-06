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
    // tile 0 painted: an added tile is still fully transparent.
    s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 0, "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }] }),
    );
    assert_eq!(s.ok("tile.add", json!({ "tileset": 0 }))["tile"], 1);
    assert_eq!(
        s.ok("tileset.list", json!({}))["tilesets"][0]["tile_count"],
        2
    );
    assert_eq!(s.ok("tile.add", json!({ "tileset": 0 }))["tile"], 2);
    for tile in [1, 2] {
        assert_eq!(
            s.ok("tile.read_pixels", json!({ "tileset": 0, "tile": tile }))["rows"],
            json!(vec!["#00000000 #00000000 #00000000 #00000000"; 2]),
            "tile {tile}: transparent, 4x2"
        );
    }
    // A second tileset is listed after the first, with its own id, size
    // and count; tile.add grows only the named tileset.
    let out = s.ok(
        "tileset.add",
        json!({ "name": "wall", "tile_width": 3, "tile_height": 5 }),
    );
    assert_eq!(out["tileset"], 1);
    assert_eq!(s.ok("tile.add", json!({ "tileset": 1 }))["tile"], 1);
    assert_eq!(
        s.ok("tileset.list", json!({}))["tilesets"],
        json!([
            { "id": 0, "name": "ground", "tile_width": 4, "tile_height": 2, "tile_count": 3 },
            { "id": 1, "name": "wall", "tile_width": 3, "tile_height": 5, "tile_count": 2 },
        ])
    );
    assert_eq!(
        s.ok("tile.read_pixels", json!({ "tileset": 1, "tile": 1 }))["rows"],
        json!(vec!["#00000000 #00000000 #00000000"; 5])
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
    // Too high is refused as too wide is; a region larger than 64×64
    // inside a big tile too; a 128×128 tile needs `region`.
    s.ok(
        "tileset.add",
        json!({ "name": "tall", "tile_width": 8, "tile_height": 65 }),
    );
    s.ok(
        "tileset.add",
        json!({ "name": "huge", "tile_width": 128, "tile_height": 128 }),
    );
    for args in [
        json!({ "tileset": 1, "tile": 0 }),
        json!({ "tileset": 2, "tile": 0 }),
        json!({ "tileset": 2, "tile": 0, "region": { "x": 0, "y": 0, "width": 65, "height": 1 } }),
        json!({ "tileset": 2, "tile": 0, "region": { "x": 0, "y": 0, "width": 1, "height": 65 } }),
    ] {
        let e = s.err("tile.read_pixels", args.clone());
        assert!(e.contains("64×64") && e.contains("`region`"), "{args}: {e}");
    }
    // 64×64 itself is read.
    let rows = s.ok(
        "tile.read_pixels",
        json!({ "tileset": 2, "tile": 0, "region": { "x": 64, "y": 64, "width": 64, "height": 64 } }),
    )["rows"]
        .clone();
    assert_eq!(rows.as_array().map(Vec::len), Some(64));
    let e = s.err(
        "tile.read_pixels",
        json!({ "tileset": 1, "tile": 0, "region": { "x": 0, "y": 60, "width": 8, "height": 6 } }),
    );
    assert!(e.contains("8×65"), "{e}");
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
    // The first offending pixel is named, with the tile size; a pixel
    // outside the tile in y counts as one outside in x.
    let ok = json!({ "x": 0, "y": 0, "color": "#ff0000" });
    for (pixels, needle) in [
        (
            json!([ok, { "x": 0, "y": 4, "color": "#ff0000" }, { "x": 5, "y": 0, "color": "#ff0000" }]),
            "pixels[1] (0, 4) lies outside the 4×4 tile",
        ),
        (
            json!([ok, { "x": 0, "y": 0, "color": "nope" }, { "x": 5, "y": 0, "color": "#ff0000" }]),
            "pixels[1].color",
        ),
        (
            json!([ok, { "x": 4, "y": 4, "color": "#ff0000" }, { "x": 0, "y": 0, "color": "nope" }]),
            "pixels[1] (4, 4) lies outside the 4×4 tile",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(
            "tile.set_pixels",
            json!({ "tileset": 0, "tile": 0, "pixels": pixels }),
        );
        assert!(e.contains(needle), "{e}");
        assert!(!e.contains("pixels[2]"), "{e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn tile_set_pixels_equals_the_editors_tile_editor() {
    let mut s = S::new();
    let id = s.new_doc(12, 8);
    s.ok("frame.add", json!({}));
    for name in ["a", "b"] {
        s.ok(
            "tileset.add",
            json!({ "name": name, "tile_width": 4, "tile_height": 4 }),
        );
    }
    s.ok("tile.add", json!({ "tileset": 0 }));
    s.ok("tile.add", json!({ "tileset": 0 }));
    s.ok("tile.add", json!({ "tileset": 1 }));
    // Paint tile 2 and tileset 1's tile 1 so a stray write would show.
    for ts in [0, 1] {
        let tile = 2 - ts;
        s.ok(
            "tile.set_pixels",
            json!({ "tileset": ts, "tile": tile, "pixels": [{ "x": 0, "y": 0, "color": "#00ff00" }] }),
        );
    }
    // Two tilemap layers show tile 1 on both frames.
    for _ in 0..2 {
        let layer = s.ok("layer.add", json!({ "kind": "tilemap", "tileset": 0 }))["layer"].clone();
        for frame in [0, 1] {
            s.ok(
                "tilemap.place",
                json!({ "layer": layer, "frame": frame, "cells": [{ "col": frame, "row": 1, "tile": 1 }] }),
            );
        }
    }
    let pixels: [(u32, u32, [u8; 4]); 3] = [
        (0, 0, [255, 0, 0, 255]),
        (3, 2, [0, 0, 255, 128]),
        (1, 3, [255, 255, 255, 255]),
    ];
    let want = {
        let (mut sprite, mut cels) = s.state(&id);
        let ts = sprite.tilesets[0].id;
        let cmds: Vec<pincel_core::AnyCommand> = pixels
            .iter()
            .map(|&(x, y, [r, g, b, a])| {
                pincel_core::SetTilePixel::new(ts, 1, x, y, pincel_core::Rgba { r, g, b, a }).into()
            })
            .collect();
        pincel_core::Bus::new()
            .execute(
                pincel_core::CommandBatch::new(cmds).into(),
                &mut sprite,
                &mut cels,
            )
            .expect("editor");
        (sprite, cels)
    };
    s.ok(
        "tile.set_pixels",
        json!({ "tileset": 0, "tile": 1, "pixels": [
        { "x": 0, "y": 0, "color": "#ff0000" },
        { "x": 3, "y": 2, "color": "#0000ff80" },
        { "x": 1, "y": 3, "color": "#ffffff" },
    ] }),
    );
    assert_eq!(
        s.state(&id),
        want,
        "only the listed pixels of tile 1 changed"
    );
    // Every cel that shows tile 1 composes with the new pixels: layers 1
    // and 2, frame 0 at cell (0, 1) and frame 1 at cell (1, 1).
    for layer in [1, 2] {
        for frame in [0u32, 1] {
            let rows = s.ok(
                "doc.read_pixels",
                json!({ "layer": layer, "frame": frame, "region": { "x": frame * 4, "y": 4, "width": 4, "height": 4 } }),
            )["rows"]
                .clone();
            let row = |y: usize| -> Vec<String> {
                rows[y]
                    .as_str()
                    .expect("row")
                    .split(' ')
                    .map(str::to_owned)
                    .collect()
            };
            assert_eq!(row(0)[0], "#FF0000FF", "{layer}/{frame}");
            assert_eq!(row(2)[3], "#0000FF80", "{layer}/{frame}");
            assert_eq!(row(3)[1], "#FFFFFFFF", "{layer}/{frame}");
        }
    }
}
