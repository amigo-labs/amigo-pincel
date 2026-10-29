//! `layer.*`, `frame.*`, `tag.*`, `palette.*` and locked layers (AC-15 …
//! AC-18, AC-28).

use pincel_core::{Palette, PalettePreset};
use serde_json::json;

use super::{S, px};
use crate::args::hex;

#[test]
fn layer_tools_change_and_list_the_stack() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let g = s.ok("layer.add", json!({ "name": "Body", "kind": "group" }))["layer"].clone();
    assert_eq!(g, 1);
    let out = s.ok("layer.add", json!({ "name": "Eyes", "parent": 1 }));
    assert_eq!(out["layer"], 2);
    s.ok(
        "layer.set",
        json!({ "layer": 0, "name": "Base", "visible": false, "opacity": 100, "blend": "multiply", "locked": true }),
    );
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    let names: Vec<&str> = list
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["name"].as_str().expect("name"))
        .collect();
    // Groups precede their children in the flat stack (pincel-core).
    assert_eq!(names, ["Base", "Body", "Eyes"]);
    assert_eq!(list[0]["visible"], false);
    assert_eq!(list[0]["opacity"], 100);
    assert_eq!(list[0]["blend"], "multiply");
    assert_eq!(list[0]["locked"], true);
    assert_eq!(list[2]["parent"], 1);
    assert_eq!(list[1]["kind"], "group");
    // Move an existing layer into the group.
    s.ok("layer.add", json!({ "name": "Hat" }));
    s.ok("layer.set", json!({ "layer": 3, "parent": 1 }));
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    let hat = list
        .as_array()
        .expect("layers")
        .iter()
        .find(|l| l["name"] == "Hat")
        .expect("hat");
    assert_eq!(hat["parent"], 1);
    s.ok("layer.remove", json!({ "layer": 1 }));
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    assert_eq!(list.as_array().map(Vec::len), Some(1));
}

#[test]
fn frame_tools_change_frames_and_clone_cels() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 2, "color": "#ff0000" }] }),
    );
    s.ok(
        "frame.set_duration",
        json!({ "frame": 0, "duration_ms": 120 }),
    );
    let out = s.ok("frame.add", json!({ "clone_from": 0 }));
    assert_eq!(out["frame"], 1);
    s.ok("frame.add", json!({ "duration_ms": 30 }));
    let frames = s.ok("frame.list", json!({}))["frames"].clone();
    assert_eq!(
        frames,
        json!([
            { "index": 0, "duration_ms": 120 },
            { "index": 1, "duration_ms": 120 },
            { "index": 2, "duration_ms": 30 },
        ])
    );
    assert_eq!(s.cel_rgba(&id, 0, 1), s.cel_rgba(&id, 0, 0));
    assert_eq!(px(&s.cel_rgba(&id, 0, 1), 4, 1, 2), [255, 0, 0, 255]);
    s.ok("frame.remove", json!({ "frame": 0 }));
    let frames = s.ok("frame.list", json!({}))["frames"].clone();
    assert_eq!(frames.as_array().map(Vec::len), Some(2));
    assert_eq!(frames[1]["duration_ms"], 30);
}

#[test]
fn tag_tools_change_and_list_tags() {
    let mut s = S::new();
    s.new_doc(4, 4);
    s.ok("frame.add", json!({}));
    s.ok("frame.add", json!({}));
    s.ok("tag.add", json!({ "name": "walk", "from": 0, "to": 1 }));
    s.ok(
        "tag.add",
        json!({ "name": "jump", "from": 1, "to": 2, "direction": "pingpong" }),
    );
    let tags = s.ok("tag.list", json!({}))["tags"].clone();
    assert_eq!(
        tags,
        json!([
            { "index": 0, "name": "walk", "from": 0, "to": 1, "direction": "forward" },
            { "index": 1, "name": "jump", "from": 1, "to": 2, "direction": "pingpong" },
        ])
    );
    s.ok("tag.remove", json!({ "tag": 0 }));
    let tags = s.ok("tag.list", json!({}))["tags"].clone();
    assert_eq!(tags.as_array().map(Vec::len), Some(1));
    assert_eq!(tags[0]["name"], "jump");
    assert!(
        s.err("tag.add", json!({ "name": "x", "from": 0, "to": 9 }))
            .contains("frame 9")
    );
}

#[test]
fn palette_tools_set_and_load_presets() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let got = s.ok("palette.get", json!({}));
    assert_eq!(got["size"], 32);
    s.ok(
        "palette.set",
        json!({ "entries": ["#000000", "#FF000080"] }),
    );
    assert_eq!(
        s.ok("palette.get", json!({}))["entries"],
        json!(["#000000FF", "#FF000080"])
    );
    for preset in PalettePreset::ALL {
        s.ok("palette.load_preset", json!({ "preset": preset.name() }));
        let expected: Vec<String> = Palette::preset(preset)
            .colors
            .iter()
            .map(|e| hex(e.rgba))
            .collect();
        assert_eq!(s.ok("palette.get", json!({}))["entries"], json!(expected));
    }
    let e = s.err("palette.set", json!({ "entries": ["#00000"] }));
    assert!(e.contains("entries[0]"), "{e}");
}

#[test]
fn locked_layer_or_locked_group_refuses_edits() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "layer.set",
        json!({ "layer": 0, "name": "Ink", "locked": true }),
    );
    let before = s.state(&id);
    for (name, args) in [
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ffffff" }] }),
        ),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ffffff" }),
        ),
        ("draw.fill", json!({ "x": 0, "y": 0, "color": "#ffffff" })),
        ("draw.clear", json!({})),
        ("effect.apply", json!({ "id": "invert" })),
        ("layer.remove", json!({ "layer": 0 })),
    ] {
        let e = s.err(name, args);
        assert!(e.contains("locked") && e.contains("Ink"), "{name}: {e}");
        assert_eq!(s.state(&id), before, "{name}");
    }
    // A layer inside a locked group.
    s.ok(
        "layer.add",
        json!({ "name": "G", "kind": "group", "locked": true }),
    );
    s.ok("layer.add", json!({ "name": "Child", "parent": 1 }));
    let e = s.err(
        "draw.pixels",
        json!({ "layer": 2, "pixels": [{ "x": 0, "y": 0, "color": "#ffffff" }] }),
    );
    assert!(e.contains("Child") && e.contains("locked"), "{e}");
}
