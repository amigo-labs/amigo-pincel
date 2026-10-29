//! Registry, session, undo and file tools (AC-02 … AC-10, AC-29 … AC-33,
//! AC-46).

use std::collections::HashSet;

use pincel_core::{
    AtlasOptions, CelMap, Palette, PalettePreset, Sprite, aseprite_sheet_json, export_atlas_png,
    import_png, read_aseprite,
};
use serde_json::{Value, json};

use super::S;
use crate::{AgentError, list_tools, schema};

const EXPECTED_TOOLS: &[&str] = &[
    "doc.new",
    "doc.open",
    "doc.list",
    "doc.info",
    "doc.close",
    "doc.preview",
    "doc.read_pixels",
    "doc.undo",
    "doc.redo",
    "doc.save",
    "export.png",
    "export.sheet",
    "layer.add",
    "layer.remove",
    "layer.list",
    "layer.set",
    "frame.add",
    "frame.remove",
    "frame.set_duration",
    "frame.list",
    "tag.add",
    "tag.remove",
    "tag.list",
    "palette.get",
    "palette.set",
    "palette.load_preset",
    "draw.pixels",
    "draw.line",
    "draw.rect",
    "draw.ellipse",
    "draw.fill",
    "draw.clear",
    "selection.set_rect",
    "selection.clear",
    "effect.apply",
];

/// One valid example call per tool.
fn example(name: &str) -> Value {
    let region = json!({ "x": 0, "y": 0, "width": 2, "height": 2 });
    match name {
        "doc.new" => json!({ "mode": "pixel", "width": 16, "height": 16, "palette": "pico8" }),
        "doc.open" => json!({ "path": "hero.aseprite" }),
        "doc.preview" => json!({ "frame": 0, "scale": 4, "grid": true, "region": region }),
        "doc.read_pixels" => json!({ "region": region, "layer": 0, "frame": 0, "format": "hex" }),
        "doc.save" => json!({ "path": "hero.aseprite", "doc": "doc-1" }),
        "export.png" => json!({ "path": "hero.png", "frame": 0, "scale": 2 }),
        "export.sheet" => {
            json!({ "columns": 4, "path": "s.png", "tag": "walk", "scale": 2, "json": "s.json" })
        }
        "layer.add" => json!({ "name": "Outline", "kind": "image", "parent": 1 }),
        "layer.remove" => json!({ "layer": 1 }),
        "layer.set" => {
            json!({ "layer": 0, "name": "A", "visible": false, "opacity": 128, "blend": "multiply", "locked": true })
        }
        "frame.add" => json!({ "clone_from": 0, "duration_ms": 80 }),
        "frame.remove" => json!({ "frame": 1 }),
        "frame.set_duration" => json!({ "frame": 0, "duration_ms": 120 }),
        "tag.add" => json!({ "name": "walk", "from": 0, "to": 1, "direction": "pingpong" }),
        "tag.remove" => json!({ "tag": 0 }),
        "palette.set" => json!({ "entries": ["#000000", "#ffffff80"] }),
        "palette.load_preset" => json!({ "preset": "nes" }),
        "draw.pixels" => {
            json!({ "pixels": [{ "x": 1, "y": 2, "color": "#ff0000" }, { "x": 0, "y": 0, "color": { "index": 3 } }], "layer": 0, "frame": 0 })
        }
        "draw.line" => json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#00ff00" }),
        "draw.rect" | "draw.ellipse" => {
            json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#00ff00", "filled": true })
        }
        "draw.fill" => json!({ "x": 1, "y": 1, "color": "#0000ff", "contiguous": false }),
        "draw.clear" => json!({ "region": region }),
        "selection.set_rect" => json!({ "x": 0, "y": 0, "width": 4, "height": 4 }),
        "effect.apply" => json!({ "id": "gaussian_blur", "params": { "radius": 2 } }),
        _ => json!({}),
    }
}

#[test]
fn tool_schemas_are_valid() {
    let tools = list_tools();
    let mut names = HashSet::new();
    for t in &tools {
        assert!(names.insert(t.name), "duplicate tool {}", t.name);
        let (area, verb) = t.name.split_once('.').expect("area.verb");
        assert!(!area.is_empty() && !verb.is_empty() && !verb.contains('.'));
        assert!(!t.description.is_empty(), "{} has no description", t.name);
        let s = &t.input_schema;
        assert_eq!(s["type"], "object", "{}", t.name);
        let props = s["properties"].as_object().expect("properties");
        for r in s["required"].as_array().expect("required") {
            assert!(
                props.contains_key(r.as_str().expect("str")),
                "{}: {r}",
                t.name
            );
        }
        assert!(props.contains_key("doc"), "{} lacks `doc`", t.name);
        schema::validate(s, &example(t.name))
            .unwrap_or_else(|e| panic!("example for {} invalid: {e}", t.name));
    }
    for name in EXPECTED_TOOLS {
        assert!(names.contains(name), "missing tool {name}");
    }
}

#[test]
fn doc_new_creates_active_document_and_writes_nothing() {
    let mut s = S::new();
    let out = s.ok(
        "doc.new",
        json!({ "mode": "pixel", "width": 24, "height": 12 }),
    );
    assert_eq!(out["doc"], "doc-1");
    assert_eq!(
        (out["width"].as_u64(), out["height"].as_u64()),
        (Some(24), Some(12))
    );
    assert_eq!(s.ws.active(), Some("doc-1"));
    assert_eq!(s.doc("doc-1").sprite.width, 24);
    assert!(s.host.files.is_empty());
    let second = s.new_doc(4, 4);
    assert_eq!(second, "doc-2");
    assert_eq!(s.ws.active(), Some("doc-2"));
}

#[test]
fn doc_new_palette_is_preset_or_db32() {
    let mut s = S::new();
    s.ok(
        "doc.new",
        json!({ "mode": "pixel", "width": 4, "height": 4, "palette": "gameboy" }),
    );
    assert_eq!(
        s.doc("doc-1").sprite.palette,
        Palette::preset(PalettePreset::GameBoy)
    );
    s.new_doc(4, 4);
    assert_eq!(s.doc("doc-2").sprite.palette, Palette::db32());
}

#[test]
fn doc_new_out_of_range_size_fails_without_adding_a_document() {
    let mut s = S::new();
    let e = s.err(
        "doc.new",
        json!({ "mode": "pixel", "width": 5000, "height": 4 }),
    );
    assert!(e.contains("width must be 1–4096"), "{e}");
    let e = s.err(
        "doc.new",
        json!({ "mode": "pixel", "width": 4, "height": 0 }),
    );
    assert!(e.contains("height must be 1–4096"), "{e}");
    assert!(s.ws.is_empty());
}

#[test]
fn tools_act_on_active_doc_or_named_doc_only() {
    let mut s = S::new();
    let a = s.new_doc(4, 4);
    let b = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    assert_eq!(super::px(&s.cel_rgba(&b, 0, 0), 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(super::px(&s.cel_rgba(&a, 0, 0), 4, 0, 0), [0, 0, 0, 0]);
    let b_before = s.state(&b);
    s.ok(
        "draw.pixels",
        json!({ "doc": a, "pixels": [{ "x": 1, "y": 1, "color": "#00ff00" }] }),
    );
    assert_eq!(super::px(&s.cel_rgba(&a, 0, 0), 4, 1, 1), [0, 255, 0, 255]);
    assert_eq!(s.state(&b), b_before);
}

#[test]
fn unknown_doc_or_no_open_doc_fails_and_changes_nothing() {
    let mut s = S::new();
    let e = s.err("draw.pixels", json!({ "pixels": [] }));
    assert!(e.contains("no document is open"), "{e}");
    let id = s.new_doc(4, 4);
    let before = s.state(&id);
    let e = s.err("layer.add", json!({ "doc": "doc-9" }));
    assert!(e.contains("doc-9"), "{e}");
    assert_eq!(s.state(&id), before);
    assert!(matches!(
        s.call("doc.info", json!({ "doc": "doc-9" })),
        Err(AgentError::Tool(_))
    ));
}

#[test]
fn doc_info_and_list_report_fields_and_dirty_state() {
    let mut s = S::new();
    let id = s.new_doc(8, 6);
    let info = s.ok("doc.info", json!({}));
    for key in [
        "width",
        "height",
        "layers",
        "frames",
        "tags",
        "palette_size",
        "active_layer",
        "active_frame",
        "dirty",
    ] {
        assert!(info.get(key).is_some(), "missing {key}");
    }
    assert_eq!(info["layers"], 1);
    assert_eq!(info["palette_size"], 32);
    assert_eq!(info["active_layer"], 0);
    assert_eq!(info["dirty"], false);
    s.ok("frame.add", json!({}));
    let list = s.ok("doc.list", json!({}));
    let d = &list["documents"][0];
    assert_eq!(
        (d["doc"].as_str(), d["frames"].as_u64()),
        (Some(id.as_str()), Some(2))
    );
    assert_eq!(d["dirty"], true);
    assert_eq!(d["active_frame"], 1);
    s.ok("doc.save", json!({ "path": "a.aseprite" }));
    assert_eq!(s.ok("doc.info", json!({}))["dirty"], false);
}

#[test]
fn doc_close_removes_document_and_later_calls_fail() {
    let mut s = S::new();
    let a = s.new_doc(4, 4);
    let b = s.new_doc(4, 4);
    s.ok("doc.close", json!({ "doc": b }));
    let list = s.ok("doc.list", json!({}));
    assert_eq!(list["documents"].as_array().map(Vec::len), Some(1));
    assert_eq!(list["active"], json!(a));
    let e = s.err("doc.info", json!({ "doc": b }));
    assert!(e.contains(&b), "{e}");
}

/// Every mutating tool: one undo restores the exact prior state, one redo
/// the exact post state.
#[test]
fn every_mutating_call_is_exactly_one_undo_step() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let calls: Vec<(&str, Value)> = vec![
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }, { "x": 2, "y": 1, "color": "#00ff00" }, { "x": 1, "y": 1, "color": "#0000ff" }] }),
        ),
        ("layer.add", json!({ "name": "G", "kind": "group" })),
        ("layer.add", json!({ "name": "Child", "parent": 1 })),
        (
            "layer.set",
            json!({ "layer": 2, "name": "Renamed", "visible": false, "opacity": 10, "blend": "screen", "locked": true }),
        ),
        (
            "layer.set",
            json!({ "layer": 2, "locked": false, "visible": true }),
        ),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 0, "x1": 7, "y1": 7, "color": "#ffffff" }),
        ),
        ("frame.add", json!({ "clone_from": 0, "duration_ms": 40 })),
        (
            "frame.set_duration",
            json!({ "frame": 0, "duration_ms": 70 }),
        ),
        ("tag.add", json!({ "name": "walk", "from": 0, "to": 1 })),
        (
            "draw.rect",
            json!({ "x0": 1, "y0": 1, "x1": 5, "y1": 4, "color": "#123456", "filled": true }),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 0, "y0": 0, "x1": 7, "y1": 5, "color": "#654321" }),
        ),
        ("draw.fill", json!({ "x": 0, "y": 7, "color": "#abcdef" })),
        ("effect.apply", json!({ "id": "invert" })),
        (
            "draw.clear",
            json!({ "region": { "x": 0, "y": 0, "width": 3, "height": 3 } }),
        ),
        ("palette.set", json!({ "entries": ["#000000", "#ffffff"] })),
        ("palette.load_preset", json!({ "preset": "pico8" })),
        ("tag.remove", json!({ "tag": 0 })),
        ("frame.remove", json!({ "frame": 1 })),
        ("layer.remove", json!({ "layer": 1 })),
    ];
    for (name, args) in calls {
        let before = s.state(&id);
        let depth = s.doc(&id).bus.undo_depth();
        s.ok(name, args);
        let after = s.state(&id);
        assert_ne!(before, after, "{name} changed nothing");
        assert_eq!(s.doc(&id).bus.undo_depth(), depth + 1, "{name}");
        s.ok("doc.undo", json!({}));
        assert_eq!(s.state(&id), before, "undo of {name}");
        s.ok("doc.redo", json!({}));
        assert_eq!(s.state(&id), after, "redo of {name}");
    }
}

#[test]
fn draw_pixels_empty_batch_records_no_undo_step() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok("draw.pixels", json!({ "pixels": [] }));
    assert_eq!(s.doc(&id).bus.undo_depth(), 0);
    assert!(!s.doc(&id).dirty);
}

#[test]
fn undo_and_redo_report_depths() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let out = s.ok("doc.undo", json!({}));
    assert_eq!(
        (out["undo_depth"].as_u64(), out["redo_depth"].as_u64()),
        (Some(0), Some(0))
    );
    s.ok("frame.add", json!({}));
    s.ok("frame.add", json!({}));
    let out = s.ok("doc.undo", json!({}));
    assert_eq!(
        (out["undo_depth"].as_u64(), out["redo_depth"].as_u64()),
        (Some(1), Some(1))
    );
    let out = s.ok("doc.redo", json!({}));
    assert_eq!(
        (out["undo_depth"].as_u64(), out["redo_depth"].as_u64()),
        (Some(2), Some(0))
    );
}

/// Same layers, frames, tags, palette and pixels (the file adds header
/// metadata such as the pixel ratio and grid).
#[track_caller]
fn assert_same_content(a: &(Sprite, CelMap), b: &(Sprite, CelMap)) {
    assert_eq!(a.0.layers, b.0.layers);
    assert_eq!(a.0.frames, b.0.frames);
    assert_eq!(a.0.tags, b.0.tags);
    assert_eq!(a.0.palette, b.0.palette);
    assert_eq!(a.1, b.1);
}

fn sample_aseprite(s: &mut S) -> Vec<u8> {
    s.new_doc(6, 5);
    s.ok(
        "draw.rect",
        json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 2, "color": "#ff8800", "filled": true }),
    );
    s.ok("layer.add", json!({ "name": "Top" }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 5, "y": 4, "color": "#00ff0080" }] }),
    );
    s.ok("frame.add", json!({ "clone_from": 0, "duration_ms": 50 }));
    s.ok(
        "tag.add",
        json!({ "name": "idle", "from": 0, "to": 1, "direction": "reverse" }),
    );
    s.ok("palette.load_preset", json!({ "preset": "db16" }));
    s.ok("doc.save", json!({ "path": "art/hero.aseprite" }));
    s.host.files["art/hero.aseprite"].clone()
}

#[test]
fn doc_open_matches_what_the_editor_reads_from_the_file() {
    let mut s = S::new();
    let bytes = sample_aseprite(&mut s);
    let out = s.ok("doc.open", json!({ "path": "art/hero.aseprite" }));
    let id = out["doc"].as_str().expect("id").to_owned();
    assert_eq!(s.ws.active(), Some(id.as_str()));
    assert_eq!(out["layers"], 2);
    let editor = read_aseprite(&bytes).expect("editor read");
    assert_eq!(s.state(&id), (editor.sprite, editor.cels));
}

#[test]
fn doc_open_corrupt_file_fails_naming_it() {
    let mut s = S::new();
    s.host.files.insert("bad.aseprite".into(), b"nope".to_vec());
    let e = s.err("doc.open", json!({ "path": "bad.aseprite" }));
    assert!(e.contains("bad.aseprite"), "{e}");
    let e = s.err("doc.open", json!({ "path": "pic.png" }));
    assert!(e.contains("pic.png"), "{e}");
    assert!(s.ws.is_empty());
}

#[test]
fn doc_save_round_trips_and_defaults_to_the_document_file() {
    let mut s = S::new();
    let bytes = sample_aseprite(&mut s);
    let saved = read_aseprite(&bytes).expect("read back");
    assert_same_content(&s.state("doc-1"), &(saved.sprite, saved.cels));
    // Without path: back to the same file.
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 4, "color": "#ffffff" }] }),
    );
    s.host.files.clear();
    let out = s.ok("doc.save", json!({}));
    assert_eq!(out["path"], "art/hero.aseprite");
    let again = read_aseprite(&s.host.files["art/hero.aseprite"]).expect("read");
    assert_same_content(&s.state("doc-1"), &(again.sprite, again.cels));
    // Opened documents save to the file they came from.
    s.ok("doc.open", json!({ "path": "art/hero.aseprite" }));
    s.host.files.clear();
    s.ok("doc.save", json!({}));
    assert!(s.host.files.contains_key("art/hero.aseprite"));
}

#[test]
fn doc_save_without_path_on_new_document_fails_and_writes_nothing() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let e = s.err("doc.save", json!({}));
    assert!(e.contains("`path`"), "{e}");
    assert!(s.host.files.is_empty());
}

#[test]
fn export_png_equals_preview_without_grid() {
    let mut s = S::new();
    s.new_doc(5, 3);
    s.ok(
        "draw.line",
        json!({ "x0": 0, "y0": 0, "x1": 4, "y1": 2, "color": "#ff00ff" }),
    );
    s.ok("export.png", json!({ "path": "out.png", "scale": 3 }));
    let file = import_png(&s.host.files["out.png"]).expect("png");
    let preview = s
        .call("doc.preview", json!({ "scale": 3 }))
        .expect("preview");
    let shown = import_png(&preview.image_png.expect("image")).expect("png");
    assert_eq!((file.width, file.height), (15, 9));
    assert_eq!(file.rgba, shown.rgba);
}

#[test]
fn export_sheet_lays_out_frames_and_writes_sheet_json() {
    let mut s = S::new();
    let id = s.new_doc(4, 2);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 0, "color": "#00ff00" }] }),
    );
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 2, "y": 0, "color": "#0000ff" }] }),
    );
    s.ok("tag.add", json!({ "name": "b", "from": 1, "to": 2 }));
    s.ok(
        "export.sheet",
        json!({ "columns": 2, "path": "out/all.png", "json": "out/all.json" }),
    );
    let sheet = import_png(&s.host.files["out/all.png"]).expect("png");
    assert_eq!((sheet.width, sheet.height), (8, 4));
    let (sprite, cels) = s.state(&id);
    let reference = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("atlas");
    assert_eq!(sheet.rgba, import_png(&reference.png).expect("png").rgba);
    // Frame 2 sits in row 1, column 0.
    assert_eq!(super::px(&sheet.rgba, 8, 2, 2), [0, 0, 255, 255]);
    let data: Value = serde_json::from_slice(&s.host.files["out/all.json"]).expect("sheet json");
    assert_eq!(data["meta"]["image"], "all.png");
    assert_eq!(
        String::from_utf8(s.host.files["out/all.json"].clone()).expect("utf8"),
        aseprite_sheet_json(&sprite, &reference.manifest, "all.png")
    );
    // Tag only, scaled.
    s.ok(
        "export.sheet",
        json!({ "columns": 1, "tag": "b", "scale": 2, "path": "b.png" }),
    );
    let b = import_png(&s.host.files["b.png"]).expect("png");
    assert_eq!((b.width, b.height), (8, 8));
    assert_eq!(super::px(&b.rgba, 8, 2, 0), [0, 255, 0, 255]);
}
