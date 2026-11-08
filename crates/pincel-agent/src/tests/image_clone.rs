//! `draw.clone`, the Image editor's Clone Stamp (clone-stamp AC-19, AC-20).

use serde_json::{Value, json};

use super::S;
use super::image_brushes::{editor, layers, painted};
use crate::list_tools;

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// The editor's `clone_stroke` with `offset` through `points`, at the
/// brush defaults `draw.clone` uses; `extra` overrides fields.
fn clone_spec(offset: [i32; 2], points: Value, extra: Value) -> Value {
    let mut spec = json!({
        "type": "clone_stroke", "layer": 0, "size": 8, "opacity": 1.0, "shape": "hard_round",
        "hardness": 1.0, "offset": offset, "sample": "current_layer", "points": points,
        "angle": 45.0, "stroke_id": 1,
    });
    for (k, v) in extra.as_object().expect("obj") {
        spec[k] = v.clone();
    }
    spec
}

#[test]
fn draw_clone_is_listed_for_image_documents() {
    let tool = list_tools()
        .into_iter()
        .find(|t| t.name == "draw.clone")
        .expect("draw.clone is listed");
    let props = &tool.input_schema["properties"];
    for key in [
        "points", "source", "layer", "size", "hardness", "opacity", "shape", "angle", "sample",
    ] {
        assert!(props.get(key).is_some(), "{key}");
    }
    assert_eq!(
        props["sample"]["enum"],
        json!(["current_layer", "all_layers"])
    );
    assert_eq!(tool.input_schema["required"], json!(["points", "source"]));
}

#[test]
fn draw_clone_equals_the_editors_clone_stamp() {
    // The source pixel (5, 6) goes to the pixel under the first point,
    // (16, 8) — floor, not round, of (16.7, 8.6): offset (-11, -2).
    let mut s = S::new();
    let id = painted(&mut s);
    let points = json!([[16.7, 8.6], [21.0, 8.0]]);
    let want = editor(&s, &id, clone_spec([-11, -2], points.clone(), json!({})));
    let out = s.ok("draw.clone", json!({ "points": points, "source": [5, 6] }));
    assert_eq!(out["changed"], true);
    assert_eq!(layers(&s, &id), want);
    // The brush arguments of draw.stroke.
    let points = json!([[18.0, 3.0], [19.0, 12.0]]);
    let want = editor(
        &s,
        &id,
        clone_spec(
            [-10, 2],
            points.clone(),
            json!({ "size": 5, "hardness": 0.4, "opacity": 0.7, "shape": "flat", "angle": 120.0, "stroke_id": 2 }),
        ),
    );
    s.ok(
        "draw.clone",
        json!({ "points": points, "source": [8, 5], "size": 5, "hardness": 40, "opacity": 70, "shape": "flat", "angle": 120 }),
    );
    assert_eq!(
        layers(&s, &id),
        want,
        "size, hardness, opacity, shape, angle"
    );
}

#[test]
fn draw_clone_samples_the_layer_or_the_visible_composite() {
    // A transparent layer on top: Current Layer copies its transparency
    // (no change), All Layers the visible red and blue below; a hidden
    // layer is not part of the composite.
    let mut s = S::new();
    let id = painted(&mut s);
    s.ok("layer.add", json!({ "name": "Hidden" }));
    s.ok(
        "draw.fill",
        json!({ "x": 0, "y": 0, "color": "#00ff00", "layer": 1 }),
    );
    s.ok("layer.set", json!({ "layer": 1, "visible": false }));
    let top = s.ok("layer.add", json!({ "name": "Top" }))["layer"]
        .as_u64()
        .expect("id");
    let points = json!([[18.0, 8.0], [22.0, 8.0]]);
    let args = json!({ "points": points, "source": [8, 8], "layer": top });
    let out = s.ok("draw.clone", args.clone());
    assert_eq!(out["changed"], false, "current_layer reads the empty layer");
    let mut with_sample = args.clone();
    with_sample["sample"] = json!("all_layers");
    let want = editor(
        &s,
        &id,
        clone_spec(
            [-10, 0],
            points,
            json!({ "layer": 2, "sample": "all_layers" }),
        ),
    );
    s.ok("draw.clone", with_sample);
    assert_eq!(layers(&s, &id), want);
    let top_pixels = &layers(&s, &id)[2].pixels;
    assert_eq!(
        top_pixels.get_pixel(18, 8).map(|c| [c.r, c.g, c.b, c.a]),
        Some([255, 0, 0, 255]),
        "red from the visible composite, never the hidden green"
    );
}

#[test]
fn draw_clone_is_one_undo_step_per_changing_call() {
    let mut s = S::new();
    let id = painted(&mut s);
    let before = layers(&s, &id);
    let depth = undo_depth(&mut s);
    let args = json!({ "points": [[17, 8], [21, 8]], "source": [6, 6] });
    s.ok("draw.clone", args.clone());
    let once = layers(&s, &id);
    assert_eq!(undo_depth(&mut s), depth + 1);
    // A second call never merges with the first.
    s.ok(
        "draw.clone",
        json!({ "points": [[17, 2], [21, 2]], "source": [6, 6] }),
    );
    assert_eq!(undo_depth(&mut s), depth + 2);
    s.ok("doc.undo", json!({}));
    assert_eq!(layers(&s, &id), once);
    s.ok("doc.undo", json!({}));
    assert_eq!(layers(&s, &id), before, "undo restores byte for byte");
    // Copying blue onto blue changes nothing: no undo step.
    let out = s.ok(
        "draw.clone",
        json!({ "points": [[20, 14], [22, 14]], "source": [20, 14] }),
    );
    assert_eq!(out["changed"], false);
    // Every source position off the canvas: no step either. The first
    // point lies beyond the right edge, so the dabs read left of x = 0.
    let out = s.ok(
        "draw.clone",
        json!({ "points": [[40, 8], [22, 8]], "source": [0, 8], "size": 4 }),
    );
    assert_eq!(out["changed"], false);
    assert_eq!(undo_depth(&mut s), depth);
    assert_eq!(layers(&s, &id), before);
}

#[test]
fn draw_clone_refuses_bad_arguments_and_targets() {
    let mut s = S::new();
    let pixel = s.new_doc(8, 8);
    let before = s.state(&pixel);
    let e = s.err(
        "draw.clone",
        json!({ "points": [[1, 1]], "source": [0, 0] }),
    );
    assert!(
        e.contains("Pixel document") && e.contains("Image documents only"),
        "{e}"
    );
    assert_eq!(s.state(&pixel), before);

    let id = painted(&mut s);
    s.ok(
        "layer.set",
        json!({ "layer": 0, "name": "Base", "locked": true }),
    );
    let free = s.ok("layer.add", json!({ "name": "Free" }))["layer"]
        .as_u64()
        .expect("layer id");
    let group = s.ok("layer.add", json!({ "kind": "group", "name": "Box" }))["layer"]
        .as_u64()
        .expect("group id");
    let inner = s.ok("layer.add", json!({ "name": "Inner", "parent": group }))["layer"]
        .as_u64()
        .expect("layer id");
    let ok = |extra: Value| -> Value {
        let mut a = json!({ "points": [[10, 8]], "source": [2, 2], "layer": free });
        for (k, v) in extra.as_object().expect("obj") {
            if v.is_null() {
                a.as_object_mut().expect("obj").remove(k);
            } else {
                a[k] = v.clone();
            }
        }
        a
    };
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    for (args, needles) in [
        (ok(json!({ "source": null })), vec!["`source`"]),
        (ok(json!({ "source": [1.5, 2] })), vec!["source"]),
        (ok(json!({ "source": [1] })), vec!["source"]),
        (ok(json!({ "source": ["a", 1] })), vec!["source"]),
        (
            ok(json!({ "source": [24, 0] })),
            vec!["`source`", "outside"],
        ),
        (
            ok(json!({ "source": [0, -1] })),
            vec!["`source`", "outside"],
        ),
        (ok(json!({ "points": [] })), vec!["points"]),
        (
            ok(json!({ "sample": "screen" })),
            vec!["sample", "current_layer", "all_layers"],
        ),
        (ok(json!({ "size": 0 })), vec!["size", "1–500"]),
        (ok(json!({ "size": 501 })), vec!["size", "1–500"]),
        (ok(json!({ "hardness": 101 })), vec!["hardness", "0–100"]),
        (ok(json!({ "opacity": -1 })), vec!["opacity", "0–100"]),
        (
            ok(json!({ "shape": "square" })),
            vec!["shape", "hard_round"],
        ),
        (ok(json!({ "angle": 181 })), vec!["angle", "0–180"]),
        (
            ok(json!({ "color": "#ff0000" })),
            vec!["unknown argument `color`"],
        ),
        (
            ok(json!({ "layer": 0 })),
            vec!["layer 0 (\"Base\") is locked"],
        ),
        (
            ok(json!({ "layer": inner })),
            vec!["is locked", "\"Inner\""],
        ),
        (ok(json!({ "layer": group })), vec!["is a group", "\"Box\""]),
    ] {
        let before = layers(&s, &id);
        let depth = undo_depth(&mut s);
        let e = s.err("draw.clone", args.clone());
        for needle in needles {
            assert!(e.contains(needle), "{args}: {needle} in {e}");
        }
        assert_eq!(layers(&s, &id), before, "{args}");
        assert_eq!(undo_depth(&mut s), depth, "{args}");
    }
    // The valid call itself works.
    s.ok("draw.clone", ok(json!({})));
}
