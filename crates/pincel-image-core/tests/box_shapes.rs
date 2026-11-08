//! The box shapes of docs/specs/image-shapes-curves-shapes.md in the core:
//! geometry (AC-02 – AC-07), painting (AC-08, AC-09), symmetry (AC-10),
//! selection (AC-13) and degenerate boxes (AC-14, AC-15).

use pincel_image_core::{
    Color, CommandBus, DashPattern, Document, ImageBuffer, Point, Rect, SelectionMask, Shape,
    ShapeMode, ShapeStyle, Shapes,
};

const W: u32 = 80;
const H: u32 = 64;
const RED: Color = Color {
    r: 220,
    g: 0,
    b: 0,
    a: 255,
};
const BLUE: Color = Color {
    r: 0,
    g: 0,
    b: 230,
    a: 255,
};

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn style(mode: ShapeMode, width: f32, aa: bool) -> ShapeStyle {
    ShapeStyle {
        mode,
        stroke_width: width,
        stroke_color: RED,
        fill_color: BLUE,
        anti_alias: aa,
        dash: DashPattern::Solid,
        ..ShapeStyle::default()
    }
}

fn fill(aa: bool) -> ShapeStyle {
    style(ShapeMode::Fill, 1.0, aa)
}

/// The layer after drawing on a transparent `W × H` layer (`None`: the
/// shape drew nothing).
fn draw_sel(shape: Shape, style: ShapeStyle, sel: Option<SelectionMask>) -> Option<Vec<[u8; 4]>> {
    let mut doc = Document::from_pixels(ImageBuffer::new_transparent(W, H)).expect("doc");
    doc.selection = sel;
    let mut bus = CommandBus::new(doc);
    let cmd = Shapes::new(shape, style).draw(0, &bus.document)?;
    bus.apply(Box::new(cmd)).expect("apply");
    Some(
        bus.document.layers()[0]
            .pixels
            .data()
            .as_chunks::<4>()
            .0
            .to_vec(),
    )
}

fn draw(shape: Shape, style: ShapeStyle) -> Vec<[u8; 4]> {
    draw_sel(shape, style, None).expect("draws")
}

fn alpha(px: &[[u8; 4]], x: u32, y: u32) -> u8 {
    px[(y * W + x) as usize][3]
}

/// Signed distance to the polygon `v` (negative inside) — the test's own.
fn sd(x: f32, y: f32, v: &[Point]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[(i + 1) % v.len()]);
        let (ex, ey, wx, wy) = (b.x - a.x, b.y - a.y, x - a.x, y - a.y);
        let t = ((wx * ex + wy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        d = d.min((wx - ex * t).hypot(wy - ey * t));
        if (a.y > y) != (b.y > y) && x < a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    if inside { -d } else { d }
}

/// AC-09 against the polygon `v`: anti-aliased fill covers every pixel
/// whose centre lies more than 0.5 px inside and nothing more than 0.5 px
/// outside.
#[track_caller]
fn fill_matches(shape: Shape, v: &[Point]) {
    let px = draw(shape, fill(true));
    let mut inside = 0;
    for y in 0..H {
        for x in 0..W {
            let d = sd(x as f32 + 0.5, y as f32 + 0.5, v);
            let a = alpha(&px, x, y);
            if d < -0.5 - 1e-3 {
                assert_eq!(a, 255, "{shape:?}: ({x}, {y}) inside at {d}");
                inside += 1;
            } else if d > 0.5 + 1e-3 {
                assert_eq!(a, 0, "{shape:?}: ({x}, {y}) outside at {d}");
            }
        }
    }
    assert!(inside > 20, "{shape:?} fills something");
}

#[test]
fn triangle_and_diamond_fill_their_box() {
    let (a, b) = (p(10.0, 6.0), p(70.0, 54.0));
    fill_matches(
        Shape::Triangle { a, b },
        &[p(10.0, 54.0), p(40.0, 6.0), p(70.0, 54.0)],
    );
    // The drag direction does not matter.
    fill_matches(
        Shape::Triangle { a: b, b: a },
        &[p(10.0, 54.0), p(40.0, 6.0), p(70.0, 54.0)],
    );
    fill_matches(
        Shape::Diamond { a, b },
        &[p(40.0, 6.0), p(70.0, 30.0), p(40.0, 54.0), p(10.0, 30.0)],
    );
}

/// A star's vertices as AC-04 / Technical notes define them.
fn star(cx: f32, cy: f32, r: f32, n: u32, ratio: f32) -> Vec<Point> {
    (0..2 * n)
        .map(|k| {
            let rad = if k % 2 == 0 { r } else { r * ratio };
            let ang = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / n as f32;
            p(cx + rad * ang.cos(), cy + rad * ang.sin())
        })
        .collect()
}

#[test]
fn a_star_has_its_points_and_inner_radius() {
    for (n, ratio) in [(5, 0.38), (3, 0.9), (8, 0.5)] {
        let shape = Shape::Star {
            center: p(40.0, 32.0),
            radius: 28.0,
            points: n,
            inner_ratio: ratio,
            rotation: 0.0,
        };
        fill_matches(shape, &star(40.0, 32.0, 28.0, n, ratio));
    }
    // One point straight up: the top-centre pixel near the tip is painted,
    // the pixels beside the tip are not.
    let px = draw(
        Shape::Star {
            center: p(40.0, 32.0),
            radius: 28.0,
            points: 5,
            inner_ratio: 0.38,
            rotation: 0.0,
        },
        fill(true),
    );
    assert_eq!(alpha(&px, 39, 7), 255);
    assert_eq!(alpha(&px, 33, 7), 0);
}

/// Block Arrow vertices pointing right in `(x0, y0)–(x1, y1)` (Technical
/// notes): head min(A / 2, T), shaft T / 2.
fn arrow_right(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Point> {
    let (a, t) = (x1 - x0, y1 - y0);
    let head = (a / 2.0).min(t);
    let cy = (y0 + y1) / 2.0;
    vec![
        p(x0, cy - t / 4.0),
        p(x1 - head, cy - t / 4.0),
        p(x1 - head, y0),
        p(x1, cy),
        p(x1 - head, y1),
        p(x1 - head, cy + t / 4.0),
        p(x0, cy + t / 4.0),
    ]
}

#[test]
fn a_block_arrow_points_from_the_drag_start_along_the_longer_side() {
    let (l, t, r, b) = (8.0, 14.0, 72.0, 50.0);
    // Dragged left to right: points right.
    fill_matches(
        Shape::BlockArrow {
            start: p(l, t),
            end: p(r, b),
        },
        &arrow_right(l, t, r, b),
    );
    // Right to left: the mirror image, pointing left.
    let left: Vec<Point> = arrow_right(l, t, r, b)
        .iter()
        .map(|q| p(l + r - q.x, q.y))
        .collect();
    fill_matches(
        Shape::BlockArrow {
            start: p(r, b),
            end: p(l, t),
        },
        &left,
    );
    // Taller than wide: vertical; dragged upward, it points up.
    let (l, t, r, b) = (24.0, 4.0, 56.0, 60.0);
    let up: Vec<Point> = arrow_right(t, l, b, r)
        .iter()
        .map(|q| p(q.y, t + b - q.x))
        .collect();
    fill_matches(
        Shape::BlockArrow {
            start: p(l, b),
            end: p(r, t),
        },
        &up,
    );
    // A tie (square box) points horizontally.
    let px = draw(
        Shape::BlockArrow {
            start: p(10.0, 10.0),
            end: p(50.0, 50.0),
        },
        fill(true),
    );
    // Near the right tip (inside a right-pointing head, outside a
    // down-pointing shaft) and its transpose (the reverse).
    assert_eq!(alpha(&px, 45, 28), 255, "pointing right");
    assert_eq!(alpha(&px, 28, 45), 0, "not pointing down");
}

#[test]
fn a_heart_touches_its_box_with_lobes_up_and_its_point_down() {
    let (x0, y0, x1, y1) = (10u32, 8u32, 70u32, 56u32);
    let px = draw(
        Shape::Heart {
            a: p(x0 as f32, y0 as f32),
            b: p(x1 as f32, y1 as f32),
        },
        fill(true),
    );
    let row = |y: u32| {
        (x0..x1)
            .filter(|&x| alpha(&px, x, y) > 0)
            .collect::<Vec<_>>()
    };
    let col = |x: u32| (y0..y1).filter(|&y| alpha(&px, x, y) > 0).count();
    // Touches all four edges, paints nothing outside the box.
    assert!(!row(y0).is_empty() && !row(y1 - 1).is_empty());
    assert!(col(x0) > 0 && col(x1 - 1) > 0);
    for (i, c) in px.iter().enumerate() {
        let (x, y) = (i as u32 % W, i as u32 / W);
        if x < x0 || x >= x1 || y < y0 || y >= y1 {
            assert_eq!(c[3], 0, "({x}, {y}) outside the box");
        }
    }
    // Two lobes on the top row (the notch between them is empty), the
    // point at the bottom centre.
    let top = row(y0);
    let mid = (x0 + x1) / 2;
    assert!(top.iter().any(|&x| x < mid - 2) && top.iter().any(|&x| x > mid + 1));
    assert_eq!(alpha(&px, mid, y0 + 2), 0, "the notch");
    assert_eq!(row(y1 - 1), vec![mid - 1, mid], "the point");
    assert_eq!(alpha(&px, mid, (y0 + y1) / 2), 255, "solid inside");
}

#[test]
fn a_callout_is_a_body_with_a_tail_to_the_start_corner_in_one_outline() {
    for (start, end) in [
        ((8.0, 6.0), (72.0, 58.0)),
        ((72.0, 58.0), (8.0, 6.0)),
        ((8.0, 58.0), (72.0, 6.0)),
    ] {
        let (s, e) = (p(start.0, start.1), p(end.0, end.1));
        let shape = Shape::Callout { start: s, end: e };
        let px = draw(shape, fill(true));
        // The tip on the start corner.
        let tip = |dx: f32, dy: f32| {
            let x = (s.x + dx * if e.x > s.x { 1.0 } else { -1.0 }).floor() as u32;
            let y = (s.y + dy * if e.y > s.y { 1.0 } else { -1.0 }).floor() as u32;
            alpha(&px, x, y)
        };
        assert!(
            tip(1.5, 1.5) > 0,
            "{start:?}: the tail reaches the start corner"
        );
        // The body: the far 80 % of the box, solid in its middle; the near
        // 20 % holds only the tail.
        assert_eq!(alpha(&px, 40, ((s.y + e.y) / 2.0) as u32), 255);
        assert_eq!(tip(56.0, 4.0), 0, "{start:?}: beside the tail is empty");
        // One contour: an outline has no stroke across the tail's base.
        let outline = draw(shape, style(ShapeMode::Outline, 2.0, true));
        let base = tip(16.0, 0.2 * 52.0);
        let base_outline = {
            let x = (s.x + 16.0 * if e.x > s.x { 1.0 } else { -1.0 }).floor() as u32;
            let y = (s.y + 0.2 * 52.0 * if e.y > s.y { 1.0 } else { -1.0 }).floor() as u32;
            alpha(&outline, x, y)
        };
        assert_eq!(base, 255, "{start:?}: inside the union");
        assert_eq!(
            base_outline, 0,
            "{start:?}: no stroke between body and tail"
        );
    }
}

#[test]
fn new_shapes_paint_like_the_existing_ones() {
    let shapes = all_shapes(p(10.0, 6.0), p(70.0, 58.0));
    for shape in shapes {
        // Fill + outline: blue inside, red on the border.
        let px = draw(shape, style(ShapeMode::FillAndOutline, 3.0, false));
        assert!(px.iter().any(|c| c == &[0, 0, 230, 255]), "{shape:?}: fill");
        assert!(
            px.iter().any(|c| c == &[220, 0, 0, 255]),
            "{shape:?}: outline"
        );
        // Anti-alias off: every pixel unchanged or written at full coverage.
        for c in &px {
            assert!(
                c[3] == 0 || c == &[0, 0, 230, 255] || c == &[220, 0, 0, 255],
                "{shape:?}: {c:?}"
            );
        }
        // Outline only: the inside stays empty; dashes paint less than solid.
        let outline = draw(shape, style(ShapeMode::Outline, 2.0, true));
        let mut dashed_style = style(ShapeMode::Outline, 2.0, true);
        dashed_style.dash = DashPattern::DashDot;
        let dashed = draw(shape, dashed_style);
        let count = |v: &[[u8; 4]]| v.iter().filter(|c| c[3] > 0).count();
        assert!(count(&dashed) < count(&outline), "{shape:?}: dashes");
    }
}

fn all_shapes(a: Point, b: Point) -> Vec<Shape> {
    let c = p((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let r = (b.x - a.x).abs().min((b.y - a.y).abs()) / 2.0;
    vec![
        Shape::Triangle { a, b },
        Shape::Diamond { a, b },
        Shape::Star {
            center: c,
            radius: r,
            points: 5,
            inner_ratio: 0.38,
            rotation: 0.0,
        },
        Shape::BlockArrow { start: a, end: b },
        Shape::Heart { a, b },
        Shape::Callout { start: a, end: b },
    ]
}

#[test]
fn symmetric_shapes_paint_mirror_symmetric_pixels() {
    let (a, b) = (p(9.0, 5.0), p(69.0, 59.0));
    let c = p(39.0, 32.0);
    let vertical = [
        Shape::Triangle { a, b },
        Shape::Diamond { a, b },
        Shape::Star {
            center: c,
            radius: 26.0,
            points: 5,
            inner_ratio: 0.38,
            rotation: 0.0,
        },
        Shape::Star {
            center: c,
            radius: 26.0,
            points: 7,
            inner_ratio: 0.6,
            rotation: 0.0,
        },
        Shape::Heart { a, b },
        Shape::BlockArrow {
            start: p(24.0, 4.0),
            end: p(54.0, 60.0),
        },
        Shape::BlockArrow {
            start: p(54.0, 60.0),
            end: p(24.0, 4.0),
        },
    ];
    let horizontal = [
        Shape::Diamond { a, b },
        Shape::BlockArrow {
            start: p(8.0, 14.0),
            end: p(70.0, 50.0),
        },
        Shape::BlockArrow {
            start: p(70.0, 50.0),
            end: p(8.0, 14.0),
        },
    ];
    for aa in [true, false] {
        for mode in [
            ShapeMode::Fill,
            ShapeMode::Outline,
            ShapeMode::FillAndOutline,
        ] {
            for shape in vertical {
                let px = draw(shape, style(mode, 3.0, aa));
                let axis2 = match shape {
                    Shape::BlockArrow { start, end } => (start.x + end.x) as u32,
                    Shape::Star { center, .. } => (2.0 * center.x) as u32,
                    _ => (a.x + b.x) as u32,
                };
                for y in 0..H {
                    for x in 0..W {
                        let mx = axis2 as i64 - 1 - x as i64;
                        if (0..W as i64).contains(&mx) {
                            assert_eq!(
                                px[(y * W + x) as usize],
                                px[(y * W + mx as u32) as usize],
                                "{shape:?} {mode:?} aa={aa} ({x}, {y})"
                            );
                        }
                    }
                }
            }
            for shape in horizontal {
                let px = draw(shape, style(mode, 3.0, aa));
                let axis2 = match shape {
                    Shape::BlockArrow { start, end } => (start.y + end.y) as u32,
                    _ => (a.y + b.y) as u32,
                };
                for y in 0..H {
                    let my = axis2 as i64 - 1 - y as i64;
                    if !(0..H as i64).contains(&my) {
                        continue;
                    }
                    for x in 0..W {
                        assert_eq!(
                            px[(y * W + x) as usize],
                            px[(my as u32 * W + x) as usize],
                            "{shape:?} {mode:?} aa={aa} ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn new_shapes_change_only_selected_pixels() {
    let mask = SelectionMask::rectangle(W, H, Rect::new(0, 0, 40, H));
    for shape in all_shapes(p(10.0, 6.0), p(70.0, 58.0)) {
        let s = style(ShapeMode::FillAndOutline, 2.0, true);
        let free = draw(shape, s);
        let bounded = draw_sel(shape, s, Some(mask.clone())).expect("draws");
        for (i, (f, b)) in free.iter().zip(&bounded).enumerate() {
            if (i as u32 % W) < 40 {
                assert_eq!(f, b, "{shape:?}");
            } else {
                assert_eq!(b[3], 0, "{shape:?}: unselected pixel {i}");
            }
        }
    }
}

#[test]
fn zero_area_boxes_draw_nothing_and_tiny_boxes_stay_inside() {
    let doc = Document::from_pixels(ImageBuffer::new_transparent(W, H)).expect("doc");
    for shape in all_shapes(p(20.0, 20.0), p(20.0, 40.0))
        .into_iter()
        .chain(all_shapes(p(20.0, 20.0), p(50.0, 20.0)))
    {
        assert!(
            Shapes::new(shape, fill(true)).draw(0, &doc).is_none(),
            "{shape:?}"
        );
    }
    let star0 = Shape::Star {
        center: p(30.0, 30.0),
        radius: 0.0,
        points: 5,
        inner_ratio: 0.38,
        rotation: 0.0,
    };
    assert!(Shapes::new(star0, fill(true)).draw(0, &doc).is_none());

    // Boxes under 3 px and a 50-point star in a 6 px box: drawn without an
    // error and nothing beyond the box plus half the stroke and 1 px.
    let mut cases = Vec::new();
    for (w, h) in [(1.0, 1.0), (2.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.5, 1.5)] {
        cases.extend(all_shapes(p(30.0, 30.0), p(30.0 + w, 30.0 + h)));
    }
    cases.push(Shape::Star {
        center: p(33.0, 33.0),
        radius: 3.0,
        points: 50,
        inner_ratio: 0.9,
        rotation: 0.0,
    });
    for shape in cases {
        for (mode, width) in [(ShapeMode::Fill, 1.0), (ShapeMode::FillAndOutline, 3.0)] {
            let Some(px) = draw_sel(shape, style(mode, width, true), None) else {
                continue;
            };
            let reach = width / 2.0 + 1.0;
            for (i, c) in px.iter().enumerate() {
                let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5);
                let inside =
                    x > 30.0 - reach && x < 36.0 + reach && y > 30.0 - reach && y < 36.0 + reach;
                assert!(c[3] == 0 || inside, "{shape:?}: stray pixel at ({x}, {y})");
            }
        }
    }
}
