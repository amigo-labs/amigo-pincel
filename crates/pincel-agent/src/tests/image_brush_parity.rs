//! Image brush options the first tests left loose: Recolor's default
//! tolerance, Smudge's hardness, the eraser's nib angle and the exact
//! undo / redo of one call (mcp-parity-drawing-image AC-01, AC-03, AC-05,
//! AC-13).

use pincel_image_core::{Layer, SelectionMask};
use serde_json::{Value, json};

use super::S;
use super::image_brushes::{editor, layers, painted, stroke_base};

/// A 24×16 Image document of four vertical bands: `#0000ff` and three
/// blues at distance 32, 33 and 39 from it.
fn blue_bands(s: &mut S) -> String {
    let id = s.new_image(24, 16);
    for (x0, color) in [
        (0, "#0000ff"),
        (6, "#0000df"),
        (12, "#0000de"),
        (18, "#0000d8"),
    ] {
        s.ok(
            "draw.shape",
            json!({ "shape": "rect", "x0": x0, "y0": 0, "x1": x0 + 6, "y1": 16, "fill": color, "anti_alias": false }),
        );
    }
    id
}

fn recolor_spec(tolerance: u8) -> Value {
    let mut spec = stroke_base("recolor_stroke");
    spec["from"] = json!([0, 0, 255, 255]);
    spec["to"] = json!([0, 0, 0, 255]);
    spec["tolerance"] = json!(tolerance);
    spec["points"] = json!([[1.0, 8.0], [23.0, 8.0]]);
    spec
}

#[test]
fn draw_recolor_without_tolerance_replaces_colors_within_32() {
    let mut s = S::new();
    let id = blue_bands(&mut s);
    let want = editor(&s, &id, recolor_spec(32));
    // The bands tell 31, 32 and 33 apart, so the default is pinned.
    for other in [0, 31, 33] {
        assert_ne!(editor(&s, &id, recolor_spec(other)), want, "{other}");
    }
    s.ok(
        "draw.recolor",
        json!({ "points": [[1, 8], [23, 8]], "from": "#0000ff" }),
    );
    assert_eq!(layers(&s, &id), want);
}

#[test]
fn draw_smudge_passes_hardness_to_a_soft_tip() {
    for shape in ["soft_round", "flat"] {
        let mut s = S::new();
        let id = painted(&mut s);
        let smudge = |hardness: f64| {
            let mut spec = stroke_base("smudge_stroke");
            spec["strength"] = json!(0.6);
            spec["shape"] = json!(shape);
            spec["hardness"] = json!(hardness);
            spec["size"] = json!(10);
            spec
        };
        let want = editor(&s, &id, smudge(0.3));
        assert_ne!(editor(&s, &id, smudge(1.0)), want, "{shape}");
        s.ok(
            "draw.smudge",
            json!({ "points": [[2, 8], [20, 8]], "shape": shape, "hardness": 30, "size": 10 }),
        );
        assert_eq!(layers(&s, &id), want, "{shape}");
    }
}

#[test]
fn the_eraser_turns_a_flat_tip_by_angle() {
    let eraser = |angle: f64| {
        let mut spec = stroke_base("eraser_stroke");
        spec["shape"] = json!("flat");
        spec["angle"] = json!(angle);
        spec["mode"] = json!("to_transparent");
        spec["points"] = json!([[2.0, 3.0], [20.0, 12.0]]);
        spec
    };
    for (angle, wire) in [(Some(120.0), 120.0), (None, 45.0)] {
        let mut s = S::new();
        let id = painted(&mut s);
        let want = editor(&s, &id, eraser(wire));
        assert_ne!(editor(&s, &id, eraser(90.0)), want, "{wire}");
        let mut args = json!({ "points": [[2, 3], [20, 12]], "tool": "eraser", "shape": "flat" });
        if let Some(a) = angle {
            args["angle"] = json!(a);
        }
        s.ok("draw.stroke", args);
        assert_eq!(layers(&s, &id), want, "angle {angle:?}");
    }
}

type State = (Vec<Layer>, Option<SelectionMask>);

fn state(s: &S, id: &str) -> State {
    let d = s.image(id).document();
    (d.layers().to_vec(), d.selection.clone())
}

/// Two calls of one tool are two undo steps: undo restores the exact
/// state before each, redo the exact state after each.
#[track_caller]
fn two_separate_steps(s: &mut S, id: &str, name: &str, first: Value, second: Value) {
    let before = state(s, id);
    s.ok(name, first);
    let mid = state(s, id);
    s.ok(name, second);
    let after = state(s, id);
    assert_ne!(before, mid, "{name} changes the document");
    assert_ne!(mid, after, "{name} again changes it again");
    s.ok("doc.undo", json!({}));
    assert_eq!(state(s, id), mid, "{name} undo of the second call");
    s.ok("doc.undo", json!({}));
    assert_eq!(state(s, id), before, "{name} undo of the first call");
    s.ok("doc.redo", json!({}));
    assert_eq!(state(s, id), mid, "{name} redo of the first call");
    s.ok("doc.redo", json!({}));
    assert_eq!(state(s, id), after, "{name} redo of the second call");
}

#[test]
fn recolor_smudge_and_eraser_undo_and_redo_exactly_one_call() {
    let path = |y: u32| json!([[2, y], [20, y]]);
    for (name, first, second) in [
        (
            "draw.recolor",
            json!({ "points": path(8), "from": "#ff0000", "color": "#808080" }),
            json!({ "points": path(6), "from": "#0000ff", "color": "#00ff00" }),
        ),
        (
            "draw.smudge",
            json!({ "points": path(8) }),
            json!({ "points": path(5) }),
        ),
        (
            "draw.stroke",
            json!({ "points": path(8), "tool": "eraser", "eraser_mode": "to_background", "background": "#00ff0080" }),
            json!({ "points": path(4), "tool": "eraser", "eraser_mode": "to_background" }),
        ),
    ] {
        let mut s = S::new();
        let id = painted(&mut s);
        s.ok(
            "select.ellipse",
            json!({ "x": 1, "y": 1, "width": 22, "height": 14 }),
        );
        two_separate_steps(&mut s, &id, name, first, second);
    }
}
