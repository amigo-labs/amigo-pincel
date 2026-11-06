//! `export.svg` in both modes (mcp-parity-export AC-05 … AC-10).

use serde_json::json;

use super::S;
use super::export::composite;
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
