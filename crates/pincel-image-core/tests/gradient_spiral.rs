//! Spiral gradient kinds (docs/specs/image-palette-gradient-spiral.md),
//! driven through `run_command` with the commands the editor sends: a
//! Gradient drag begins a live gradient, and Enter commits it.

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{
    Color, CommandBus, Document, ImageBuffer, Rect, SelectionMask, SetPixels, SetSelection,
};
use serde_json::{Value, json};

const KINDS: [(&str, bool); 2] = [("spiral_cw", true), ("spiral_ccw", false)];

/// A one-layer document filled with `fill`, its history empty.
fn bus(w: u32, h: u32, fill: impl Fn(u32, u32) -> Color) -> CommandBus {
    let mut bus = CommandBus::new(Document::new(w, h).expect("doc"));
    let mut patch = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            patch.set_pixel(x, y, fill(x, y));
        }
    }
    bus.apply(Box::new(SetPixels::new(0, Rect::new(0, 0, w, h), patch)))
        .expect("fill");
    bus.history.clear();
    bus
}

fn clear(w: u32, h: u32) -> CommandBus {
    bus(w, h, |_, _| Color::TRANSPARENT)
}

fn run(bus: &mut CommandBus, cmd: Value) {
    let spec: CommandSpec = serde_json::from_value(cmd).expect("valid CommandSpec");
    run_command(bus, spec, &mut StrokeScratch::default(), &[]).expect("runs");
}

/// The fields both gradient commands share: black → white, opaque.
fn fields(kind: &str, channel: &str, a: (f32, f32), b: (f32, f32)) -> Value {
    json!({
        "layer": 0, "kind": kind, "channel": channel,
        "x0": a.0, "y0": a.1, "x1": b.0, "y1": b.1,
        "from": [0, 0, 0, 255], "to": [255, 255, 255, 255], "opacity": 1.0
    })
}

fn with_type(mut v: Value, ty: &str) -> Value {
    v["type"] = json!(ty);
    v
}

/// The editor's Gradient tool: the drag begins a live gradient, which is
/// committed.
fn drag(bus: &mut CommandBus, kind: &str, channel: &str, a: (f32, f32), b: (f32, f32)) {
    run(
        bus,
        with_type(fields(kind, channel, a, b), "begin_live_gradient"),
    );
    run(bus, json!({ "type": "commit_live" }));
}

fn px(bus: &CommandBus, x: u32, y: u32) -> Color {
    bus.document.layers()[0]
        .pixels
        .get_pixel(x, y)
        .expect("in bounds")
}

/// `t` for the pixel `(x, y)` straight from the spec's terms (AC-02), in
/// degrees and `f64`, independent of the core's math.
fn expected_t(x: u32, y: u32, a: (f32, f32), b: (f32, f32), clockwise: bool) -> f64 {
    let (ax, ay) = (f64::from(a.0), f64::from(a.1));
    let (bx, by) = (f64::from(b.0), f64::from(b.1));
    let (dx, dy) = (f64::from(x) + 0.5 - ax, f64::from(y) + 0.5 - ay);
    let len = (bx - ax).hypot(by - ay);
    let r = dx.hypot(dy);
    let theta = if r == 0.0 {
        0.0
    } else {
        (dy.atan2(dx).to_degrees() - (by - ay).atan2(bx - ax).to_degrees()).rem_euclid(360.0)
    };
    let v = if clockwise {
        r / len - theta / 360.0
    } else {
        r / len + theta / 360.0
    };
    v - v.floor()
}

/// `got` is `want` ±1, or both sit on either side of the sharp seam where
/// the ramp wraps (the reference math and the core's round differently
/// there).
fn close(got: u8, want: u8) -> bool {
    let d = got.abs_diff(want);
    d <= 1 || (d >= 253 && (got.min(want) <= 2))
}

fn grey(t: f64) -> u8 {
    (255.0 * t).round() as u8
}

#[test]
fn spiral_every_pixel_follows_the_spiral_formula() {
    // A drag up and to the left, off the pixel grid: the pattern turns with
    // the drag direction (Edge cases).
    let (a, b) = ((20.3, 17.6), (9.1, 6.4));
    for (kind, cw) in KINDS {
        let mut bus = clear(32, 28);
        drag(&mut bus, kind, "color", a, b);
        for y in 0..28 {
            for x in 0..32 {
                let want = grey(expected_t(x, y, a, b, cw));
                let got = px(&bus, x, y);
                assert!(
                    close(got.r, want) && got.r == got.g && got.g == got.b && got.a == 255,
                    "{kind} ({x}, {y}): got {got:?}, want grey {want}"
                );
            }
        }
    }
}

#[test]
fn spiral_pins_the_spec_example_values() {
    // AC-03: from the centre of (8, 8) to the centre of (24, 8).
    let (a, b) = ((8.5, 8.5), (24.5, 8.5));
    for (kind, want) in [
        ("spiral_cw", [(16, 8, 128), (8, 16, 64), (8, 0, 191)]),
        ("spiral_ccw", [(16, 8, 128), (8, 16, 191), (8, 0, 64)]),
    ] {
        let mut bus = clear(32, 32);
        drag(&mut bus, kind, "color", a, b);
        for (x, y, v) in want {
            let got = px(&bus, x, y);
            for c in [got.r, got.g, got.b] {
                assert!(
                    c.abs_diff(v) <= 1,
                    "{kind} ({x}, {y}): got {got:?}, want {v}"
                );
            }
            assert_eq!(got.a, 255, "{kind} ({x}, {y})");
        }
    }
}

#[test]
fn spiral_start_pixel_takes_the_start_color_for_any_direction() {
    // The pixel whose centre is the drag start has r = 0 and θ = 0°, so
    // t = 0, also when the drag does not point along +x (Edge cases).
    for (kind, _) in KINDS {
        let mut bus = clear(16, 16);
        drag(&mut bus, kind, "color", (10.5, 10.5), (3.5, 2.5));
        assert_eq!(px(&bus, 10, 10), Color::BLACK, "{kind}");
    }
}

#[test]
fn spiral_ccw_mirrors_spiral_cw_across_a_horizontal_drag_on_a_boundary() {
    // AC-04: the drag lies on the boundary y = 8, so row y mirrors to row
    // 15 − y; both directions of the drag.
    for (a, b) in [((5.0, 8.0), (19.0, 8.0)), ((27.0, 8.0), (13.0, 8.0))] {
        let mut cw = clear(32, 16);
        drag(&mut cw, "spiral_cw", "color", a, b);
        let mut ccw = clear(32, 16);
        drag(&mut ccw, "spiral_ccw", "color", a, b);
        for y in 0..16 {
            for x in 0..32 {
                let (m, c) = (px(&cw, x, 15 - y), px(&ccw, x, y));
                for (g, w) in [(c.r, m.r), (c.g, m.g), (c.b, m.b), (c.a, m.a)] {
                    assert!(
                        close(g, w),
                        "{a:?}→{b:?} ({x}, {y}): ccw {c:?}, mirrored cw {m:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn spiral_repeats_the_ramp_every_drag_length_on_the_drag_line() {
    // AC-05: L = 8 along row 1 of a 64 × 3 layer, from the centre of
    // (24, 1). Forward of `a` (θ = 0°) the ramp at L … 2L repeats 0 … L;
    // behind `a` (θ = 180°) it repeats every L, shifted by half a period.
    let (a, b) = ((24.5, 1.5), (32.5, 1.5));
    for (kind, _) in KINDS {
        let mut bus = clear(64, 3);
        drag(&mut bus, kind, "color", a, b);
        let fwd = |r: u32| px(&bus, 24 + r, 1);
        let back = |r: u32| px(&bus, 24 - r, 1);
        for r in 0..8 {
            assert_eq!(fwd(r + 8), fwd(r), "{kind} forward r = {r}");
            assert_eq!(fwd(r + 16), fwd(r), "{kind} forward r = {r} + 2L");
        }
        // The seam: the end color just inside L, the start color at L.
        assert!(fwd(7).r > 200, "{kind}: {:?}", fwd(7));
        assert_eq!(fwd(8), Color::BLACK, "{kind}");
        for r in 1..=16 {
            assert_eq!(back(r + 8), back(r), "{kind} behind r = {r}");
            assert_eq!(
                back(r),
                fwd((r + 4) % 8),
                "{kind} behind r = {r}, half period"
            );
        }
    }
}

#[test]
fn spiral_transparency_fades_alpha_along_t_and_keeps_the_color() {
    // AC-06: an opaque colored layer keeps each pixel's color; alpha falls
    // from opaque (t = 0) to clear (t = 1).
    let (a, b) = ((8.5, 8.5), (24.5, 8.5));
    let base = Color::rgba(40, 120, 200, 255);
    for (kind, cw) in KINDS {
        let mut bus = bus(32, 32, |_, _| base);
        drag(&mut bus, kind, "transparency", a, b);
        for y in 0..32 {
            for x in 0..32 {
                let want = 255 - grey(expected_t(x, y, a, b, cw));
                let got = px(&bus, x, y);
                assert!(
                    close(got.a, want),
                    "{kind} ({x}, {y}): {got:?}, want alpha {want}"
                );
                if got.a > 0 {
                    assert_eq!((got.r, got.g, got.b), (40, 120, 200), "{kind} ({x}, {y})");
                }
            }
        }
    }
}

#[test]
fn spiral_inside_a_feathered_selection_changes_only_selected_pixels() {
    // AC-07: fully selected pixels take the spiral, half-selected ones mix
    // it with the layer by coverage, the rest keep the layer.
    let (a, b) = ((12.5, 12.5), (20.5, 15.5));
    let base = |x: u32, y: u32| Color::rgba((x * 7) as u8, (y * 9) as u8, 90, 255);
    let mut mask = SelectionMask::new_empty(32, 32);
    for y in 4..24 {
        for x in 4..28 {
            mask.set(x, y, if x < 20 { 255 } else { 128 });
        }
    }
    for (kind, _) in KINDS {
        let mut full = bus(32, 32, base);
        drag(&mut full, kind, "color", a, b);
        let mut sel = bus(32, 32, base);
        sel.apply(Box::new(SetSelection::replace(mask.clone())))
            .expect("select");
        drag(&mut sel, kind, "color", a, b);
        for y in 0..32 {
            for x in 0..32 {
                let (got, before, spiral) = (px(&sel, x, y), base(x, y), px(&full, x, y));
                let cov = u32::from(mask.coverage(x, y));
                let lerp = |o: u8, n: u8| {
                    ((u32::from(o) * (255 - cov) + u32::from(n) * cov + 127) / 255) as u8
                };
                let want = Color::rgba(
                    lerp(before.r, spiral.r),
                    lerp(before.g, spiral.g),
                    lerp(before.b, spiral.b),
                    lerp(before.a, spiral.a),
                );
                assert_eq!(got, want, "{kind} ({x}, {y}) coverage {cov}");
            }
        }
        // The spiral really changed the selected part.
        assert_ne!(px(&sel, 10, 10), base(10, 10), "{kind}");
    }
}

#[test]
fn spiral_is_one_undo_step_and_undo_restores_the_layer_exactly() {
    // AC-08, through the editor's drag and through `draw_gradient`.
    let base = |x: u32, y: u32| Color::rgba((x * 11) as u8, (y * 13) as u8, 77, 200);
    for (kind, _) in KINDS {
        for live in [true, false] {
            let mut bus = bus(24, 20, base);
            let before = bus.document.layers()[0].pixels.clone();
            let (a, b) = ((7.25, 9.5), (15.0, 4.75));
            if live {
                drag(&mut bus, kind, "color", a, b);
            } else {
                run(
                    &mut bus,
                    with_type(fields(kind, "color", a, b), "draw_gradient"),
                );
            }
            let after = bus.document.layers()[0].pixels.clone();
            assert_ne!(after, before, "{kind} live={live}");
            assert_eq!(bus.history.undo_depth(), 1, "{kind} live={live}");
            assert!(bus.undo().expect("undo"), "{kind} live={live}");
            assert_eq!(
                bus.document.layers()[0].pixels,
                before,
                "{kind} live={live}"
            );
            assert!(!bus.history.can_undo(), "{kind} live={live}");
            bus.redo().expect("redo");
            assert_eq!(bus.document.layers()[0].pixels, after, "{kind} live={live}");
        }
    }
}

#[test]
fn spiral_zero_length_drag_changes_nothing_and_adds_no_step() {
    // AC-09, through the editor's drag and through `draw_gradient`.
    let base = |x: u32, y: u32| Color::rgba((x * 11) as u8, (y * 13) as u8, 77, 255);
    for (kind, _) in KINDS {
        for channel in ["color", "transparency"] {
            let mut bus = bus(16, 16, base);
            let before = bus.document.layers()[0].pixels.clone();
            let at = (6.5, 7.25);
            run(
                &mut bus,
                with_type(fields(kind, channel, at, at), "begin_live_gradient"),
            );
            assert!(bus.live_object().is_none(), "{kind} {channel}");
            run(
                &mut bus,
                with_type(fields(kind, channel, at, at), "draw_gradient"),
            );
            assert_eq!(bus.document.layers()[0].pixels, before, "{kind} {channel}");
            assert_eq!(bus.history.undo_depth(), 0, "{kind} {channel}");
        }
    }
}

#[test]
fn spiral_draw_gradient_paints_what_the_editor_drag_paints() {
    // `draw_gradient` (MCP's path) and the live gradient agree pixel for
    // pixel, in both channels.
    let base = |x: u32, y: u32| Color::rgba((x * 5) as u8, 60, (y * 6) as u8, 255);
    for (kind, _) in KINDS {
        for channel in ["color", "transparency"] {
            let (a, b) = ((3.75, 18.0), (21.5, 2.25));
            let mut live = bus(30, 26, base);
            drag(&mut live, kind, channel, a, b);
            let mut once = bus(30, 26, base);
            run(
                &mut once,
                with_type(fields(kind, channel, a, b), "draw_gradient"),
            );
            assert_eq!(
                live.document.layers()[0].pixels,
                once.document.layers()[0].pixels,
                "{kind} {channel}"
            );
        }
    }
}
