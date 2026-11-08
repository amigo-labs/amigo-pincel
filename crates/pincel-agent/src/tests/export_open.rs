//! Opening PSD files and PNGs as Pixel documents (mcp-parity-export
//! AC-21 … AC-25, AC-29).

use pincel_image_core::codec::psd::{PsdLimits, from_psd_bytes};
use pincel_image_core::codec::{to_jpeg_bytes, to_png_bytes};
use serde_json::json;

use super::S;
use super::export::{SAMPLE_PSD, pattern};

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
    for (path, reason) in [
        ("big.psd", "PSB (large document format) is not supported"),
        ("cut.psd", "the file ends early"),
    ] {
        let e = s.err("doc.open", json!({ "path": path }));
        assert!(e.contains(path) && e.contains(reason), "{e}");
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
    assert!(
        e.contains("pass a `path`") && e.contains("\"shot.aseprite\""),
        "{e}"
    );
    assert_eq!(s.host.files["shot.png"], png, "the PNG is never written");
    // In a subdirectory the suggestion keeps the directory.
    s.host.files.insert("art/hero.v2.png".into(), png.clone());
    s.ok(
        "doc.open",
        json!({ "path": "art/hero.v2.png", "mode": "pixel" }),
    );
    let e = s.err("doc.save", json!({}));
    assert!(e.contains("\"art/hero.v2.aseprite\""), "{e}");
    assert_eq!(s.host.files.len(), 2, "nothing is written");
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

const FLAT_PSD: &[u8] = include_bytes!("../../../pincel-image-core/tests/fixtures/flat.psd");
const IMAGEMAGICK_PSD: &[u8] =
    include_bytes!("../../../pincel-image-core/tests/fixtures/imagemagick-layers.psd");

/// `d` as JSON with its random ids (document, layers, parents) replaced
/// by stable names, so two opens of one file compare equal.
fn normalized(d: &pincel_image_core::Document) -> serde_json::Value {
    fn rename(v: &mut serde_json::Value, ids: &[(String, String)]) {
        match v {
            serde_json::Value::String(s) => {
                if let Some((_, name)) = ids.iter().find(|(id, _)| id == s) {
                    *s = name.clone();
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|i| rename(i, ids)),
            serde_json::Value::Object(map) => map.values_mut().for_each(|i| rename(i, ids)),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(d).expect("serialize");
    let mut ids = vec![(
        v["id"].to_string().trim_matches('"').to_owned(),
        "doc".to_owned(),
    )];
    for (i, l) in v["layers"].as_array().expect("layers").iter().enumerate() {
        ids.push((
            l["id"].as_str().expect("id").to_owned(),
            format!("layer {i}"),
        ));
    }
    rename(&mut v, &ids);
    v
}

/// The editor's PSD limits (16384 px per side, 64 Mpx).
fn editor_limits() -> PsdLimits {
    PsdLimits {
        max_dim: 16_384,
        max_area: 8192 * 8192,
    }
}

#[test]
fn doc_open_of_a_psd_equals_the_editors_document_and_becomes_active() {
    // AC-21: the whole document (layers, groups, properties, pixels), the
    // active document, and the notices of each fixture in order.
    let notices_of = |bytes: &[u8]| -> Vec<String> {
        let mut s = S::new();
        s.host.files.insert("x.psd".into(), bytes.to_vec());
        let out = s.ok("doc.open", json!({ "path": "x.psd" }));
        let id = out["doc"].as_str().expect("id").to_owned();
        assert_eq!(s.ws.active(), Some(id.as_str()));
        let editor = from_psd_bytes(bytes, editor_limits()).expect("editor");
        assert_eq!(
            normalized(s.image(&id).document()),
            normalized(&editor.document)
        );
        out["notices"]
            .as_array()
            .expect("notices")
            .iter()
            .map(|n| n.as_str().expect("string").to_owned())
            .collect()
    };
    assert_eq!(
        notices_of(SAMPLE_PSD),
        [
            "pass-through group \"Folder\" opened as an isolated Normal group",
            "text layer \"Title\" opened as pixels",
            "adjustment or fill layer \"Curves\" left out",
        ]
    );
    assert_eq!(
        notices_of(FLAT_PSD),
        ["opened as one flattened layer: the file has no layers"]
    );
    assert!(notices_of(IMAGEMAGICK_PSD).is_empty(), "nothing dropped");
}

#[test]
fn doc_open_recognizes_psd_by_content_first_then_by_name() {
    // AC-21: an unknown content named `.psd` goes to the PSD reader; PNG
    // content named `.psd` opens as the PNG it is.
    let mut s = S::new();
    s.host
        .files
        .insert("odd.psd".into(), b"not an image".to_vec());
    let e = s.err("doc.open", json!({ "path": "odd.psd" }));
    let reason = from_psd_bytes(b"not an image", editor_limits())
        .expect_err("the PSD reader refuses")
        .to_string();
    assert!(e.contains("odd.psd") && e.contains(&reason), "{e}");
    s.host
        .files
        .insert("png.psd".into(), to_png_bytes(&pattern(), 6).expect("png"));
    let out = s.ok("doc.open", json!({ "path": "png.psd" }));
    assert_eq!(
        (out["mode"].clone(), out["layers"].clone()),
        (json!("image"), json!(1))
    );
    assert!(out.get("notices").is_none(), "{out}");
    assert_eq!(s.ws.len(), 1);
}

#[test]
fn doc_open_of_an_unsupported_psd_states_the_editors_reason() {
    // AC-22: 16 bits, a non-RGB mode, an invalid header, an unreadable
    // composite and both size limits.
    let patched = |at: usize, bytes: &[u8]| -> Vec<u8> {
        let mut out = FLAT_PSD.to_vec();
        out[at..at + bytes.len()].copy_from_slice(bytes);
        out
    };
    let cases: [(&str, Vec<u8>, &str); 6] = [
        (
            "deep.psd",
            patched(22, &16u16.to_be_bytes()),
            "16 bits per channel",
        ),
        ("cmyk.psd", patched(24, &4u16.to_be_bytes()), "CMYK"),
        (
            "version.psd",
            patched(4, &7u16.to_be_bytes()),
            "invalid PSD header",
        ),
        (
            "composite.psd",
            FLAT_PSD[..FLAT_PSD.len() - 3].to_vec(),
            "flattened image cannot be read",
        ),
        (
            "wide.psd",
            patched(18, &16_385u32.to_be_bytes()),
            "exceeds the size limit",
        ),
        (
            "area.psd",
            [
                patched(14, &9000u32.to_be_bytes())[..18].to_vec(),
                9000u32.to_be_bytes().to_vec(),
                FLAT_PSD[22..].to_vec(),
            ]
            .concat(),
            "exceeds the size limit",
        ),
    ];
    let mut s = S::new();
    for (path, bytes, reason) in cases {
        let editor = from_psd_bytes(&bytes, editor_limits())
            .expect_err("the editor refuses")
            .to_string();
        assert!(editor.contains(reason), "{path}: {editor}");
        s.host.files.insert(path.to_owned(), bytes);
        let e = s.err("doc.open", json!({ "path": path }));
        assert!(e.contains(path) && e.contains(&editor), "{path}: {e}");
    }
    assert!(s.ws.is_empty(), "no document is added");
}

#[test]
fn doc_open_mode_pixel_equals_a_new_pixel_document_with_the_pixels() {
    // AC-23: the sprite of a New Pixel document of that size (one layer
    // `Layer 1`, one 100 ms frame, its palette), the decoded pixels, and
    // no file of its own.
    let mut s = S::new();
    let png = to_png_bytes(&pattern(), 6).expect("png");
    s.host.files.insert("shot.png".into(), png.clone());
    let id = s.ok("doc.open", json!({ "path": "shot.png", "mode": "pixel" }))["doc"]
        .as_str()
        .expect("id")
        .to_owned();
    let fresh = s.new_doc(6, 4);
    assert_eq!(s.doc(&id).sprite, s.doc(&fresh).sprite);
    assert_eq!(
        s.cel_rgba(&id, 0, 0),
        pincel_core::import_png(&png).expect("decode").rgba
    );
    let info = s.ok("doc.info", json!({ "doc": id }));
    assert_eq!(
        (info["path"].clone(), info["dirty"].clone()),
        (json!(null), json!(false))
    );
}

#[test]
fn doc_open_mode_routes_by_name_then_content() {
    // AC-24: without `mode` `.ase` opens as Pixel; with `mode: "pixel"` an
    // `.ase` opens as without it and PNG content under any name opens as
    // Pixel; `mode: "image"` opens a PSD and a JPEG as Image.
    let mut s = S::new();
    s.new_doc(2, 2);
    s.ok("doc.save", json!({ "path": "a.ase" }));
    let png = to_png_bytes(&pattern(), 6).expect("png");
    s.host.files.insert("png.bin".into(), png);
    s.host.files.insert("art.psd".into(), SAMPLE_PSD.to_vec());
    s.host
        .files
        .insert("j.jpg".into(), to_jpeg_bytes(&pattern(), 90).expect("jpg"));
    for (args, mode) in [
        (json!({ "path": "a.ase" }), "pixel"),
        (json!({ "path": "a.ase", "mode": "pixel" }), "pixel"),
        (json!({ "path": "png.bin", "mode": "pixel" }), "pixel"),
        (json!({ "path": "art.psd", "mode": "image" }), "image"),
        (json!({ "path": "j.jpg", "mode": "image" }), "image"),
        (json!({ "path": "j.jpg" }), "image"),
    ] {
        assert_eq!(s.ok("doc.open", args.clone())["mode"], mode, "{args}");
    }
}

#[test]
fn doc_open_with_an_unfitting_mode_names_the_mode_and_detected_format() {
    // AC-25: each refusal names the mode and the detected format.
    let mut s = S::new();
    s.new_doc(2, 2);
    s.ok("doc.save", json!({ "path": "a.ase" }));
    s.host
        .files
        .insert("p.png".into(), to_png_bytes(&pattern(), 6).expect("png"));
    s.host.files.insert(
        "jpeg.png".into(),
        to_jpeg_bytes(&pattern(), 90).expect("jpg"),
    );
    s.host.files.insert("art.psd".into(), SAMPLE_PSD.to_vec());
    s.host.files.insert("x.bin".into(), b"nothing".to_vec());
    let open = s.ws.len();
    for (args, mode, format) in [
        (
            json!({ "path": "jpeg.png", "mode": "pixel" }),
            "\"pixel\"",
            "JPEG",
        ),
        (
            json!({ "path": "art.psd", "mode": "pixel" }),
            "\"pixel\"",
            "PSD",
        ),
        (
            json!({ "path": "x.bin", "mode": "pixel" }),
            "\"pixel\"",
            "unknown",
        ),
        (
            json!({ "path": "a.ase", "mode": "image" }),
            "\"image\"",
            "Aseprite",
        ),
        (
            json!({ "path": "p.png", "mode": "vector" }),
            "\"vector\"",
            "PNG",
        ),
        (
            json!({ "path": "a.ase", "mode": "vector" }),
            "\"vector\"",
            "Aseprite",
        ),
    ] {
        let e = s.err("doc.open", args.clone());
        assert!(e.contains(mode) && e.contains(format), "{args}: {e}");
    }
    assert_eq!(s.ws.len(), open, "no document is added");
}

#[test]
fn doc_open_mode_pixel_of_a_corrupt_png_states_the_decoders_reason() {
    // AC-29: the file and the decoder's reason.
    let mut png = to_png_bytes(&pattern(), 6).expect("png");
    png.truncate(40);
    let reason = pincel_core::import_png(&png)
        .expect_err("the decoder refuses")
        .to_string();
    let mut s = S::new();
    s.host.files.insert("bad.png".into(), png);
    let e = s.err("doc.open", json!({ "path": "bad.png", "mode": "pixel" }));
    assert!(e.contains("`bad.png`") && e.contains(&reason), "{e}");
    assert!(s.ws.is_empty());
}
