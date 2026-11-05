//! `select.*` in both modes (mcp-parity-selection-transform AC-01 …
//! AC-23, AC-34, AC-35).

use pincel_core::{
    CelData, ColorMode, FrameIndex, LayerId, Rect, SelectionMask, SelectionMode, Sprite,
    composite_canvas, layer_canvas,
};
use pincel_image_core::SelectionMask as ImMask;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::{Value, json};

use super::{S, px};
use crate::AgentError;

/// The Image selection after the editor runs `spec` on a copy.
fn image_editor(s: &S, id: &str, spec: &str) -> Option<ImMask> {
    let spec: CommandSpec = serde_json::from_str(spec).expect("spec");
    let mut bus = CommandBus::new(s.image(id).document().clone());
    run_command(&mut bus, spec, &mut StrokeScratch::default(), &[]).expect("editor");
    bus.document.selection
}

fn image_sel(s: &S, id: &str) -> Option<ImMask> {
    s.image(id).document().selection.clone()
}

fn pixel_sel(s: &S, id: &str) -> (Option<Rect>, Option<SelectionMask>) {
    let sp = &s.doc(id).sprite;
    (sp.selection, sp.selection_mask.clone())
}

fn sprite_sel(sp: &Sprite) -> (Option<Rect>, Option<SelectionMask>) {
    (sp.selection, sp.selection_mask.clone())
}

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// A Pixel document with a few colored pixels on layer 0.
fn pixel_doc(s: &mut S) -> String {
    let id = s.new_doc(8, 6);
    s.ok(
        "draw.rect",
        json!({ "x0": 1, "y0": 1, "x1": 4, "y1": 3, "color": "#ff0000", "filled": true }),
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 6, "y": 4, "color": "#ff0000" }, { "x": 2, "y": 2, "color": "#fe0000" }] }),
    );
    id
}

/// An Image document with a painted layer.
fn image_doc(s: &mut S) -> String {
    let id = s.new_image(10, 8);
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 1, "y0": 1, "x1": 6, "y1": 5, "fill": "#3366cc", "anti_alias": false }),
    );
    id
}

#[test]
fn selection_set_rect_and_selection_clear_are_unknown_tools() {
    let mut s = S::new();
    s.new_doc(4, 4);
    for name in ["selection.set_rect", "selection.clear"] {
        let r = s.call(name, json!({}));
        assert!(matches!(r, Err(AgentError::UnknownTool(_))), "{name}");
    }
}

#[test]
fn select_shapes_combine_like_the_editor_in_both_modes() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let (w, h) = (8, 6);
    let mut editor = s.doc(&id).sprite.clone();
    let steps: Vec<(&str, Value, SelectionMask, SelectionMode)> = vec![
        (
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 4, "height": 3 }),
            SelectionMask::rect(w, h, Rect::new(1, 1, 4, 3)),
            SelectionMode::Replace,
        ),
        (
            "select.ellipse",
            json!({ "x": 3, "y": 0, "width": 5, "height": 6, "mode": "add" }),
            SelectionMask::ellipse(w, h, Rect::new(3, 0, 5, 6)),
            SelectionMode::Add,
        ),
        (
            "select.polygon",
            json!({ "points": [[0, 0], [5, 0], [0, 5]], "mode": "subtract" }),
            SelectionMask::polygon(w, h, &[(0, 0), (5, 0), (0, 5)]),
            SelectionMode::Subtract,
        ),
        (
            "select.rect",
            json!({ "x": 2, "y": 2, "width": 6, "height": 4, "mode": "intersect" }),
            SelectionMask::rect(w, h, Rect::new(2, 2, 6, 4)),
            SelectionMode::Intersect,
        ),
    ];
    for (tool, args, shape, mode) in steps {
        editor.combine_selection(shape, mode);
        s.ok(tool, args.clone());
        assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor), "{tool} {args}");
    }
    // A 4×4 ellipse on a 4×4 canvas leaves the four corners out.
    let mut s4 = S::new();
    s4.new_doc(4, 4);
    let out = s4.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 4, "height": 4 }),
    );
    assert_eq!(out["count"], 12);

    let mut s = S::new();
    let id = image_doc(&mut s);
    for (tool, args, spec) in [
        (
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 4, "height": 3 }),
            r#"{"type":"select_rectangle","x":1,"y":1,"w":4,"h":3,"mode":"replace","feather":0}"#,
        ),
        (
            "select.ellipse",
            json!({ "x": 3, "y": 0, "width": 6, "height": 7, "mode": "add" }),
            r#"{"type":"select_ellipse","x":3,"y":0,"w":6,"h":7,"mode":"add","feather":0}"#,
        ),
        (
            "select.polygon",
            json!({ "points": [[0, 0], [5.5, 0], [0, 5.5]], "mode": "subtract" }),
            r#"{"type":"select_polygon","points":[[0,0],[5.5,0],[0,5.5]],"mode":"subtract","feather":0}"#,
        ),
        (
            "select.rect",
            json!({ "x": 2, "y": 2, "width": 7, "height": 5, "mode": "intersect" }),
            r#"{"type":"select_rectangle","x":2,"y":2,"w":7,"h":5,"mode":"intersect","feather":0}"#,
        ),
    ] {
        let want = image_editor(&s, &id, spec);
        s.ok(tool, args.clone());
        assert_eq!(image_sel(&s, &id), want, "{tool} {args}");
    }
}

#[test]
fn select_wand_matches_the_editor_with_each_modes_defaults() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    let d = s.doc(&id);
    let mut editor = d.sprite.clone();
    let frame = FrameIndex::new(0);
    // Default: tolerance 0, contiguous, the target layer.
    let buf = layer_canvas(&d.sprite, &d.cels, LayerId::new(0), frame);
    editor.combine_selection(
        SelectionMask::wand(&buf, (1, 1), 0, true),
        SelectionMode::Replace,
    );
    s.ok("select.wand", json!({ "x": 1, "y": 1, "layer": 0 }));
    assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor));
    assert!(!s.doc(&id).sprite.selection_contains(2, 2), "tolerance 0");
    let d = s.doc(&id);
    let buf = composite_canvas(&d.sprite, &d.cels, frame).expect("compose");
    editor.combine_selection(
        SelectionMask::wand(&buf, (0, 0), 4, false),
        SelectionMode::Add,
    );
    s.ok(
        "select.wand",
        json!({ "x": 0, "y": 0, "tolerance": 4, "contiguous": false, "sample": "all_layers", "mode": "add" }),
    );
    assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor));
    assert!(s.doc(&id).sprite.selection_contains(6, 4));
    assert!(s.doc(&id).sprite.selection_contains(2, 2));

    let mut s = S::new();
    let id = image_doc(&mut s);
    let want = image_editor(
        &s,
        &id,
        r#"{"type":"select_wand","layer":0,"x":2.5,"y":2.5,"tolerance":32,"contiguous":true,"sample":"current_layer","mode":"replace"}"#,
    );
    s.ok("select.wand", json!({ "x": 2.5, "y": 2.5 }));
    assert_eq!(image_sel(&s, &id), want);
    let want = image_editor(
        &s,
        &id,
        r#"{"type":"select_wand","layer":0,"x":0,"y":0,"tolerance":5,"contiguous":false,"sample":"all_layers","mode":"add"}"#,
    );
    s.ok(
        "select.wand",
        json!({ "x": 0, "y": 0, "tolerance": 5, "contiguous": false, "sample": "all_layers", "mode": "add" }),
    );
    assert_eq!(image_sel(&s, &id), want);
}

#[test]
fn pixel_subtract_or_intersect_without_selection_selects_nothing() {
    let mut s = S::new();
    let id = s.new_doc(6, 6);
    for mode in ["subtract", "intersect"] {
        let out = s.ok(
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 2, "height": 2, "mode": mode }),
        );
        assert_eq!(out["selection"], Value::Null, "{mode}");
        assert!(!s.doc(&id).sprite.has_selection());
    }
    let out = s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2, "mode": "add" }),
    );
    assert_eq!(out["selection"], json!({ "x": 1, "y": 1, "w": 2, "h": 2 }));
}

#[test]
fn image_feather_matches_the_editors_coverage() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    for (tool, args, spec) in [
        (
            "select.rect",
            json!({ "x": 2, "y": 2, "width": 5, "height": 4, "feather": 2 }),
            r#"{"type":"select_rectangle","x":2,"y":2,"w":5,"h":4,"mode":"replace","feather":2}"#,
        ),
        (
            "select.ellipse",
            json!({ "x": 0, "y": 0, "width": 6, "height": 6, "feather": 3, "mode": "add" }),
            r#"{"type":"select_ellipse","x":0,"y":0,"w":6,"h":6,"mode":"add","feather":3}"#,
        ),
        (
            "select.polygon",
            json!({ "points": [[1, 1], [9, 2], [4, 7]], "feather": 1 }),
            r#"{"type":"select_polygon","points":[[1,1],[9,2],[4,7]],"mode":"replace","feather":1}"#,
        ),
    ] {
        let want = image_editor(&s, &id, spec);
        s.ok(tool, args);
        assert_eq!(image_sel(&s, &id), want, "{tool}");
    }
    let sel = image_sel(&s, &id).expect("selection");
    let data: Vec<u8> = (0..8)
        .flat_map(|y| (0..10).map(move |x| (x, y)))
        .map(|(x, y)| sel.coverage(x, y))
        .collect();
    assert!(data.iter().any(|&c| c > 0 && c < 255), "feathered edge");
}

#[test]
fn select_all_invert_expand_contract_match_the_editor() {
    // Pixel.
    let mut s = S::new();
    let id = s.new_doc(10, 10);
    // Without a selection: invert selects everything; expand / contract
    // change nothing.
    s.ok("select.expand", json!({ "radius": 2 }));
    s.ok("select.contract", json!({ "radius": 2 }));
    assert!(!s.doc(&id).sprite.has_selection());
    let out = s.ok("select.invert", json!({}));
    assert_eq!(out["count"], 100);
    s.ok(
        "select.rect",
        json!({ "x": 3, "y": 3, "width": 2, "height": 2 }),
    );
    let mut editor = s.doc(&id).sprite.clone();
    let grown = editor.selection_as_mask().expect("mask").expand(2);
    editor.set_selection_mask(grown);
    s.ok("select.expand", json!({ "radius": 2 }));
    assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor));
    let shrunk = editor.selection_as_mask().expect("mask").contract(1);
    editor.set_selection_mask(shrunk);
    s.ok("select.contract", json!({ "radius": 1 }));
    assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor));
    let mut inv = editor.selection_as_mask().expect("mask");
    inv.invert();
    editor.set_selection_mask(inv);
    s.ok("select.invert", json!({}));
    assert_eq!(pixel_sel(&s, &id), sprite_sel(&editor));
    let out = s.ok("select.all", json!({}));
    assert_eq!(
        out["selection"],
        json!({ "x": 0, "y": 0, "w": 10, "h": 10 })
    );
    assert_eq!(s.ok("select.expand", json!({ "radius": 50 }))["count"], 100);
    assert_eq!(
        s.ok("select.contract", json!({ "radius": 50 }))["selection"],
        Value::Null
    );

    // Image.
    let mut s = S::new();
    let id = image_doc(&mut s);
    let out = s.ok("select.invert", json!({}));
    assert_eq!(
        out["count"], 0,
        "Image: invert of no selection selects nothing"
    );
    s.ok("select.clear", json!({}));
    s.ok(
        "select.rect",
        json!({ "x": 3, "y": 3, "width": 2, "height": 2 }),
    );
    for (tool, args, spec) in [
        (
            "select.expand",
            json!({ "radius": 2 }),
            r#"{"type":"expand_selection","radius":2}"#,
        ),
        (
            "select.contract",
            json!({ "radius": 1 }),
            r#"{"type":"contract_selection","radius":1}"#,
        ),
        ("select.invert", json!({}), r#"{"type":"invert_selection"}"#),
        ("select.all", json!({}), r#"{"type":"select_all"}"#),
    ] {
        let want = image_editor(&s, &id, spec);
        s.ok(tool, args);
        assert_eq!(image_sel(&s, &id), want, "{tool}");
    }
    s.ok("select.clear", json!({}));
    s.ok("select.expand", json!({ "radius": 3 }));
    assert_eq!(image_sel(&s, &id), None);
}

#[test]
fn select_feather_matches_the_editor_on_image_documents() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 2, "width": 5, "height": 4 }),
    );
    let want = image_editor(&s, &id, r#"{"type":"feather_selection","radius":2}"#);
    s.ok("select.feather", json!({ "radius": 2 }));
    assert_eq!(image_sel(&s, &id), want);
}

#[test]
fn select_clear_lets_edits_act_on_the_whole_layer() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(4, 4);
        } else {
            s.new_doc(4, 4);
        }
        s.ok(
            "select.rect",
            json!({ "x": 0, "y": 0, "width": 1, "height": 1 }),
        );
        let out = s.ok("select.clear", json!({}));
        assert_eq!(out["selection"], Value::Null);
        assert_eq!(out["count"], 0);
        s.ok("draw.fill", json!({ "x": 2, "y": 2, "color": "#00ff00" }));
        let rows = s.ok(
            "doc.read_pixels",
            json!({ "region": { "x": 0, "y": 0, "width": 4, "height": 4 } }),
        )["rows"]
            .clone();
        assert!(
            rows.as_array().expect("rows").iter().all(|r| r
                .as_str()
                .expect("row")
                .split(' ')
                .all(|p| p == "#00FF00FF")),
            "image={image}: {rows}"
        );
    }
}

#[test]
fn selection_undo_follows_each_editor() {
    // Image: a change is one step; an unchanged selection adds none.
    let mut s = S::new();
    s.new_image(6, 6);
    let depth = undo_depth(&mut s);
    let out = s.ok("select.all", json!({}));
    assert_eq!(out["selection"], json!({ "x": 0, "y": 0, "w": 6, "h": 6 }));
    assert_eq!(out["count"], 36);
    assert_eq!(undo_depth(&mut s), depth + 1);
    s.ok("select.all", json!({}));
    assert_eq!(undo_depth(&mut s), depth + 1, "second select.all");
    s.ok("select.clear", json!({}));
    assert_eq!(undo_depth(&mut s), depth + 2);
    s.ok("select.clear", json!({}));
    assert_eq!(undo_depth(&mut s), depth + 2, "clear without selection");
    s.ok("doc.undo", json!({}));
    assert_eq!(s.ok("doc.info", json!({}))["selection"]["w"], 6);

    // Pixel: never a step, and undo leaves the selection alone.
    let mut s = S::new();
    let id = s.new_doc(6, 6);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ffffff" }] }),
    );
    let depth = undo_depth(&mut s);
    for (tool, args) in [
        (
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
        ),
        (
            "select.ellipse",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "mode": "add" }),
        ),
        (
            "select.wand",
            json!({ "x": 3, "y": 3, "mode": "intersect" }),
        ),
        ("select.invert", json!({})),
        ("select.expand", json!({ "radius": 1 })),
        ("select.contract", json!({ "radius": 1 })),
        ("select.all", json!({})),
    ] {
        let out = s.ok(tool, args);
        assert!(out["count"].is_u64(), "{tool}");
        assert_eq!(undo_depth(&mut s), depth, "{tool}");
    }
    let before = pixel_sel(&s, &id);
    s.ok("doc.undo", json!({}));
    assert_eq!(pixel_sel(&s, &id), before);
    // A selection change alone leaves a Pixel document clean.
    let mut s = S::new();
    s.new_doc(4, 4);
    s.ok("select.all", json!({}));
    assert_eq!(s.ok("doc.info", json!({}))["dirty"], false);
}

#[test]
fn shaped_selections_limit_fill_adjust_and_effect_to_the_shape() {
    let mut s = S::new();
    let id = s.new_doc(6, 6);
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 6, "height": 6 }),
    );
    s.ok("draw.fill", json!({ "x": 3, "y": 3, "color": "#ff0000" }));
    let rgba = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&rgba, 6, 3, 3), [255, 0, 0, 255]);
    assert_eq!(
        px(&rgba, 6, 0, 0),
        [0, 0, 0, 0],
        "corner outside the ellipse"
    );
    s.ok("select.all", json!({}));
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 5, "color": "#0000ff", "filled": true }),
    );
    s.ok(
        "select.polygon",
        json!({ "points": [[0, 0], [5, 0], [0, 5]] }),
    );
    s.ok("effect.apply", json!({ "id": "invert" }));
    let rgba = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&rgba, 6, 5, 5), [0, 0, 255, 255], "outside the triangle");
    assert_ne!(px(&rgba, 6, 0, 0), [0, 0, 255, 255]);

    let mut s = S::new();
    let id = s.new_image(6, 6);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#0000ff" }));
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 6, "height": 6 }),
    );
    s.ok("adjust.apply", json!({ "id": "invert" }));
    let px_at = |s: &S, x, y| {
        s.image(&id).document().layers()[0]
            .pixels
            .get_pixel(x, y)
            .expect("pixel")
    };
    assert_eq!(px_at(&s, 0, 0).b, 255, "corner outside the ellipse");
    assert_ne!(px_at(&s, 3, 3).b, 255);
    s.ok("effect.apply", json!({ "id": "gaussian_blur" }));
    assert_eq!(px_at(&s, 0, 0).b, 255);
}

#[test]
fn feather_on_a_pixel_document_fails_naming_feather_and_the_mode() {
    let mut s = S::new();
    let id = s.new_doc(6, 6);
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
    );
    for (tool, args) in [
        ("select.feather", json!({ "radius": 2 })),
        (
            "select.rect",
            json!({ "x": 0, "y": 0, "width": 4, "height": 4, "feather": 2 }),
        ),
        (
            "select.ellipse",
            json!({ "x": 0, "y": 0, "width": 4, "height": 4, "feather": 1 }),
        ),
        (
            "select.polygon",
            json!({ "points": [[0, 0], [4, 0], [0, 4]], "feather": 3 }),
        ),
    ] {
        let before = pixel_sel(&s, &id);
        let e = s.err(tool, args);
        assert!(
            e.contains("feather") && e.contains("Pixel document"),
            "{tool}: {e}"
        );
        assert_eq!(pixel_sel(&s, &id), before, "{tool}");
    }
}

#[test]
fn select_polygon_with_bad_points_fails_naming_the_entry() {
    for image in [false, true] {
        let mut s = S::new();
        let id = if image {
            s.new_image(6, 6)
        } else {
            s.new_doc(6, 6)
        };
        let mut cases = vec![
            (json!({ "points": [[0, 0], [1, 1]] }), "points"),
            (json!({ "points": [[0, 0], [1, 1], [2]] }), "points[2]"),
            (json!({ "points": [[0, 0], ["a", 1], [2, 2]] }), "points[1]"),
        ];
        if !image {
            cases.push((json!({ "points": [[0, 0], [1.5, 1], [2, 2]] }), "points[1]"));
        }
        for (args, needle) in cases {
            let e = s.err("select.polygon", args.clone());
            assert!(e.contains(needle), "image={image} {args}: {e}");
            if image {
                assert_eq!(image_sel(&s, &id), None);
            } else {
                assert_eq!(pixel_sel(&s, &id), (None, None));
            }
        }
    }
}

#[test]
fn select_wand_seed_outside_the_canvas_fails_naming_seed_and_canvas() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(6, 4);
        } else {
            s.new_doc(6, 4);
        }
        let e = s.err("select.wand", json!({ "x": 6, "y": 0 }));
        assert!(e.contains("seed") && e.contains("6×4"), "{e}");
        assert_eq!(s.ok("doc.info", json!({}))["selection"], Value::Null);
    }
}

#[test]
fn bad_selection_arguments_fail_naming_the_argument() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(6, 6);
        } else {
            s.new_doc(6, 6);
        }
        let mut cases = vec![
            (
                "select.rect",
                json!({ "x": 0, "y": 0, "width": 2, "height": 2, "mode": "xor" }),
                "mode",
            ),
            (
                "select.wand",
                json!({ "x": 0, "y": 0, "sample": "screen" }),
                "sample",
            ),
            (
                "select.wand",
                json!({ "x": 0, "y": 0, "tolerance": 256 }),
                "tolerance",
            ),
            ("select.expand", json!({ "radius": 0 }), "radius"),
            ("select.contract", json!({ "radius": 251 }), "radius"),
            (
                "select.rect",
                json!({ "x": 0, "y": 0, "width": 0, "height": 2 }),
                "width",
            ),
            (
                "select.rect",
                json!({ "x": 0.5, "y": 0, "width": 2, "height": 2 }),
                "x",
            ),
            (
                "select.ellipse",
                json!({ "x": 0, "y": 0, "width": 2, "height": 2.5 }),
                "height",
            ),
        ];
        if image {
            cases.push((
                "select.rect",
                json!({ "x": 0, "y": 0, "width": 2, "height": 2, "feather": 251 }),
                "feather",
            ));
            cases.push(("select.feather", json!({ "radius": 300 }), "radius"));
        } else {
            cases.push(("select.wand", json!({ "x": 0.5, "y": 0 }), "x"));
        }
        for (tool, args, arg) in cases {
            let before = s.ok("doc.info", json!({}));
            let e = s.err(tool, args.clone());
            assert!(e.contains(arg), "image={image} {tool} {args}: {e}");
            assert_eq!(s.ok("doc.info", json!({})), before);
        }
    }
}

#[test]
fn select_get_reports_bounds_count_and_coverage_rows() {
    let mut s = S::new();
    s.new_doc(6, 5);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 2 }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 1, "width": 1, "height": 1, "mode": "subtract" }),
    );
    let out = s.ok(
        "select.get",
        json!({ "region": { "x": 0, "y": 0, "width": 5, "height": 3 } }),
    );
    assert_eq!(out["selection"], json!({ "x": 1, "y": 1, "w": 3, "h": 2 }));
    assert_eq!(out["count"], 5);
    assert_eq!(
        out["region"],
        json!({ "x": 0, "y": 0, "width": 5, "height": 3 })
    );
    assert_eq!(
        out["rows"],
        json!(["00 00 00 00 00", "00 ff 00 ff 00", "00 ff ff ff 00"])
    );
    // Without a region: the selection's bounds.
    let out = s.ok("select.get", json!({}));
    assert_eq!(
        out["region"],
        json!({ "x": 1, "y": 1, "width": 3, "height": 2 })
    );
    assert_eq!(out["rows"], json!(["ff 00 ff", "ff ff ff"]));

    let mut s = S::new();
    let id = s.new_image(8, 8);
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 8, "height": 8, "feather": 2 }),
    );
    let out = s.ok("select.get", json!({}));
    let sel = image_sel(&s, &id).expect("selection");
    assert_eq!(out["count"], sel.selected_count());
    let rows: Vec<String> = (0..8)
        .map(|y| {
            (0..8)
                .map(|x| format!("{:02x}", sel.coverage(x, y)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(out["rows"], json!(rows));
    assert!(
        rows.iter()
            .any(|r| r.split(' ').any(|v| v != "00" && v != "ff"))
    );
}

#[test]
fn select_get_without_selection_returns_null_bounds_and_no_rows() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(4, 4);
        } else {
            s.new_doc(4, 4);
        }
        let out = s.ok("select.get", json!({}));
        assert_eq!(out["selection"], Value::Null);
        assert_eq!(out["count"], 0);
        assert_eq!(out["rows"], json!([]));
        // A region without a selection reads all 00.
        let out = s.ok(
            "select.get",
            json!({ "region": { "x": 0, "y": 0, "width": 2, "height": 1 } }),
        );
        assert_eq!(out["rows"], json!(["00 00"]));
    }
}

#[test]
fn select_get_refuses_large_or_off_canvas_regions() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(100, 80);
        } else {
            s.new_doc(100, 80);
        }
        let e = s.err(
            "select.get",
            json!({ "region": { "x": 0, "y": 0, "width": 65, "height": 2 } }),
        );
        assert!(e.contains("64×64") && e.contains("region"), "{e}");
        let e = s.err(
            "select.get",
            json!({ "region": { "x": 90, "y": 0, "width": 20, "height": 2 } }),
        );
        assert!(e.contains("100×80"), "{e}");
        s.ok("select.all", json!({}));
        let e = s.err("select.get", json!({}));
        assert!(e.contains("64×64"), "{e}");
    }
}

#[test]
fn select_get_changes_nothing() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(6, 6);
        } else {
            s.new_doc(6, 6);
        }
        s.ok(
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
        );
        let before = s.ok("doc.info", json!({}));
        s.ok("select.get", json!({}));
        assert_eq!(s.ok("doc.info", json!({})), before);
    }
}

#[test]
fn select_delete_clears_the_selected_pixels_like_the_editor() {
    // Pixel: a hole in the selection keeps its pixels.
    let mut s = S::new();
    let id = s.new_doc(5, 5);
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 4, "y1": 4, "color": "#ff0000", "filled": true }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 3 }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 2, "width": 1, "height": 1, "mode": "subtract" }),
    );
    let depth = undo_depth(&mut s);
    let out = s.ok("select.delete", json!({}));
    assert_eq!(out, json!({ "doc": id, "layer": 0, "changed": true }));
    assert_eq!(undo_depth(&mut s), depth + 1);
    let rgba = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&rgba, 5, 1, 1), [0, 0, 0, 0]);
    assert_eq!(px(&rgba, 5, 2, 2), [255, 0, 0, 255], "hole");
    assert_eq!(px(&rgba, 5, 0, 0), [255, 0, 0, 255], "outside");
    assert!(s.doc(&id).sprite.has_selection(), "selection stays");
    // Nothing opaque left: no change, no step.
    let out = s.ok("select.delete", json!({}));
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth + 1);

    // Image: coverage-scaled, equal to the editor's DeleteSelection.
    let mut s = S::new();
    let id = s.new_image(8, 8);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#ff0000" }));
    s.ok(
        "select.ellipse",
        json!({ "x": 1, "y": 1, "width": 6, "height": 6, "feather": 2 }),
    );
    let mut bus = CommandBus::new(s.image(&id).document().clone());
    run_command(
        &mut bus,
        CommandSpec::DeleteSelection { layer: 0 },
        &mut StrokeScratch::default(),
        &[],
    )
    .expect("editor");
    let depth = undo_depth(&mut s);
    let out = s.ok("select.delete", json!({}));
    assert_eq!(out["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(s.image(&id).document().layers(), bus.document.layers());
    assert!(s.image(&id).document().selection.is_some());
}

#[test]
fn select_delete_without_selection_follows_each_editor() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ffffff" }] }),
    );
    let depth = undo_depth(&mut s);
    let out = s.ok("select.delete", json!({}));
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth);
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 4, 1, 1), [255, 255, 255, 255]);

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" }));
    let depth = undo_depth(&mut s);
    assert_eq!(s.ok("select.delete", json!({}))["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    let layer = &s.image(&id).document().layers()[0].pixels;
    assert!(layer.data().iter().all(|&b| b == 0));
    assert_eq!(s.ok("select.delete", json!({}))["changed"], false);
    assert_eq!(undo_depth(&mut s), depth + 1);
}

#[test]
fn select_delete_on_a_locked_group_or_tilemap_layer_fails_with_the_reason() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok("select.all", json!({}));
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(pincel_core::Layer::tilemap(
            LayerId::new(7),
            "Map",
            pincel_core::TilesetId::new(0),
        ));
    for (layer, reason) in [(0, "locked"), (1, "a group"), (7, "a tilemap layer")] {
        let before = s.state(&id);
        let e = s.err("select.delete", json!({ "layer": layer }));
        assert!(
            e.contains(reason) && e.contains(&format!("layer {layer}")),
            "{e}"
        );
        assert_eq!(s.state(&id), before);
    }

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    for (layer, reason) in [(0, "locked"), (1, "group")] {
        let before = s.image(&id).document().clone();
        let e = s.err("select.delete", json!({ "layer": layer }));
        assert!(e.contains(reason), "{e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn selection_tools_naming_a_missing_layer_or_frame_fail_with_argument_and_value() {
    let mut s = S::new();
    s.new_doc(4, 4);
    for (tool, args, needle) in [
        (
            "select.wand",
            json!({ "x": 0, "y": 0, "layer": 9 }),
            "`layer`",
        ),
        (
            "select.wand",
            json!({ "x": 0, "y": 0, "frame": 9 }),
            "frame 9",
        ),
        ("select.delete", json!({ "layer": 9 }), "`layer`"),
        ("select.delete", json!({ "frame": 9 }), "frame 9"),
    ] {
        let e = s.err(tool, args);
        assert!(e.contains(needle) && e.contains('9'), "{tool}: {e}");
    }
    let mut s = S::new();
    s.new_image(4, 4);
    for (tool, args) in [
        ("select.wand", json!({ "x": 0, "y": 0, "layer": 9 })),
        ("select.delete", json!({ "layer": 9 })),
    ] {
        let e = s.err(tool, args);
        assert!(e.contains("`layer`") && e.contains('9'), "{tool}: {e}");
    }
}

#[test]
fn a_refusing_core_command_fails_with_its_reason() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ffffff" }] }),
    );
    s.ok("select.all", json!({}));
    {
        let d = s.ws.get_mut(&id).expect("doc");
        let cel = d
            .cels
            .get_mut(LayerId::new(0), FrameIndex::new(0))
            .expect("cel");
        if let CelData::Image(buf) = &mut cel.data {
            buf.color_mode = ColorMode::Indexed {
                transparent_index: 0,
            };
        }
    }
    let before = s.state(&id);
    let e = s.err("select.delete", json!({}));
    assert!(e.contains("color mode"), "{e}");
    assert_eq!(s.state(&id), before);
}
