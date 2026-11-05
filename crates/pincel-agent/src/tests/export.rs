//! Image save formats, BMP / TIFF export, opening PSD and PNG as Pixel,
//! animations and SVG (mcp-parity-export).

use pincel_image_core::codec::{
    decode, to_bmp_bytes, to_jpeg_bytes, to_png_bytes, to_tiff_bytes, to_webp_bytes,
};
use pincel_image_core::{ImageBuffer, compose};
use serde_json::json;

use super::S;

/// An opaque 6×4 test pattern.
fn pattern() -> ImageBuffer {
    let mut data = Vec::new();
    for y in 0..4u32 {
        for x in 0..6u32 {
            data.extend_from_slice(&[(x * 40) as u8, (y * 60) as u8, 90, 255]);
        }
    }
    ImageBuffer::from_raw(6, 4, data).expect("buffer")
}

fn composite(s: &S, id: &str) -> ImageBuffer {
    compose(s.image(id).document().layers())
}

fn opened(s: &mut S, name: &str, bytes: Vec<u8>) -> String {
    s.host.files.insert(name.to_owned(), bytes);
    s.ok("doc.open", json!({ "path": name }))["doc"]
        .as_str()
        .expect("id")
        .to_owned()
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
        "\"Ink\" with blend mode",
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
        assert!(e.contains("Image documents only"), "{args}: {e}");
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
    assert_eq!(
        decode(&s.host.files["o.tif"]).expect("tif").data(),
        img.data()
    );
    assert_eq!(s.ok("doc.info", json!({})), info);

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

const SAMPLE_PSD: &[u8] = include_bytes!("../../../pincel-image-core/tests/fixtures/sample.psd");

#[test]
fn doc_open_of_a_psd_opens_the_editors_layers_and_notices() {
    let mut s = S::new();
    s.host.files.insert("art.psd".into(), SAMPLE_PSD.to_vec());
    // Content first: a PSD under another name opens too.
    s.host.files.insert("art.bin".into(), SAMPLE_PSD.to_vec());
    for path in ["art.psd", "art.bin"] {
        let out = s.ok("doc.open", json!({ "path": path }));
        let id = out["doc"].as_str().expect("id").to_owned();
        assert_eq!(out["mode"], "image");
        assert_eq!(
            out["notices"],
            json!([
                "pass-through group \"Folder\" opened as an isolated Normal group",
                "text layer \"Title\" opened as pixels",
                "adjustment or fill layer \"Curves\" left out",
            ])
        );
        let editor = pincel_image_core::codec::psd::from_psd_bytes(
            SAMPLE_PSD,
            pincel_image_core::codec::psd::PsdLimits {
                max_dim: 16_384,
                max_area: 8192 * 8192,
            },
        )
        .expect("editor");
        let view = |d: &pincel_image_core::Document| -> Vec<(String, Vec<u8>, bool, bool)> {
            d.layers()
                .iter()
                .map(|l| {
                    (
                        l.name.clone(),
                        l.pixels.data().to_vec(),
                        l.visible,
                        l.is_group(),
                    )
                })
                .collect()
        };
        assert_eq!(view(s.image(&id).document()), view(&import_doc(&editor)));
        let list = s.ok("layer.list", json!({}))["layers"].clone();
        assert_eq!(list[1]["kind"], "group");
        assert_eq!(list[2]["parent"], list[1]["id"]);
    }
}

fn import_doc(i: &pincel_image_core::codec::psd::PsdImport) -> pincel_image_core::Document {
    i.document.clone()
}

#[test]
fn doc_open_of_a_bad_psd_names_the_file_and_reason() {
    let mut s = S::new();
    let mut psb = SAMPLE_PSD.to_vec();
    psb[5] = 2; // version 2: PSB
    s.host.files.insert("big.psd".into(), psb);
    s.host
        .files
        .insert("cut.psd".into(), SAMPLE_PSD[..40].to_vec());
    for path in ["big.psd", "cut.psd"] {
        let e = s.err("doc.open", json!({ "path": path }));
        assert!(e.contains(path), "{e}");
    }
    assert!(s.ws.is_empty());
}

#[test]
fn doc_open_mode_pixel_opens_a_png_as_the_pixel_editor_does() {
    let mut s = S::new();
    let png = to_png_bytes(&pattern(), 6).expect("png");
    s.host.files.insert("shot.png".into(), png.clone());
    let out = s.ok("doc.open", json!({ "path": "shot.png", "mode": "pixel" }));
    let id = out["doc"].as_str().expect("id").to_owned();
    assert_eq!(out["mode"], "pixel");
    let d = s.doc(&id);
    assert_eq!(d.sprite.layers.len(), 1);
    assert_eq!(d.sprite.layers[0].name, "Layer 1");
    assert_eq!(d.sprite.frames.len(), 1);
    assert_eq!(d.sprite.frames[0].duration_ms, 100);
    assert_eq!(d.sprite.palette, pincel_core::Palette::db32());
    assert_eq!(s.cel_rgba(&id, 0, 0), pattern().data().to_vec());
    let e = s.err("doc.save", json!({}));
    assert!(e.contains("path"), "{e}");
    assert_eq!(s.host.files["shot.png"], png, "the PNG is never written");
}

#[test]
fn doc_open_mode_selects_or_refuses_the_mode() {
    let mut s = S::new();
    s.new_doc(2, 2);
    s.ok("doc.save", json!({ "path": "a.aseprite" }));
    s.host
        .files
        .insert("p.png".into(), to_png_bytes(&pattern(), 6).expect("png"));
    s.host
        .files
        .insert("j.jpg".into(), to_jpeg_bytes(&pattern(), 90).expect("jpg"));
    assert_eq!(
        s.ok("doc.open", json!({ "path": "a.aseprite", "mode": "pixel" }))["mode"],
        "pixel"
    );
    assert_eq!(
        s.ok("doc.open", json!({ "path": "p.png", "mode": "image" }))["mode"],
        "image"
    );
    assert_eq!(
        s.ok("doc.open", json!({ "path": "p.png" }))["mode"],
        "image"
    );
    let open = s.ws.len();
    for (args, needle) in [
        (json!({ "path": "j.jpg", "mode": "pixel" }), "JPEG"),
        (json!({ "path": "a.aseprite", "mode": "image" }), "Aseprite"),
        (json!({ "path": "p.png", "mode": "vector" }), "\"vector\""),
    ] {
        let e = s.err("doc.open", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
    }
    assert_eq!(s.ws.len(), open);
}

#[test]
fn doc_open_mode_pixel_of_a_corrupt_png_names_the_file() {
    let mut s = S::new();
    let mut png = to_png_bytes(&pattern(), 6).expect("png");
    png.truncate(30);
    s.host.files.insert("bad.png".into(), png);
    let e = s.err("doc.open", json!({ "path": "bad.png", "mode": "pixel" }));
    assert!(e.contains("bad.png"), "{e}");
    assert!(s.ws.is_empty());
}
