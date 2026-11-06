//! `export.apng` and `export.gif` on Pixel documents (mcp-parity-export
//! AC-02, AC-11 … AC-13, AC-27).

use serde_json::json;

use super::S;

/// A 4×4 Pixel document with three frames and a pingpong tag repeating
/// twice.
pub(super) fn animated(s: &mut S) -> String {
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
        (
            json!({ "path": "x.gif", "tag": "jump" }),
            "tag \"jump\" not found",
        ),
        (json!({ "path": "x.gif", "scale": 0 }), "scale"),
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
    assert!(
        e.contains("20000") && e.contains("16384") && e.contains("exceeds"),
        "{e}"
    );
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
