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

const COLORS_APPROXIMATED: &str =
    "A frame has more colors than a GIF color table holds (256); its colors are approximated.";
const TRANSPARENCY_ON_OFF: &str =
    "A frame has semi-transparent pixels; GIF stores transparency only as on or off.";

/// `n` opaque pixels of distinct colors on a 20×20 Pixel document.
fn distinct_colors(n: u32) -> Vec<serde_json::Value> {
    (0..n)
        .map(|i| json!({ "x": i % 20, "y": i / 20, "color": format!("#{:02x}{:02x}40", i % 256, i / 256) }))
        .collect()
}

#[test]
fn export_gif_warns_about_each_case_on_its_own() {
    // AC-13: each warning appears exactly when its case applies, with its
    // English message, and the GIF is written as the editor writes it.
    use pincel_core::AnimationFormat::Gif;
    let cases: [(&str, Vec<serde_json::Value>, serde_json::Value); 4] = [
        ("plain", distinct_colors(4), json!([])),
        (
            "many colors",
            distinct_colors(300),
            json!([{ "code": "colors_approximated", "message": COLORS_APPROXIMATED }]),
        ),
        (
            "semi-transparent",
            vec![json!({ "x": 0, "y": 0, "color": "#ff000080" })],
            json!([{ "code": "transparency_on_off", "message": TRANSPARENCY_ON_OFF }]),
        ),
        (
            "both",
            distinct_colors(300)
                .into_iter()
                .chain([json!({ "x": 19, "y": 19, "color": "#00ff00fe" })])
                .collect(),
            json!([
                { "code": "colors_approximated", "message": COLORS_APPROXIMATED },
                { "code": "transparency_on_off", "message": TRANSPARENCY_ON_OFF },
            ]),
        ),
    ];
    for (case, pixels, warnings) in cases {
        let mut s = S::new();
        let id = s.new_doc(20, 20);
        s.ok("draw.pixels", json!({ "pixels": pixels }));
        let out = s.ok("export.gif", json!({ "path": "c.gif" }));
        assert_eq!(out["warnings"], warnings, "{case}");
        assert_eq!(
            s.host.files["c.gif"],
            editor_animation(&s, &id, Gif, None, 1, 0),
            "{case}: the GIF is still written"
        );
        let out = s.ok("export.apng", json!({ "path": "c.png" }));
        assert!(out.get("warnings").is_none(), "{case}: {out}");
    }
}

#[test]
fn animation_loops_follow_the_tag_or_the_argument() {
    // AC-11 / AC-12: a tag without repeats plays forever, `loops` overrides
    // the tag's repeat count, and a tag exports only its frames.
    use pincel_core::AnimationFormat::{Apng, Gif};
    let mut s = S::new();
    let id = animated(&mut s);
    s.ok(
        "tag.add",
        json!({ "name": "back", "from": 1, "to": 2, "direction": "reverse" }),
    );
    let out = s.ok("export.gif", json!({ "path": "b.gif", "tag": "back" }));
    assert_eq!(
        out,
        json!({ "doc": id, "path": "b.gif", "frames": 2, "width": 4, "height": 4, "loops": 0, "warnings": [] })
    );
    assert_eq!(
        s.host.files["b.gif"],
        editor_animation(&s, &id, Gif, Some("back"), 1, 0)
    );
    let out = s.ok(
        "export.apng",
        json!({ "path": "w.png", "tag": "walk", "loops": 1, "scale": 2 }),
    );
    assert_eq!(
        out,
        json!({ "doc": id, "path": "w.png", "frames": 4, "width": 8, "height": 8, "loops": 1 })
    );
    assert_eq!(
        s.host.files["w.png"],
        editor_animation(&s, &id, Apng, Some("walk"), 2, 1)
    );
    // A GIF played once has no NETSCAPE loop extension.
    s.ok("export.gif", json!({ "path": "o.gif", "loops": 1 }));
    let gif = &s.host.files["o.gif"];
    assert!(!gif.windows(8).any(|w| w == b"NETSCAPE"), "loops 1");
    s.ok("export.gif", json!({ "path": "f.gif" }));
    assert!(s.host.files["f.gif"].windows(8).any(|w| w == b"NETSCAPE"));
}

#[test]
fn animation_errors_state_the_editors_reason() {
    // AC-27: a missing tag, a too tall output and a sprite the renderer
    // refuses fail with the editor's export error; nothing is written.
    let editor_err = |s: &S, id: &str, tag: Option<&str>, scale: u32| -> String {
        let d = s.doc(id);
        let mut opts =
            pincel_core::AnimationOptions::new(pincel_core::AnimationFormat::Gif).with_scale(scale);
        opts.tag = tag.map(str::to_owned);
        pincel_core::export_animation(&d.sprite, &d.cels, &opts)
            .expect_err("the editor refuses")
            .to_string()
    };
    let mut s = S::new();
    let id = animated(&mut s);
    let e = s.err("export.gif", json!({ "path": "x.gif", "tag": "jump" }));
    assert!(e.contains(&editor_err(&s, &id, Some("jump"), 1)), "{e}");

    let mut tall = S::new();
    let id = tall.new_doc(10, 2000);
    let e = tall.err("export.gif", json!({ "path": "x.gif", "scale": 10 }));
    assert!(e.contains(&editor_err(&tall, &id, None, 10)), "{e}");
    assert!(e.contains("20000"), "{e}");

    let mut indexed = S::new();
    let id = indexed.new_doc(4, 4);
    indexed.ws.get_mut(&id).expect("doc").sprite.color_mode = pincel_core::ColorMode::Indexed {
        transparent_index: 0,
    };
    for tool in ["export.gif", "export.apng"] {
        let e = indexed.err(tool, json!({ "path": "x" }));
        assert!(
            e.contains(&editor_err(&indexed, &id, None, 1)),
            "{tool}: {e}"
        );
    }
    assert!(s.host.files.is_empty() && tall.host.files.is_empty() && indexed.host.files.is_empty());
}
