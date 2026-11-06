//! `select.fill`: the Image editor's Edit ▸ Fill Selection
//! (docs/specs/image-edit-extras.md AC-10 – AC-12).

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

/// The layers after the editor runs Fill Selection with `color` on `layer`
/// of a copy of document `id`.
fn editor_fill(s: &S, id: &str, layer: usize, color: [u8; 4]) -> CommandBus {
    let mut bus = CommandBus::new(s.image(id).document().clone());
    run_command(
        &mut bus,
        CommandSpec::FillSelection { layer, color },
        &mut StrokeScratch::default(),
        &[],
    )
    .expect("editor");
    bus
}

/// An 8×8 Image document with a red layer under a painted top layer.
fn image_doc(s: &mut S) -> String {
    let id = s.new_image(8, 8);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#ff0000" }));
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 4, "y1": 4, "fill": "#3366cc", "anti_alias": false }),
    );
    id
}

#[test]
fn select_fill_is_listed_with_doc_layer_and_color_and_no_frame() {
    let tool = list_tools()
        .into_iter()
        .find(|t| t.name == "select.fill")
        .expect("select.fill is listed");
    assert!(!tool.description.is_empty());
    let schema = &tool.input_schema;
    assert_eq!(schema["type"], "object");
    let props = schema["properties"].as_object().expect("properties");
    let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["color", "doc", "layer"]);
    assert_eq!(schema["required"], json!([]));
    assert!(
        props["layer"]["description"]
            .as_str()
            .is_some_and(|d| d.contains("default: the active layer"))
    );
    assert!(
        props["color"]["description"]
            .as_str()
            .is_some_and(|d| d.contains("#000000") && d.contains("#RRGGBBAA"))
    );
}

#[test]
fn select_fill_matches_the_editor_with_a_feathered_selection() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.ellipse",
        json!({ "x": 1, "y": 1, "width": 6, "height": 6, "feather": 2 }),
    );
    let selection = s.image(&id).document().selection.clone();
    let want = editor_fill(&s, &id, 1, [0x20, 0xc0, 0x40, 0x80]);
    let depth = undo_depth(&mut s);
    let out = s.ok("select.fill", json!({ "color": "#20c04080" }));
    assert_eq!(out, json!({ "doc": id, "layer": 1, "changed": true }));
    assert_eq!(undo_depth(&mut s), depth + 1, "one undo step");
    assert_eq!(s.image(&id).document().layers(), want.document.layers());
    assert_eq!(
        s.image(&id).document().selection,
        selection,
        "selection stays"
    );
}

#[test]
fn select_fill_that_changes_nothing_reports_unchanged_and_adds_no_step() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 2, "width": 4, "height": 3 }),
    );
    assert_eq!(
        s.ok("select.fill", json!({ "color": "#20c04080" }))["changed"],
        true
    );
    let depth = undo_depth(&mut s);
    let before = s.image(&id).document().clone();
    let out = s.ok("select.fill", json!({ "color": "#20c04080" }));
    assert_eq!(out, json!({ "doc": id, "layer": 1, "changed": false }));
    assert_eq!(undo_depth(&mut s), depth);
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn select_fill_defaults_to_black_on_the_active_layer_and_fills_it_whole_without_a_selection() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let below = s.image(&id).document().layers()[0].clone();
    let want = editor_fill(&s, &id, 1, [0, 0, 0, 255]);
    let out = s.ok("select.fill", json!({}));
    assert_eq!(out, json!({ "doc": id, "layer": 1, "changed": true }));
    let doc = s.image(&id).document();
    assert_eq!(doc.layers(), want.document.layers());
    assert!(
        doc.layers()[1]
            .pixels
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255]),
        "the whole layer is black"
    );
    assert_eq!(doc.layers()[0], below, "other layer unchanged");
    assert!(doc.selection.is_none());

    // `layer` picks another layer.
    let want = editor_fill(&s, &id, 0, [0, 0, 255, 255]);
    let out = s.ok("select.fill", json!({ "layer": 0, "color": "#0000ff" }));
    assert_eq!(out, json!({ "doc": id, "layer": 0, "changed": true }));
    assert_eq!(s.image(&id).document().layers(), want.document.layers());
}

#[test]
fn select_fill_refuses_pixel_documents_bad_layers_colors_and_frames() {
    let mut s = S::new();
    let px = s.new_doc(4, 4);
    let before = s.state(&px);
    let e = s.err("select.fill", json!({}));
    assert!(
        e.contains("Pixel document") && e.contains("Image documents only"),
        "{e}"
    );
    assert_eq!(s.state(&px), before);

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.ok("layer.add", json!({}));
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    let cases: [(Value, &[&str]); 6] = [
        (json!({ "layer": 0 }), &["layer 0", "locked"]),
        (json!({ "layer": 2 }), &["layer 2", "group"]),
        (json!({ "layer": 9 }), &["`layer`", "9"]),
        (
            json!({ "layer": 1, "color": "#12345" }),
            &["`color`", "#12345"],
        ),
        (
            json!({ "layer": 1, "color": { "index": 2 } }),
            &["`color`", "index"],
        ),
        (json!({ "layer": 1, "frame": 7 }), &["`frame`", "7"]),
    ];
    for (args, needles) in cases {
        let before = s.image(&id).document().clone();
        let e = s.err("select.fill", args.clone());
        assert!(needles.iter().all(|n| e.contains(n)), "{args}: {e}");
        assert_eq!(s.image(&id).document(), &before, "{args}");
    }
    // A layer inside a locked group.
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 2, "place": "into" }),
    );
    s.ok("layer.set", json!({ "layer": 2, "locked": true }));
    let before = s.image(&id).document().clone();
    let e = s.err("select.fill", json!({ "layer": 0 }));
    assert!(e.contains("locked") && e.contains("layer 0"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}
