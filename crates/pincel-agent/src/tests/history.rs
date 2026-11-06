//! History kinds of MCP steps (docs/specs/history-panel.md AC-15): every
//! call names its step after the editor action it mirrors.

use serde_json::{Value, json};

use super::S;

fn last_kind(s: &S, id: &str) -> String {
    s.doc(id)
        .bus
        .undo_kinds()
        .last()
        .map(|k| k.id().into_owned())
        .unwrap_or_default()
}

#[test]
fn pixel_calls_name_their_step_after_the_editor_action() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let calls: Vec<(&str, Value, &str)> = vec![
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }] }),
            "draw_pixels",
        ),
        ("draw.stroke", json!({ "points": [[1, 1], [5, 3]] }), "pencil"),
        (
            "draw.stroke",
            json!({ "points": [[1, 1], [5, 3]], "tool": "eraser" }),
            "eraser",
        ),
        (
            "draw.shape",
            json!({ "shape": "rounded", "x0": 0, "y0": 0, "x1": 6, "y1": 6, "fill": "#00ff00" }),
            "rounded_rectangle",
        ),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 0, "x1": 7, "y1": 7, "color": "#ffffff" }),
            "line",
        ),
        ("effect.apply", json!({ "id": "invert" }), "effect.invert"),
        (
            "draw.clear",
            json!({ "region": { "x": 0, "y": 0, "width": 3, "height": 3 } }),
            "delete_selection",
        ),
        ("layer.duplicate", json!({ "layer": 0 }), "duplicate_layer"),
        ("layer.add", json!({ "name": "G", "kind": "group" }), "add_group"),
        ("layer.set", json!({ "layer": 1, "name": "Renamed" }), "rename_layer"),
        (
            "layer.set",
            json!({ "layer": 1, "name": "Again", "visible": false }),
            "layer_properties",
        ),
        ("frame.add", json!({}), "add_frame"),
        ("frame.add", json!({ "clone_from": 0 }), "duplicate_frame"),
        ("tag.add", json!({ "name": "walk", "from": 0, "to": 1 }), "add_tag"),
        (
            "tag.set",
            json!({ "tag": 0, "name": "run", "direction": "reverse" }),
            "tag_properties",
        ),
        ("image.flip", json!({ "direction": "horizontal" }), "flip_canvas_horizontal"),
        ("layer.flip", json!({ "layer": 0, "direction": "vertical" }), "flip_layer_vertical"),
    ];
    for (name, args, want) in calls {
        s.ok(name, args);
        assert_eq!(last_kind(&s, &id), want, "{name}");
    }
    s.ok("select.rect", json!({ "x": 1, "y": 1, "width": 4, "height": 4 }));
    s.ok("draw.pixels", json!({ "pixels": [{ "x": 2, "y": 2, "color": "#ff0000" }] }));
    s.ok("select.move_content", json!({ "dx": 1, "dy": 0 }));
    assert_eq!(last_kind(&s, &id), "move_selection");
    s.ok("image.crop_to_selection", json!({}));
    assert_eq!(last_kind(&s, &id), "crop_to_selection");
}
