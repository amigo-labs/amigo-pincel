//! `draw.gradient` with the spiral kinds
//! (docs/specs/image-palette-gradient-spiral.md AC-10 … AC-13).

use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::{Value, json};

use super::S;
use crate::list_tools;

const KINDS: [&str; 2] = ["spiral_cw", "spiral_ccw"];

/// The layer pixels after the editor's commands `cmds` on a copy of the
/// document.
fn editor(s: &S, id: &str, cmds: &[Value]) -> Vec<u8> {
    let mut bus = CommandBus::new(s.image(id).document().clone());
    for cmd in cmds {
        let spec: CommandSpec = serde_json::from_value(cmd.clone()).expect("valid CommandSpec");
        run_command(&mut bus, spec, &mut StrokeScratch::default(), &[]).expect("runs");
    }
    bus.document.layers()[0].pixels.data().to_vec()
}

fn layer(s: &S, id: &str) -> Vec<u8> {
    s.image(id).document().layers()[0].pixels.data().to_vec()
}

#[test]
fn tools_list_offers_the_spiral_kinds_for_draw_gradient() {
    let tools = list_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == "draw.gradient")
        .expect("draw.gradient listed");
    let kinds: Vec<&str> = tool.input_schema["properties"]["kind"]["enum"]
        .as_array()
        .expect("kind enum")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        kinds,
        [
            "linear",
            "reflected",
            "radial",
            "conical",
            "diamond",
            "spiral_cw",
            "spiral_ccw"
        ]
    );
    for kind in KINDS {
        assert!(
            tool.description.contains(kind),
            "description names {kind}: {}",
            tool.description
        );
    }
}

#[test]
fn draw_gradient_spiral_matches_the_editor_gradient_tool() {
    let mut s = S::new();
    let id = s.new_image(30, 24);
    s.ok(
        "draw.gradient",
        json!({ "x0": 0, "y0": 0, "x1": 30, "y1": 24, "from": "#102030", "to": "#e0c040" }),
    );
    // A feathered ellipse, so the selection clips and mixes by coverage.
    s.ok(
        "select.ellipse",
        json!({ "x": 3, "y": 2, "width": 22, "height": 19, "feather": 3 }),
    );
    for kind in KINDS {
        for (channel, opacity) in [("color", 60), ("transparency", 100), ("color", 100)] {
            let (x0, y0, x1, y1) = (14.5, 11.25, 6.0, 4.5);
            let wire = json!({
                "layer": 0, "kind": kind, "channel": channel,
                "x0": x0, "y0": y0, "x1": x1, "y1": y1,
                "from": [200, 30, 40, 255], "to": [16, 64, 224, 128],
                "opacity": f64::from(opacity) / 100.0
            });
            let mut begin = wire.clone();
            begin["type"] = json!("begin_live_gradient");
            let drag = editor(&s, &id, &[begin, json!({ "type": "commit_live" })]);
            let mut draw = wire;
            draw["type"] = json!("draw_gradient");
            assert_eq!(editor(&s, &id, &[draw]), drag, "{kind} {channel}");

            let before = layer(&s, &id);
            let depth = s.image(&id).bus.history.undo_depth();
            s.ok(
                "draw.gradient",
                json!({
                    "x0": x0, "y0": y0, "x1": x1, "y1": y1, "from": "#c81e28", "to": "#1040e080",
                    "kind": kind, "channel": channel, "opacity": opacity
                }),
            );
            assert_ne!(layer(&s, &id), before, "{kind} {channel} paints");
            assert_eq!(layer(&s, &id), drag, "{kind} {channel} {opacity}");
            assert_eq!(
                s.image(&id).bus.history.undo_depth(),
                depth + 1,
                "{kind} {channel}: one undo step"
            );
            s.ok("doc.undo", json!({}));
            assert_eq!(layer(&s, &id), before, "{kind} {channel} undo");
        }
    }
}

#[test]
fn draw_gradient_with_an_unknown_kind_lists_the_accepted_kinds() {
    let mut s = S::new();
    let id = s.new_image(12, 8);
    let before = layer(&s, &id);
    for kind in ["spiral", "spiral_clockwise"] {
        let e = s.err(
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 6, "y1": 0, "from": "#000000", "to": "#ffffff", "kind": kind }),
        );
        for accepted in [
            "linear",
            "reflected",
            "radial",
            "conical",
            "diamond",
            "spiral_cw",
            "spiral_ccw",
        ] {
            assert!(e.contains(accepted), "{kind}: {e}");
        }
        assert_eq!(layer(&s, &id), before, "{kind}");
        assert_eq!(s.image(&id).bus.history.undo_depth(), 0, "{kind}");
    }
}

#[test]
fn draw_gradient_spiral_on_a_pixel_document_is_a_mode_error() {
    let mut s = S::new();
    let id = s.new_doc(12, 8);
    let before = s.state(&id);
    for kind in KINDS {
        let e = s.err(
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 6, "y1": 0, "from": "#000000", "to": "#ffffff", "kind": kind }),
        );
        assert!(
            e.contains("`kind`")
                && e.contains(kind)
                && e.contains("works on Image documents only")
                && e.contains("is a Pixel document"),
            "{kind}: {e}"
        );
        assert!(s.state(&id) == before, "{kind}: document unchanged");
    }
}
