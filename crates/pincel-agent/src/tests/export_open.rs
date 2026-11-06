//! Opening PSD files and PNGs as Pixel documents (mcp-parity-export
//! AC-21 … AC-25, AC-29).

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
