//! Image save formats and BMP / TIFF export (mcp-parity-export AC-01,
//! AC-14 … AC-20, AC-28). The other tools of the spec are tested in
//! `export_open`, `export_anim` and `export_svg`.

use pincel_image_core::codec::{
    decode, to_bmp_bytes, to_jpeg_bytes, to_ora_bytes, to_png_bytes, to_tiff_bytes, to_webp_bytes,
};
use pincel_image_core::{ImageBuffer, compose};
use serde_json::json;

use super::S;

/// An opaque 6×4 test pattern.
pub(super) fn pattern() -> ImageBuffer {
    let mut data = Vec::new();
    for y in 0..4u32 {
        for x in 0..6u32 {
            data.extend_from_slice(&[(x * 40) as u8, (y * 60) as u8, 90, 255]);
        }
    }
    ImageBuffer::from_raw(6, 4, data).expect("buffer")
}

pub(super) fn composite(s: &S, id: &str) -> ImageBuffer {
    compose(s.image(id).document().layers())
}

pub(super) fn opened(s: &mut S, name: &str, bytes: Vec<u8>) -> String {
    s.host.files.insert(name.to_owned(), bytes);
    s.ok("doc.open", json!({ "path": name }))["doc"]
        .as_str()
        .expect("id")
        .to_owned()
}

pub(super) const SAMPLE_PSD: &[u8] =
    include_bytes!("../../../pincel-image-core/tests/fixtures/sample.psd");

#[test]
fn export_tools_are_listed_and_open_save_take_the_new_arguments() {
    let tools = crate::list_tools();
    for name in [
        "export.svg",
        "export.apng",
        "export.gif",
        "export.bmp",
        "export.tiff",
    ] {
        assert!(tools.iter().any(|t| t.name == name), "{name}");
    }
    let props = |name: &str| {
        tools
            .iter()
            .find(|t| t.name == name)
            .expect("tool")
            .input_schema["properties"]
            .clone()
    };
    assert!(props("doc.open").get("mode").is_some());
    for key in ["flatten", "quality"] {
        assert!(props("doc.save").get(key).is_some(), "{key}");
    }
}

#[test]
fn doc_save_without_path_writes_back_the_opened_format() {
    for (name, bytes) in [
        ("a.png", to_png_bytes(&pattern(), 6).expect("png")),
        ("b.jpg", to_jpeg_bytes(&pattern(), 90).expect("jpg")),
        ("c.webp", to_webp_bytes(&pattern()).expect("webp")),
        ("d.bmp", to_bmp_bytes(&pattern()).expect("bmp")),
        ("e.tif", to_tiff_bytes(&pattern()).expect("tif")),
    ] {
        let mut s = S::new();
        let id = opened(&mut s, name, bytes);
        s.ok(
            "draw.fill",
            json!({ "x": 0, "y": 0, "color": "#ff0000", "tolerance": 0 }),
        );
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], true);
        let out = s.ok("doc.save", json!({}));
        assert_eq!(out["path"], name);
        let img = composite(&s, &id);
        let want = match name {
            "a.png" => to_png_bytes(&img, 6),
            "b.jpg" => to_jpeg_bytes(&img, 90),
            "c.webp" => to_webp_bytes(&img),
            "d.bmp" => to_bmp_bytes(&img),
            _ => to_tiff_bytes(&img),
        }
        .expect("encode");
        assert_eq!(s.host.files[name], want, "{name}");
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], false);
        assert_eq!(s.image(&id).document().layer_count(), 1);
        s.host.files.remove(name);
        s.ok("doc.save", json!({}));
        assert!(
            s.host.files.contains_key(name),
            "a second save writes the same file"
        );
    }
}

#[test]
fn a_lossy_flatten_needs_flatten_true_and_names_every_cause() {
    let mut s = S::new();
    let id = opened(&mut s, "a.png", to_png_bytes(&pattern(), 6).expect("png"));
    s.ok(
        "layer.add",
        json!({ "name": "Ink", "opacity": 128, "blend": "multiply" }),
    );
    s.ok("layer.add", json!({ "name": "Off", "visible": false }));
    let before = s.host.files["a.png"].clone();
    let e = s.err("doc.save", json!({}));
    for cause in [
        "3 layers",
        "hidden layer \"Off\"",
        "\"Ink\" at 50 % opacity",
        "\"Ink\" with blend mode Multiply",
    ] {
        assert!(e.contains(cause), "{cause}: {e}");
    }
    assert!(e.contains("flatten: true") && e.contains(".ora"), "{e}");
    assert_eq!(s.host.files["a.png"], before);
    s.ok("doc.save", json!({ "flatten": true }));
    assert_eq!(
        s.host.files["a.png"],
        to_png_bytes(&composite(&s, &id), 6).expect("png")
    );
    assert_eq!(
        s.image(&id).document().layer_count(),
        3,
        "the document keeps its layers"
    );
}

#[test]
fn doc_save_with_a_path_picks_the_format_by_extension() {
    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    for (path, check) in [
        ("p.PNG", "PNG"),
        ("q.jpeg", "JPEG"),
        ("r.webp", "WebP"),
        ("s.bmp", "BMP"),
        ("t.TIFF", "TIFF"),
        ("u.ora", "ORA"),
    ] {
        let out = s.ok("doc.save", json!({ "path": path }));
        assert_eq!(out["format"], check, "{path}");
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], false);
        if check != "ORA" {
            let decoded = decode(&s.host.files[path]).expect("decode");
            if check != "JPEG" {
                assert_eq!(decoded.data(), composite(&s, &id).data(), "{path}");
            }
        }
    }
    // The last path is the document's file now.
    s.host.files.clear();
    s.ok("doc.save", json!({}));
    assert!(s.host.files.contains_key("u.ora"));
    s.ok("doc.save", json!({ "path": "v.jpg" }));
    s.host.files.clear();
    s.ok("doc.save", json!({}));
    assert!(s.host.files.contains_key("v.jpg"));
}

#[test]
fn jpeg_quality_is_remembered_per_document() {
    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    s.ok("doc.save", json!({ "path": "a.jpg" }));
    assert_eq!(
        s.host.files["a.jpg"],
        to_jpeg_bytes(&composite(&s, &id), 90).expect("jpg")
    );
    s.ok("doc.save", json!({ "path": "a.jpg", "quality": 40 }));
    assert_eq!(
        s.host.files["a.jpg"],
        to_jpeg_bytes(&composite(&s, &id), 40).expect("jpg")
    );
    s.ok("doc.save", json!({}));
    assert_eq!(
        s.host.files["a.jpg"],
        to_jpeg_bytes(&composite(&s, &id), 40).expect("jpg")
    );
}

#[test]
fn bad_save_arguments_name_the_argument() {
    let mut s = S::new();
    s.new_image(4, 4);
    for q in [json!(0), json!(101), json!(50.5)] {
        let e = s.err("doc.save", json!({ "path": "a.jpg", "quality": q }));
        assert!(e.contains("quality"), "{e}");
    }
    s.new_doc(4, 4);
    for args in [
        json!({ "path": "a.aseprite", "flatten": true }),
        json!({ "path": "a.aseprite", "quality": 80 }),
    ] {
        let e = s.err("doc.save", args.clone());
        let key = args
            .as_object()
            .and_then(|o| o.keys().find(|k| *k != "path").cloned())
            .expect("the Image-only argument");
        assert!(
            e.contains(&format!("`{key}`")) && e.contains("Image documents only"),
            "{args}: {e}"
        );
    }
    assert!(s.host.files.is_empty());
}

#[test]
fn unwritable_formats_are_refused_listing_the_extensions() {
    let mut s = S::new();
    let gif = {
        let img = image::RgbaImage::from_raw(6, 4, pattern().data().to_vec()).expect("img");
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Gif)
            .expect("gif");
        out
    };
    opened(&mut s, "a.gif", gif);
    let e = s.err("doc.save", json!({}));
    assert!(e.contains("GIF") && e.contains(".ora, .png"), "{e}");
    s.ok("layer.add", json!({}));
    for path in ["x.gif", "x.psd", "x.xyz"] {
        let e = s.err("doc.save", json!({ "path": path }));
        assert!(e.contains(".ora") && e.contains(".tif"), "{path}: {e}");
    }
    assert_eq!(s.host.files.len(), 1);
}

/// Where a `.ora` path is suggested ahead of the extension list, as the
/// editor's Save As preselects `.ora` (AC-19).
fn suggests_ora_first(e: &str) -> bool {
    match (e.find("`.ora` path"), e.find("a path ending in")) {
        (Some(ora), Some(list)) => ora < list,
        _ => false,
    }
}

#[test]
fn doc_save_of_a_psd_source_or_layered_document_suggests_ora_first() {
    let mut s = S::new();
    opened(&mut s, "art.psd", SAMPLE_PSD.to_vec());
    s.ok("layer.flatten", json!({}));
    assert_eq!(s.ok("doc.info", json!({}))["layers"], 1);
    let e = s.err("doc.save", json!({}));
    assert!(
        e.contains("PSD file `art.psd`") && e.contains("cannot write"),
        "{e}"
    );
    assert!(
        e.contains(".ora, .png, .jpg / .jpeg, .webp, .bmp, .tif / .tiff"),
        "{e}"
    );
    assert!(suggests_ora_first(&e), "a PSD source, even one layer: {e}");
    // The same with a path of an extension doc.save does not write.
    for path in ["x.gif", "x.psd", "x.xyz"] {
        let e = s.err("doc.save", json!({ "path": path }));
        assert!(
            e.contains(&format!("cannot write `{path}`"))
                && e.contains(".ora, .png, .jpg / .jpeg, .webp, .bmp, .tif / .tiff"),
            "{e}"
        );
        assert!(suggests_ora_first(&e), "a PSD source, {path}: {e}");
    }

    let gif = {
        let img = image::RgbaImage::from_raw(6, 4, pattern().data().to_vec()).expect("img");
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Gif)
            .expect("gif");
        out
    };
    opened(&mut s, "a.gif", gif);
    let e = s.err("doc.save", json!({}));
    assert!(!suggests_ora_first(&e), "one layer from a GIF: {e}");
    let e = s.err("doc.save", json!({ "path": "x.gif" }));
    assert!(
        !suggests_ora_first(&e) && e.contains("cannot write `x.gif`"),
        "one layer from a GIF, a path: {e}"
    );
    s.ok("layer.add", json!({}));
    let e = s.err("doc.save", json!({}));
    assert!(suggests_ora_first(&e), "more than one layer: {e}");
    let e = s.err("doc.save", json!({ "path": "x.gif" }));
    assert!(suggests_ora_first(&e), "more than one layer, a path: {e}");
    assert_eq!(s.host.files.len(), 2, "nothing is written");
}

#[test]
fn export_bmp_and_tiff_write_the_composite_and_leave_the_document() {
    let mut s = S::new();
    let id = opened(&mut s, "a.png", to_png_bytes(&pattern(), 6).expect("png"));
    s.ok("layer.add", json!({}));
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff0080" }));
    let info = s.ok("doc.info", json!({}));
    s.ok("export.bmp", json!({ "path": "o.bmp" }));
    s.ok("export.tiff", json!({ "path": "o.tif" }));
    let img = composite(&s, &id);
    assert_eq!(s.host.files["o.bmp"], to_bmp_bytes(&img).expect("bmp"));
    assert_eq!(s.host.files["o.tif"], to_tiff_bytes(&img).expect("tif"));
    for path in ["o.tif", "o.bmp"] {
        assert_eq!(
            decode(&s.host.files[path]).expect("decode").data(),
            img.data(),
            "{path}"
        );
    }
    assert_eq!(s.ok("doc.info", json!({})), info);
    assert_eq!(info["dirty"], true);
    assert_eq!(
        s.image(&id).source,
        Some(("a.png".to_owned(), "PNG")),
        "the document's file and format stay"
    );
    assert_eq!(s.image(&id).path, None);
    s.host.files.clear();
    s.ok("doc.save", json!({ "flatten": true }));
    assert_eq!(
        s.host.files.keys().collect::<Vec<_>>(),
        ["a.png"],
        "a later save still writes the opened file"
    );
    assert_eq!(s.host.files["a.png"], to_png_bytes(&img, 6).expect("png"));

    let mut s = S::new();
    let id = s.new_doc(4, 4);
    for tool in ["export.bmp", "export.tiff"] {
        let before = s.state(&id);
        let e = s.err(tool, json!({ "path": "x" }));
        assert!(e.contains("Pixel document"), "{tool}: {e}");
        assert_eq!(s.state(&id), before);
    }
    assert!(s.host.files.is_empty());
}

/// The bytes `doc.save` must write for `format`, from the editor's codecs.
fn encoded(s: &S, id: &str, format: &str, quality: u8) -> Vec<u8> {
    let img = composite(s, id);
    match format {
        "PNG" => to_png_bytes(&img, 6),
        "JPEG" => to_jpeg_bytes(&img, quality),
        "WebP" => to_webp_bytes(&img),
        "BMP" => to_bmp_bytes(&img),
        "TIFF" => to_tiff_bytes(&img),
        _ => return to_ora_bytes(s.image(id).document(), 6).expect("ora"),
    }
    .expect("encode")
}

#[test]
fn doc_save_writes_each_format_and_makes_the_path_the_documents_file() {
    // AC-17: the editor's bytes per extension (any case), the dirty state
    // cleared, and a later save without `path` writing that file in that
    // format.
    let mut s = S::new();
    let id = s.new_image(6, 4);
    for (i, (path, format)) in [
        ("p.PNG", "PNG"),
        ("q.Jpeg", "JPEG"),
        ("q2.JPG", "JPEG"),
        ("r.WEBP", "WebP"),
        ("s.Bmp", "BMP"),
        ("t.TIFF", "TIFF"),
        ("t2.Tif", "TIFF"),
        ("u.ORA", "ORA"),
    ]
    .into_iter()
    .enumerate()
    {
        let color = format!("#{:02x}3366", i * 30);
        s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": color }));
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], true);
        let out = s.ok("doc.save", json!({ "path": path }));
        assert_eq!(out["format"], format, "{path}");
        assert_eq!(s.host.files[path], encoded(&s, &id, format, 90), "{path}");
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], false, "{path}");
        // The next save without `path` writes the same file and format.
        s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#ffffff" }));
        s.host.files.clear();
        s.ok("doc.save", json!({}));
        assert_eq!(
            s.host.files.keys().collect::<Vec<_>>(),
            [path],
            "{path}: the document's file"
        );
        assert_eq!(s.host.files[path], encoded(&s, &id, format, 90), "{path}");
    }
}

#[test]
fn doc_save_writes_back_the_detected_format_whatever_the_extension() {
    // AC-14 edge case: PNG bytes in `photo.jpg` are written back as PNG.
    let mut s = S::new();
    let id = opened(
        &mut s,
        "photo.jpg",
        to_png_bytes(&pattern(), 6).expect("png"),
    );
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#ff0000" }));
    let out = s.ok("doc.save", json!({}));
    assert_eq!(out["format"], "PNG");
    assert_eq!(s.host.files["photo.jpg"], encoded(&s, &id, "PNG", 90));
}

#[test]
fn a_lossy_flatten_names_each_cause_and_flatten_writes_the_visible_composite() {
    // AC-15: every hidden layer, every layer below 100 % and every
    // non-Normal blend is named; a single layer is lossy as well.
    let mut s = S::new();
    let id = opened(&mut s, "a.png", to_png_bytes(&pattern(), 6).expect("png"));
    for (name, args) in [
        ("A", json!({ "opacity": 128 })),
        ("B", json!({ "opacity": 64, "blend": "screen" })),
        ("C", json!({ "blend": "multiply" })),
        ("H1", json!({})),
        ("H2", json!({})),
    ] {
        let mut args = args;
        args["name"] = json!(name);
        let layer = s.ok("layer.add", args)["layer"].clone();
        s.ok(
            "draw.shape",
            json!({ "shape": "rect", "x0": 1, "y0": 1, "x1": 3, "y1": 2, "fill": "#00ff00" }),
        );
        if name.starts_with('H') {
            s.ok("layer.set", json!({ "layer": layer, "visible": false }));
        }
    }
    let before = s.host.files["a.png"].clone();
    let e = s.err("doc.save", json!({}));
    for cause in [
        "6 layers",
        "hidden layer \"H1\"",
        "hidden layer \"H2\"",
        "\"A\" at 50 % opacity",
        "\"B\" at 25 % opacity",
        "\"B\" with blend mode Screen",
        "\"C\" with blend mode Multiply",
    ] {
        assert!(e.contains(cause), "{cause}: {e}");
    }
    assert_eq!(s.host.files["a.png"], before, "nothing is written");

    // AC-16: `flatten: true` writes the visible composite (the hidden
    // layers' content stays out) and keeps every layer.
    let with_hidden = {
        let mut layers = s.image(&id).document().layers().to_vec();
        for l in &mut layers {
            l.visible = true;
        }
        compose(&layers)
    };
    assert_ne!(with_hidden.data(), composite(&s, &id).data());
    s.ok("doc.save", json!({ "flatten": true }));
    assert_eq!(s.host.files["a.png"], encoded(&s, &id, "PNG", 90));
    assert_eq!(s.image(&id).document().layer_count(), 6);

    for (args, cause) in [
        (json!({ "visible": false }), "hidden layer \"Background\""),
        (
            json!({ "opacity": 200 }),
            "layer \"Background\" at 78 % opacity",
        ),
        (
            json!({ "blend": "screen" }),
            "layer \"Background\" with blend mode Screen",
        ),
    ] {
        let mut s = S::new();
        opened(&mut s, "b.png", to_png_bytes(&pattern(), 6).expect("png"));
        let mut set = args.clone();
        set["layer"] = s.ok("layer.list", json!({}))["layers"][0]["id"].clone();
        s.ok("layer.set", set);
        let e = s.err("doc.save", json!({}));
        assert!(
            e.contains(&format!("would flatten the document ({cause});"))
                && e.contains("flatten: true"),
            "one layer: {e}"
        );
    }
}

#[test]
fn flatten_true_without_loss_writes_the_same_file() {
    // Edge case: `flatten: true` on a one-layer, opaque, Normal document.
    let mut s = S::new();
    opened(&mut s, "a.png", to_png_bytes(&pattern(), 6).expect("png"));
    s.ok("doc.save", json!({}));
    let plain = s.host.files["a.png"].clone();
    s.ok("doc.save", json!({ "flatten": true }));
    assert_eq!(s.host.files["a.png"], plain);
}

#[test]
fn jpeg_quality_applies_to_jpeg_only_and_per_document() {
    // AC-18: `quality` on a PNG target is ignored and not remembered; the
    // last JPEG quality carries to another JPEG path; a new document
    // starts at 90.
    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    s.ok("doc.save", json!({ "path": "a.png", "quality": 10 }));
    assert_eq!(s.host.files["a.png"], encoded(&s, &id, "PNG", 90));
    s.ok("doc.save", json!({ "path": "a.jpg" }));
    assert_eq!(s.host.files["a.jpg"], encoded(&s, &id, "JPEG", 90));
    s.ok("doc.save", json!({ "path": "b.jpg", "quality": 30 }));
    s.ok("doc.save", json!({ "path": "c.png" }));
    s.ok("doc.save", json!({ "path": "d.jpg" }));
    assert_eq!(s.host.files["d.jpg"], encoded(&s, &id, "JPEG", 30));
    assert_ne!(s.host.files["d.jpg"], encoded(&s, &id, "JPEG", 90));

    let other = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    s.ok("doc.save", json!({ "path": "e.jpg" }));
    assert_eq!(s.host.files["e.jpg"], encoded(&s, &other, "JPEG", 90));
}
