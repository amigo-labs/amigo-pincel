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

/// AC-04, second clause: with Antialiasing off, a soft round or flat dab
/// changes no pixel whose centre lies outside its outline — the circle of
/// the tip's radius, or the flat nib's ellipse (minor axis 0.35 × the
/// radius) turned to the default 45°.
#[test]
fn soft_and_flat_tips_change_nothing_outside_their_outline_off() {
    let (cx, cy, r) = (10.3_f32, 10.6_f32, 4.5_f32);
    let (sin, cos) = 45.0_f32.to_radians().sin_cos();
    for shape in ["soft_round", "flat"] {
        let alpha = dab(shape, 9, false);
        let mut painted = 0;
        for (i, &a) in alpha.iter().enumerate() {
            let (x, y) = (i as f32 % 21.0, (i / 21) as f32);
            let (dx, dy) = (x + 0.5 - cx, y + 0.5 - cy);
            let outside = if shape == "flat" {
                let (lx, ly) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                (lx / r).powi(2) + (ly / (r * 0.35)).powi(2) > 1.0
            } else {
                dx * dx + dy * dy > r * r
            };
            if outside {
                assert_eq!(a, 0, "{shape} ({x}, {y}) lies outside the outline");
            } else if a > 0 {
                painted += 1;
            }
        }
        assert!(painted > 0, "{shape} paints inside its outline");
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

#[test]
fn feather_applies_on_top_of_an_anti_aliased_outline() {
    let mut b = bus(30, 20);
    run(
        &mut b,
        r#"{"type":"select_ellipse","x":3,"y":2,"w":21,"h":15,"feather":3,"anti_alias":true}"#,
    );
    let mut want = SelectionMask::ellipse_aa(30, 20, Rect::new(3, 2, 21, 15));
    want.feather(3);
    assert_eq!(b.document.selection.as_ref(), Some(&want));
    let mut c = bus(30, 20);
    run(
        &mut c,
        r#"{"type":"select_polygon","points":[[2,2],[25,4],[9,17]],"feather":2,"anti_alias":false}"#,
    );
    let mut hard = SelectionMask::polygon(
        30,
        20,
        &[
            pincel_image_core::Point::new(2.0, 2.0),
            pincel_image_core::Point::new(25.0, 4.0),
            pincel_image_core::Point::new(9.0, 17.0),
        ],
    );
    hard.feather(2);
    assert_eq!(c.document.selection.as_ref(), Some(&hard));
}

/// A `w × h` document whose one layer is filled by `paint(x, y)`.
fn bus_with(w: u32, h: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> CommandBus {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&paint(x, y));
        }
    }
    let buf = pincel_image_core::ImageBuffer::from_raw(w, h, data).expect("buffer");
    CommandBus::new(Document::from_pixels(buf).expect("doc"))
}

fn pixels(bus: &CommandBus) -> Vec<[u8; 4]> {
    bus.document.layers()[0]
        .pixels
        .data()
        .as_chunks::<4>()
        .0
        .to_vec()
}

/// Distance from the centre of pixel (x, y) to the segment a–b.
fn centre_to_segment(x: u32, y: u32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// AC-02 for the tools besides the Brush: one hard-round dab of the Eraser
/// and the Recolor changes a pixel fully or not at all, exactly when its
/// centre lies inside the tip; with Antialiasing on, the edge blends.
#[test]
fn eraser_and_recolor_hard_round_dabs_are_all_or_nothing_off() {
    let circle = (10.3, 10.6, 5.5);
    let tools = [
        (
            "eraser",
            r#"{"type":"eraser_stroke","layer":0,"size":11,"opacity":1,"shape":"hard_round","points":[[10.3,10.6]],"anti_alias":AA,"stroke_id":1}"#,
            [255, 0, 0, 0],
        ),
        (
            "recolor",
            r#"{"type":"recolor_stroke","layer":0,"size":11,"opacity":1,"shape":"hard_round","from":[255,0,0,255],"to":[0,0,255,255],"tolerance":0,"points":[[10.3,10.6]],"anti_alias":AA,"stroke_id":1}"#,
            BLUE,
        ),
    ];
    for (tool, json, full) in tools {
        let mut partial = 0;
        for aa in [false, true] {
            let mut b = bus_with(21, 21, |_, _| RED);
            run(&mut b, &json.replace("AA", &aa.to_string()));
            for (i, px) in pixels(&b).into_iter().enumerate() {
                let (x, y) = (i as u32 % 21, i as u32 / 21);
                let inside =
                    centre_to_segment(x, y, (circle.0, circle.1), (circle.0, circle.1)) <= circle.2;
                if !aa {
                    // Erased pixels keep their colour; only alpha counts.
                    let px = if tool == "eraser" && px[3] == 0 {
                        [255, 0, 0, 0]
                    } else {
                        px
                    };
                    assert_eq!(px, if inside { full } else { RED }, "{tool} off ({x}, {y})");
                } else if px != RED && px != full {
                    partial += 1;
                }
            }
        }
        assert!(partial > 0, "{tool}: the edge blends with Antialiasing on");
    }
}

/// AC-02 for the Smudge: dragging black into white at full strength leaves
/// every pixel exactly black or white with Antialiasing off — each pixel is
/// carried fully or not at all — only pixels whose centre a dab of the drag
/// covers change. (A pixel under a dab may keep its colour: the tip carries
/// what it first sampled at that offset, which can be the white it is
/// already over.)
#[test]
fn smudge_hard_round_dabs_are_all_or_nothing_off() {
    let black = [0, 0, 0, 255];
    let white = [255, 255, 255, 255];
    let (from, to, r) = ((6.5, 10.4), (17.2, 10.7), 3.5);
    let json = r#"{"type":"smudge_stroke","layer":0,"size":7,"opacity":1,"shape":"hard_round","strength":1,"points":[[6.5,10.4],[17.2,10.7]],"anti_alias":AA,"stroke_id":1}"#;
    let start = |x: u32, _| if x < 11 { black } else { white };
    let mut greys = 0;
    let mut carried = 0;
    for aa in [false, true] {
        let mut b = bus_with(24, 21, start);
        run(&mut b, &json.replace("AA", &aa.to_string()));
        for (i, px) in pixels(&b).into_iter().enumerate() {
            let (x, y) = (i as u32 % 24, i as u32 / 24);
            if aa {
                if px != black && px != white {
                    greys += 1;
                }
                continue;
            }
            assert!(px == black || px == white, "off ({x}, {y}) mixed: {px:?}");
            if px != start(x, y) {
                assert!(
                    centre_to_segment(x, y, from, to) <= r,
                    "off ({x}, {y}) outside the drag"
                );
            }
            if start(x, y) == white && px == black {
                carried += 1;
            }
        }
    }
    assert!(carried > 0, "the drag carries black into the white half");
    assert!(greys > 0, "the edge blends with Antialiasing on");
}

/// Whether (px, py) lies inside the ellipse in the rect (3, 2, 21, 15).
fn in_ellipse(px: f32, py: f32) -> bool {
    ((px - 13.5) / 10.5).powi(2) + ((py - 9.5) / 7.5).powi(2) <= 1.0
}

const TRI: [(f32, f32); 3] = [(2.0, 2.0), (27.0, 5.0), (9.0, 18.0)];

/// Signed side of (px, py) against every edge of the triangle; `None` when
/// the point lies exactly on an edge.
fn in_triangle(px: f32, py: f32) -> Option<bool> {
    let side =
        |(ax, ay): (f32, f32), (bx, by): (f32, f32)| (bx - ax) * (py - ay) - (by - ay) * (px - ax);
    let s = [
        side(TRI[0], TRI[1]),
        side(TRI[1], TRI[2]),
        side(TRI[2], TRI[0]),
    ];
    if s.contains(&0.0) {
        return None;
    }
    Some(s.iter().all(|&v| v > 0.0) || s.iter().all(|&v| v < 0.0))
}

/// Where a pixel lies against a convex outline, sampled on a 17 × 17 grid
/// over the whole pixel square (edges included): fully inside, fully
/// outside, or crossed by the outline.
fn place(x: u32, y: u32, inside: impl Fn(f32, f32) -> bool) -> (bool, bool) {
    let mut any_in = false;
    let mut any_out = false;
    for j in 0..=16 {
        for i in 0..=16 {
            if inside(x as f32 + i as f32 / 16.0, y as f32 + j as f32 / 16.0) {
                any_in = true;
            } else {
                any_out = true;
            }
        }
    }
    (any_in && !any_out, any_out && !any_in)
}

/// AC-06 over every pixel: with Antialiasing on, an Ellipse Select or
/// (Polygon) Lasso outline gives every pixel fully inside coverage 255 and
/// every pixel fully outside 0, and blends the edge; off, coverage is 0 or
/// 255 decided by the pixel's centre.
#[test]
fn ellipse_and_polygon_coverage_follows_the_outline_for_every_pixel() {
    let tri: Vec<_> = TRI
        .iter()
        .map(|&(x, y)| pincel_image_core::Point::new(x, y))
        .collect();
    let rect = Rect::new(3, 2, 21, 15);
    let cases = [
        (
            "ellipse",
            SelectionMask::ellipse_aa(30, 20, rect),
            SelectionMask::ellipse(30, 20, rect),
        ),
        (
            "polygon",
            SelectionMask::polygon_aa(30, 20, &tri),
            SelectionMask::polygon(30, 20, &tri),
        ),
    ];
    for (name, on, off) in cases {
        let inside = |px: f32, py: f32| {
            if name == "ellipse" {
                in_ellipse(px, py)
            } else {
                in_triangle(px, py).unwrap_or(false)
            }
        };
        let mut partial = 0;
        for y in 0..20 {
            for x in 0..30 {
                let (full, empty) = place(x, y, inside);
                let c = on.coverage(x, y);
                if full {
                    assert_eq!(c, 255, "{name} on: ({x}, {y}) lies fully inside");
                } else if empty {
                    assert_eq!(c, 0, "{name} on: ({x}, {y}) lies fully outside");
                } else if (1..255).contains(&c) {
                    partial += 1;
                }
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let centre = if name == "ellipse" {
                    Some(in_ellipse(cx, cy))
                } else {
                    in_triangle(cx, cy)
                };
                if let Some(centre) = centre {
                    let want = if centre { 255 } else { 0 };
                    assert_eq!(off.coverage(x, y), want, "{name} off: ({x}, {y})");
                }
            }
        }
        assert!(partial > 0, "{name}: the edge blends");
    }
}

/// AC-06, last sentence: an anti-aliased outline joins the selection by the
/// mode's coverage rule — subtract takes its coverage away, intersect keeps
/// the minimum.
#[test]
fn anti_aliased_outlines_subtract_and_intersect_by_coverage() {
    let tri = SelectionMask::polygon_aa(
        30,
        20,
        &[
            pincel_image_core::Point::new(0.0, 0.0),
            pincel_image_core::Point::new(10.0, 0.0),
            pincel_image_core::Point::new(0.0, 10.0),
        ],
    );
    let ellipse = SelectionMask::ellipse_aa(30, 20, Rect::new(3, 2, 21, 15));
    for (mode, rule) in [
        (
            "subtract",
            (|a: u8, b: u8| a.saturating_sub(b)) as fn(u8, u8) -> u8,
        ),
        ("intersect", |a: u8, b: u8| a.min(b)),
    ] {
        let mut b = bus(30, 20);
        run(
            &mut b,
            r#"{"type":"select_ellipse","x":3,"y":2,"w":21,"h":15,"anti_alias":true}"#,
        );
        run(
            &mut b,
            &format!(
                r#"{{"type":"select_polygon","points":[[0,0],[10,0],[0,10]],"mode":"{mode}","anti_alias":true}}"#
            ),
        );
        let sel = b.document.selection.as_ref().expect("selection");
        let mut blended = 0;
        for y in 0..20 {
            for x in 0..30 {
                let (a, t) = (ellipse.coverage(x, y), tri.coverage(x, y));
                assert_eq!(sel.coverage(x, y), rule(a, t), "{mode} ({x}, {y})");
                if a > 0 && (1..255).contains(&t) {
                    blended += 1;
                }
            }
        }
        assert!(
            blended > 0,
            "{mode}: partial edge pixels meet the selection"
        );
    }
}
