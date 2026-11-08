//! The Line / Curve tool's rasterizer (docs/specs/image-shapes-curves.md
//! AC-01, AC-05 – AC-11, AC-14, AC-16).

use pincel_image_core::{
    Color, CommandBus, Curve, CurveLine, DashPattern, Document, LineCap, Point, Rect,
    SelectionMask, Shape, ShapeMode, ShapeStyle, Shapes,
};

const W: u32 = 120;
const H: u32 = 80;
const RED: Color = Color {
    r: 200,
    g: 20,
    b: 20,
    a: 255,
};

const DASHES: [DashPattern; 5] = [
    DashPattern::Solid,
    DashPattern::Dashed,
    DashPattern::Dotted,
    DashPattern::DashDot,
    DashPattern::DashDotDot,
];
const CAPS: [LineCap; 3] = [LineCap::Round, LineCap::Flat, LineCap::Arrow];

fn style(width: f32, dash: DashPattern, aa: bool) -> ShapeStyle {
    ShapeStyle {
        mode: ShapeMode::Outline,
        stroke_width: width,
        stroke_color: RED,
        anti_alias: aa,
        dash,
        ..ShapeStyle::default()
    }
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn curve(points: &[Point], start: LineCap, end: LineCap) -> Curve {
    Curve {
        points: points.to_vec(),
        start_cap: start,
        end_cap: end,
    }
}

/// The layer's RGBA after drawing `line` on a transparent `W × H` layer.
fn draw(line: &CurveLine) -> Vec<[u8; 4]> {
    draw_with(line, None)
}

fn draw_with(line: &CurveLine, selection: Option<SelectionMask>) -> Vec<[u8; 4]> {
    let mut doc = blank();
    doc.selection = selection;
    let mut bus = CommandBus::new(doc);
    if let Some(cmd) = line.draw(0, &bus.document) {
        bus.apply(Box::new(cmd)).expect("apply");
    }
    bus.document.layers()[0]
        .pixels
        .data()
        .as_chunks::<4>()
        .0
        .to_vec()
}

fn draw_shape(shape: Shape, style: ShapeStyle) -> Vec<[u8; 4]> {
    let mut bus = CommandBus::new(blank());
    if let Some(cmd) = Shapes::new(shape, style).draw(0, &bus.document) {
        bus.apply(Box::new(cmd)).expect("apply");
    }
    bus.document.layers()[0]
        .pixels
        .data()
        .as_chunks::<4>()
        .0
        .to_vec()
}

/// A transparent `W × H` document.
fn blank() -> Document {
    Document::from_pixels(pincel_image_core::ImageBuffer::new_transparent(W, H)).expect("doc")
}

fn centre(i: usize) -> (f32, f32) {
    ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5)
}

// --- An independent reference of the spline (dense samples) -------------

/// Centripetal Catmull-Rom through `pts` (phantom ends reflected), sampled
/// densely: the test's own evaluation, not the tool's flattening.
fn reference(pts: &[Point]) -> Vec<Point> {
    let n = pts.len();
    let get = |i: isize| -> Point {
        if i < 0 {
            p(2.0 * pts[0].x - pts[1].x, 2.0 * pts[0].y - pts[1].y)
        } else if i as usize >= n {
            p(
                2.0 * pts[n - 1].x - pts[n - 2].x,
                2.0 * pts[n - 1].y - pts[n - 2].y,
            )
        } else {
            pts[i as usize]
        }
    };
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (
            get(i as isize - 1),
            get(i as isize),
            get(i as isize + 1),
            get(i as isize + 2),
        );
        let k = |a: Point, b: Point| ((a.x - b.x).hypot(a.y - b.y)).sqrt();
        let (t0, t1) = (0.0, k(p0, p1));
        let t2 = t1 + k(p1, p2);
        let t3 = t2 + k(p2, p3);
        for s in 0..=2000 {
            let t = t1 + (t2 - t1) * s as f32 / 2000.0;
            let l = |a: Point, b: Point, ta: f32, tb: f32| {
                let w = (t - ta) / (tb - ta);
                p(a.x + (b.x - a.x) * w, a.y + (b.y - a.y) * w)
            };
            let (a1, a2, a3) = (l(p0, p1, t0, t1), l(p1, p2, t1, t2), l(p2, p3, t2, t3));
            let (b1, b2) = (l(a1, a2, t0, t2), l(a2, a3, t1, t3));
            out.push(l(b1, b2, t1, t2));
        }
    }
    out
}

fn dist_to(samples: &[Point], x: f32, y: f32) -> f32 {
    samples
        .iter()
        .map(|q| (q.x - x).hypot(q.y - y))
        .fold(f32::MAX, f32::min)
}

/// Signed distance to a triangle (negative inside).
fn sd_triangle(x: f32, y: f32, t: [Point; 3]) -> f32 {
    let seg = |a: Point, b: Point| {
        let (ex, ey, wx, wy) = (b.x - a.x, b.y - a.y, x - a.x, y - a.y);
        let u = ((wx * ex + wy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        (wx - ex * u).hypot(wy - ey * u)
    };
    let d = seg(t[0], t[1]).min(seg(t[1], t[2])).min(seg(t[2], t[0]));
    let cross = |a: Point, b: Point| (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
    let s = [cross(t[0], t[1]), cross(t[1], t[2]), cross(t[2], t[0])];
    let inside = s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0);
    if inside { -d } else { d }
}

/// The arrow head at `tip` pointing along `dir` (unit), `len` long.
fn head(tip: Point, dir: (f32, f32), len: f32) -> [Point; 3] {
    let base = p(tip.x - dir.0 * len, tip.y - dir.1 * len);
    let (nx, ny) = (-dir.1 * len / 2.0, dir.0 * len / 2.0);
    [
        tip,
        p(base.x + nx, base.y + ny),
        p(base.x - nx, base.y - ny),
    ]
}

/// The outward unit tangent at the start (`end` false) or end of `samples`.
fn tangent(samples: &[Point], end: bool) -> (f32, f32) {
    let (a, b) = if end {
        (samples[samples.len() - 1], samples[samples.len() - 2])
    } else {
        (samples[0], samples[1])
    };
    let l = (a.x - b.x).hypot(a.y - b.y);
    ((a.x - b.x) / l, (a.y - b.y) / l)
}

const BENT: [Point; 4] = [
    Point { x: 12.0, y: 60.0 },
    Point { x: 38.0, y: 14.0 },
    Point { x: 70.0, y: 62.0 },
    Point { x: 104.0, y: 22.0 },
];

// --- AC-01 / AC-06: straight lines stay today's pixels -------------------

#[test]
fn a_two_point_round_line_is_todays_line() {
    let (a, b) = (p(10.3, 12.7), p(97.6, 55.2));
    for dash in DASHES {
        for aa in [true, false] {
            for width in [1.0, 5.0] {
                let s = style(width, dash, aa);
                let line = CurveLine::new(Curve::straight(a, b), s);
                assert_eq!(
                    draw(&line),
                    draw_shape(Shape::Line { a, b }, s),
                    "{dash:?} aa={aa} w={width}"
                );
            }
        }
    }
}

#[test]
fn collinear_control_points_draw_the_straight_line() {
    let (a, b) = (p(8.5, 70.25), p(110.0, 9.5));
    let at = |t: f32| p(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    for dash in DASHES {
        for start in CAPS {
            for end in CAPS {
                let s = style(4.0, dash, true);
                let bent =
                    CurveLine::new(curve(&[a, at(0.2), at(0.55), at(0.9), b], start, end), s);
                let straight = CurveLine::new(curve(&[a, b], start, end), s);
                assert_eq!(draw(&bent), draw(&straight), "{dash:?} {start:?} {end:?}");
            }
        }
    }
}

// --- AC-05: the band around the curve --------------------------------------

#[test]
fn a_curve_paints_only_its_band_and_heads_and_fills_its_core() {
    let reference = reference(&BENT);
    // The distance of every pixel centre to the curve, computed once.
    let field: Vec<f32> = (0..(W * H) as usize)
        .map(|i| {
            let (x, y) = centre(i);
            dist_to(&reference, x, y)
        })
        .collect();
    let width = 6.0;
    let half = width / 2.0;
    for dash in DASHES {
        for start in CAPS {
            for end in CAPS {
                let line = CurveLine::new(curve(&BENT, start, end), style(width, dash, true));
                let len = line.head_length();
                let heads: Vec<[Point; 3]> = [(start, false), (end, true)]
                    .into_iter()
                    .filter(|(c, _)| *c == LineCap::Arrow)
                    .map(|(_, e)| {
                        let tip = if e { BENT[3] } else { BENT[0] };
                        head(tip, tangent(&reference, e), len)
                    })
                    .collect();
                let px = draw(&line);
                for (i, c) in px.iter().enumerate() {
                    let (x, y) = centre(i);
                    let d = field[i];
                    // A small tolerance for the triangle and tangent the
                    // test rebuilds from samples.
                    let in_head = heads.iter().any(|&t| sd_triangle(x, y, t) <= 0.5 + 0.1);
                    if d > half + 0.5 + 0.05 && !in_head {
                        assert_eq!(c[3], 0, "{dash:?} {start:?} {end:?}: ({x}, {y}) at {d}");
                    }
                    let near_end = [BENT[0], BENT[3]]
                        .iter()
                        .any(|e| (e.x - x).hypot(e.y - y) <= half + 0.5 + len);
                    let roundish = start == LineCap::Round && end == LineCap::Round;
                    if dash == DashPattern::Solid
                        && d < half - 0.5 - 0.05
                        && (roundish || !near_end)
                    {
                        assert_eq!(c[3], 255, "{start:?} {end:?}: core ({x}, {y}) at {d}");
                    }
                }
            }
        }
    }
}

// --- AC-07: anti-alias off ---------------------------------------------------

#[test]
fn without_anti_alias_pixels_are_full_and_a_thin_curve_is_8_connected() {
    let pts = [p(10.5, 70.5), p(30.5, 8.5), p(55.5, 66.5), p(100.5, 30.5)];
    for dash in DASHES {
        for cap in CAPS {
            let px = draw(&CurveLine::new(
                curve(&pts, cap, cap),
                style(5.0, dash, false),
            ));
            for (i, c) in px.iter().enumerate() {
                assert!(
                    c[3] == 0 || *c == [200, 20, 20, 255],
                    "{dash:?} {cap:?} pixel {i}: {c:?}"
                );
            }
        }
    }
    let px = draw(&CurveLine::new(
        curve(&pts, LineCap::Round, LineCap::Round),
        style(1.0, DashPattern::Solid, false),
    ));
    let painted = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && x < W as i32
            && y < H as i32
            && px[(y as u32 * W + x as u32) as usize][3] > 0
    };
    let (start, goal) = ((10, 70), (100, 30));
    assert!(painted(start.0, start.1) && painted(goal.0, goal.1));
    let mut seen = vec![false; (W * H) as usize];
    let mut stack = vec![start];
    while let Some((x, y)) = stack.pop() {
        let i = (y as u32 * W + x as u32) as usize;
        if seen[i] {
            continue;
        }
        seen[i] = true;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if painted(x + dx, y + dy) {
                    stack.push((x + dx, y + dy));
                }
            }
        }
    }
    assert!(
        seen[(goal.1 as u32 * W + goal.0 as u32) as usize],
        "the path reaches the end"
    );
}

// --- AC-08 / AC-09: caps ---------------------------------------------------

/// Painted pixels near `tip` that lie more than 1 px beyond it along `dir`.
fn beyond(px: &[[u8; 4]], tip: Point, dir: (f32, f32), near: f32) -> Vec<(f32, f32)> {
    px.iter()
        .enumerate()
        .filter(|(_, c)| c[3] > 0)
        .map(|(i, _)| centre(i))
        .filter(|&(x, y)| (x - tip.x).hypot(y - tip.y) <= near)
        .filter(|&(x, y)| (x - tip.x) * dir.0 + (y - tip.y) * dir.1 > 1.0)
        .collect()
}

#[test]
fn an_arrow_head_sits_on_the_end_solid_and_sized_by_the_width() {
    let reference = reference(&BENT);
    for width in [1.0, 4.0] {
        for dash in [DashPattern::Solid, DashPattern::Dotted] {
            let line = CurveLine::new(
                curve(&BENT, LineCap::Round, LineCap::Arrow),
                style(width, dash, true),
            );
            let len = line.head_length();
            assert_eq!(len, (4.0_f32 * width).max(8.0));
            let px = draw(&line);
            let dir = tangent(&reference, true);
            let tri = head(BENT[3], dir, len);
            // Solid inside, whatever the dash pattern.
            for (i, c) in px.iter().enumerate() {
                let (x, y) = centre(i);
                if sd_triangle(x, y, tri) < -0.5 - 0.1 {
                    assert_eq!(c[3], 255, "w={width} {dash:?}: head interior ({x}, {y})");
                }
            }
            assert!(
                beyond(&px, BENT[3], dir, len + width + 2.0).is_empty(),
                "nothing past the tip"
            );
        }
    }
}

#[test]
fn flat_ends_stop_at_the_end_and_round_ends_are_half_discs() {
    let (a, b) = (p(20.5, 40.5), p(90.5, 40.5));
    let flat = draw(&CurveLine::new(
        curve(&[a, b], LineCap::Flat, LineCap::Flat),
        style(10.0, DashPattern::Solid, true),
    ));
    assert!(beyond(&flat, b, (1.0, 0.0), 12.0).is_empty());
    assert!(beyond(&flat, a, (-1.0, 0.0), 12.0).is_empty());
    // Round: a half disc of radius 5 past each end.
    let round = draw(&CurveLine::new(
        curve(&[a, p(55.5, 40.5), b], LineCap::Round, LineCap::Round),
        style(10.0, DashPattern::Solid, true),
    ));
    for (i, c) in round.iter().enumerate() {
        let (x, y) = centre(i);
        if x > b.x {
            let r = (x - b.x).hypot(y - b.y);
            if r < 4.5 {
                assert_eq!(c[3], 255, "inside the half disc ({x}, {y})");
            }
            if r > 5.5 {
                assert_eq!(c[3], 0, "outside the half disc ({x}, {y})");
            }
        }
    }
    // The same on a bent curve: nothing past a flat end along its tangent.
    let reference = reference(&BENT);
    let bent = draw(&CurveLine::new(
        curve(&BENT, LineCap::Flat, LineCap::Flat),
        style(6.0, DashPattern::Solid, true),
    ));
    assert!(beyond(&bent, BENT[3], tangent(&reference, true), 8.0).is_empty());
    assert!(beyond(&bent, BENT[0], tangent(&reference, false), 8.0).is_empty());
}

// --- AC-10: heads longer than the curve -------------------------------------

#[test]
fn heads_longer_than_the_curve_shrink_to_fit() {
    // A 10 px line, width 4: each head would be 16 px.
    let (a, b) = (p(50.5, 40.5), p(60.5, 40.5));
    let one = draw(&CurveLine::new(
        curve(&[a, b], LineCap::Flat, LineCap::Arrow),
        style(4.0, DashPattern::Solid, true),
    ));
    // The single head spans the whole line: base on the start point, tip on
    // the end, its base 10 px wide.
    let at =
        |px: &[[u8; 4]], x: f32, y: f32| px[((y - 0.5) as u32 * W + (x - 0.5) as u32) as usize][3];
    assert_eq!(
        at(&one, 51.5, 43.5),
        255,
        "the head is wide near the start point"
    );
    assert_eq!(at(&one, 51.5, 47.5), 0, "no wider than 10 px");
    assert!(beyond(&one, b, (1.0, 0.0), 20.0).is_empty());
    assert!(
        beyond(&one, a, (-1.0, 0.0), 20.0).is_empty(),
        "the head's base sits on the start point"
    );

    let two = draw(&CurveLine::new(
        curve(&[a, b], LineCap::Arrow, LineCap::Arrow),
        style(4.0, DashPattern::Solid, true),
    ));
    // Each head is 5 px long: they meet at the middle (x = 55.5), where both
    // bases are 5 px wide.
    assert_eq!(at(&two, 55.5, 42.5), 255);
    assert_eq!(at(&two, 55.5, 44.5), 0);
    assert!(beyond(&two, b, (1.0, 0.0), 20.0).is_empty());
    assert!(beyond(&two, a, (-1.0, 0.0), 20.0).is_empty());
}

// --- AC-11: the new dash patterns -----------------------------------------

#[test]
fn dash_dot_patterns_repeat_dash_gap_dot_gap() {
    // Width 2: dash 6, gap 3, dot 2, gap 3 (period 14); dash-dot-dot adds
    // dot 2, gap 3 (period 19). Round dash ends reach 1 px past each run.
    let (a, b) = (p(0.0, 40.5), p(119.0, 40.5));
    let row = |dash| {
        let px = draw(&CurveLine::new(
            curve(&[a, b], LineCap::Flat, LineCap::Flat),
            style(2.0, dash, true),
        ));
        move |x: f32| px[(40 * W + x as u32) as usize][3]
    };
    let dd = row(DashPattern::DashDot);
    for (x, on) in [
        (3.0, true),
        (7.5, false),
        (10.0, true),
        (12.5, false),
        (17.0, true),
        (21.5, false),
        (24.0, true),
    ] {
        assert_eq!(dd(x) > 128, on, "dash-dot at {x}");
    }
    let ddd = row(DashPattern::DashDotDot);
    for (x, on) in [
        (3.0, true),
        (7.5, false),
        (10.0, true),
        (12.5, false),
        (15.0, true),
        (17.5, false),
        (22.0, true),
    ] {
        assert_eq!(ddd(x) > 128, on, "dash-dot-dot at {x}");
    }
    // Every shape's outline takes the patterns too.
    let rect = Shape::Rectangle {
        a: p(10.0, 10.0),
        b: p(100.0, 70.0),
    };
    let solid = draw_shape(rect, style(2.0, DashPattern::Solid, true));
    for dash in [DashPattern::DashDot, DashPattern::DashDotDot] {
        let dashed = draw_shape(rect, style(2.0, dash, true));
        let count = |px: &[[u8; 4]]| px.iter().filter(|c| c[3] > 0).count();
        assert!(count(&dashed) < count(&solid) * 9 / 10, "{dash:?}");
    }
}

// --- AC-14: the selection bounds the curve --------------------------------

#[test]
fn a_curve_changes_only_selected_pixels() {
    let line = CurveLine::new(
        curve(&BENT, LineCap::Arrow, LineCap::Arrow),
        style(5.0, DashPattern::Solid, true),
    );
    let free = draw(&line);
    let mask = SelectionMask::rectangle(W, H, Rect::new(0, 0, 60, H));
    let bounded = draw_with(&line, Some(mask));
    let mut inside = 0;
    for (i, (f, b)) in free.iter().zip(&bounded).enumerate() {
        if (i as u32 % W) < 60 {
            assert_eq!(f, b);
            inside += usize::from(f[3] > 0);
        } else {
            assert_eq!(b[3], 0, "unselected pixel {i} changed");
        }
    }
    assert!(inside > 0);
}

// --- AC-16: coincident points --------------------------------------------

#[test]
fn coincident_points_act_as_one() {
    let s = style(3.0, DashPattern::Dashed, true);
    let doubled = [BENT[0], BENT[1], BENT[1], BENT[2], BENT[3], BENT[3]];
    assert_eq!(
        draw(&CurveLine::new(
            curve(&doubled, LineCap::Arrow, LineCap::Flat),
            s
        )),
        draw(&CurveLine::new(
            curve(&BENT, LineCap::Arrow, LineCap::Flat),
            s
        )),
    );
    let spot = p(40.3, 30.6);
    let all = [spot, spot, spot];
    assert_eq!(
        draw(&CurveLine::new(
            curve(&all, LineCap::Round, LineCap::Round),
            s
        )),
        draw_shape(Shape::Line { a: spot, b: spot }, s),
    );
    let doc = blank();
    for (start, end) in [
        (LineCap::Flat, LineCap::Round),
        (LineCap::Round, LineCap::Arrow),
    ] {
        assert!(
            CurveLine::new(curve(&all, start, end), s)
                .draw(0, &doc)
                .is_none()
        );
    }
}

#[test]
fn a_straight_line_and_a_curve_are_both_line_steps() {
    let doc = Document::new(W, H).expect("doc");
    let s = style(2.0, DashPattern::Solid, true);
    let kind = |pts: &[Point], start| {
        let cmd = CurveLine::new(curve(pts, start, LineCap::Round), s)
            .draw(0, &doc)
            .expect("draws");
        pincel_image_core::command::Command::kind(&cmd)
    };
    let line = pincel_image_core::command::HistoryKind::Line;
    assert_eq!(kind(&[BENT[0], BENT[3]], LineCap::Flat), line);
    assert_eq!(kind(&[BENT[0], BENT[3]], LineCap::Round), line);
    assert_eq!(kind(&BENT, LineCap::Flat), line);
}

/// The stroke and its arrow head, and two shrunk heads meeting base to
/// base, join without a lighter seam along the line's axis.
#[test]
fn the_stroke_and_its_heads_join_without_a_seam() {
    let row = |pts: &[Point], start, end| {
        let px = draw(&CurveLine::new(
            curve(pts, start, end),
            style(4.0, DashPattern::Solid, true),
        ));
        move |x: u32| px[(40 * W + x) as usize][3]
    };
    let one = row(
        &[p(10.5, 40.5), p(80.5, 40.5)],
        LineCap::Round,
        LineCap::Arrow,
    );
    for x in 10..=78 {
        assert_eq!(one(x), 255, "stroke into head at x = {x}");
    }
    let two = row(
        &[p(50.5, 40.5), p(60.5, 40.5)],
        LineCap::Arrow,
        LineCap::Arrow,
    );
    for x in 52..=58 {
        assert_eq!(two(x), 255, "heads meeting at x = {x}");
    }
}
