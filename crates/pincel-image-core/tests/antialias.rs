//! The shared Antialiasing setting in the core
//! (docs/specs/image-edit-extras-tool-options.md AC-02 – AC-04, AC-06, AC-07).

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{CommandBus, Document, Rect, SelectionMask};

fn bus(w: u32, h: u32) -> CommandBus {
    CommandBus::new(Document::new(w, h).expect("doc"))
}

fn run(bus: &mut CommandBus, json: &str) {
    let spec: CommandSpec = serde_json::from_str(json).expect("json");
    run_command(bus, spec, &mut StrokeScratch::default(), &[]).expect("run");
}

/// Alpha of every pixel after one dab of `shape` at (10.3, 10.6).
fn dab(shape: &str, size: u32, anti_alias: bool) -> Vec<u8> {
    let mut b = bus(21, 21);
    run(
        &mut b,
        &format!(
            r#"{{"type":"pencil_stroke","layer":0,"size":{size},"color":[255,0,0,255],"opacity":1,"shape":"{shape}","hardness":0.5,"points":[[10.3,10.6]],"anti_alias":{anti_alias},"stroke_id":1}}"#
        ),
    );
    b.document.layers()[0]
        .pixels
        .data()
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .collect()
}

/// Whether the whole pixel (x, y) lies inside / outside the circle of radius
/// `r` around (cx, cy).
fn corners(x: u32, y: u32, (cx, cy, r): (f32, f32, f32)) -> (bool, bool) {
    let d = |px: f32, py: f32| ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    let cs = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
        .map(|(ox, oy)| d(x as f32 + ox, y as f32 + oy));
    let nearest_x = (cx).clamp(x as f32, x as f32 + 1.0);
    let nearest_y = (cy).clamp(y as f32, y as f32 + 1.0);
    let inside = cs.iter().all(|&c| c <= r);
    let outside = d(nearest_x, nearest_y) >= r;
    (inside, outside)
}

#[test]
fn hard_round_dab_is_all_or_nothing_off_and_blends_its_edge_on() {
    let circle = (10.3, 10.6, 5.5);
    let off = dab("hard_round", 11, false);
    let on = dab("hard_round", 11, true);
    let mut partial = 0;
    for y in 0..21u32 {
        for x in 0..21u32 {
            let i = (y * 21 + x) as usize;
            // Off: fully painted exactly when the centre lies inside.
            let centre_in =
                ((x as f32 + 0.5 - circle.0).powi(2) + (y as f32 + 0.5 - circle.1).powi(2)).sqrt()
                    <= circle.2;
            assert_eq!(off[i], if centre_in { 255 } else { 0 }, "off ({x}, {y})");
            let (inside, outside) = corners(x, y, circle);
            if inside {
                assert_eq!(on[i], 255, "on inside ({x}, {y})");
            }
            if outside {
                assert_eq!(on[i], 0, "on outside ({x}, {y})");
            }
            if on[i] > 0 && on[i] < 255 {
                partial += 1;
            }
        }
    }
    assert!(partial > 0, "the edge blends");
}

#[test]
fn soft_and_flat_tips_ignore_the_setting() {
    for shape in ["soft_round", "flat"] {
        assert_eq!(dab(shape, 9, true), dab(shape, 9, false), "{shape}");
    }
}

#[test]
fn ellipse_and_lasso_outlines_blend_their_edge_only_when_on() {
    let hard = SelectionMask::ellipse(30, 20, Rect::new(3, 2, 21, 15));
    let soft = SelectionMask::ellipse_aa(30, 20, Rect::new(3, 2, 21, 15));
    let tri = [
        pincel_image_core::Point::new(2.0, 2.0),
        pincel_image_core::Point::new(27.0, 5.0),
        pincel_image_core::Point::new(9.0, 18.0),
    ];
    let hard_poly = SelectionMask::polygon(30, 20, &tri);
    let soft_poly = SelectionMask::polygon_aa(30, 20, &tri);
    for (h, s) in [(&hard, &soft), (&hard_poly, &soft_poly)] {
        let mut partial = 0;
        for y in 0..20 {
            for x in 0..30 {
                let hc = h.coverage(x, y);
                assert!(hc == 0 || hc == 255);
                let sc = s.coverage(x, y);
                if (1..255).contains(&sc) {
                    partial += 1;
                }
            }
        }
        assert!(partial > 0);
    }
    // Deep inside and far outside are full / empty.
    assert_eq!(soft.coverage(13, 9), 255);
    assert_eq!(soft.coverage(0, 0), 0);
    assert_eq!(soft_poly.coverage(10, 6), 255);
    assert_eq!(soft_poly.coverage(28, 18), 0);
}

#[test]
fn select_ellipse_and_polygon_commands_take_the_setting_and_combine() {
    let mut b = bus(30, 20);
    run(
        &mut b,
        r#"{"type":"select_ellipse","x":3,"y":2,"w":21,"h":15,"anti_alias":true}"#,
    );
    let want = SelectionMask::ellipse_aa(30, 20, Rect::new(3, 2, 21, 15));
    assert_eq!(b.document.selection.as_ref(), Some(&want));
    // Add = maximum of both coverages.
    run(
        &mut b,
        r#"{"type":"select_polygon","points":[[0,0],[10,0],[0,10]],"mode":"add","anti_alias":true}"#,
    );
    let sel = b.document.selection.as_ref().expect("selection");
    let tri = SelectionMask::polygon_aa(
        30,
        20,
        &[
            pincel_image_core::Point::new(0.0, 0.0),
            pincel_image_core::Point::new(10.0, 0.0),
            pincel_image_core::Point::new(0.0, 10.0),
        ],
    );
    for y in 0..20 {
        for x in 0..30 {
            assert_eq!(
                sel.coverage(x, y),
                want.coverage(x, y).max(tri.coverage(x, y))
            );
        }
    }
    // Without the field the outline stays hard, as before (rectangle and
    // wand never had one).
    let mut c = bus(30, 20);
    run(
        &mut c,
        r#"{"type":"select_ellipse","x":3,"y":2,"w":21,"h":15}"#,
    );
    assert_eq!(
        c.document.selection.as_ref(),
        Some(&SelectionMask::ellipse(30, 20, Rect::new(3, 2, 21, 15)))
    );
}
