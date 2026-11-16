//! `draw.strokes` (a batch of `draw.stroke` strokes as one undo step) and
//! per-point `pressure` on Image strokes (docs/specs/agent-painting.md).

use serde_json::{Value, json};

use super::S;
use super::image_brushes::editor;
use crate::list_tools;

/// Every layer's pixels (layer ids differ between workspaces).
fn layers(s: &S, id: &str) -> Vec<Vec<u8>> {
    pixels(&super::image_brushes::layers(s, id))
}

fn pixels(layers: &[pincel_image_core::Layer]) -> Vec<Vec<u8>> {
    layers.iter().map(|l| l.pixels.data().to_vec()).collect()
}

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// Image strokes covering every brush variant `draw.stroke` takes.
fn image_items() -> Vec<Value> {
    vec![
        json!({ "points": [[2, 3], [28, 5], [30, 20]], "color": "#30346a", "size": 9 }),
        json!({ "points": [[4, 18], [26, 10]], "color": "#ff000080", "size": 6, "shape": "soft_round", "hardness": 40, "opacity": 70 }),
        json!({ "points": [[6, 2], [20, 22]], "color": "#00ff00", "size": 7, "shape": "flat", "angle": 120 }),
        json!({ "points": [[3, 12], [29, 12]], "tool": "eraser", "size": 4 }),
        json!({ "points": [[10, 4], [10, 20]], "color": "#ffff00", "size": 5, "blend": "overwrite", "anti_alias": false }),
        json!({ "points": [[1, 1], [16, 8], [31, 23]], "color": "#123456", "size": 12, "pressure": [0.1, 1.0, 0.3], "pressure_opacity": true }),
        json!({ "points": [[24, 2], [24, 22]], "tool": "eraser", "eraser_mode": "to_background", "background": "#ff00ff", "size": 3, "pressure": [1.0, 0.0] }),
    ]
}

/// Pixel strokes covering the Pencil's options, shading, mirror and wrap.
fn pixel_items() -> Vec<Value> {
    vec![
        json!({ "points": [[1, 1], [10, 6]], "color": "#ff0000" }),
        json!({ "points": [[2, 9], [12, 9], [12, 14]], "color": { "index": 5 }, "size": 2, "shape": "round", "pixel_perfect": true }),
        json!({ "points": [[0, 3], [7, 3]], "ink": "shading" }),
        json!({ "points": [[3, 12], [5, 15]], "color": "#00ff00", "mirror": "x" }),
        json!({ "points": [[13, 1], [18, 4]], "color": "#0000ff", "wrap": "xy", "size": 3 }),
        json!({ "points": [[4, 4], [9, 9]], "tool": "eraser" }),
    ]
}

/// A 32×24 Image document with a filled background layer.
fn image_doc(s: &mut S) -> String {
    let id = s.new_image(32, 24);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#808080" }));
    id
}

#[test]
fn strokes_is_listed_with_draw_strokes_item_arguments() {
    let tools = list_tools();
    let stroke = tools
        .iter()
        .find(|t| t.name == "draw.stroke")
        .expect("stroke");
    let batch = tools
        .iter()
        .find(|t| t.name == "draw.strokes")
        .expect("draw.strokes");
    let schema = &batch.input_schema;
    assert_eq!(schema["required"], json!(["strokes"]));
    for key in ["doc", "layer", "frame"] {
        assert!(schema["properties"].get(key).is_some(), "top-level {key}");
    }
    let strokes = &schema["properties"]["strokes"];
    assert_eq!(strokes["type"], "array");
    assert_eq!(strokes["minItems"], 1);
    assert_eq!(strokes["maxItems"], 1000);
    let item = &strokes["items"];
    assert_eq!(item["additionalProperties"], false);
    assert_eq!(item["required"], json!(["points"]));
    let mut want: Vec<&String> = stroke.input_schema["properties"]
        .as_object()
        .expect("props")
        .keys()
        .filter(|k| !["doc", "layer", "frame"].contains(&k.as_str()))
        .collect();
    let mut got: Vec<&String> = item["properties"]
        .as_object()
        .expect("props")
        .keys()
        .collect();
    want.sort();
    got.sort();
    assert_eq!(
        got, want,
        "items take draw.stroke's arguments except the target"
    );
}

#[test]
fn strokes_on_image_match_one_stroke_call_per_item() {
    let (mut a, mut b) = (S::new(), S::new());
    let (ia, ib) = (image_doc(&mut a), image_doc(&mut b));
    a.ok(
        "select.rect",
        json!({ "x": 2, "y": 2, "width": 26, "height": 18 }),
    );
    b.ok(
        "select.rect",
        json!({ "x": 2, "y": 2, "width": 26, "height": 18 }),
    );
    let out = a.ok("draw.strokes", json!({ "strokes": image_items() }));
    assert_eq!(out["changed"], true);
    for item in image_items() {
        b.ok("draw.stroke", item);
    }
    assert_eq!(layers(&a, &ia), layers(&b, &ib));
}

#[test]
fn strokes_on_pixel_match_one_stroke_call_per_item() {
    let (mut a, mut b) = (S::new(), S::new());
    let (ia, ib) = (a.new_doc(20, 16), b.new_doc(20, 16));
    a.ok("draw.strokes", json!({ "strokes": pixel_items() }));
    for item in pixel_items() {
        b.ok("draw.stroke", item);
    }
    assert_eq!(a.cel_rgba(&ia, 0, 0), b.cel_rgba(&ib, 0, 0));
    assert_ne!(
        a.cel_rgba(&ia, 0, 0),
        vec![0; 20 * 16 * 4],
        "the strokes painted"
    );
}

#[test]
fn strokes_is_one_undo_step_in_both_modes() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let before = layers(&s, &id);
    let depth = undo_depth(&mut s);
    s.ok("draw.strokes", json!({ "strokes": image_items() }));
    let after = layers(&s, &id);
    assert_eq!(undo_depth(&mut s), depth + 1);
    s.ok("doc.undo", json!({}));
    assert_eq!(layers(&s, &id), before);
    s.ok("doc.redo", json!({}));
    assert_eq!(layers(&s, &id), after);
    s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]] }] }),
    );
    assert_eq!(
        undo_depth(&mut s),
        depth + 2,
        "never merges with the call before"
    );

    let px = s.new_doc(20, 16);
    let before = s.state(&px);
    s.ok("draw.strokes", json!({ "strokes": pixel_items() }));
    assert_eq!(undo_depth(&mut s), 1);
    let after = s.state(&px);
    s.ok("doc.undo", json!({}));
    assert_eq!(s.state(&px), before);
    s.ok("doc.redo", json!({}));
    assert_eq!(s.state(&px), after);
}

#[test]
fn strokes_that_change_nothing_add_no_undo_step() {
    let mut s = S::new();
    image_doc(&mut s);
    let depth = undo_depth(&mut s);
    let out = s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[-50, -50], [-40, -45]] }, { "points": [[100, 100]] }] }),
    );
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth);
    s.new_doc(8, 8);
    let out = s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[-9, -9]] }] }),
    );
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), 0);
}

#[test]
fn strokes_result_names_doc_layer_changed_and_count() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let out = s.ok("draw.strokes", json!({ "strokes": image_items() }));
    assert_eq!(out["doc"], id.as_str());
    assert_eq!(out["layer"], 0);
    assert_eq!(out["changed"], true);
    assert_eq!(out["strokes"], 7);
    let px = s.new_doc(20, 16);
    let out = s.ok("draw.strokes", json!({ "strokes": pixel_items() }));
    assert_eq!(out["doc"], px.as_str());
    assert_eq!(out["layer"], 0);
    assert_eq!(out["frame"], 0);
    assert_eq!(out["strokes"], 6);
}

#[test]
fn strokes_over_the_limits_fail_and_change_nothing() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let before = layers(&s, &id);
    let e = s.err("draw.strokes", json!({ "strokes": [] }));
    assert!(e.contains("strokes") && e.contains("at least 1"), "{e}");
    let many = vec![json!({ "points": [[1, 1]] }); 1001];
    let e = s.err("draw.strokes", json!({ "strokes": many }));
    assert!(e.contains("strokes") && e.contains("1000"), "{e}");
    let exactly = vec![json!({ "points": [[1, 1]] }); 1000];
    s.ok("draw.strokes", json!({ "strokes": exactly }));
    s.ok("doc.undo", json!({}));
    let half: Vec<[u32; 2]> = (0..50_001).map(|i| [i % 32, 5]).collect();
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": half }, { "points": half }] }),
    );
    assert!(e.contains("100000") && e.contains("points"), "{e}");
    assert_eq!(layers(&s, &id), before);
}

#[test]
fn strokes_with_an_invalid_item_fail_naming_it_and_paint_nothing() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let before = layers(&s, &id);
    let depth = undo_depth(&mut s);
    let cases = [
        (json!({ "points": [[1, 1]], "size": 900 }), "size"),
        (json!({ "points": [] }), "points"),
        (json!({ "points": [[1, 1]], "color": "red" }), "color"),
        (json!({ "points": [[1, 1]], "ink": "shading" }), "ink"),
        (json!({ "points": [[1, 1]], "tool": "smudge" }), "tool"),
        (json!({ "points": [[1, 1]], "colour": "#ff0000" }), "colour"),
        (json!({ "points": [[1, 1]], "layer": 0 }), "layer"),
        (json!({ "points": [[1, 1]], "doc": id }), "doc"),
    ];
    for (bad, arg) in cases {
        let e = s.err(
            "draw.strokes",
            json!({ "strokes": [{ "points": [[2, 2], [20, 20]] }, bad] }),
        );
        assert!(e.contains("strokes[1]") && e.contains(arg), "{arg}: {e}");
        assert_eq!(layers(&s, &id), before, "{arg}: nothing painted");
        assert_eq!(undo_depth(&mut s), depth, "{arg}: no undo step");
    }
    s.new_doc(8, 8);
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]] }, { "points": [[1, 1]], "opacity": 50 }] }),
    );
    assert!(e.contains("strokes[1]") && e.contains("opacity"), "{e}");
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]], "size": 9 }] }),
    );
    assert!(e.contains("strokes[0]") && e.contains("size"), "{e}");
}

#[test]
fn strokes_on_a_locked_layer_or_group_fail_as_draw_stroke_does() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    let before = layers(&s, &id);
    let batch = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]] }] }),
    );
    let single = s.err("draw.stroke", json!({ "points": [[1, 1]] }));
    assert_eq!(batch, single);
    assert!(batch.contains("locked"), "{batch}");
    assert_eq!(layers(&s, &id), before);
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    let group = s.ok("layer.add", json!({ "kind": "group", "name": "G" }))["layer"].clone();
    let batch = s.err(
        "draw.strokes",
        json!({ "layer": group, "strokes": [{ "points": [[1, 1]] }] }),
    );
    let single = s.err("draw.stroke", json!({ "layer": group, "points": [[1, 1]] }));
    assert_eq!(batch, single);
    assert!(batch.contains("group"), "{batch}");
}

/// The editor's Brush spec for `points` / `pressure` with the given switches.
fn pencil_spec(points: Value, pressure: Value, size_on: bool, opacity_on: bool) -> Value {
    json!({
        "type": "pencil_stroke", "layer": 0, "size": 14, "color": [0, 0, 0, 255], "opacity": 1.0,
        "shape": "hard_round", "hardness": 1.0, "points": points, "pressure": pressure,
        "pressure_size": size_on, "pressure_opacity": opacity_on, "anti_alias": true, "stroke_id": 1,
    })
}

#[test]
fn stroke_pressure_matches_the_editor_brush_with_pressure() {
    let points = json!([[3, 12], [16, 12], [29, 12]]);
    let pressure = json!([0.1, 0.9, 0.3]);
    for (size_on, opacity_on, args) in [
        (true, false, json!({})),
        (true, true, json!({ "pressure_opacity": true })),
        (
            false,
            true,
            json!({ "pressure_size": false, "pressure_opacity": true }),
        ),
    ] {
        let mut s = S::new();
        let id = s.new_image(32, 24);
        let want = pixels(&editor(
            &s,
            &id,
            pencil_spec(points.clone(), pressure.clone(), size_on, opacity_on),
        ));
        let mut call = json!({ "points": points, "pressure": pressure, "size": 14 });
        for (k, v) in args.as_object().expect("args") {
            call[k] = v.clone();
        }
        s.ok("draw.stroke", call);
        assert_eq!(layers(&s, &id), want, "size {size_on} opacity {opacity_on}");
    }
    let mut s = S::new();
    let id = s.new_image(32, 24);
    let plain = pixels(&editor(
        &s,
        &id,
        pencil_spec(points.clone(), json!([]), false, false),
    ));
    s.ok(
        "draw.stroke",
        json!({ "points": points, "pressure": pressure, "size": 14 }),
    );
    assert_ne!(layers(&s, &id), plain, "light pressure paints thinner dabs");
}

#[test]
fn stroke_eraser_pressure_matches_the_editor_eraser() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let want = pixels(&editor(
        &s,
        &id,
        json!({
            "type": "eraser_stroke", "layer": 0, "size": 10, "opacity": 1.0, "shape": "hard_round",
            "hardness": 1.0, "points": [[2, 6], [30, 6]], "pressure": [0.2, 1.0],
            "pressure_size": true, "pressure_opacity": true, "anti_alias": true, "stroke_id": 1,
        }),
    ));
    s.ok(
        "draw.stroke",
        json!({ "tool": "eraser", "points": [[2, 6], [30, 6]], "size": 10, "pressure": [0.2, 1.0], "pressure_opacity": true }),
    );
    assert_eq!(layers(&s, &id), want);
}

#[test]
fn stroke_without_effective_pressure_paints_as_without_pressure() {
    let points = json!([[3, 12], [16, 4], [29, 12]]);
    let reference = {
        let mut s = S::new();
        let id = s.new_image(32, 24);
        s.ok(
            "draw.stroke",
            json!({ "points": points, "size": 11, "opacity": 60 }),
        );
        layers(&s, &id)
    };
    for extra in [
        json!({ "pressure": [1, 1, 1], "pressure_opacity": true }),
        json!({ "pressure": [0.1, 0.2, 0.0], "pressure_size": false }),
        json!({ "pressure_size": true, "pressure_opacity": true }),
    ] {
        let mut s = S::new();
        let id = s.new_image(32, 24);
        let mut call = json!({ "points": points, "size": 11, "opacity": 60 });
        for (k, v) in extra.as_object().expect("extra") {
            call[k] = v.clone();
        }
        s.ok("draw.stroke", call);
        assert_eq!(layers(&s, &id), reference, "{extra}");
    }
}

#[test]
fn strokes_item_pressure_paints_as_draw_stroke() {
    let item = json!({ "points": [[2, 2], [30, 20]], "size": 16, "pressure": [0.0, 0.7], "pressure_opacity": true });
    let (mut a, mut b) = (S::new(), S::new());
    let (ia, ib) = (a.new_image(32, 24), b.new_image(32, 24));
    a.ok("draw.strokes", json!({ "strokes": [item] }));
    b.ok("draw.stroke", item);
    assert_eq!(layers(&a, &ia), layers(&b, &ib));
}

#[test]
fn pressure_with_a_wrong_length_or_value_fails() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let before = layers(&s, &id);
    let e = s.err(
        "draw.stroke",
        json!({ "points": [[1, 1], [5, 5]], "pressure": [0.5] }),
    );
    assert!(e.contains("pressure"), "{e}");
    let e = s.err(
        "draw.stroke",
        json!({ "points": [[1, 1], [5, 5]], "pressure": [0.5, 1.5] }),
    );
    assert!(e.contains("pressure"), "{e}");
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]] }, { "points": [[1, 1], [5, 5]], "pressure": [0.5, 0.5, 0.5] }] }),
    );
    assert!(e.contains("strokes[1]") && e.contains("pressure"), "{e}");
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]], "pressure": [-0.1] }] }),
    );
    assert!(e.contains("strokes[0]") && e.contains("pressure"), "{e}");
    assert_eq!(layers(&s, &id), before);
}

#[test]
fn pressure_on_a_pixel_document_fails_naming_the_argument_and_mode() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let before = s.state(&id);
    for arg in ["pressure", "pressure_size", "pressure_opacity"] {
        let value = if arg == "pressure" {
            json!([1.0, 1.0])
        } else {
            json!(true)
        };
        let mut call = json!({ "points": [[1, 1], [4, 4]] });
        call[arg] = value;
        let e = s.err("draw.stroke", call.clone());
        assert!(e.contains(arg) && e.contains("Image"), "{arg}: {e}");
        let e = s.err("draw.strokes", json!({ "strokes": [call] }));
        assert!(
            e.contains("strokes[0]") && e.contains(arg) && e.contains("Image"),
            "{arg}: {e}"
        );
    }
    assert_eq!(s.state(&id), before);
}

#[test]
fn guide_names_draw_strokes_and_pressure() {
    let guide = crate::read_resource("pincel://guide")
        .expect("guide")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for needle in [
        "draw.strokes",
        "1000",
        "100000",
        "one undo step",
        "pressure",
        "pressure_size",
        "pressure_opacity",
    ] {
        assert!(guide.contains(needle), "guide lacks {needle}");
    }
}

/// `draw.strokes` and `draw.stroke` on `target` fail with the same message,
/// leave the document as it was and add no undo step (AC-08).
fn assert_refused_like_stroke(s: &mut S, target: u64, needle: &str) {
    let depth = undo_depth(s);
    let id = s.ok("doc.info", json!({}))["doc"]
        .as_str()
        .expect("doc")
        .to_owned();
    let before = s.ok(
        "doc.read_pixels",
        json!({ "doc": id, "layer": 0, "region": { "x": 0, "y": 0, "width": 8, "height": 8 } }),
    );
    let batch = s.err(
        "draw.strokes",
        json!({ "layer": target, "strokes": [{ "points": [[1, 1], [3, 3]] }] }),
    );
    let single = s.err(
        "draw.stroke",
        json!({ "layer": target, "points": [[1, 1], [3, 3]] }),
    );
    assert_eq!(batch, single, "target {target}");
    assert!(batch.contains(needle), "target {target}: {batch}");
    assert_eq!(undo_depth(s), depth, "target {target}: no undo step");
    assert_eq!(
        s.ok(
            "doc.read_pixels",
            json!({ "doc": id, "layer": 0, "region": { "x": 0, "y": 0, "width": 8, "height": 8 } })
        ),
        before
    );
}

#[test]
fn strokes_inside_a_locked_group_or_on_pixel_locks_and_groups_fail_as_draw_stroke_does() {
    // Image: a layer inside a locked group.
    let mut s = S::new();
    image_doc(&mut s);
    let group = s.ok("layer.add", json!({ "kind": "group", "name": "Box" }))["layer"]
        .as_u64()
        .expect("group");
    let inner = s.ok("layer.add", json!({ "name": "Inner", "parent": group }))["layer"]
        .as_u64()
        .expect("inner");
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    assert_refused_like_stroke(&mut s, inner, "locked");

    // Pixel: a locked layer, a layer inside a locked group, a group.
    let mut s = S::new();
    s.new_doc(8, 8);
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    assert_refused_like_stroke(&mut s, 0, "locked");
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    let group = s.ok("layer.add", json!({ "name": "G", "kind": "group" }))["layer"]
        .as_u64()
        .expect("group");
    let inner = s.ok("layer.add", json!({ "name": "A", "parent": group }))["layer"]
        .as_u64()
        .expect("inner");
    assert_refused_like_stroke(&mut s, group, "group");
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    assert_refused_like_stroke(&mut s, inner, "locked");
}

#[test]
fn strokes_limits_come_before_item_errors_and_the_count_limit_first() {
    let mut s = S::new();
    image_doc(&mut s);
    let half: Vec<[u32; 2]> = (0..50_001).map(|i| [i % 32, 5]).collect();
    // Point limit broken and an item with an unknown argument.
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": half }, { "points": half, "colour": "#ff0000" }] }),
    );
    assert!(e.contains("100000") && !e.contains("colour"), "{e}");
    // Point limit broken and an item with a bad value.
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [{ "points": half, "size": 900 }, { "points": half }] }),
    );
    assert!(e.contains("100000") && !e.contains("size"), "{e}");
    // Both limits broken: the stroke count is named.
    let mut many = vec![json!({ "points": [[1, 1]] }); 1001];
    many[0] = json!({ "points": half });
    many[1] = json!({ "points": half });
    let e = s.err("draw.strokes", json!({ "strokes": many }));
    assert!(e.contains("1000") && !e.contains("100000"), "{e}");
    // Exactly 100000 points is accepted.
    let fifty: Vec<[u32; 2]> = (0..50_000).map(|i| [i % 32, 5]).collect();
    s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": fifty }, { "points": fifty }] }),
    );
}

#[test]
fn strokes_report_the_first_invalid_item_by_index() {
    let mut s = S::new();
    image_doc(&mut s);
    let e = s.err(
        "draw.strokes",
        json!({ "strokes": [
            { "points": [[1, 1]] },
            { "points": [[1, 1]], "size": 900 },
            { "points": [[1, 1]], "color": "red" },
        ] }),
    );
    assert!(e.contains("strokes[1]") && e.contains("size"), "{e}");
    assert!(!e.contains("strokes[2]"), "{e}");
}

#[test]
fn strokes_undo_step_has_the_first_strokes_kind_beyond_the_history_capacity() {
    use pincel_image_core::command::HistoryKind;
    let mut s = S::new();
    let id = image_doc(&mut s);
    let mut items = vec![json!({ "tool": "eraser", "points": [[1, 1], [6, 6]] })];
    items.extend((0..150).map(|i| json!({ "points": [[i % 30, 10]], "size": 3 })));
    s.ok("draw.strokes", json!({ "strokes": items }));
    let kinds: Vec<HistoryKind> = s.image(&id).bus.history.undo_kinds().collect();
    assert_eq!(kinds.last(), Some(&HistoryKind::Eraser), "{kinds:?}");
}

#[test]
fn strokes_on_pixel_never_merge_with_the_call_before() {
    let mut s = S::new();
    s.new_doc(8, 8);
    s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[1, 1]] }] }),
    );
    s.ok(
        "draw.strokes",
        json!({ "strokes": [{ "points": [[2, 2]] }] }),
    );
    assert_eq!(undo_depth(&mut s), 2);
}

#[test]
fn guide_gives_the_pressure_defaults_and_that_pressure_is_image_only() {
    let guide = crate::read_resource("pincel://guide")
        .expect("guide")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let start = guide.find("- Pen pressure").expect("pressure bullet");
    // The bullet and what follows it, cut by characters (never past the
    // end of the guide or inside a multi-byte character).
    let bullet: String = guide[start..].chars().take(600).collect();
    for needle in [
        "Image documents only",
        "`pressure_size` is on (default true)",
        "`pressure_opacity` is on (default false)",
    ] {
        assert!(
            bullet.contains(needle),
            "pressure bullet lacks {needle}: {bullet}"
        );
    }
}
