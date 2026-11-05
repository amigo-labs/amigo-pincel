//! Tags, palette entries, palette files and `doc.info` (mcp-parity-structure
//! AC-02, AC-14 … AC-22, AC-24, AC-25, AC-27, AC-30).

use pincel_core::{
    ColorMode, Palette, PaletteEntry, Rgba, palette_to_png, read_gpl, read_hex, read_jasc_pal,
    read_palette,
};
use serde_json::{Value, json};

use super::S;
use super::structure::colors;

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// A Pixel document with 4 frames and two tags.
fn tagged(s: &mut S) -> String {
    let id = s.new_doc(4, 4);
    for _ in 0..3 {
        s.ok("frame.add", json!({}));
    }
    s.ok("tag.add", json!({ "name": "walk", "from": 0, "to": 1 }));
    s.ok(
        "tag.add",
        json!({ "name": "jump", "from": 2, "to": 3, "direction": "reverse" }),
    );
    id
}

fn set_mode(s: &mut S, id: &str, mode: ColorMode) {
    s.ws.get_mut(id).expect("doc").sprite.color_mode = mode;
}

#[test]
fn tag_add_reports_the_given_color_or_the_timelines_next_color() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let defaults = [
        "#5B8DEF", "#E0A83C", "#58B368", "#D0607A", "#9A6AD8", "#3CB7C4", "#5B8DEF",
    ];
    for (n, want) in defaults.iter().enumerate() {
        let out = s.ok(
            "tag.add",
            json!({ "name": format!("t{n}"), "from": 0, "to": 0 }),
        );
        assert_eq!(out["tags"][n]["color"], *want, "tag {n}");
    }
    let out = s.ok(
        "tag.add",
        json!({ "name": "c", "from": 0, "to": 0, "color": "#112233" }),
    );
    assert_eq!(out["tags"][7]["color"], "#112233");
    let list = s.ok("tag.list", json!({}))["tags"].clone();
    assert!(
        list.as_array()
            .expect("tags")
            .iter()
            .all(|t| t["color"].is_string())
    );
    let e = s.err(
        "tag.add",
        json!({ "name": "c", "from": 0, "to": 0, "color": "red" }),
    );
    assert!(e.contains("`color`"), "{e}");
}

#[test]
fn tag_set_changes_exactly_the_given_fields() {
    let mut s = S::new();
    tagged(&mut s);
    let before = s.ok("tag.list", json!({}))["tags"].clone();
    for (args, field, value) in [
        (json!({ "tag": 0, "name": "run" }), "name", json!("run")),
        (
            json!({ "tag": 0, "direction": "pingpong" }),
            "direction",
            json!("pingpong"),
        ),
    ] {
        let mut want = s.ok("tag.list", json!({}))["tags"].clone();
        want[0][field] = value;
        s.ok("tag.set", args);
        assert_eq!(s.ok("tag.list", json!({}))["tags"], want);
    }
    s.ok("tag.set", json!({ "tag": 1, "from": 1, "to": 2 }));
    let tags = s.ok("tag.list", json!({}))["tags"].clone();
    let mut want = before.clone();
    want[0]["name"] = json!("run");
    want[0]["direction"] = json!("pingpong");
    want[1]["from"] = json!(1);
    want[1]["to"] = json!(2);
    assert_eq!(tags, want);
    // A name another tag has is allowed, as in the editor.
    s.ok("tag.set", json!({ "tag": 1, "name": "run" }));
}

#[test]
fn tag_set_with_bad_arguments_fails_naming_the_argument() {
    let mut s = S::new();
    let id = tagged(&mut s);
    for (args, needle) in [
        (json!({ "tag": 7, "name": "x" }), "`tag`"),
        (json!({ "tag": 0 }), "at least one"),
        (json!({ "tag": 0, "name": "" }), "`name`"),
        (json!({ "tag": 0, "from": 2, "to": 1 }), "`from`"),
        (json!({ "tag": 0, "from": 0, "to": 4 }), "`to`"),
        (json!({ "tag": 0, "from": 1 }), "`from` needs `to`"),
        (json!({ "tag": 0, "to": 1 }), "`to` needs `from`"),
    ] {
        let before = s.state(&id);
        let e = s.err("tag.set", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
        assert_eq!(s.state(&id), before, "{args}");
    }
    let e = s.err("tag.set", json!({ "tag": 7, "name": "x" }));
    assert!(e.contains('7'), "{e}");
}

#[test]
fn palette_get_reports_entry_names() {
    let mut s = S::new();
    s.new_doc(2, 2);
    s.host.files.insert(
        "named.gpl".into(),
        b"GIMP Palette\nName: test\n255 0 0 Red\n0 255 0\n".to_vec(),
    );
    s.ok("palette.import", json!({ "path": "named.gpl" }));
    let entries = s.ok("palette.get", json!({}))["entries"].clone();
    assert_eq!(
        entries,
        json!([
            { "color": "#FF0000FF", "name": "Red" },
            { "color": "#00FF00FF", "name": "" },
        ])
    );
}

#[test]
fn palette_entry_tools_change_exactly_that_entry() {
    let mut s = S::new();
    s.new_doc(2, 2);
    s.ok(
        "palette.set",
        json!({ "entries": ["#000000", "#111111", "#222222", "#333333"] }),
    );
    s.ok(
        "palette.set_entry",
        json!({ "index": 1, "color": "#ff000080" }),
    );
    assert_eq!(
        colors(&s.ok("palette.get", json!({}))),
        ["#000000FF", "#FF000080", "#222222FF", "#333333FF"]
    );
    let out = s.ok("palette.add_entry", json!({ "color": "#444444" }));
    assert_eq!(out["index"], 4);
    assert_eq!(colors(&out)[4], "#444444FF");
    s.ok("palette.remove_entry", json!({ "index": 0 }));
    assert_eq!(
        colors(&s.ok("palette.get", json!({}))),
        ["#FF000080", "#222222FF", "#333333FF", "#444444FF"]
    );
    s.ok("palette.move_entry", json!({ "from": 0, "to": 2 }));
    assert_eq!(
        colors(&s.ok("palette.get", json!({}))),
        ["#222222FF", "#333333FF", "#FF000080", "#444444FF"]
    );
    s.ok("palette.move_entry", json!({ "from": 3, "to": 0 }));
    assert_eq!(
        colors(&s.ok("palette.get", json!({}))),
        ["#444444FF", "#222222FF", "#333333FF", "#FF000080"]
    );
}

#[test]
fn palette_tools_refuse_a_full_palette_and_non_rgba_documents() {
    let mut s = S::new();
    let id = s.new_doc(2, 2);
    let full: Vec<String> = (0..256).map(|i| format!("#{i:02x}0000")).collect();
    s.ok("palette.set", json!({ "entries": full }));
    let before = s.state(&id);
    let e = s.err("palette.add_entry", json!({ "color": "#ffffff" }));
    assert!(e.contains("full") && e.contains("256"), "{e}");
    assert_eq!(s.state(&id), before);

    s.host.files.insert("p.hex".into(), b"ff0000\n".to_vec());
    for mode in [
        ColorMode::Indexed {
            transparent_index: 0,
        },
        ColorMode::Grayscale,
    ] {
        set_mode(&mut s, &id, mode);
        for (tool, args) in [
            ("palette.remove_entry", json!({ "index": 0 })),
            ("palette.move_entry", json!({ "from": 0, "to": 1 })),
            ("palette.import", json!({ "path": "p.hex" })),
        ] {
            let before = s.state(&id);
            let e = s.err(tool, args);
            assert!(e.contains("rgba") && e.contains("position"), "{tool}: {e}");
            assert_eq!(s.state(&id), before, "{tool}");
        }
        // Recoloring keeps every pixel's index valid.
        s.ok(
            "palette.set_entry",
            json!({ "index": 0, "color": "#123456" }),
        );
    }
}

fn png_palette() -> Vec<u8> {
    // Two rows: red, green / green, blue → distinct colors in reading order.
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 255, 0, 255, 0, 0, 255, 255,
    ];
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 2, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("header");
        w.write_image_data(&rgba).expect("data");
    }
    out
}

#[test]
fn palette_import_reads_all_four_formats_whatever_the_extension() {
    let files: Vec<(&str, Vec<u8>)> = vec![
        (
            "a.txt",
            b"GIMP Palette\n10 20 30 Dark\n200 100 0\n".to_vec(),
        ),
        ("b.gpl", b"; lospec\nff0000\n00ff00\n0000ff\n".to_vec()),
        ("c.hex", b"JASC-PAL\n0100\n2\n1 2 3\n4 5 6\n".to_vec()),
        ("d.pal", png_palette()),
    ];
    for (path, bytes) in files {
        let mut s = S::new();
        let id = s.new_doc(2, 2);
        s.host.files.insert(path.into(), bytes.clone());
        let editor = read_palette(&bytes).expect("editor import");
        let out = s.ok("palette.import", json!({ "path": path }));
        assert_eq!(out["colors"], editor.len(), "{path}");
        assert_eq!(s.doc(&id).sprite.palette, editor, "{path}");
    }
}

#[test]
fn palette_import_of_a_bad_file_fails_with_the_reason() {
    let mut s = S::new();
    let id = s.new_doc(2, 2);
    let too_many: String = (0..257).map(|i| format!("{:06x}\n", i)).collect();
    s.host
        .files
        .insert("bad.hex".into(), b"not a palette\n".to_vec());
    s.host
        .files
        .insert("empty.gpl".into(), b"GIMP Palette\n".to_vec());
    s.host.files.insert("big.hex".into(), too_many.into_bytes());
    for (path, needle) in [
        ("missing.gpl", "missing.gpl"),
        ("bad.hex", "not a colour"),
        ("empty.gpl", "no colours"),
        ("big.hex", "more than 256"),
    ] {
        let before = s.state(&id);
        let e = s.err("palette.import", json!({ "path": path }));
        assert!(e.contains(needle), "{path}: {e}");
        assert_eq!(s.state(&id), before, "{path}");
    }
}

#[test]
fn palette_export_writes_each_format_and_imports_back() {
    let mut s = S::new();
    s.new_doc(2, 2);
    s.ok(
        "palette.set",
        json!({ "entries": ["#ff0000", "#00ff0080", "#0000ff"] }),
    );
    let opaque = Palette::from_entries(vec![
        PaletteEntry::new(Rgba::new(255, 0, 0, 255)),
        PaletteEntry::new(Rgba::new(0, 255, 0, 255)),
        PaletteEntry::new(Rgba::new(0, 0, 255, 255)),
    ]);
    let rgb = |p: &Palette| -> Vec<Rgba> { p.colors.iter().map(|e| e.rgba).collect() };
    s.ok(
        "palette.export",
        json!({ "path": "out.png", "format": "gpl" }),
    );
    let gpl = String::from_utf8(s.host.files["out.png"].clone()).expect("utf8");
    assert!(gpl.starts_with("GIMP Palette\nName: palette\n"), "{gpl}");
    assert_eq!(rgb(&read_gpl(&gpl).expect("gpl")), rgb(&opaque));
    s.ok(
        "palette.export",
        json!({ "path": "n.gpl", "format": "gpl", "name": "Hero" }),
    );
    assert!(String::from_utf8_lossy(&s.host.files["n.gpl"]).contains("Name: Hero"));
    s.ok(
        "palette.export",
        json!({ "path": "p.gpl", "format": "hex" }),
    );
    let hex = String::from_utf8(s.host.files["p.gpl"].clone()).expect("utf8");
    assert_eq!(rgb(&read_hex(&hex).expect("hex")), rgb(&opaque));
    s.ok(
        "palette.export",
        json!({ "path": "p.pal", "format": "pal" }),
    );
    let pal = String::from_utf8(s.host.files["p.pal"].clone()).expect("utf8");
    assert_eq!(rgb(&read_jasc_pal(&pal).expect("pal")), rgb(&opaque));
    // PNG keeps alpha; one row of 8×8 swatches by default.
    s.ok(
        "palette.export",
        json!({ "path": "p.hex", "format": "png" }),
    );
    let png = s.host.files["p.hex"].clone();
    let current = s.doc("doc-1").sprite.palette.clone();
    assert_eq!(png, palette_to_png(&current, 8).expect("png"));
    assert_eq!(rgb(&read_palette(&png).expect("png")), rgb(&current));
    s.ok(
        "palette.export",
        json!({ "path": "c.png", "format": "png", "cell": 3 }),
    );
    assert_eq!(
        s.host.files["c.png"],
        palette_to_png(&current, 3).expect("png")
    );
    // Importing an export yields the palette again.
    s.ok("palette.import", json!({ "path": "p.hex" }));
    assert_eq!(rgb(&s.doc("doc-1").sprite.palette), rgb(&current));
}

#[test]
fn palette_export_with_bad_arguments_writes_nothing() {
    let mut s = S::new();
    let id = s.new_doc(2, 2);
    for args in [
        json!({ "path": "a", "format": "aco" }),
        json!({ "path": "a", "format": "png", "cell": 0 }),
        json!({ "path": "a", "format": "png", "cell": 65 }),
    ] {
        let e = s.err("palette.export", args.clone());
        assert!(e.contains("format") || e.contains("cell"), "{args}: {e}");
    }
    s.ws.get_mut(&id).expect("doc").sprite.palette = Palette::from_entries(Vec::new());
    let e = s.err("palette.export", json!({ "path": "a", "format": "png" }));
    assert!(e.contains("empty palette"), "{e}");
    assert!(s.host.files.is_empty());
}

#[test]
fn doc_info_reports_color_mode_and_selection_bounds() {
    let mut s = S::new();
    let px = s.new_doc(8, 6);
    let info = s.ok("doc.info", json!({}));
    assert_eq!(info["color_mode"], "rgba");
    assert_eq!(info["selection"], Value::Null);
    s.ok(
        "selection.set_rect",
        json!({ "x": -2, "y": 3, "width": 5, "height": 10 }),
    );
    assert_eq!(
        s.ok("doc.info", json!({}))["selection"],
        json!({ "x": 0, "y": 3, "w": 3, "h": 3 })
    );
    set_mode(
        &mut s,
        &px,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    );
    assert_eq!(s.ok("doc.info", json!({}))["color_mode"], "indexed");
    set_mode(&mut s, &px, ColorMode::Grayscale);
    assert_eq!(s.ok("doc.info", json!({}))["color_mode"], "grayscale");

    s.new_image(10, 10);
    s.ok(
        "select.ellipse",
        json!({ "x": 6, "y": -2, "width": 8, "height": 6, "feather": 1 }),
    );
    let info = s.ok("doc.info", json!({}));
    assert_eq!(info["color_mode"], "rgba");
    let doc_id = info["doc"].as_str().expect("id").to_owned();
    let bbox = s
        .image(&doc_id)
        .document()
        .selection
        .as_ref()
        .and_then(|m| m.bounding_box())
        .expect("selection");
    assert_eq!(
        info["selection"],
        json!({ "x": bbox.x, "y": bbox.y, "w": bbox.w, "h": bbox.h })
    );
    assert_eq!(info["selection"]["y"], 0);
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(list[0]["selection"]["w"], 3);
    assert_eq!(list[1]["selection"], info["selection"]);
    assert_eq!(list[1]["color_mode"], "rgba");
}

#[test]
fn tag_and_palette_edits_record_one_undo_step_and_none_without_a_change() {
    let mut s = S::new();
    tagged(&mut s);
    s.host
        .files
        .insert("p.hex".into(), b"ff0000\n00ff00\n".to_vec());
    for (tool, args) in [
        (
            "tag.set",
            json!({ "tag": 0, "name": "run", "from": 0, "to": 2 }),
        ),
        (
            "palette.set_entry",
            json!({ "index": 0, "color": "#ff00ff" }),
        ),
        ("palette.add_entry", json!({ "color": "#ff00ff" })),
        ("palette.remove_entry", json!({ "index": 1 })),
        ("palette.move_entry", json!({ "from": 0, "to": 2 })),
        ("palette.import", json!({ "path": "p.hex" })),
    ] {
        let depth = undo_depth(&mut s);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool} {args}");
    }
    for (tool, args) in [
        ("tag.set", json!({ "tag": 0, "name": "run" })),
        (
            "palette.set_entry",
            json!({ "index": 0, "color": "#ff0000" }),
        ),
        ("palette.move_entry", json!({ "from": 1, "to": 1 })),
        (
            "palette.export",
            json!({ "path": "x.gpl", "format": "gpl" }),
        ),
    ] {
        let depth = undo_depth(&mut s);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth, "{tool} {args}");
    }
}

#[test]
fn tag_and_palette_tools_naming_a_missing_index_fail_with_argument_and_value() {
    let mut s = S::new();
    let id = tagged(&mut s);
    for (tool, args, arg) in [
        ("tag.set", json!({ "tag": 9, "name": "x" }), "tag"),
        ("tag.remove", json!({ "tag": 9 }), "tag"),
        (
            "palette.set_entry",
            json!({ "index": 99, "color": "#000000" }),
            "index",
        ),
        ("palette.remove_entry", json!({ "index": 99 }), "index"),
        ("palette.move_entry", json!({ "from": 99, "to": 0 }), "from"),
        ("palette.move_entry", json!({ "from": 0, "to": 99 }), "to"),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("9"),
            "{tool}: {e}"
        );
        assert_eq!(s.state(&id), before, "{tool}");
    }
}

#[test]
fn tag_and_palette_tools_on_an_image_document_fail_naming_the_mode() {
    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.host.files.insert("p.hex".into(), b"ff0000\n".to_vec());
    for (tool, args) in [
        ("tag.set", json!({ "tag": 0, "name": "x" })),
        (
            "palette.set_entry",
            json!({ "index": 0, "color": "#000000" }),
        ),
        ("palette.add_entry", json!({ "color": "#000000" })),
        ("palette.remove_entry", json!({ "index": 0 })),
        ("palette.move_entry", json!({ "from": 0, "to": 1 })),
        ("palette.import", json!({ "path": "p.hex" })),
        (
            "palette.export",
            json!({ "path": "a.gpl", "format": "gpl" }),
        ),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args);
        assert!(e.contains("Image document"), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
    assert!(!s.host.files.contains_key("a.gpl"));
}
