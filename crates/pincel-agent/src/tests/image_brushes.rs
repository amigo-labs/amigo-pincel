//! Image Recolor, Smudge, eraser modes, nib angle, bucket sampling and the
//! per-mode effect defaults (mcp-parity-drawing-image).

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::Layer;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::{Value, json};

use super::S;
use crate::list_tools;

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// The layers after the editor runs `spec` (JSON) on a copy.
fn editor(s: &S, id: &str, spec: Value) -> Vec<Layer> {
    let spec: CommandSpec = serde_json::from_value(spec).expect("spec");
    let mut bus = CommandBus::new(s.image(id).document().clone());
    run_command(&mut bus, spec, &mut StrokeScratch::default(), &[]).expect("editor");
    bus.document.layers().to_vec()
}

fn layers(s: &S, id: &str) -> Vec<Layer> {
    s.image(id).document().layers().to_vec()
}

/// A 24×16 Image document: a red block on blue.
fn painted(s: &mut S) -> String {
    let id = s.new_image(24, 16);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#0000ff" }));
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 4, "y0": 4, "x1": 14, "y1": 11, "fill": "#ff0000", "anti_alias": false }),
    );
    id
}

fn stroke_base(kind: &str) -> Value {
    json!({
        "type": kind, "layer": 0, "size": 8, "opacity": 1.0, "shape": "hard_round", "hardness": 1.0,
        "points": [[2.0, 8.0], [20.0, 8.0]], "angle": 45.0, "stroke_id": 1,
    })
}

#[test]
fn recolor_and_smudge_are_listed_and_stroke_takes_brush_or_eraser() {
    let tools = list_tools();
    for name in ["draw.recolor", "draw.smudge"] {
        assert!(tools.iter().any(|t| t.name == name), "{name}");
    }
    let stroke = tools
        .iter()
        .find(|t| t.name == "draw.stroke")
        .expect("stroke");
    assert_eq!(
        stroke.input_schema["properties"]["tool"]["enum"],
        json!(["brush", "eraser"])
    );
    // Another tool's name fails naming `tool` and its values, and points
    // to the tool that has it.
    let mut s = S::new();
    let id = painted(&mut s);
    let before = layers(&s, &id);
    for (value, other) in [("recolor", "draw.recolor"), ("smudge", "draw.smudge")] {
        let e = s.err("draw.stroke", json!({ "points": [[1, 1]], "tool": value }));
        for needle in ["tool", "brush", "eraser", other] {
            assert!(e.contains(needle), "{value}: {needle} in {e}");
        }
        assert_eq!(layers(&s, &id), before);
    }
}

#[test]
fn draw_recolor_equals_the_editors_recolor_tool() {
    let mut s = S::new();
    let id = painted(&mut s);
    let mut spec = stroke_base("recolor_stroke");
    spec["from"] = json!([255, 0, 0, 255]);
    spec["to"] = json!([0, 255, 0, 255]);
    spec["tolerance"] = json!(10);
    spec["size"] = json!(6);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.recolor",
        json!({ "points": [[2, 8], [20, 8]], "from": "#ff0000", "color": "#00ff00", "tolerance": 10, "size": 6 }),
    );
    assert_eq!(layers(&s, &id), want);
    // Defaults: tolerance 32, color #000000, size 8.
    let mut spec = stroke_base("recolor_stroke");
    spec["from"] = json!([0, 0, 255, 255]);
    spec["to"] = json!([0, 0, 0, 255]);
    spec["tolerance"] = json!(32);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.recolor",
        json!({ "points": [[2, 8], [20, 8]], "from": "#0000ff" }),
    );
    assert_eq!(layers(&s, &id), want);
    // The brush arguments of draw.stroke: hardness, opacity and shape.
    let mut spec = stroke_base("recolor_stroke");
    spec["from"] = json!([0, 0, 0, 255]);
    spec["to"] = json!([255, 255, 0, 255]);
    spec["tolerance"] = json!(255);
    spec["hardness"] = json!(0.4);
    spec["opacity"] = json!(0.7);
    spec["shape"] = json!("soft_round");
    spec["stroke_id"] = json!(3);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.recolor",
        json!({ "points": [[2, 8], [20, 8]], "from": "#000000", "color": "#ffff00", "tolerance": 255, "hardness": 40, "opacity": 70, "shape": "soft_round" }),
    );
    assert_eq!(layers(&s, &id), want, "hardness, opacity, shape");
}

#[test]
fn draw_recolor_with_a_bad_color_fails_naming_it() {
    let mut s = S::new();
    let id = painted(&mut s);
    let before = layers(&s, &id);
    for (args, arg) in [
        (json!({ "points": [[1, 1]] }), "from"),
        (json!({ "points": [[1, 1]], "from": "red" }), "from"),
        (
            json!({ "points": [[1, 1]], "from": { "index": 2 } }),
            "from",
        ),
        (
            json!({ "points": [[1, 1]], "from": "#ff0000", "color": "#12" }),
            "color",
        ),
        (
            json!({ "points": [[1, 1]], "from": "#ff0000", "color": { "index": 2 } }),
            "color",
        ),
    ] {
        let e = s.err("draw.recolor", args.clone());
        // The argument and both accepted forms, a missing `from` included.
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("#RRGGBB\"") && e.contains("#RRGGBBAA\""),
            "{args}: {e}"
        );
        assert_eq!(layers(&s, &id), before);
    }
}

#[test]
fn draw_smudge_equals_the_editors_smudge_tool() {
    let mut s = S::new();
    let id = painted(&mut s);
    let mut spec = stroke_base("smudge_stroke");
    spec["strength"] = json!(0.6);
    let want = editor(&s, &id, spec);
    s.ok("draw.smudge", json!({ "points": [[2, 8], [20, 8]] }));
    assert_eq!(layers(&s, &id), want);
    let mut spec = stroke_base("smudge_stroke");
    spec["strength"] = json!(0.25);
    spec["shape"] = json!("soft_round");
    spec["stroke_id"] = json!(2);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.smudge",
        json!({ "points": [[2, 8], [20, 8]], "strength": 25, "shape": "soft_round" }),
    );
    assert_eq!(layers(&s, &id), want);
    // The other brush arguments, a fractional strength and one point.
    let mut spec = stroke_base("smudge_stroke");
    spec["strength"] = json!(0.375);
    spec["size"] = json!(5);
    spec["hardness"] = json!(0.5);
    spec["opacity"] = json!(0.8);
    spec["points"] = json!([[3.0, 2.0], [14.0, 12.0]]);
    spec["stroke_id"] = json!(3);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.smudge",
        json!({ "points": [[3, 2], [14, 12]], "strength": 37.5, "size": 5, "hardness": 50, "opacity": 80 }),
    );
    assert_eq!(layers(&s, &id), want, "size, hardness, opacity");
    // One point is one click of the Smudge tool.
    let mut spec = stroke_base("smudge_stroke");
    spec["strength"] = json!(0.6);
    spec["points"] = json!([[9.0, 8.0]]);
    spec["stroke_id"] = json!(4);
    let want = editor(&s, &id, spec);
    s.ok("draw.smudge", json!({ "points": [[9, 8]] }));
    assert_eq!(layers(&s, &id), want, "one point");
    // Smudge paints no color of its own: `color` is an unknown argument.
    let before = layers(&s, &id);
    let e = s.err(
        "draw.smudge",
        json!({ "points": [[9, 8]], "color": "#ff0000" }),
    );
    assert!(e.contains("unknown argument `color`"), "{e}");
    assert_eq!(layers(&s, &id), before);
}

#[test]
fn eraser_to_background_equals_the_editors_eraser() {
    let mut s = S::new();
    let id = painted(&mut s);
    let mut spec = stroke_base("eraser_stroke");
    spec["mode"] = json!("to_background");
    spec["background"] = json!([10, 20, 30, 255]);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 8], [20, 8]], "tool": "eraser", "eraser_mode": "to_background", "background": "#0a141e" }),
    );
    assert_eq!(layers(&s, &id), want);
    let mut spec = stroke_base("eraser_stroke");
    spec["mode"] = json!("to_background");
    spec["background"] = json!([255, 255, 255, 255]);
    spec["points"] = json!([[2.0, 2.0], [20.0, 2.0]]);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 2], [20, 2]], "tool": "eraser", "eraser_mode": "to_background" }),
    );
    assert_eq!(layers(&s, &id), want, "default background white");
    let mut spec = stroke_base("eraser_stroke");
    spec["mode"] = json!("to_background");
    spec["background"] = json!([0, 0, 0, 0]);
    spec["points"] = json!([[2.0, 5.0], [20.0, 5.0]]);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 5], [20, 5]], "tool": "eraser", "eraser_mode": "to_background", "background": "#00000000" }),
    );
    assert_eq!(layers(&s, &id), want, "transparent background swatch");
    let mut spec = stroke_base("eraser_stroke");
    spec["mode"] = json!("to_transparent");
    spec["points"] = json!([[2.0, 13.0], [20.0, 13.0]]);
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 13], [20, 13]], "tool": "eraser", "eraser_mode": "to_transparent" }),
    );
    assert_eq!(layers(&s, &id), want);
}

#[test]
fn the_nib_angle_reaches_every_image_stroke() {
    for (tool, kind, extra) in [
        (
            "draw.stroke",
            "pencil_stroke",
            json!({ "color": [0, 0, 0, 255] }),
        ),
        (
            "draw.recolor",
            "recolor_stroke",
            json!({ "from": [255, 0, 0, 255], "to": [0, 0, 0, 255], "tolerance": 32 }),
        ),
        ("draw.smudge", "smudge_stroke", json!({ "strength": 0.6 })),
    ] {
        let mut s = S::new();
        let id = painted(&mut s);
        let mut spec = stroke_base(kind);
        spec["shape"] = json!("flat");
        spec["angle"] = json!(120.0);
        for (k, v) in extra.as_object().expect("obj") {
            spec[k] = v.clone();
        }
        let want = editor(&s, &id, spec);
        let mut args = json!({ "points": [[2, 8], [20, 8]], "shape": "flat", "angle": 120 });
        if tool == "draw.recolor" {
            args["from"] = json!("#ff0000");
        }
        s.ok(tool, args);
        assert_eq!(layers(&s, &id), want, "{tool}");
        // Without `angle` a flat tip lies at 45 degrees; fractional
        // angles are accepted.
        for (angle, wire) in [(None, 45.0), (Some(22.5), 22.5)] {
            let mut spec = stroke_base(kind);
            spec["shape"] = json!("flat");
            spec["angle"] = json!(wire);
            spec["points"] = json!([[2.0, 3.0], [20.0, 12.0]]);
            spec["stroke_id"] = json!(2);
            for (k, v) in extra.as_object().expect("obj") {
                spec[k] = v.clone();
            }
            let mut probe = S::new();
            let pid = painted(&mut probe);
            let want = editor(&probe, &pid, spec);
            let mut args = json!({ "points": [[2, 3], [20, 12]], "shape": "flat" });
            if let Some(a) = angle {
                args["angle"] = json!(a);
            }
            if tool == "draw.recolor" {
                args["from"] = json!("#ff0000");
            }
            probe.ok(tool, args);
            assert_eq!(layers(&probe, &pid), want, "{tool} angle {angle:?}");
        }
    }
    // A round tip ignores the angle: its pixels equal the stroke without
    // `angle`, for every Image stroke tool and the eraser.
    for (tool, extra) in [
        ("draw.stroke", json!({})),
        (
            "draw.stroke",
            json!({ "tool": "eraser", "eraser_mode": "to_transparent" }),
        ),
        ("draw.recolor", json!({ "from": "#ff0000" })),
        ("draw.smudge", json!({})),
    ] {
        let run = |angle: Option<f64>| {
            let mut s = S::new();
            let id = painted(&mut s);
            let mut args = json!({ "points": [[2, 8], [20, 3]] });
            for (k, v) in extra.as_object().expect("obj") {
                args[k] = v.clone();
            }
            if let Some(a) = angle {
                args["angle"] = json!(a);
            }
            s.ok(tool, args);
            layers(&s, &id)[0].pixels.clone()
        };
        let plain = run(None);
        assert_eq!(run(Some(10.0)), plain, "{tool} {extra} angle 10");
        assert_eq!(run(Some(170.0)), plain, "{tool} {extra} angle 170");
    }
}

#[test]
fn draw_fill_sample_all_layers_equals_the_editors_bucket() {
    let mut s = S::new();
    let id = painted(&mut s);
    s.ok("layer.add", json!({}));
    let mut spec = json!({ "type": "fill_bucket", "layer": 1, "color": [0, 255, 0, 255], "opacity": 1.0, "tolerance": 32, "contiguous": true, "sample": "all_layers", "x": 6.0, "y": 6.0 });
    let want = editor(&s, &id, spec.clone());
    s.ok(
        "draw.fill",
        json!({ "x": 6, "y": 6, "color": "#00ff00", "sample": "all_layers" }),
    );
    assert_eq!(layers(&s, &id), want);
    s.ok("doc.undo", json!({}));
    spec["sample"] = json!("current_layer");
    let want = editor(&s, &id, spec.clone());
    s.ok(
        "draw.fill",
        json!({ "x": 6, "y": 6, "color": "#00ff00", "sample": "current_layer" }),
    );
    assert_eq!(layers(&s, &id), want);
    s.ok("doc.undo", json!({}));
    // Without `sample` the fill samples the current layer, as before.
    s.ok("draw.fill", json!({ "x": 6, "y": 6, "color": "#00ff00" }));
    assert_eq!(layers(&s, &id), want, "no sample");
    s.ok("doc.undo", json!({}));
    // With the lower layer hidden, as the editor's Bucket samples it.
    s.ok("layer.set", json!({ "layer": 0, "visible": false }));
    spec["sample"] = json!("all_layers");
    let want = editor(&s, &id, spec);
    s.ok(
        "draw.fill",
        json!({ "x": 6, "y": 6, "color": "#00ff00", "sample": "all_layers", "layer": 1 }),
    );
    assert_eq!(layers(&s, &id), want, "hidden layer");
}

#[test]
fn out_of_range_or_unknown_brush_values_fail_naming_the_argument() {
    let mut s = S::new();
    let id = painted(&mut s);
    for (tool, args, arg, allowed) in [
        (
            "draw.recolor",
            json!({ "points": [[1, 1]], "from": "#ff0000", "tolerance": 300 }),
            "tolerance",
            &["0–255"][..],
        ),
        (
            "draw.smudge",
            json!({ "points": [[1, 1]], "strength": 0 }),
            "strength",
            &["1–100"][..],
        ),
        (
            "draw.smudge",
            json!({ "points": [[1, 1]], "angle": 200 }),
            "angle",
            &["0–180"][..],
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "angle": -1 }),
            "angle",
            &["0–180"][..],
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "tool": "eraser", "eraser_mode": "to_white" }),
            "eraser_mode",
            &["to_transparent", "to_background"][..],
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "tool": "eraser", "background": "white" }),
            "background",
            &["#RRGGBB", "#RRGGBBAA"][..],
        ),
        (
            "draw.fill",
            json!({ "x": 1, "y": 1, "color": "#000000", "sample": "screen" }),
            "sample",
            &["current_layer", "all_layers"][..],
        ),
    ] {
        let before = layers(&s, &id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(arg), "{tool} {args}: {e}");
        for a in allowed {
            assert!(e.contains(a), "{tool} {args} misses {a}: {e}");
        }
        assert_eq!(layers(&s, &id), before);
    }
}

#[test]
fn image_only_brushes_and_options_fail_on_pixel_documents() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (tool, args, needle) in [
        (
            "draw.recolor",
            json!({ "points": [[1, 1]], "from": "#ff0000" }),
            "Pixel document",
        ),
        (
            "draw.smudge",
            json!({ "points": [[1, 1]] }),
            "Pixel document",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "eraser_mode": "to_background" }),
            "`eraser_mode`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "background": "#ffffff" }),
            "`background`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "angle": 30 }),
            "`angle`",
        ),
        (
            "draw.fill",
            json!({ "x": 1, "y": 1, "color": "#000000", "sample": "all_layers" }),
            "`sample`",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(needle) && e.contains("Pixel document"),
            "{tool} {args}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn recolor_and_smudge_stay_inside_the_selection() {
    for tool in ["draw.recolor", "draw.smudge"] {
        let mut s = S::new();
        let id = painted(&mut s);
        let before = layers(&s, &id)[0].pixels.clone();
        s.ok(
            "select.rect",
            json!({ "x": 8, "y": 0, "width": 4, "height": 16, "feather": 1 }),
        );
        let mut args = json!({ "points": [[0, 8], [23, 8]], "size": 12 });
        if tool == "draw.recolor" {
            args["from"] = json!("#ff0000");
            args["color"] = json!("#00ff00");
        }
        s.ok(tool, args);
        let d = s.image(&id).document();
        let sel = d.selection.clone().expect("selection");
        let after = &d.layers()[0].pixels;
        let mut changed_inside = false;
        for y in 0..16 {
            for x in 0..24 {
                let (a, b) = (after.get_pixel(x, y), before.get_pixel(x, y));
                if sel.coverage(x, y) == 0 {
                    assert_eq!(a, b, "{tool} changed ({x}, {y}) outside");
                } else if a != b {
                    changed_inside = true;
                }
            }
        }
        assert!(changed_inside, "{tool}");
    }
}

/// The painted document's layer 0 before and after `tool` runs with the
/// rectangle x 8–11 selected and feathered by 2 px, or (`hard`) with every
/// pixel that feathered selection touches fully selected; plus the
/// feathered coverage per pixel.
fn brushed_in_selection(tool: &str, hard: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut s = S::new();
    let id = painted(&mut s);
    let before = layers(&s, &id)[0].pixels.data().to_vec();
    s.ok(
        "select.rect",
        json!({ "x": 8, "y": 0, "width": 4, "height": 16, "feather": 2 }),
    );
    let soft = s
        .image(&id)
        .document()
        .selection
        .clone()
        .expect("selection");
    let cov = soft.data().to_vec();
    if hard {
        let b = soft.bounding_box().expect("bounds");
        s.ok(
            "select.rect",
            json!({ "x": b.x, "y": b.y, "width": b.w, "height": b.h }),
        );
        let mask = s.image(&id).document().selection.clone().expect("mask");
        let full: Vec<u8> = cov.iter().map(|&c| if c > 0 { 255 } else { 0 }).collect();
        assert_eq!(mask.data(), full.as_slice(), "hard mask");
    }
    let mut args = json!({ "points": [[0, 8], [23, 8]], "size": 12 });
    if tool == "draw.recolor" {
        args["from"] = json!("#ff0000");
        args["color"] = json!("#00ff00");
    }
    s.ok(tool, args);
    let after = layers(&s, &id)[0].pixels.data().to_vec();
    (cov, before, after)
}

#[test]
fn recolor_and_smudge_blend_feathered_selection_edges_by_coverage() {
    for tool in ["draw.recolor", "draw.smudge"] {
        let (cov, before, soft) = brushed_in_selection(tool, false);
        let (_, _, hard) = brushed_in_selection(tool, true);
        assert!(cov.iter().any(|&c| c > 0 && c < 255), "feathered edge");
        let mut blended = false;
        for (i, &c) in cov.iter().enumerate() {
            let px = |b: &[u8]| b[i * 4..i * 4 + 4].to_vec();
            match c {
                0 => assert_eq!(px(&soft), px(&before), "{tool} changed {i} outside"),
                255 if tool == "draw.recolor" => {
                    assert_eq!(px(&soft), px(&hard), "{tool} fully selected pixel {i}")
                }
                255 => {}
                _ => blended |= px(&soft) != px(&hard) && px(&soft) != px(&before),
            }
        }
        assert!(
            blended,
            "{tool}: no feathered pixel was blended by its coverage"
        );
    }
}

#[test]
fn recolor_and_smudge_refuse_empty_points_locked_and_group_targets() {
    let mut s = S::new();
    let id = painted(&mut s);
    s.ok(
        "layer.set",
        json!({ "layer": 0, "name": "Base", "locked": true }),
    );
    let group = s.ok("layer.add", json!({ "kind": "group", "name": "Box" }))["layer"]
        .as_u64()
        .expect("group id");
    let inner = s.ok("layer.add", json!({ "name": "Inner", "parent": group }))["layer"]
        .as_u64()
        .expect("layer id");
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    for tool in ["draw.recolor", "draw.smudge"] {
        for (args, needle) in [
            (
                json!({ "points": [], "from": "#ff0000" }),
                "points".to_owned(),
            ),
            (
                json!({ "points": [[1, 1]], "from": "#ff0000", "layer": 0 }),
                "layer 0 (\"Base\") is locked".to_owned(),
            ),
            (
                json!({ "points": [[1, 1]], "from": "#ff0000", "layer": inner }),
                format!("layer {inner} (\"Inner\") is locked"),
            ),
            (
                json!({ "points": [[1, 1]], "from": "#ff0000", "layer": group }),
                format!("layer {group} (\"Box\") is a group"),
            ),
        ] {
            let mut a = args.clone();
            if tool == "draw.smudge" {
                a.as_object_mut().expect("obj").remove("from");
            }
            let before = layers(&s, &id);
            let e = s.err(tool, a);
            assert!(e.contains(&needle), "{tool} {args}: {e}");
            assert_eq!(layers(&s, &id), before);
        }
    }
}

/// The Image editor's effect: run on the layer, commit the pixels.
fn editor_effect(s: &S, id: &str, name: &str, params: &[f64]) -> Vec<u8> {
    let doc = s.image(id).document();
    let spec = EffectSpec::from_params(name, params).expect("spec");
    let before = &doc.layers()[0].pixels;
    let src = EffectImage::from_rgba8(before.width(), before.height(), before.data().to_vec())
        .expect("image");
    spec.run_in(&src, |_, _| true).into_raw()
}

#[test]
fn image_effect_defaults_follow_the_image_editor() {
    let mut s = S::new();
    let id = painted(&mut s);
    let want = editor_effect(&s, &id, "pixelate", &[8.0, 0.0]);
    s.ok("effect.apply", json!({ "id": "pixelate" }));
    assert_eq!(
        s.image(&id).document().layers()[0].pixels.data(),
        want.as_slice()
    );
    let mut s = S::new();
    let id = painted(&mut s);
    let want = editor_effect(
        &s,
        &id,
        "drop_shadow",
        &[4.0, 4.0, 4.0, f64::from(0x0000_00FF_u32), 50.0],
    );
    s.ok("effect.apply", json!({ "id": "drop_shadow" }));
    assert_eq!(
        s.image(&id).document().layers()[0].pixels.data(),
        want.as_slice()
    );
    // A given value wins over the mode default, and only the omitted
    // parameters take Image defaults.
    for (effect, params, wire) in [
        ("pixelate", json!({ "color": "centre" }), vec![8.0, 1.0]),
        (
            "drop_shadow",
            json!({ "opacity": 80 }),
            vec![4.0, 4.0, 4.0, f64::from(0x0000_00FF_u32), 80.0],
        ),
        (
            "drop_shadow",
            json!({ "blur": 1 }),
            vec![4.0, 4.0, 1.0, f64::from(0x0000_00FF_u32), 50.0],
        ),
    ] {
        let mut s = S::new();
        let id = painted(&mut s);
        let want = editor_effect(&s, &id, effect, &wire);
        s.ok("effect.apply", json!({ "id": effect, "params": params }));
        assert_eq!(
            s.image(&id).document().layers()[0].pixels.data(),
            want.as_slice(),
            "{effect} {params}"
        );
    }
}

#[test]
fn pixel_effect_defaults_stay_the_pixel_editors() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ff0000", "filled": true }),
    );
    let mut t = S::new();
    let tid = t.new_doc(8, 8);
    t.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ff0000", "filled": true }),
    );
    s.ok("effect.apply", json!({ "id": "pixelate" }));
    t.ok(
        "effect.apply",
        json!({ "id": "pixelate", "params": { "color": "centre" } }),
    );
    assert_eq!(s.cel_rgba(&id, 0, 0), t.cel_rgba(&tid, 0, 0));
    s.ok("effect.apply", json!({ "id": "drop_shadow" }));
    t.ok(
        "effect.apply",
        json!({ "id": "drop_shadow", "params": { "blur": 0 } }),
    );
    assert_eq!(s.cel_rgba(&id, 0, 0), t.cel_rgba(&tid, 0, 0));
}

#[test]
fn effects_resource_lists_per_mode_defaults() {
    let text = crate::read_resource("pincel://effects").expect("effects");
    let v: Value = serde_json::from_str(&text).expect("json");
    let find = |id: &str, key: &str| -> Value {
        v["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|e| e["id"] == id)
            .and_then(|e| e["params"].as_array())
            .and_then(|ps| ps.iter().find(|p| p["name"] == key).cloned())
            .expect("param")
    };
    let color = find("pixelate", "color");
    assert_eq!(
        color["defaults"],
        json!({ "pixel": "centre", "image": "average" })
    );
    assert!(color.get("default").is_none());
    let blur = find("drop_shadow", "blur");
    assert_eq!(blur["defaults"], json!({ "pixel": 0.0, "image": 4.0 }));
    assert!(blur.get("default").is_none());
    // Every other parameter keeps exactly one `default`.
    for e in v["effects"].as_array().expect("effects") {
        for p in e["params"].as_array().into_iter().flatten() {
            let per_mode = [("pixelate", "color"), ("drop_shadow", "blur")].contains(&(
                e["id"].as_str().unwrap_or(""),
                p["name"].as_str().unwrap_or(""),
            ));
            assert_eq!(
                p.get("defaults").is_some(),
                per_mode,
                "{} {}",
                e["id"],
                p["name"]
            );
            assert_eq!(
                p.get("default").is_some(),
                !per_mode,
                "{} {}",
                e["id"],
                p["name"]
            );
        }
    }
    let size = find("pixelate", "cell_size");
    assert_eq!(size["default"], 8.0);
    assert!(size.get("defaults").is_none());
}

#[test]
fn image_brush_calls_are_one_undo_step_and_none_without_a_change() {
    let mut s = S::new();
    painted(&mut s);
    for (tool, args) in [
        (
            "draw.recolor",
            json!({ "points": [[2, 8], [20, 8]], "from": "#ff0000" }),
        ),
        (
            "draw.recolor",
            json!({ "points": [[2, 6], [20, 6]], "from": "#0000ff" }),
        ),
        ("draw.smudge", json!({ "points": [[2, 8], [20, 8]] })),
        ("draw.smudge", json!({ "points": [[2, 9], [20, 9]] })),
        (
            "draw.stroke",
            json!({ "points": [[2, 2], [20, 2]], "tool": "eraser", "eraser_mode": "to_background" }),
        ),
    ] {
        let depth = undo_depth(&mut s);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool} {args}");
    }
    let depth = undo_depth(&mut s);
    let out = s.ok(
        "draw.recolor",
        json!({ "points": [[2, 8], [20, 8]], "from": "#123456", "tolerance": 0 }),
    );
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth);
}
