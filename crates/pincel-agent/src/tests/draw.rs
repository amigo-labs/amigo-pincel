//! `draw.*`, `selection.*`, `effect.apply` and the paint target (AC-19 …
//! AC-27, AC-43). "Same pixels as the editor" compares against the
//! `pincel-core` command the editor's wasm layer runs for that tool.

use pincel_core::{
    AnyCommand, Bus, CelData, CelMap, DrawEllipse, DrawLine, DrawRectangle, FillRegion, FrameIndex,
    LayerId, Rgba, Sprite,
};
use pincel_effects::{EffectImage, EffectSpec};
use serde_json::json;

use super::{S, px};

const L0: LayerId = LayerId::new(0);
const F0: FrameIndex = FrameIndex::new(0);

/// The cel map after the editor runs `cmd` on `state`.
fn editor(state: &(Sprite, CelMap), cmd: AnyCommand) -> CelMap {
    let (mut sprite, mut cels) = state.clone();
    Bus::new()
        .execute(cmd, &mut sprite, &mut cels)
        .expect("editor command");
    cels
}

/// A 16×16 document with some content to draw over.
fn scene(s: &mut S) -> String {
    let id = s.new_doc(16, 16);
    s.ok(
        "draw.rect",
        json!({ "x0": 2, "y0": 2, "x1": 9, "y1": 9, "color": "#336699" }),
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 12, "y": 12, "color": "#ff0000" }] }),
    );
    id
}

#[test]
fn draw_pixels_sets_listed_pixels_only_in_all_color_forms() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let before = s.cel_rgba(&id, 0, 0);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [
            { "x": 1, "y": 1, "color": "#ff0000" },
            { "x": 2, "y": 1, "color": "#00ff0080" },
            { "x": 3, "y": 1, "color": { "index": 1 } },
        ] }),
    );
    let after = s.cel_rgba(&id, 0, 0);
    let p1 = s.doc(&id).sprite.palette.colors[1].rgba;
    assert_eq!(px(&after, 8, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&after, 8, 2, 1), [0, 255, 0, 128]);
    assert_eq!(px(&after, 8, 3, 1), [p1.r, p1.g, p1.b, p1.a]);
    for y in 0..8 {
        for x in 0..8 {
            if y != 1 || !(1..=3).contains(&x) {
                assert_eq!(px(&after, 8, x, y), px(&before, 8, x, y));
            }
        }
    }
}

#[test]
fn draw_pixels_outside_canvas_rejects_whole_batch() {
    let mut s = S::new();
    let id = s.new_doc(32, 32);
    let before = s.state(&id);
    let e = s.err(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ffffff" }, { "x": 40, "y": 3, "color": "#ffffff" }, { "x": -1, "y": 0, "color": "#ffffff" }] }),
    );
    assert!(e.contains("pixel (40, 3) outside 32×32 canvas"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn malformed_color_or_missing_palette_index_fails_naming_argument() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let before = s.state(&id);
    for (args, arg) in [
        (
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#12345" }] }),
            "pixels[0].color",
        ),
        (
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "red" }] }),
            "pixels[0].color",
        ),
        (
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#000000" }, { "x": 0, "y": 1, "color": { "index": 99 } }] }),
            "pixels[1].color",
        ),
    ] {
        let e = s.err("draw.pixels", args);
        assert!(e.contains(arg) && e.contains("#RRGGBB"), "{e}");
    }
    let e = s.err(
        "draw.line",
        json!({ "x0": 0, "y0": 0, "x1": 1, "y1": 1, "color": { "index": 32 } }),
    );
    assert!(e.contains("`color`") && e.contains("#RRGGBBAA"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn draw_shapes_match_the_editor_tools() {
    let red = Rgba::new(255, 0, 0, 255);
    type Case = (&'static str, serde_json::Value, AnyCommand);
    let cases: Vec<Case> = vec![
        (
            "draw.line",
            json!({ "x0": -3, "y0": 1, "x1": 20, "y1": 11, "color": "#ff0000" }),
            DrawLine::new(L0, F0, -3, 1, 20, 11, red).into(),
        ),
        (
            "draw.rect",
            json!({ "x0": 4, "y0": 3, "x1": 13, "y1": 18, "color": "#ff0000" }),
            DrawRectangle::new(L0, F0, (4, 3), (13, 18), false, red).into(),
        ),
        (
            "draw.rect",
            json!({ "x0": 4, "y0": 3, "x1": 13, "y1": 8, "color": "#ff0000", "filled": true }),
            DrawRectangle::new(L0, F0, (4, 3), (13, 8), true, red).into(),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 1, "y0": 1, "x1": 14, "y1": 10, "color": "#ff0000" }),
            DrawEllipse::new(L0, F0, (1, 1), (14, 10), false, red).into(),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 1, "y0": 1, "x1": 14, "y1": 10, "color": "#ff0000", "filled": true }),
            DrawEllipse::new(L0, F0, (1, 1), (14, 10), true, red).into(),
        ),
    ];
    for (name, args, cmd) in cases {
        let mut s = S::new();
        let id = scene(&mut s);
        let expected = editor(&s.state(&id), cmd);
        s.ok(name, args.clone());
        assert_eq!(s.state(&id).1, expected, "{name} {args}");
    }
}

#[test]
fn draw_fill_matches_bucket_and_non_contiguous_recolors_all() {
    let mut s = S::new();
    let id = scene(&mut s);
    let expected = editor(
        &s.state(&id),
        FillRegion::new(L0, F0, 0, 0, Rgba::new(0, 0, 255, 255)).into(),
    );
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#0000ff" }));
    assert_eq!(s.state(&id).1, expected);
    // Inside the rectangle outline stays transparent (not connected).
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 16, 5, 5), [0, 0, 0, 0]);

    let mut s = S::new();
    let id = scene(&mut s);
    s.ok(
        "draw.fill",
        json!({ "x": 0, "y": 0, "color": "#0000ff", "contiguous": false }),
    );
    let after = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&after, 16, 5, 5), [0, 0, 255, 255]);
    assert_eq!(px(&after, 16, 0, 0), [0, 0, 255, 255]);
    assert_eq!(px(&after, 16, 2, 2), [0x33, 0x66, 0x99, 255]);
    assert_eq!(px(&after, 16, 12, 12), [255, 0, 0, 255]);
}

#[test]
fn draw_clear_clears_region_or_whole_cel() {
    let mut s = S::new();
    let id = s.new_doc(6, 6);
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 5, "color": "#ffffff", "filled": true }),
    );
    s.ok(
        "draw.clear",
        json!({ "region": { "x": 1, "y": 2, "width": 3, "height": 2 } }),
    );
    let after = s.cel_rgba(&id, 0, 0);
    for y in 0..6 {
        for x in 0..6 {
            let inside = (1..4).contains(&x) && (2..4).contains(&y);
            let a = px(&after, 6, x, y)[3];
            assert_eq!(a == 0, inside, "({x}, {y})");
        }
    }
    s.ok("draw.clear", json!({}));
    assert!(s.cel_rgba(&id, 0, 0).iter().all(|&b| b == 0));
}

#[test]
fn selection_limits_fill_and_effect_until_cleared() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok(
        "selection.set_rect",
        json!({ "x": 2, "y": 2, "width": 3, "height": 3 }),
    );
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" }));
    let after = s.cel_rgba(&id, 0, 0);
    for y in 0..8 {
        for x in 0..8 {
            let inside = (2..5).contains(&x) && (2..5).contains(&y);
            assert_eq!(px(&after, 8, x, y)[3] == 255, inside, "fill ({x}, {y})");
        }
    }
    s.ok("effect.apply", json!({ "id": "invert" }));
    let inv = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&inv, 8, 3, 3), [255, 0, 255, 255]);
    assert_eq!(px(&inv, 8, 0, 0), [0, 0, 0, 0]);
    s.ok("selection.clear", json!({}));
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#0000ff" }));
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 8, 7, 7), [0, 0, 255, 255]);
}

#[test]
fn effect_apply_matches_the_editor_effect() {
    let mut s = S::new();
    let id = scene(&mut s);
    let (sprite, cels) = s.state(&id);
    let CelData::Image(buf) = &cels.get(L0, F0).expect("cel").data else {
        panic!("image cel");
    };
    let src = EffectImage::from_rgba8(buf.width, buf.height, buf.data.clone()).expect("img");
    // The editor's Levels dialog sends 0–255 points divided by 255.
    let spec =
        EffectSpec::from_params("levels", &[0.0, 10.0 / 255.0, 200.0 / 255.0, 1.5, 0.0, 1.0])
            .expect("spec");
    let expected = spec.run(&src).into_raw();
    s.ok(
        "effect.apply",
        json!({ "id": "levels", "params": { "in_black": 10, "in_white": 200, "gamma": 1.5 } }),
    );
    let (_, after) = s.state(&id);
    let CelData::Image(got) = &after.get(L0, F0).expect("cel").data else {
        panic!("image cel");
    };
    assert_eq!(got.data, expected);
    drop(sprite);
    // Blur with explicit params.
    let src = EffectImage::from_rgba8(got.width, got.height, got.data.clone()).expect("img");
    let expected = EffectSpec::from_params("gaussian_blur", &[1.5])
        .expect("spec")
        .run(&src);
    s.ok(
        "effect.apply",
        json!({ "id": "gaussian_blur", "params": { "radius": 1.5 } }),
    );
    let (_, after) = s.state(&id);
    let CelData::Image(got) = &after.get(L0, F0).expect("cel").data else {
        panic!("image cel");
    };
    assert_eq!(got.data, expected.into_raw());
}

#[test]
fn effect_apply_unknown_id_or_out_of_range_param_fails() {
    let mut s = S::new();
    let id = scene(&mut s);
    let before = s.state(&id);
    let e = s.err("effect.apply", json!({ "id": "glow" }));
    assert!(e.contains("glow"), "{e}");
    let e = s.err(
        "effect.apply",
        json!({ "id": "posterize", "params": { "levels": 100 } }),
    );
    assert!(e.contains("`levels`") && e.contains("2–64"), "{e}");
    let e = s.err(
        "effect.apply",
        json!({ "id": "posterize", "params": { "steps": 3 } }),
    );
    assert!(e.contains("steps"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn paint_target_defaults_to_active_layer_and_frame() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    // layer.add makes the new layer active.
    s.ok("layer.add", json!({ "name": "Top" }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    assert_eq!(px(&s.cel_rgba(&id, 1, 0), 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 4, 0, 0), [0, 0, 0, 0]);
    // Explicit layer / frame do not change the active ones.
    s.ok("frame.add", json!({}));
    assert_eq!(s.ok("doc.info", json!({}))["active_frame"], 1);
    s.ok(
        "draw.pixels",
        json!({ "layer": 0, "frame": 0, "pixels": [{ "x": 1, "y": 0, "color": "#00ff00" }] }),
    );
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 4, 1, 0), [0, 255, 0, 255]);
    let info = s.ok("doc.info", json!({}));
    assert_eq!(
        (info["active_layer"].as_u64(), info["active_frame"].as_u64()),
        (Some(1), Some(1))
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 2, "y": 0, "color": "#0000ff" }] }),
    );
    assert_eq!(px(&s.cel_rgba(&id, 1, 1), 4, 2, 0), [0, 0, 255, 255]);
    // A group as active layer falls back to the lowest image layer.
    s.ok("layer.add", json!({ "name": "G", "kind": "group" }));
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], 0);
    s.ok("effect.apply", json!({ "id": "invert", "frame": 0 }));
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 4, 1, 0), [255, 0, 255, 255]);
    // Unknown layer / frame arguments are refused.
    assert!(s.err("draw.clear", json!({ "layer": 9 })).contains("9"));
    assert!(
        s.err("draw.clear", json!({ "frame": 9 }))
            .contains("frame 9")
    );
    assert!(
        s.err(
            "selection.set_rect",
            json!({ "x": 0, "y": 0, "width": 1, "height": 1, "frame": 5 })
        )
        .contains("frame 5")
    );
}
