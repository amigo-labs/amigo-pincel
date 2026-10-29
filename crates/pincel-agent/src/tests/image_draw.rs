//! Image editing tools against the editor's own commands: strokes, shapes,
//! text, fill, gradient, selections, adjustments / effects and canvas
//! transforms (mcp-headless-image AC-06 … AC-14, AC-21).

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::command::{CommandBus, SetPixels};
use pincel_image_core::{Document, ImageBuffer, Layer, SelectionMask, masked_region};
use serde_json::{Value, json};

use super::S;
use crate::image::FONT;

/// Parse a command exactly as the editor sends it.
fn spec(v: Value) -> CommandSpec {
    serde_json::from_value(v).expect("valid CommandSpec")
}

/// The editor's result: `cmd` run on a copy of the document.
fn editor(s: &S, id: &str, cmds: Vec<Value>) -> Document {
    let mut bus = CommandBus::new(s.image(id).document().clone());
    for cmd in cmds {
        run_command(&mut bus, spec(cmd), &mut StrokeScratch::default(), &[FONT]).expect("runs");
    }
    bus.document
}

fn pixels(doc: &Document, layer: usize) -> Vec<u8> {
    doc.layers()[layer].pixels.data().to_vec()
}

fn agent_pixels(s: &S, id: &str, layer: usize) -> Vec<u8> {
    pixels(s.image(id).document(), layer)
}

fn state(s: &S, id: &str) -> (u32, u32, Vec<Layer>, Option<SelectionMask>) {
    let d = s.image(id).document();
    (
        d.canvas.width(),
        d.canvas.height(),
        d.layers().to_vec(),
        d.selection.clone(),
    )
}

/// A document with a varied background to edit on.
fn painted(s: &mut S, w: u32, h: u32) -> String {
    let id = s.new_image(w, h);
    s.ok(
        "draw.gradient",
        json!({ "x0": 0, "y0": 0, "x1": w, "y1": h, "from": "#102030", "to": "#e0c040" }),
    );
    id
}

/// Assert that one call is exactly one undo step that restores the state.
#[track_caller]
fn one_undo_step(s: &mut S, id: &str, name: &str, args: Value) {
    let before = state(s, id);
    let depth = s.image(id).bus.history.undo_depth();
    s.ok(name, args);
    assert_eq!(s.image(id).bus.history.undo_depth(), depth + 1, "{name}");
    s.ok("doc.undo", json!({}));
    assert_eq!(state(s, id), before, "{name} undo");
    s.ok("doc.redo", json!({}));
}

#[test]
fn draw_stroke_matches_brush_and_eraser() {
    let mut s = S::new();
    let id = painted(&mut s, 24, 16);
    let points = json!([[2.5, 3.5], [12.0, 9.0], [20.5, 4.0]]);
    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "pencil_stroke", "layer": 0, "size": 5, "color": [200, 30, 40, 255],
            "opacity": 0.6, "shape": "soft_round", "hardness": 0.4, "points": points, "stroke_id": 1
        })],
    );
    s.ok(
        "draw.stroke",
        json!({ "points": points, "size": 5, "hardness": 40, "opacity": 60, "color": "#c81e28", "shape": "soft_round" }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));

    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "eraser_stroke", "layer": 0, "size": 3, "opacity": 1.0,
            "points": [[1, 1], [10, 10]], "stroke_id": 2
        })],
    );
    s.ok(
        "draw.stroke",
        json!({ "tool": "eraser", "points": [[1, 1], [10, 10]], "size": 3 }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));

    // A single point is one dab; two calls are two undo steps.
    let depth = s.image(&id).bus.history.undo_depth();
    s.ok(
        "draw.stroke",
        json!({ "points": [[5, 5]], "color": "#000000" }),
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[5, 5]], "color": "#000000" }),
    );
    assert_eq!(s.image(&id).bus.history.undo_depth(), depth + 2);
    one_undo_step(
        &mut s,
        &id,
        "draw.stroke",
        json!({ "points": [[3, 3], [9, 3]], "color": "#00ff00" }),
    );
}

#[test]
fn draw_stroke_with_empty_points_fails_naming_points() {
    let mut s = S::new();
    let id = s.new_image(8, 8);
    let before = state(&s, &id);
    let e = s.err("draw.stroke", json!({ "points": [], "color": "#000000" }));
    assert!(e.contains("points"), "{e}");
    assert_eq!(state(&s, &id), before);
}

#[test]
fn draw_shape_matches_the_shape_tools() {
    let cases = [
        (
            json!({ "shape": "rect", "x0": 2, "y0": 2, "x1": 14, "y1": 10, "stroke": "#ff0000", "stroke_width": 2 }),
            json!({ "type": "draw_shape", "layer": 0, "shape": "rectangle", "points": [[2, 2], [14, 10]], "mode": "outline", "stroke_width": 2, "stroke_color": [255, 0, 0, 255], "anti_alias": true }),
        ),
        (
            json!({ "shape": "rounded", "x0": 1, "y0": 1, "x1": 18, "y1": 12, "fill": "#00ff0080", "corner_radius": 4 }),
            json!({ "type": "draw_shape", "layer": 0, "shape": "rounded_rectangle", "points": [[1, 1], [18, 12]], "corner_radius": 4, "mode": "fill", "stroke_width": 3, "fill_color": [0, 255, 0, 128], "anti_alias": true }),
        ),
        (
            json!({ "shape": "ellipse", "x0": 3, "y0": 2, "x1": 17, "y1": 13, "stroke": "#0000ff", "fill": "#ffff00", "anti_alias": false, "dash": "dashed" }),
            json!({ "type": "draw_shape", "layer": 0, "shape": "ellipse", "points": [[3, 2], [17, 13]], "mode": "fill_and_outline", "stroke_width": 3, "stroke_color": [0, 0, 255, 255], "fill_color": [255, 255, 0, 255], "anti_alias": false, "dash": "dashed" }),
        ),
        (
            json!({ "shape": "polygon", "cx": 10, "cy": 8, "radius": 6, "sides": 6, "rotation": 90, "fill": "#123456" }),
            json!({ "type": "draw_shape", "layer": 0, "shape": "polygon", "center": [10, 8], "radius": 6, "sides": 6, "rotation": 90f32.to_radians(), "mode": "fill", "stroke_width": 3, "fill_color": [18, 52, 86, 255], "anti_alias": true }),
        ),
        (
            json!({ "shape": "line", "x0": 0, "y0": 15, "x1": 19, "y1": 0, "stroke": "#ffffff", "stroke_width": 1 }),
            json!({ "type": "draw_shape", "layer": 0, "shape": "line", "points": [[0, 15], [19, 0]], "mode": "outline", "stroke_width": 1, "stroke_color": [255, 255, 255, 255], "anti_alias": true }),
        ),
    ];
    for (args, cmd) in cases {
        let mut s = S::new();
        let id = painted(&mut s, 20, 16);
        let want = editor(&s, &id, vec![cmd]);
        let before = agent_pixels(&s, &id, 0);
        s.ok("draw.shape", args.clone());
        assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0), "{args}");
        assert_ne!(agent_pixels(&s, &id, 0), before, "{args} drew something");
    }
    let mut s = S::new();
    let id = s.new_image(8, 8);
    let e = s.err(
        "draw.shape",
        json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 4, "y1": 4 }),
    );
    assert!(e.contains("stroke") && e.contains("fill"), "{e}");
    let e = s.err(
        "draw.shape",
        json!({ "shape": "polygon", "fill": "#000000" }),
    );
    assert!(e.contains("cx"), "{e}");
    assert_eq!(state(&s, &id).2.len(), 1);
}

#[test]
fn draw_text_matches_the_text_tool() {
    let mut s = S::new();
    let id = s.new_image(120, 40);
    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "draw_text", "layer": 0, "font_id": 0, "text": "Hi there", "x": 60, "y": 4,
            "size": 18, "color": [10, 20, 200, 255], "bold": true, "anti_alias": true, "align": "center"
        })],
    );
    s.ok(
        "draw.text",
        json!({ "text": "Hi there", "x": 60, "y": 4, "size": 18, "color": "#0a14c8", "align": "center", "bold": true }),
    );
    let got = agent_pixels(&s, &id, 0);
    assert_eq!(got, pixels(&want, 0));
    assert!(got.chunks(4).any(|p| p[3] > 0), "text was drawn");
}

#[test]
fn draw_fill_and_gradient_match_bucket_and_gradient_tools() {
    let mut s = S::new();
    let id = s.new_image(16, 12);
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 2, "y0": 2, "x1": 10, "y1": 8, "stroke": "#000000", "anti_alias": false, "stroke_width": 1 }),
    );
    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "fill_bucket", "layer": 0, "color": [255, 0, 0, 255], "opacity": 0.5,
            "tolerance": 10, "contiguous": true, "x": 5, "y": 5
        })],
    );
    s.ok(
        "draw.fill",
        json!({ "x": 5, "y": 5, "color": "#ff0000", "tolerance": 10, "opacity": 50 }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));

    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "fill_bucket", "layer": 0, "color": [0, 0, 255, 255], "opacity": 1.0,
            "tolerance": 32, "contiguous": false, "x": 0, "y": 0
        })],
    );
    s.ok(
        "draw.fill",
        json!({ "x": 0, "y": 0, "color": "#0000ff", "contiguous": false }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));

    let want = editor(
        &s,
        &id,
        vec![json!({
            "type": "draw_gradient", "layer": 0, "kind": "radial", "channel": "color",
            "x0": 8, "y0": 6, "x1": 15, "y1": 6, "from": [255, 255, 255, 255], "to": [0, 0, 0, 0],
            "opacity": 0.75
        })],
    );
    s.ok(
        "draw.gradient",
        json!({ "x0": 8, "y0": 6, "x1": 15, "y1": 6, "from": "#ffffff", "to": "#00000000", "kind": "radial", "opacity": 75 }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));
    one_undo_step(
        &mut s,
        &id,
        "draw.fill",
        json!({ "x": 1, "y": 1, "color": "#00ff00" }),
    );
    one_undo_step(
        &mut s,
        &id,
        "draw.gradient",
        json!({ "x0": 0, "y0": 0, "x1": 4, "y1": 0, "from": "#ff0000", "to": "#0000ff" }),
    );
}

#[test]
fn select_tools_match_the_selection_tools_and_clear_removes_it() {
    let mut s = S::new();
    let id = painted(&mut s, 20, 14);
    let want = editor(
        &s,
        &id,
        vec![
            json!({ "type": "select_rectangle", "x": 2, "y": 3, "w": 8, "h": 6, "mode": "replace", "feather": 2 }),
        ],
    );
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 3, "width": 8, "height": 6, "feather": 2 }),
    );
    assert_eq!(s.image(&id).document().selection, want.selection);

    let want = editor(
        &s,
        &id,
        vec![json!({ "type": "select_ellipse", "x": 6, "y": 1, "w": 10, "h": 9, "mode": "add" })],
    );
    s.ok(
        "select.ellipse",
        json!({ "x": 6, "y": 1, "width": 10, "height": 9, "mode": "add" }),
    );
    assert_eq!(s.image(&id).document().selection, want.selection);

    let want = editor(
        &s,
        &id,
        vec![
            json!({ "type": "select_wand", "layer": 0, "x": 3, "y": 3, "tolerance": 20, "contiguous": true, "mode": "replace" }),
        ],
    );
    s.ok("select.wand", json!({ "x": 3, "y": 3, "tolerance": 20 }));
    assert_eq!(s.image(&id).document().selection, want.selection);
    one_undo_step(
        &mut s,
        &id,
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 3, "height": 3 }),
    );

    s.ok("select.clear", json!({}));
    assert_eq!(s.image(&id).document().selection, None);

    // The wand on a uniform layer selects the whole canvas.
    let mut s = S::new();
    let id = s.new_image(9, 7);
    s.ok("select.wand", json!({ "x": 4, "y": 4 }));
    let sel = s
        .image(&id)
        .document()
        .selection
        .clone()
        .expect("selection");
    assert_eq!(sel.selected_count(), 9 * 7);
}

/// The editor's `apply_effect`: run on the layer, keep the result inside
/// the selection by coverage, commit as `SetPixels`.
fn editor_effect(s: &S, id: &str, name: &str, params: &[f64]) -> Vec<u8> {
    let doc = s.image(id).document();
    let spec = EffectSpec::from_params(name, params).expect("spec");
    let before = &doc.layers()[0].pixels;
    let src = EffectImage::from_rgba8(before.width(), before.height(), before.data().to_vec())
        .expect("image");
    let after = ImageBuffer::from_raw(before.width(), before.height(), spec.run(&src).into_raw())
        .expect("buffer");
    let mut bus = CommandBus::new(doc.clone());
    if let Some((rect, region)) = masked_region(before, &after, doc.selection.as_ref()) {
        bus.apply(Box::new(SetPixels::new(0, rect, region)))
            .expect("apply");
    }
    pixels(&bus.document, 0)
}

#[test]
fn adjust_and_effect_apply_match_the_editor() {
    let mut s = S::new();
    let id = painted(&mut s, 16, 12);
    let want = editor_effect(&s, &id, "gaussian_blur", &[1.5]);
    s.ok(
        "effect.apply",
        json!({ "id": "gaussian_blur", "params": { "radius": 1.5 } }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), want);

    let want = editor_effect(&s, &id, "hue_saturation", &[30.0, -20.0, 10.0, 0.0]);
    s.ok(
        "adjust.apply",
        json!({ "id": "hue_saturation", "params": { "hue": 30, "saturation": -20, "lightness": 10 } }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), want);
    one_undo_step(&mut s, &id, "adjust.apply", json!({ "id": "invert" }));
}

#[test]
fn adjust_and_effect_apply_refuse_unknown_ids_and_bad_params() {
    let mut s = S::new();
    let id = painted(&mut s, 8, 8);
    let before = state(&s, &id);
    let e = s.err("effect.apply", json!({ "id": "sparkle" }));
    assert!(e.contains("sparkle"), "{e}");
    let e = s.err(
        "adjust.apply",
        json!({ "id": "posterize", "params": { "levels": 200 } }),
    );
    assert!(e.contains("levels") && e.contains("2–64"), "{e}");
    let e = s.err("adjust.apply", json!({ "id": "gaussian_blur" }));
    assert!(
        e.contains("gaussian_blur") && e.contains("effect.apply"),
        "{e}"
    );
    assert_eq!(state(&s, &id), before);
}

#[test]
fn selection_limits_fill_adjust_and_effect_with_feathered_edges() {
    let mut s = S::new();
    let id = painted(&mut s, 20, 16);
    s.ok(
        "select.ellipse",
        json!({ "x": 4, "y": 3, "width": 10, "height": 9, "feather": 2 }),
    );
    let mask = s.image(&id).document().selection.clone().expect("mask");
    let outside: Vec<(u32, u32)> = (0..16)
        .flat_map(|y| (0..20).map(move |x| (x, y)))
        .filter(|&(x, y)| mask.coverage(x, y) == 0)
        .collect();
    let original = agent_pixels(&s, &id, 0);
    let unchanged_outside = |s: &S| {
        let now = agent_pixels(s, &id, 0);
        outside.iter().all(|&(x, y)| {
            let i = ((y * 20 + x) * 4) as usize;
            now[i..i + 4] == original[i..i + 4]
        })
    };

    let want = editor(
        &s,
        &id,
        vec![
            json!({ "type": "fill_bucket", "layer": 0, "color": [255, 0, 255, 255], "opacity": 1.0, "tolerance": 255, "contiguous": false, "x": 8, "y": 8 }),
        ],
    );
    s.ok(
        "draw.fill",
        json!({ "x": 8, "y": 8, "color": "#ff00ff", "tolerance": 255, "contiguous": false }),
    );
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));
    assert!(unchanged_outside(&s));

    let want = editor_effect(&s, &id, "invert", &[]);
    s.ok("adjust.apply", json!({ "id": "invert" }));
    assert_eq!(agent_pixels(&s, &id, 0), want);
    assert!(unchanged_outside(&s));

    let want = editor_effect(&s, &id, "box_blur", &[3.0, 3.0]);
    s.ok("effect.apply", json!({ "id": "box_blur" }));
    assert_eq!(agent_pixels(&s, &id, 0), want);
    assert!(unchanged_outside(&s));
}

#[test]
fn canvas_and_image_transforms_match_the_editor_and_undo_in_one_step() {
    let cases: Vec<(&str, Value, Value)> = vec![
        (
            "canvas.resize",
            json!({ "width": 30, "height": 10, "anchor": "center" }),
            json!({ "type": "resize_canvas", "width": 30, "height": 10, "anchor": "center" }),
        ),
        (
            "image.scale",
            json!({ "width": 10, "height": 24, "interpolation": "nearest" }),
            json!({ "type": "scale_image", "width": 10, "height": 24, "interpolation": "nearest" }),
        ),
        (
            "image.flip",
            json!({ "direction": "vertical" }),
            json!({ "type": "flip_canvas", "horizontal": false }),
        ),
        (
            "image.rotate",
            json!({ "angle": 90 }),
            json!({ "type": "rotate_canvas", "rotation": "cw90" }),
        ),
        (
            "image.rotate",
            json!({ "angle": 270 }),
            json!({ "type": "rotate_canvas", "rotation": "ccw90" }),
        ),
    ];
    for (name, args, cmd) in cases {
        let mut s = S::new();
        let id = painted(&mut s, 20, 12);
        s.ok("layer.add", json!({}));
        s.ok(
            "draw.stroke",
            json!({ "points": [[3, 3], [15, 9]], "color": "#ff0000" }),
        );
        let want = editor(&s, &id, vec![cmd]);
        let before = state(&s, &id);
        s.ok(name, args.clone());
        let got = s.image(&id).document();
        assert_eq!(
            (got.canvas.width(), got.canvas.height()),
            (want.canvas.width(), want.canvas.height()),
            "{name} {args}"
        );
        for l in 0..2 {
            assert_eq!(pixels(got, l), pixels(&want, l), "{name} {args} layer {l}");
        }
        s.ok("doc.undo", json!({}));
        assert_eq!(state(&s, &id), before, "{name} undo");
    }
    // 90° on a non-square image swaps width and height.
    let mut s = S::new();
    s.new_image(20, 12);
    let out = s.ok("image.rotate", json!({ "angle": 90 }));
    assert_eq!(
        (out["width"].as_u64(), out["height"].as_u64()),
        (Some(12), Some(20))
    );

    // Crop to the selection.
    let mut s = S::new();
    let id = painted(&mut s, 20, 12);
    s.ok(
        "select.rect",
        json!({ "x": 3, "y": 2, "width": 7, "height": 5 }),
    );
    let want = editor(&s, &id, vec![json!({ "type": "crop_to_selection" })]);
    one_undo_step(&mut s, &id, "image.crop_to_selection", json!({}));
    assert_eq!(s.image(&id).document().canvas.width(), 7);
    assert_eq!(agent_pixels(&s, &id, 0), pixels(&want, 0));
}

#[test]
fn crop_to_selection_without_selection_fails() {
    let mut s = S::new();
    let id = painted(&mut s, 8, 8);
    let before = state(&s, &id);
    let depth = s.image(&id).bus.history.undo_depth();
    let e = s.err("image.crop_to_selection", json!({}));
    assert!(e.contains("selection"), "{e}");
    assert_eq!(state(&s, &id), before);
    assert_eq!(s.image(&id).bus.history.undo_depth(), depth);
}

#[test]
fn resize_and_scale_refuse_sizes_above_the_limits() {
    let mut s = S::new();
    let id = s.new_image(64, 64);
    let before = state(&s, &id);
    let e = s.err("canvas.resize", json!({ "width": 20000, "height": 10 }));
    assert!(e.contains("16384") && e.contains("20000×10"), "{e}");
    let e = s.err("image.scale", json!({ "width": 9000, "height": 9000 }));
    assert!(e.contains("64 Mpx") && e.contains("9000×9000"), "{e}");
    assert_eq!(state(&s, &id), before);
}

#[test]
fn image_tools_refuse_locked_layers_and_pixel_documents() {
    let mut s = S::new();
    let id = s.new_image(8, 8);
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    let before = state(&s, &id);
    for (name, args) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "color": "#000000" }),
        ),
        ("draw.fill", json!({ "x": 1, "y": 1, "color": "#000000" })),
        ("adjust.apply", json!({ "id": "invert" })),
        ("draw.text", json!({ "text": "A", "x": 0, "y": 0 })),
    ] {
        let e = s.err(name, args);
        assert!(e.contains("locked"), "{name}: {e}");
    }
    assert_eq!(state(&s, &id), before);

    let px = s.new_doc(8, 8);
    for (name, args) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "color": "#000000" }),
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000" }),
        ),
        ("draw.text", json!({ "text": "A", "x": 0, "y": 0 })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff" }),
        ),
        (
            "select.rect",
            json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
        ),
        (
            "select.ellipse",
            json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
        ),
        ("select.wand", json!({ "x": 0, "y": 0 })),
        ("select.clear", json!({})),
        ("canvas.resize", json!({ "width": 4, "height": 4 })),
        ("image.scale", json!({ "width": 4, "height": 4 })),
        ("image.crop_to_selection", json!({})),
        ("image.flip", json!({ "direction": "horizontal" })),
        ("image.rotate", json!({ "angle": 180 })),
        (
            "draw.fill",
            json!({ "x": 1, "y": 1, "color": "#000000", "tolerance": 4 }),
        ),
    ] {
        let before = s.state(&px);
        let e = s.err(name, args);
        assert!(
            e.contains("Pixel document") || e.contains("Image documents"),
            "{name}: {e}"
        );
        assert_eq!(s.state(&px), before, "{name}");
    }
}
