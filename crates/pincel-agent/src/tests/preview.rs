//! `doc.preview` and `doc.read_pixels` (AC-11 … AC-14, AC-44, AC-45).

use pincel_core::import_png;
use serde_json::json;

use super::{S, px};
use crate::preview::blend_grid;

fn preview(s: &mut S, args: serde_json::Value) -> (pincel_core::ImportedImage, serde_json::Value) {
    let out = s.call("doc.preview", args).expect("preview");
    let img = import_png(&out.image_png.expect("image content")).expect("valid png");
    (img, out.json)
}

#[test]
fn preview_scales_nearest_and_returns_png() {
    let mut s = S::new();
    s.new_doc(32, 16);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 3, "y": 2, "color": "#ff0000" }] }),
    );
    let (img, meta) = preview(&mut s, json!({}));
    // Longest side 32 → scale 8 → 256×128.
    assert_eq!((img.width, img.height), (256, 128));
    assert_eq!(
        (meta["width"].as_u64(), meta["height"].as_u64()),
        (Some(256), Some(128))
    );
    assert_eq!(meta["scale"], 8);
    for y in 16..24 {
        for x in 24..32 {
            assert_eq!(px(&img.rgba, 256, x, y), [255, 0, 0, 255]);
        }
    }
    assert_eq!(px(&img.rgba, 256, 23, 16), [0, 0, 0, 0]);
    let (img, _) = preview(&mut s, json!({ "scale": 2 }));
    assert_eq!((img.width, img.height), (64, 32));
}

#[test]
fn preview_of_one_pixel_sprite_is_256_square() {
    let mut s = S::new();
    s.new_doc(1, 1);
    let (img, _) = preview(&mut s, json!({}));
    assert_eq!((img.width, img.height), (256, 256));
    s.new_doc(300, 10);
    let (img, meta) = preview(&mut s, json!({}));
    assert_eq!(meta["scale"], 1);
    assert_eq!((img.width, img.height), (300, 10));
}

#[test]
fn preview_frame_and_region_select_what_is_shown() {
    let mut s = S::new();
    s.new_doc(8, 8);
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 5, "y": 6, "color": "#00ff00" }] }),
    );
    let (f0, _) = preview(&mut s, json!({ "frame": 0, "scale": 1 }));
    assert_eq!(px(&f0.rgba, 8, 5, 6), [0, 0, 0, 0]);
    let (f1, _) = preview(&mut s, json!({ "frame": 1, "scale": 1 }));
    assert_eq!(px(&f1.rgba, 8, 5, 6), [0, 255, 0, 255]);
    let (r, _) = preview(
        &mut s,
        json!({ "frame": 1, "scale": 2, "region": { "x": 4, "y": 6, "width": 2, "height": 1 } }),
    );
    assert_eq!((r.width, r.height), (4, 2));
    assert_eq!(px(&r.rgba, 4, 2, 0), [0, 255, 0, 255]);
    assert_eq!(px(&r.rgba, 4, 1, 1), [0, 0, 0, 0]);
}

#[test]
fn preview_grid_only_changes_block_borders_and_is_omitted_below_scale_4() {
    let mut s = S::new();
    s.new_doc(3, 2);
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 1, "y1": 1, "color": "#ffffff", "filled": true }),
    );
    let (plain, _) = preview(&mut s, json!({ "scale": 4 }));
    let (grid, meta) = preview(&mut s, json!({ "scale": 4, "grid": true }));
    assert_eq!(meta["grid"], true);
    for y in 0..plain.height {
        for x in 0..plain.width {
            let base = px(&plain.rgba, plain.width, x, y);
            let got = px(&grid.rgba, grid.width, x, y);
            if x % 4 == 0 || y % 4 == 0 {
                assert_eq!(got, blend_grid(base), "({x}, {y})");
            } else {
                assert_eq!(got, base, "({x}, {y})");
            }
        }
    }
    let (small, meta) = preview(&mut s, json!({ "scale": 3, "grid": true }));
    let (small_plain, _) = preview(&mut s, json!({ "scale": 3 }));
    assert_eq!(small.rgba, small_plain.rgba);
    assert_eq!(meta["note"], "grid omitted below scale 4");
}

#[test]
fn preview_above_2048_cap_fails_suggesting_region() {
    let mut s = S::new();
    s.new_doc(4096, 4096);
    let e = s.err("doc.preview", json!({}));
    assert!(e.contains("2048") && e.contains("region"), "{e}");
    s.new_doc(100, 100);
    let e = s.err("doc.preview", json!({ "scale": 21 }));
    assert!(e.contains("2048") && e.contains("region"), "{e}");
    let (img, _) = preview(&mut s, json!({ "scale": 20 }));
    assert_eq!(img.width, 2000);
}

#[test]
fn read_pixels_hex_grid_matches_cel() {
    let mut s = S::new();
    s.new_doc(8, 8);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 2, "y": 3, "color": "#ff000080" }] }),
    );
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 3, "y": 3, "color": "#0000ff" }] }),
    );
    let out = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 2, "y": 3, "width": 2, "height": 2 }, "layer": 0, "format": "hex" }),
    );
    assert_eq!(
        out["rows"],
        json!(["#FF000080 #00000000", "#00000000 #00000000"])
    );
    // Without layer: the composed frame.
    let out = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 2, "y": 3, "width": 2, "height": 1 } }),
    );
    assert_eq!(out["rows"], json!(["#FF000080 #0000FFFF"]));
}

#[test]
fn read_pixels_index_format_uses_first_exact_palette_match() {
    let mut s = S::new();
    s.new_doc(4, 1);
    s.ok(
        "palette.set",
        json!({ "entries": ["#112233", "#445566", "#112233"] }),
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": { "index": 2 } }, { "x": 1, "y": 0, "color": "#445566" }, { "x": 2, "y": 0, "color": "#445567" }] }),
    );
    let out = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 0, "y": 0, "width": 4, "height": 1 }, "format": "index" }),
    );
    assert_eq!(out["rows"], json!(["0 1 - -"]));
}

#[test]
fn read_pixels_limits_region_size_and_canvas() {
    let mut s = S::new();
    s.new_doc(100, 100);
    let ok = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 0, "y": 0, "width": 64, "height": 64 } }),
    );
    assert_eq!(ok["rows"].as_array().map(Vec::len), Some(64));
    let e = s.err(
        "doc.read_pixels",
        json!({ "region": { "x": 0, "y": 0, "width": 65, "height": 2 } }),
    );
    assert!(e.contains("64×64"), "{e}");
    let e = s.err(
        "doc.read_pixels",
        json!({ "region": { "x": 90, "y": 0, "width": 20, "height": 2 } }),
    );
    assert!(e.contains("100×100"), "{e}");
}
