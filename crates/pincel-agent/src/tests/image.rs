//! Image documents through the shared tools: create, open, info, preview,
//! readback, undo, layers, mode errors, save, export and limits
//! (mcp-headless-image AC-01 … AC-05, AC-15, AC-16, AC-19 … AC-21).

use std::io::Cursor;

use pincel_image_core::codec::{decode, from_ora_bytes, to_bmp_bytes, to_jpeg_bytes, to_png_bytes};
use pincel_image_core::{Document, ImageBuffer, Layer, compose};
use serde_json::{Value, json};

use super::S;

/// A `w`×`h` RGBA test pattern with varied colors and alpha.
fn pattern(w: u32, h: u32) -> ImageBuffer {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&[
                (x * 40) as u8,
                (y * 50) as u8,
                ((x + y) * 30) as u8,
                if (x + y) % 3 == 0 { 128 } else { 255 },
            ]);
        }
    }
    ImageBuffer::from_raw(w, h, data).expect("buffer")
}

/// Encode `buf` with the `image` crate (formats the core only decodes).
fn encode_with_image(buf: &ImageBuffer, format: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(buf.width(), buf.height(), buf.data().to_vec())
        .expect("rgba image");
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), format)
        .expect("encode");
    out
}

/// Everything a document shows: size, layers (with ids) and selection.
fn state(
    s: &S,
    id: &str,
) -> (
    u32,
    u32,
    Vec<Layer>,
    Option<pincel_image_core::SelectionMask>,
) {
    let d = s.image(id).document();
    (
        d.canvas.width(),
        d.canvas.height(),
        d.layers().to_vec(),
        d.selection.clone(),
    )
}

/// Layer content as the editor shows it (no UUIDs).
fn layer_view(layers: &[Layer]) -> Vec<(String, Vec<u8>, u32, bool, bool)> {
    layers
        .iter()
        .map(|l| {
            (
                l.name.clone(),
                l.pixels.data().to_vec(),
                (l.opacity * 1000.0).round() as u32,
                l.visible,
                l.locked,
            )
        })
        .collect()
}

fn decode_png(bytes: &[u8]) -> ImageBuffer {
    decode(bytes).expect("png decodes")
}

#[test]
fn image_doc_new_creates_active_image_document_and_writes_nothing() {
    let mut s = S::new();
    s.new_doc(4, 4);
    let out = s.ok(
        "doc.new",
        json!({ "mode": "image", "width": 320, "height": 200 }),
    );
    assert_eq!(out["doc"], "doc-2");
    assert_eq!(out["mode"], "image");
    assert_eq!(
        (out["width"].as_u64(), out["height"].as_u64()),
        (Some(320), Some(200))
    );
    assert_eq!(s.ws.active(), Some("doc-2"));
    let d = s.image("doc-2").document();
    assert_eq!(
        (d.canvas.width(), d.canvas.height(), d.layer_count()),
        (320, 200, 1)
    );
    assert!(d.layers()[0].pixels.data().iter().all(|&b| b == 0));
    assert!(s.host.files.is_empty());
}

#[test]
fn image_doc_open_matches_what_the_editor_opens() {
    let mut s = S::new();
    let src = pattern(6, 5);
    let files = [
        ("a.png", to_png_bytes(&src, 6).expect("png")),
        ("a.jpg", to_jpeg_bytes(&src, 90).expect("jpeg")),
        ("a.webp", encode_with_image(&src, image::ImageFormat::WebP)),
        ("a.bmp", to_bmp_bytes(&src).expect("bmp")),
        ("a.gif", encode_with_image(&src, image::ImageFormat::Gif)),
        ("a.tiff", encode_with_image(&src, image::ImageFormat::Tiff)),
    ];
    for (path, bytes) in files {
        s.host.files.insert(path.to_owned(), bytes.clone());
        let out = s.ok("doc.open", json!({ "path": path }));
        let id = out["doc"].as_str().expect("id").to_owned();
        assert_eq!(out["mode"], "image", "{path}");
        assert_eq!(s.ws.active(), Some(id.as_str()));
        // The editor's Open: decode + Document::from_pixels (open_image).
        let editor = Document::from_pixels(decode(&bytes).expect("decode")).expect("doc");
        assert_eq!(
            layer_view(s.image(&id).document().layers()),
            layer_view(editor.layers()),
            "{path}"
        );
    }
    // An .ora project keeps its layers.
    let mut doc = Document::new(4, 3).expect("doc");
    doc.add_layer("Top").expect("layer");
    let ora = pincel_image_core::codec::to_ora_bytes(&doc, 1).expect("ora");
    s.host.files.insert("p.ora".to_owned(), ora.clone());
    let id = s.ok("doc.open", json!({ "path": "p.ora" }))["doc"]
        .as_str()
        .expect("id")
        .to_owned();
    let editor = from_ora_bytes(&ora).expect("ora opens");
    assert_eq!(
        layer_view(s.image(&id).document().layers()),
        layer_view(editor.layers())
    );
    // .aseprite still opens a Pixel document.
    let px = s.new_doc(4, 4);
    s.ok("doc.save", json!({ "doc": px, "path": "x.aseprite" }));
    let out = s.ok("doc.open", json!({ "path": "x.aseprite" }));
    assert_eq!(out["mode"], "pixel");
}

#[test]
fn image_shared_tools_report_preview_read_and_undo() {
    let mut s = S::new();
    let src = pattern(5, 4);
    s.host
        .files
        .insert("p.png".to_owned(), to_png_bytes(&src, 6).expect("png"));
    let id = s.ok("doc.open", json!({ "path": "p.png" }))["doc"]
        .as_str()
        .expect("id")
        .to_owned();

    // doc.info / doc.list
    let info = s.ok("doc.info", json!({}));
    assert_eq!(
        (info["width"].as_u64(), info["height"].as_u64()),
        (Some(5), Some(4))
    );
    assert_eq!(info["layers"], 1);
    assert_eq!(info["dirty"], false);
    let list = s.ok("doc.list", json!({}));
    assert_eq!(list["documents"][0]["mode"], "image");

    // doc.preview: PNG of the composed image, nearest upscale.
    let out = s
        .call("doc.preview", json!({ "scale": 2 }))
        .expect("preview");
    let png = decode_png(&out.image_png.expect("image content"));
    assert_eq!((png.width(), png.height()), (10, 8));
    let composed = compose(s.image(&id).document().layers());
    for (x, y) in [(0, 0), (3, 2), (4, 3)] {
        assert_eq!(
            png.get_pixel(x * 2 + 1, y * 2 + 1),
            composed.get_pixel(x, y),
            "({x}, {y})"
        );
    }

    // doc.read_pixels hex
    let out = s.ok(
        "doc.read_pixels",
        json!({ "region": { "x": 1, "y": 1, "width": 2, "height": 2 }, "format": "hex" }),
    );
    let c = composed.get_pixel(2, 1).expect("pixel");
    let want = format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a);
    let row0 = out["rows"][0].as_str().expect("row");
    assert_eq!(row0.split(' ').nth(1), Some(want.as_str()));
    assert_eq!(out["rows"].as_array().map(Vec::len), Some(2));

    // Every mutating call is one undo step; redo re-applies it.
    let calls: Vec<(&str, Value)> = vec![
        ("layer.add", json!({ "name": "Ink", "opacity": 128 })),
        (
            "layer.set",
            json!({ "layer": 0, "name": "Base", "visible": false, "blend": "multiply" }),
        ),
        ("layer.remove", json!({ "layer": 1 })),
    ];
    for (name, args) in calls {
        let before = state(&s, &id);
        let depth = s.image(&id).bus.history.undo_depth();
        s.ok(name, args);
        assert_eq!(s.image(&id).bus.history.undo_depth(), depth + 1, "{name}");
        assert_eq!(s.ok("doc.info", json!({}))["dirty"], true);
        let after = state(&s, &id);
        let undo = s.ok("doc.undo", json!({}));
        assert_eq!(undo["undone"], true);
        assert_eq!(state(&s, &id), before, "{name} undo");
        s.ok("doc.redo", json!({}));
        assert_eq!(state(&s, &id), after, "{name} redo");
    }
}

#[test]
fn image_layer_tools_change_and_list_the_stack() {
    let mut s = S::new();
    s.new_image(8, 8);
    let out = s.ok(
        "layer.add",
        json!({ "name": "Ink", "visible": false, "opacity": 51, "blend": "screen", "locked": true }),
    );
    let ink = out["layer"].as_u64().expect("id");
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], ink);
    s.ok("layer.add", json!({}));
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    assert_eq!(list.as_array().map(Vec::len), Some(3));
    assert_eq!(list[1]["name"], "Ink");
    assert_eq!(list[1]["visible"], false);
    assert_eq!(list[1]["opacity"], 51);
    assert_eq!(list[1]["blend"], "screen");
    assert_eq!(list[1]["locked"], true);
    assert_eq!(list[1]["id"], ink);

    s.ok(
        "layer.set",
        json!({ "layer": ink, "name": "Lines", "visible": true, "opacity": 255, "blend": "normal", "locked": false }),
    );
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    assert_eq!(list[1]["name"], "Lines");
    assert_eq!(list[1]["visible"], true);
    assert_eq!(list[1]["opacity"], 255);
    assert_eq!(list[1]["locked"], false);

    s.ok("layer.remove", json!({ "layer": 0 }));
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    assert_eq!(list.as_array().map(Vec::len), Some(2));
    assert_eq!(list[0]["id"], ink, "ids stay stable after a removal");
    // An undone removal brings the layer back with its id.
    s.ok("doc.undo", json!({}));
    assert_eq!(s.ok("layer.list", json!({}))["layers"][0]["id"], 0);
}

#[test]
fn image_layer_remove_of_only_layer_fails_like_the_editor() {
    let mut s = S::new();
    let id = s.new_image(4, 4);
    let before = state(&s, &id);
    let e = s.err("layer.remove", json!({ "layer": 0 }));
    assert!(e.contains("last") || e.contains("only"), "{e}");
    assert_eq!(state(&s, &id), before);
}

#[test]
fn tools_of_the_other_mode_fail_naming_the_mode_and_change_nothing() {
    let mut s = S::new();
    let img = s.new_image(8, 8);
    let region = json!({ "x": 0, "y": 0, "width": 2, "height": 2 });
    let pixel_only: Vec<(&str, Value)> = vec![
        ("frame.add", json!({})),
        ("frame.list", json!({})),
        ("frame.remove", json!({ "frame": 0 })),
        (
            "frame.set_duration",
            json!({ "frame": 0, "duration_ms": 50 }),
        ),
        ("tag.add", json!({ "name": "t", "from": 0, "to": 0 })),
        ("tag.list", json!({})),
        ("tag.remove", json!({ "tag": 0 })),
        ("palette.get", json!({})),
        ("palette.set", json!({ "entries": ["#000000"] })),
        ("palette.load_preset", json!({ "preset": "nes" })),
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
        ),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ff0000" }),
        ),
        (
            "draw.rect",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ff0000" }),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#ff0000" }),
        ),
        ("draw.clear", json!({})),
        (
            "selection.set_rect",
            json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
        ),
        ("selection.clear", json!({})),
        ("export.sheet", json!({ "columns": 1, "path": "s.png" })),
        (
            "doc.read_pixels",
            json!({ "region": region, "format": "index" }),
        ),
        ("layer.add", json!({ "parent": 0 })),
        ("layer.add", json!({ "kind": "group" })),
        ("layer.set", json!({ "layer": 0, "parent": 0 })),
    ];
    for (name, args) in pixel_only {
        let before = state(&s, &img);
        let e = s.err(name, args);
        assert!(e.contains("Image document"), "{name}: {e}");
        assert_eq!(state(&s, &img), before, "{name}");
    }
    let px = s.new_doc(8, 8);
    let image_only: Vec<(&str, Value)> = vec![
        ("export.jpeg", json!({ "path": "a.jpg" })),
        ("export.webp", json!({ "path": "a.webp" })),
    ];
    for (name, args) in image_only {
        let before = s.state(&px);
        let e = s.err(name, args);
        assert!(e.contains("Pixel document"), "{name}: {e}");
        assert_eq!(s.state(&px), before, "{name}");
    }
    assert!(s.host.files.is_empty());
}

#[test]
fn image_doc_save_writes_ora_that_reopens_identically() {
    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok(
        "layer.add",
        json!({ "name": "Top", "opacity": 128, "blend": "multiply", "visible": false, "locked": true }),
    );
    s.ok("doc.save", json!({ "path": "out/p.ora" }));
    assert_eq!(s.ok("doc.info", json!({}))["dirty"], false);
    let bytes = s.host.files.get("out/p.ora").expect("written").clone();
    let reopened = from_ora_bytes(&bytes).expect("valid ora");
    let mine = s.image(&id).document();
    assert_eq!(layer_view(reopened.layers()), layer_view(mine.layers()));
    assert_eq!(
        reopened
            .layers()
            .iter()
            .map(|l| l.blend_mode)
            .collect::<Vec<_>>(),
        mine.layers()
            .iter()
            .map(|l| l.blend_mode)
            .collect::<Vec<_>>()
    );
    // Without path it saves to the same file again.
    s.ok("layer.add", json!({}));
    s.ok("doc.save", json!({}));
    let again = from_ora_bytes(s.host.files.get("out/p.ora").expect("file")).expect("ora");
    assert_eq!(again.layer_count(), 3);
    // Reopened through the tool, it is the same document.
    let out = s.ok("doc.open", json!({ "path": "out/p.ora" }));
    assert_eq!(out["layers"], 3);
}

#[test]
fn image_doc_open_corrupt_or_unsupported_file_fails_naming_it() {
    let mut s = S::new();
    s.host
        .files
        .insert("bad.png".to_owned(), b"\x89PNG broken".to_vec());
    s.host
        .files
        .insert("notes.txt".to_owned(), b"hello".to_vec());
    s.host
        .files
        .insert("bad.ora".to_owned(), b"PK\x03\x04 nope".to_vec());
    for path in ["bad.png", "notes.txt", "bad.ora", "missing.png"] {
        let e = s.err("doc.open", json!({ "path": path }));
        assert!(e.contains(&format!("`{path}`")), "{e}");
    }
    assert!(s.ws.is_empty());
}

#[test]
fn image_doc_save_without_path_after_flat_open_fails_and_writes_nothing() {
    let mut s = S::new();
    let png = to_png_bytes(&pattern(3, 3), 6).expect("png");
    s.host.files.insert("shot.png".to_owned(), png.clone());
    s.ok("doc.open", json!({ "path": "shot.png" }));
    s.ok("layer.add", json!({}));
    let e = s.err("doc.save", json!({}));
    assert!(e.contains("PNG") && e.contains("shot.png"), "{e}");
    assert!(e.contains(".ora") && e.contains("export.png"), "{e}");
    assert_eq!(s.host.files.len(), 1);
    assert_eq!(s.host.files.get("shot.png"), Some(&png));
    // A new document asks for a path; a non-.ora path is refused.
    s.new_image(2, 2);
    assert!(s.err("doc.save", json!({})).contains("path"));
    assert!(
        s.err("doc.save", json!({ "path": "x.png" }))
            .contains(".ora")
    );
    assert_eq!(s.host.files.len(), 1);
}

#[test]
fn image_exports_decode_to_the_composed_image() {
    let mut s = S::new();
    let png = to_png_bytes(&pattern(6, 5), 6).expect("png");
    s.host.files.insert("p.png".to_owned(), png);
    let id = s.ok("doc.open", json!({ "path": "p.png" }))["doc"]
        .as_str()
        .expect("id")
        .to_owned();
    let composed = compose(s.image(&id).document().layers());
    s.ok("export.png", json!({ "path": "o.png" }));
    s.ok("export.webp", json!({ "path": "o.webp" }));
    s.ok("export.jpeg", json!({ "path": "o.jpg", "quality": 95 }));
    for path in ["o.png", "o.webp"] {
        let out = decode(s.host.files.get(path).expect("written")).expect("decodes");
        assert_eq!(out.data(), composed.data(), "{path} is lossless");
    }
    let jpeg_bytes = s.host.files.get("o.jpg").expect("jpeg");
    assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8]));
    // Exactly the editor's JPEG export (alpha flattened the same way).
    assert_eq!(jpeg_bytes, &to_jpeg_bytes(&composed, 95).expect("jpeg"));
    let jpeg = decode(jpeg_bytes).expect("jpeg decodes");
    assert_eq!((jpeg.width(), jpeg.height()), (6, 5));
    // Scaled PNG export.
    s.ok("export.png", json!({ "path": "big.png", "scale": 3 }));
    let big = decode(s.host.files.get("big.png").expect("big")).expect("png");
    assert_eq!((big.width(), big.height()), (18, 15));
    assert_eq!(big.get_pixel(4, 7), composed.get_pixel(1, 2));
}

#[test]
fn image_size_limits_refuse_new_and_open() {
    let mut s = S::new();
    let e = s.err(
        "doc.new",
        json!({ "mode": "image", "width": 20000, "height": 100 }),
    );
    assert!(e.contains("16384") && e.contains("20000×100"), "{e}");
    let e = s.err(
        "doc.new",
        json!({ "mode": "image", "width": 9000, "height": 9000 }),
    );
    assert!(e.contains("64 Mpx") && e.contains("9000×9000"), "{e}");
    let wide = to_png_bytes(&pattern(16_385, 1), 1).expect("png");
    s.host.files.insert("wide.png".to_owned(), wide);
    let e = s.err("doc.open", json!({ "path": "wide.png" }));
    assert!(
        e.contains("16384") && e.contains("16385×1") && e.contains("wide.png"),
        "{e}"
    );
    assert!(s.ws.is_empty());
    // Exactly at the limits is allowed (no pixel data touched here).
    assert!(super::super::image::check_size(16_384, 4096).is_ok());
    assert!(super::super::image::check_size(8192, 8192).is_ok());
    assert!(super::super::image::check_size(8193, 8192).is_err());
}
