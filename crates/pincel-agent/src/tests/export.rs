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

/// A 4×4 Pixel document with three frames and a pingpong tag repeating
/// twice.
fn animated(s: &mut S) -> String {
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#00ff00" }], "frame": 1 }),
    );
    s.ok("frame.add", json!({}));
    s.ok(
        "tag.add",
        json!({ "name": "walk", "from": 0, "to": 2, "direction": "pingpong" }),
    );
    s.ws.get_mut(&id).expect("doc").sprite.tags[0].repeats = 2;
    id
}

fn editor_animation(
    s: &S,
    id: &str,
    format: pincel_core::AnimationFormat,
    tag: Option<&str>,
    scale: u32,
    loops: u16,
) -> Vec<u8> {
    let d = s.doc(id);
    let mut opts = pincel_core::AnimationOptions::new(format)
        .with_scale(scale)
        .with_loops(loops);
    opts.tag = tag.map(str::to_owned);
    pincel_core::export_animation(&d.sprite, &d.cels, &opts).expect("editor export")
}

#[test]
fn export_apng_and_gif_equal_the_editors_animation_export() {
    use pincel_core::AnimationFormat::{Apng, Gif};
    let mut s = S::new();
    let id = animated(&mut s);
    let out = s.ok("export.apng", json!({ "path": "a.png" }));
    assert_eq!(
        out,
        json!({ "doc": id, "path": "a.png", "frames": 3, "width": 4, "height": 4, "loops": 0 })
    );
    assert_eq!(
        s.host.files["a.png"],
        editor_animation(&s, &id, Apng, None, 1, 0)
    );
    let out = s.ok(
        "export.gif",
        json!({ "path": "w.gif", "tag": "walk", "scale": 3 }),
    );
    assert_eq!(out["frames"], 4, "pingpong counts the way back");
    assert_eq!(out["loops"], 2, "the tag's repeat count");
    assert_eq!(
        (out["width"].clone(), out["height"].clone()),
        (json!(12), json!(12))
    );
    assert_eq!(out["warnings"], json!([]));
    assert_eq!(
        s.host.files["w.gif"],
        editor_animation(&s, &id, Gif, Some("walk"), 3, 2)
    );
    s.ok(
        "export.apng",
        json!({ "path": "l.png", "tag": "walk", "loops": 0 }),
    );
    assert_eq!(
        s.host.files["l.png"],
        editor_animation(&s, &id, Apng, Some("walk"), 1, 0)
    );
}

#[test]
fn export_gif_warns_about_what_gif_cannot_keep() {
    let mut s = S::new();
    s.new_doc(20, 20);
    let pixels: Vec<serde_json::Value> = (0..400u32)
        .map(|i| json!({ "x": i % 20, "y": i / 20, "color": format!("#{:02x}{:02x}{:02x}", i % 256, i / 2 % 256, 255 - i % 256) }))
        .collect();
    s.ok("draw.pixels", json!({ "pixels": pixels }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff000080" }] }),
    );
    let out = s.ok("export.gif", json!({ "path": "c.gif" }));
    let codes: Vec<&str> = out["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .map(|w| w["code"].as_str().expect("code"))
        .collect();
    assert_eq!(codes, ["colors_approximated", "transparency_on_off"]);
    assert!(
        out["warnings"][0]["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty())
    );
    assert!(s.host.files.contains_key("c.gif"));
    let out = s.ok("export.apng", json!({ "path": "c.png" }));
    assert!(out.get("warnings").is_none());
}

#[test]
fn bad_animation_arguments_write_nothing() {
    let mut s = S::new();
    animated(&mut s);
    for (args, needle) in [
        (json!({ "path": "x.gif", "tag": "jump" }), "jump"),
        (json!({ "path": "x.gif", "scale": 11 }), "scale"),
        (json!({ "path": "x.gif", "loops": -1 }), "loops"),
        (json!({ "path": "x.gif", "loops": 65536 }), "loops"),
    ] {
        let e = s.err("export.gif", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
    }
    let mut big = S::new();
    big.new_doc(2000, 10);
    let e = big.err("export.apng", json!({ "path": "x.png", "scale": 10 }));
    assert!(e.contains("16384") || e.contains("20000"), "{e}");
    assert!(s.host.files.is_empty() && big.host.files.is_empty());
}

#[test]
fn animation_exports_on_image_documents_fail_naming_the_mode() {
    let mut s = S::new();
    s.new_image(4, 4);
    for tool in ["export.apng", "export.gif"] {
        let e = s.err(tool, json!({ "path": "x" }));
        assert!(e.contains("Image document"), "{tool}: {e}");
    }
    assert!(s.host.files.is_empty());
}

/// The dialog's default option vector (`vectorize-dialog`).
const SVG_DEFAULTS: [f64; 9] = [6.0, -1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.2];

fn dialog_svg(rgba: &[u8], w: u32, h: u32, mode: u8, params: &[f64]) -> String {
    let opts = pincel_trace::TraceOptions::from_wire(mode, params).expect("options");
    let mut session = pincel_trace::TraceSession::new(rgba, w, h).expect("session");
    pincel_trace::to_svg(&session.trace(&opts).expect("trace"))
}

fn pixel_frame(s: &S, id: &str, frame: u32) -> Vec<u8> {
    let d = s.doc(id);
    let req = pincel_core::ComposeRequest::full(
        pincel_core::FrameIndex::new(frame),
        d.sprite.width,
        d.sprite.height,
    );
    let mut out = Vec::new();
    pincel_core::compose(&d.sprite, &d.cels, &req, &mut out).expect("compose");
    out
}

#[test]
fn export_svg_defaults_equal_the_dialog_in_each_mode() {
    let mut s = S::new();
    let id = animated(&mut s);
    s.ok("layer.add", json!({ "visible": false }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 3, "y": 3, "color": "#0000ff" }] }),
    );
    // The active frame is 2 after frame.add; trace frame 1 explicitly too.
    let out = s.ok("export.svg", json!({ "path": "p.svg", "frame": 1 }));
    let want = dialog_svg(&pixel_frame(&s, &id, 1), 4, 4, 2, &SVG_DEFAULTS);
    assert_eq!(s.host.files["p.svg"], want.as_bytes());
    assert_eq!(out["bytes"], want.len());
    assert_eq!(out["threshold"], serde_json::Value::Null);
    assert!(out["layers"].as_u64().is_some() && out["outlines"].as_u64().is_some());
    assert!(
        !want.contains("0000ff") && !want.contains("0000FF"),
        "hidden layers stay out"
    );
    s.ok("export.svg", json!({ "path": "a.svg" }));
    let active = s.doc(&id).active_frame.0;
    assert_eq!(
        s.host.files["a.svg"],
        dialog_svg(&pixel_frame(&s, &id, active), 4, 4, 2, &SVG_DEFAULTS).as_bytes()
    );

    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 1, "y0": 1, "x1": 4, "y1": 3, "fill": "#ff0000" }),
    );
    s.ok("export.svg", json!({ "path": "i.svg" }));
    let img = composite(&s, &id);
    assert_eq!(
        s.host.files["i.svg"],
        dialog_svg(img.data(), 6, 4, 0, &SVG_DEFAULTS).as_bytes()
    );
}

#[test]
fn export_svg_options_equal_the_dialogs() {
    let mut s = S::new();
    let id = s.new_image(8, 6);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#eeeeee" }));
    s.ok(
        "draw.shape",
        json!({ "shape": "ellipse", "x0": 1, "y0": 1, "x1": 6, "y1": 5, "fill": "#202020" }),
    );
    let img = composite(&s, &id);
    let out = s.ok(
        "export.svg",
        json!({ "path": "m.svg", "mode": "mono", "threshold": 100, "invert": true, "fill": "#ff0000", "despeckle": true, "min_area": 0, "alpha_max": 0.5, "opt_curve": false, "opt_tolerance": 1.5 }),
    );
    let want = dialog_svg(
        img.data(),
        8,
        6,
        1,
        &[
            6.0,
            100.0,
            1.0,
            f64::from(0xff0000_u32),
            1.0,
            0.0,
            0.5,
            0.0,
            1.5,
        ],
    );
    assert_eq!(s.host.files["m.svg"], want.as_bytes());
    assert_eq!(out["threshold"], 100);
    let out = s.ok(
        "export.svg",
        json!({ "path": "c.svg", "mode": "colors", "colors": 3 }),
    );
    let want = dialog_svg(
        img.data(),
        8,
        6,
        0,
        &[3.0, -1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.2],
    );
    assert_eq!(s.host.files["c.svg"], want.as_bytes());
    assert_eq!(out["threshold"], serde_json::Value::Null);
    // Options a mode ignores do not change the file.
    s.ok("export.svg", json!({ "path": "c2.svg", "mode": "colors", "colors": 3, "invert": true, "fill": "#00ff00" }));
    assert_eq!(s.host.files["c2.svg"], s.host.files["c.svg"]);
}

#[test]
fn bad_svg_arguments_write_nothing() {
    let mut s = S::new();
    s.new_image(4, 4);
    for (args, needle) in [
        (json!({ "path": "x.svg", "colors": 1 }), "colors"),
        (
            json!({ "path": "x.svg", "mode": "mono", "fill": "#ff000080" }),
            "fill",
        ),
        (json!({ "path": "x.svg", "mode": "outline" }), "mode"),
        (json!({ "path": "x.svg", "alpha_max": 2 }), "alpha_max"),
        (json!({ "path": "x.svg", "threshold": 300 }), "threshold"),
        (json!({ "path": "x.svg", "min_area": 200000 }), "min_area"),
        (
            json!({ "path": "x.svg", "opt_tolerance": -1 }),
            "opt_tolerance",
        ),
        (json!({ "path": "x.svg", "frame": 1 }), "frame"),
    ] {
        let e = s.err("export.svg", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
    }
    let mut p = S::new();
    p.new_doc(4, 4);
    let e = p.err("export.svg", json!({ "path": "x.svg", "frame": 9 }));
    assert!(e.contains("frame 9"), "{e}");
    assert!(s.host.files.is_empty() && p.host.files.is_empty());
}

#[test]
fn pixel_exact_svg_refuses_more_than_256_colors() {
    let mut s = S::new();
    s.new_doc(20, 20);
    let pixels: Vec<serde_json::Value> = (0..300u32)
        .map(|i| json!({ "x": i % 20, "y": i / 20, "color": format!("#{:02x}{:02x}00", i % 256, i / 256) }))
        .collect();
    s.ok("draw.pixels", json!({ "pixels": pixels }));
    let e = s.err("export.svg", json!({ "path": "x.svg" }));
    assert!(e.contains("300") && e.contains("mode: \"colors\""), "{e}");
    assert!(s.host.files.is_empty());
}
