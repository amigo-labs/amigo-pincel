//! `export.svg` in both modes (mcp-parity-export AC-05 … AC-10).

use pincel_image_core::ImageBuffer;
use pincel_image_core::codec::to_png_bytes;
use serde_json::{Value, json};

use super::S;
use super::export::{composite, opened};
use super::export_anim::animated;

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
        (
            json!({ "path": "x.svg", "colors": 1 }),
            "colors must be 2–16",
        ),
        (
            json!({ "path": "x.svg", "mode": "mono", "fill": "#ff000080" }),
            "fill",
        ),
        (
            json!({ "path": "x.svg", "mode": "outline" }),
            "mode must be one of: pixel_exact, colors, mono",
        ),
        (
            json!({ "path": "x.svg", "alpha_max": 2 }),
            "alpha_max must be 0–1.34",
        ),
        (json!({ "path": "x.svg", "threshold": 300 }), "0–255"),
        (
            json!({ "path": "x.svg", "min_area": 200000 }),
            "min_area must be 0–100000",
        ),
        (
            json!({ "path": "x.svg", "opt_tolerance": -1 }),
            "opt_tolerance must be at least 0",
        ),
        (
            json!({ "path": "x.svg", "frame": 1 }),
            "Image documents have a single frame (0)",
        ),
        // Options the chosen mode ignores are still checked.
        (
            json!({ "path": "x.svg", "mode": "mono", "colors": 40 }),
            "colors must be 2–16",
        ),
        (
            json!({ "path": "x.svg", "mode": "colors", "fill": "#ff000080" }),
            "fill",
        ),
        (json!({ "path": "x.svg", "threshold": "low" }), "threshold"),
        (json!({ "path": "x.svg", "colors": 3.5 }), "colors"),
    ] {
        let e = s.err("export.svg", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
    }
    let mut p = S::new();
    p.new_doc(4, 4);
    let e = p.err("export.svg", json!({ "path": "x.svg", "frame": 9 }));
    assert!(e.contains("frame 9") && e.contains("0–0"), "{e}");
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

/// The dialog's trace of `rgba` with its result, for the counts.
fn dialog_trace(
    rgba: &[u8],
    w: u32,
    h: u32,
    mode: u8,
    params: &[f64],
) -> (String, pincel_trace::TraceResult) {
    let opts = pincel_trace::TraceOptions::from_wire(mode, params).expect("options");
    let mut session = pincel_trace::TraceSession::new(rgba, w, h).expect("session");
    let result = session.trace(&opts).expect("trace");
    (pincel_trace::to_svg(&result), result)
}

/// A 32×32 opaque image every tracing option changes: a dark ellipse
/// (curves), a strip of eight colors (more than the default six), a gray
/// band (between the auto and a high threshold) and four one-pixel
/// specks (despeckle, `min_area`).
fn option_fixture() -> ImageBuffer {
    const STRIP: [[u8; 3]; 8] = [
        [230, 25, 75],
        [60, 180, 75],
        [255, 225, 25],
        [0, 130, 200],
        [245, 130, 48],
        [145, 30, 180],
        [70, 240, 240],
        [240, 50, 230],
    ];
    let mut data = Vec::new();
    for y in 0..32u32 {
        for x in 0..32u32 {
            let (dx, dy) = (f64::from(x) - 15.5, f64::from(y) - 15.5);
            let rgb = if [(5, 5), (26, 6), (6, 25), (25, 26)].contains(&(x, y)) {
                [20, 20, 20]
            } else if dx * dx / 121.0 + dy * dy / 64.0 <= 1.0 {
                [32, 32, 32]
            } else if y < 3 {
                STRIP[(x / 4) as usize]
            } else if y >= 29 {
                [144, 144, 144]
            } else {
                [255, 255, 255]
            };
            data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    ImageBuffer::from_raw(32, 32, data).expect("fixture")
}

/// An Image document opened from `img`; returns its id.
fn open_image(s: &mut S, img: &ImageBuffer) -> String {
    opened(s, "t.png", to_png_bytes(img, 6).expect("png"))
}

/// One non-default value per option: the argument, its value, its slot in
/// the dialog's option vector and the value there.
fn alternatives() -> Vec<(&'static str, Value, usize, f64)> {
    vec![
        ("colors", json!(3), 0, 3.0),
        ("threshold", json!(200), 1, 200.0),
        ("invert", json!(true), 2, 1.0),
        ("fill", json!("#ff0000"), 3, f64::from(0xff0000_u32)),
        ("despeckle", json!(true), 4, 1.0),
        ("min_area", json!(0), 5, 0.0),
        ("alpha_max", json!(0.0), 6, 0.0),
        ("opt_curve", json!(false), 7, 0.0),
        ("opt_tolerance", json!(0.0), 8, 0.0),
    ]
}

/// The options each trace mode uses; it ignores the rest.
const USED: [(&str, u8, &[&str]); 3] = [
    (
        "colors",
        0,
        &[
            "colors",
            "despeckle",
            "min_area",
            "alpha_max",
            "opt_curve",
            "opt_tolerance",
        ],
    ),
    (
        "mono",
        1,
        &[
            "threshold",
            "invert",
            "fill",
            "despeckle",
            "min_area",
            "alpha_max",
            "opt_curve",
            "opt_tolerance",
        ],
    ),
    ("pixel_exact", 2, &[]),
];

#[test]
fn export_svg_each_option_and_default_is_the_dialogs() {
    // AC-06 / AC-07: per mode, the call without options is the dialog's
    // defaults, each option the mode uses changes the file exactly as in
    // the dialog, and each option it ignores leaves the file unchanged.
    let img = option_fixture();
    let mut s = S::new();
    open_image(&mut s, &img);
    for (mode, wire_mode, uses) in USED {
        let defaults = dialog_svg(img.data(), 32, 32, wire_mode, &SVG_DEFAULTS);
        s.ok("export.svg", json!({ "path": "d.svg", "mode": mode }));
        assert_eq!(
            s.host.files["d.svg"],
            defaults.as_bytes(),
            "{mode} defaults"
        );
        for (key, value, slot, wire) in alternatives() {
            let mut params = SVG_DEFAULTS;
            params[slot] = wire;
            let want = dialog_svg(img.data(), 32, 32, wire_mode, &params);
            assert_eq!(
                want != defaults,
                uses.contains(&key),
                "{mode}: the fixture must make `{key}` matter exactly where the mode uses it"
            );
            s.ok(
                "export.svg",
                json!({ "path": "o.svg", "mode": mode, key: value }),
            );
            assert_eq!(s.host.files["o.svg"], want.as_bytes(), "{mode} {key}");
        }
    }
    // `threshold: "auto"` is the default.
    s.ok(
        "export.svg",
        json!({ "path": "a.svg", "mode": "mono", "threshold": "auto" }),
    );
    assert_eq!(
        s.host.files["a.svg"],
        dialog_svg(img.data(), 32, 32, 1, &SVG_DEFAULTS).as_bytes()
    );
}

#[test]
fn export_svg_reports_the_dialogs_counts() {
    // AC-05: `layers`, `outlines`, `bytes` and `threshold` of the trace.
    let img = option_fixture();
    let mut s = S::new();
    let id = open_image(&mut s, &img);
    for (mode, wire_mode, _) in USED {
        let out = s.ok("export.svg", json!({ "path": "c.svg", "mode": mode }));
        let (svg, r) = dialog_trace(img.data(), 32, 32, wire_mode, &SVG_DEFAULTS);
        assert!(
            !r.layers.is_empty() && r.stats.paths as usize != r.layers.len(),
            "{mode}: layers and outlines must differ to tell them apart"
        );
        assert_eq!(
            out,
            json!({
                "doc": id,
                "path": "c.svg",
                "layers": r.layers.len(),
                "outlines": r.stats.paths,
                "bytes": svg.len(),
                "threshold": r.threshold_used,
            }),
            "{mode}"
        );
        assert_eq!(s.host.files["c.svg"].len(), svg.len());
        assert_eq!(out["threshold"].is_null(), mode != "mono", "{mode}");
    }
}

#[test]
fn export_svg_of_nothing_visible_has_no_layers() {
    // Edge cases: a fully transparent document, an all-white image in
    // mono mode, and a Pixel document whose content is on a hidden layer.
    let mut s = S::new();
    s.new_image(4, 4);
    let out = s.ok("export.svg", json!({ "path": "t.svg" }));
    assert_eq!(
        (out["layers"].clone(), out["outlines"].clone()),
        (json!(0), json!(0))
    );
    assert!(String::from_utf8_lossy(&s.host.files["t.svg"]).contains("<svg"));
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#ffffff" }));
    let out = s.ok("export.svg", json!({ "path": "w.svg", "mode": "mono" }));
    assert_eq!(out["layers"], 0);

    let mut p = S::new();
    p.new_doc(4, 4);
    p.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }] }),
    );
    p.ok("layer.set", json!({ "layer": 0, "visible": false }));
    let out = p.ok("export.svg", json!({ "path": "h.svg" }));
    assert_eq!(out["layers"], 0);
}

#[test]
fn export_svg_traces_the_active_or_given_frame_and_skips_hidden_layers() {
    // AC-08, Pixel: the active frame (here the middle one) by default.
    let mut s = S::new();
    let id = animated(&mut s);
    s.ws.get_mut(&id).expect("doc").active_frame = pincel_core::FrameIndex::new(1);
    let frames: Vec<String> = (0..3)
        .map(|f| dialog_svg(&pixel_frame(&s, &id, f), 4, 4, 2, &SVG_DEFAULTS))
        .collect();
    assert!(frames[0] != frames[1] && frames[1] != frames[2] && frames[0] != frames[2]);
    s.ok("export.svg", json!({ "path": "a.svg" }));
    assert_eq!(s.host.files["a.svg"], frames[1].as_bytes());
    for f in [0, 2] {
        s.ok("export.svg", json!({ "path": "f.svg", "frame": f }));
        assert_eq!(s.host.files["f.svg"], frames[f].as_bytes());
    }

    // AC-08, Image: hidden layers stay out, and `frame: 0` is accepted.
    let mut s = S::new();
    let id = s.new_image(6, 4);
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#336699" }));
    let hidden = s.ok("layer.add", json!({ "name": "Off" }))["layer"].clone();
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 1, "y0": 1, "x1": 4, "y1": 3, "fill": "#ff0000" }),
    );
    let shown = composite(&s, &id);
    s.ok("layer.set", json!({ "layer": hidden, "visible": false }));
    let visible = composite(&s, &id);
    let want = dialog_svg(visible.data(), 6, 4, 0, &SVG_DEFAULTS);
    assert_ne!(
        want,
        dialog_svg(shown.data(), 6, 4, 0, &SVG_DEFAULTS),
        "the hidden layer must matter"
    );
    s.ok("export.svg", json!({ "path": "i.svg", "frame": 0 }));
    assert_eq!(s.host.files["i.svg"], want.as_bytes());
}

#[test]
fn export_svg_defaults_to_pixel_exact_on_pixel_and_colors_on_image() {
    // AC-06: the preselected mode differs per document mode.
    let mut s = S::new();
    let id = animated(&mut s);
    s.ws.get_mut(&id).expect("doc").active_frame = pincel_core::FrameIndex::new(0);
    let rgba = pixel_frame(&s, &id, 0);
    let by_mode: Vec<String> = (0..3)
        .map(|m| dialog_svg(&rgba, 4, 4, m, &SVG_DEFAULTS))
        .collect();
    s.ok("export.svg", json!({ "path": "p.svg" }));
    assert_eq!(s.host.files["p.svg"], by_mode[2].as_bytes());
    assert!(by_mode[2] != by_mode[0] && by_mode[2] != by_mode[1]);

    let img = option_fixture();
    let mut s = S::new();
    open_image(&mut s, &img);
    let by_mode: Vec<String> = (0..3)
        .map(|m| dialog_svg(img.data(), 32, 32, m, &SVG_DEFAULTS))
        .collect();
    s.ok("export.svg", json!({ "path": "i.svg" }));
    assert_eq!(s.host.files["i.svg"], by_mode[0].as_bytes());
    assert!(by_mode[0] != by_mode[1] && by_mode[0] != by_mode[2]);
}

#[test]
fn bad_svg_argument_forms_name_the_argument_and_its_range() {
    // AC-09: wrong forms name the argument and what it accepts; with
    // several invalid arguments the message names one of them.
    let mut s = S::new();
    s.new_image(4, 4);
    for (args, name, range) in [
        (json!({ "colors": 3.5 }), "colors", "2–16"),
        (json!({ "min_area": 2.5 }), "min_area", "0–100000"),
        (json!({ "alpha_max": "x" }), "alpha_max", "0–1.34"),
        (
            json!({ "opt_tolerance": "x" }),
            "opt_tolerance",
            "at least 0",
        ),
        (json!({ "mode": 5 }), "mode", "pixel_exact, colors, mono"),
        (json!({ "threshold": 12.5 }), "threshold", "0–255"),
        (json!({ "threshold": "low" }), "threshold", "\"auto\""),
        (json!({ "fill": 3 }), "fill", "#RRGGBB"),
        (json!({ "fill": "red" }), "fill", "#RRGGBB"),
        (json!({ "fill": "#f00" }), "fill", "#RRGGBB"),
        (json!({ "invert": "yes" }), "invert", "boolean"),
        (json!({ "despeckle": 1 }), "despeckle", "boolean"),
        (json!({ "opt_curve": "no" }), "opt_curve", "boolean"),
        (json!({ "frame": -1 }), "frame", "at least 0"),
        (json!({ "frame": 2 }), "frame", "single frame (0)"),
    ] {
        let mut call = args.clone();
        call["path"] = json!("x.svg");
        let e = s.err("export.svg", call);
        assert!(e.contains(name) && e.contains(range), "{args}: {e}");
    }
    let e = s.err(
        "export.svg",
        json!({ "path": "x.svg", "colors": 1, "alpha_max": 2 }),
    );
    assert!(
        e.contains("colors") != e.contains("alpha_max"),
        "one argument: {e}"
    );
    assert!(s.host.files.is_empty());
}

#[test]
fn pixel_exact_svg_counts_only_visible_colors_up_to_256() {
    // AC-10: 256 distinct colors with alpha > 0 still trace (the
    // transparent rest does not count); 257 fail stating 257.
    let colored = |n: u32| -> Vec<Value> {
        (0..n)
            .map(|i| json!({ "x": i % 20, "y": i / 20, "color": format!("#{:02x}{:02x}80", i % 256, i / 256) }))
            .collect()
    };
    let mut s = S::new();
    s.new_doc(20, 20);
    s.ok("draw.pixels", json!({ "pixels": colored(256) }));
    s.ok("export.svg", json!({ "path": "ok.svg" }));
    let mut s = S::new();
    s.new_doc(20, 20);
    s.ok("draw.pixels", json!({ "pixels": colored(257) }));
    let e = s.err("export.svg", json!({ "path": "x.svg" }));
    assert!(e.contains("257") && e.contains("mode: \"colors\""), "{e}");
    assert!(s.host.files.is_empty());
}
