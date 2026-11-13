//! The Line / Curve tool's geometry and rasterizer
//! (docs/specs/image-shapes-curves.md).
//!
//! A line is its two endpoints plus up to 62 control points in between; the
//! curve is a centripetal Catmull-Rom spline (α = 0.5) through all of them,
//! with phantom end points reflected through the ends. It is flattened to a
//! polyline that stays within [`FLATNESS`] px of the exact curve; coverage
//! is the distance-to-polyline band [`Shape::Line`] uses, so joins are round
//! and dashes walk the polyline's arc length. Each end has a [`LineCap`]:
//! Round (the capsule end), Flat (cut perpendicular to the end direction)
//! or Arrow (a filled isosceles head).
//!
//! A two-point line with Round caps, and a curve whose control points lie in
//! order on the straight segment, draw exactly what [`Shapes`] draws for
//! [`Shape::Line`] (AC-01, AC-06).
use super::shapes::{band_coverage, dash_segments, dist_segment, inside_coverage, sd_polygon};
use super::{Shape, ShapeStyle, Shapes};
use crate::color::Color;
use crate::command::{HistoryKind, SetPixels};
use crate::document::Document;
use crate::geometry::{Point, Rect};

/// Most points a line may have, endpoints included (AC-15).
pub const MAX_LINE_POINTS: usize = 64;

/// Largest distance between the flattened polyline and the exact curve, px.
const FLATNESS: f32 = 0.02;

/// Points closer than this act as one (AC-16).
const SAME_POINT: f32 = 1e-3;

/// How one end of a line ends (AC-08, AC-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// A half disc of radius stroke width / 2 around the endpoint (today's end).
    #[default]
    Round,
    /// Cut perpendicular to the curve's direction at the endpoint.
    Flat,
    /// A filled arrow head in the stroke color, its tip on the endpoint.
    Arrow,
}

impl LineCap {
    /// The wire name (`round` / `flat` / `arrow`).
    pub fn name(self) -> &'static str {
        match self {
            LineCap::Round => "round",
            LineCap::Flat => "flat",
            LineCap::Arrow => "arrow",
        }
    }

    /// Parses a wire name; `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "round" => Some(LineCap::Round),
            "flat" => Some(LineCap::Flat),
            "arrow" => Some(LineCap::Arrow),
            _ => None,
        }
    }
}

/// A line through `points` (endpoints first and last) with its two caps.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// The start point, the control points in order, and the end point.
    pub points: Vec<Point>,
    /// The cap at the start point.
    pub start_cap: LineCap,
    /// The cap at the end point.
    pub end_cap: LineCap,
}

impl Curve {
    /// A straight line from `a` to `b` with Round caps.
    pub fn straight(a: Point, b: Point) -> Self {
        Self {
            points: vec![a, b],
            start_cap: LineCap::Round,
            end_cap: LineCap::Round,
        }
    }

    /// The points with coincident neighbours merged, and — when every
    /// control point lies in order on the straight segment — reduced to the
    /// two endpoints (AC-06, AC-16).
    pub fn effective_points(&self) -> Vec<Point> {
        let mut pts: Vec<Point> = Vec::with_capacity(self.points.len());
        for &p in &self.points {
            if pts.last().is_none_or(|q| q.distance(p) > SAME_POINT) {
                pts.push(p);
            }
        }
        if pts.len() > 2 && collinear_in_order(&pts) {
            let (first, last) = (pts[0], pts[pts.len() - 1]);
            pts = vec![first, last];
        }
        pts
    }

    /// The flattened curve: a polyline within [`FLATNESS`] px of the spline
    /// through [`Curve::effective_points`].
    pub fn polyline(&self) -> Vec<Point> {
        flatten(&self.effective_points())
    }

    /// The curve's arc length (of its polyline).
    pub fn length(&self) -> f32 {
        arc_length(&self.polyline())
    }

    /// The point at half the arc length and the unit normal there — where
    /// the editor puts the line's move grip (AC-22).
    pub fn midpoint(&self) -> (Point, Point) {
        let poly = self.polyline();
        let half = arc_length(&poly) / 2.0;
        let mut walked = 0.0;
        for w in poly.windows(2) {
            let seg = w[0].distance(w[1]);
            if seg > 0.0 && walked + seg >= half {
                let t = (half - walked) / seg;
                let (dx, dy) = ((w[1].x - w[0].x) / seg, (w[1].y - w[0].y) / seg);
                let at = Point::new(w[0].x + dx * seg * t, w[0].y + dy * seg * t);
                return (at, Point::new(-dy, dx));
            }
            walked += seg;
        }
        let at = poly.first().copied().unwrap_or(Point::new(0.0, 0.0));
        (at, Point::new(0.0, -1.0))
    }
}

/// Whether every interior point lies on the segment between the first and
/// the last point, in order.
fn collinear_in_order(pts: &[Point]) -> bool {
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let len = a.distance(b);
    if len <= SAME_POINT {
        return false;
    }
    let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    let mut last = 0.0;
    for p in &pts[1..pts.len() - 1] {
        let (wx, wy) = (p.x - a.x, p.y - a.y);
        let along = wx * ux + wy * uy;
        let off = (wx * uy - wy * ux).abs();
        if off > SAME_POINT || along < last || along > len {
            return false;
        }
        last = along;
    }
    true
}

fn arc_length(poly: &[Point]) -> f32 {
    poly.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// One centripetal Catmull-Rom segment from `p1` to `p2` (neighbours `p0`,
/// `p3`), evaluated by the Barry–Goldman pyramid.
#[derive(Clone, Copy)]
struct Segment {
    p: [Point; 4],
    t: [f32; 4],
}

impl Segment {
    fn new(p0: Point, p1: Point, p2: Point, p3: Point) -> Self {
        let knot = |a: Point, b: Point| a.distance(b).sqrt().max(1e-6);
        let t1 = knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        Self {
            p: [p0, p1, p2, p3],
            t: [0.0, t1, t2, t3],
        }
    }

    /// The point at `u` in `[0, 1]` (p1 at 0, p2 at 1).
    fn at(&self, u: f32) -> Point {
        let [p0, p1, p2, p3] = self.p;
        let [t0, t1, t2, t3] = self.t;
        let t = t1 + (t2 - t1) * u;
        let lerp = |a: Point, b: Point, ta: f32, tb: f32| {
            let w = (t - ta) / (tb - ta);
            Point::new(a.x + (b.x - a.x) * w, a.y + (b.y - a.y) * w)
        };
        let a1 = lerp(p0, p1, t0, t1);
        let a2 = lerp(p1, p2, t1, t2);
        let a3 = lerp(p2, p3, t2, t3);
        let b1 = lerp(a1, a2, t0, t2);
        let b2 = lerp(a2, a3, t1, t3);
        lerp(b1, b2, t1, t2)
    }
}

/// The spline segments through `pts` (at least two distinct points).
fn segments(pts: &[Point]) -> Vec<Segment> {
    let n = pts.len();
    let reflect = |a: Point, b: Point| Point::new(2.0 * a.x - b.x, 2.0 * a.y - b.y);
    (0..n - 1)
        .map(|i| {
            let p0 = if i == 0 {
                reflect(pts[0], pts[1])
            } else {
                pts[i - 1]
            };
            let p3 = if i + 2 < n {
                pts[i + 2]
            } else {
                reflect(pts[n - 1], pts[n - 2])
            };
            Segment::new(p0, pts[i], pts[i + 1], p3)
        })
        .collect()
}

/// The polyline through `pts`: the straight segment for two points, else
/// the spline flattened by recursive subdivision.
fn flatten(pts: &[Point]) -> Vec<Point> {
    match pts.len() {
        0 => return Vec::new(),
        1 | 2 => return pts.to_vec(),
        _ => {}
    }
    let mut out = vec![pts[0]];
    for seg in segments(pts) {
        subdivide(&seg, 0.0, 1.0, seg.at(0.0), seg.at(1.0), 0, &mut out);
    }
    out
}

/// Appends the polyline of `seg` between `u0` and `u1` (excluding the start).
fn subdivide(
    seg: &Segment,
    u0: f32,
    u1: f32,
    a: Point,
    b: Point,
    depth: u32,
    out: &mut Vec<Point>,
) {
    let um = (u0 + u1) / 2.0;
    let m = seg.at(um);
    // Also probe the quarter points: a symmetric bulge can hide from the
    // midpoint alone.
    let q1 = seg.at((u0 + um) / 2.0);
    let q3 = seg.at((um + u1) / 2.0);
    let flat = [m, q1, q3]
        .iter()
        .all(|&p| dist_segment(p.x, p.y, a, b) <= FLATNESS);
    if depth >= 14 || (depth >= 2 && flat) {
        out.push(b);
        return;
    }
    subdivide(seg, u0, um, a, m, depth + 1, out);
    subdivide(seg, um, u1, m, b, depth + 1, out);
}

/// The unit direction pointing out of the curve at its start (`at_end`
/// false) or end, taken from the spline's tangent there.
fn outward(pts: &[Point], at_end: bool) -> Point {
    let (tip, inner) = if pts.len() == 2 {
        if at_end {
            (pts[1], pts[0])
        } else {
            (pts[0], pts[1])
        }
    } else {
        let segs = segments(pts);
        if at_end {
            let s = segs[segs.len() - 1];
            (s.at(1.0), s.at(1.0 - 1e-3))
        } else {
            let s = segs[0];
            (s.at(0.0), s.at(1e-3))
        }
    };
    let len = tip.distance(inner).max(1e-9);
    Point::new((tip.x - inner.x) / len, (tip.y - inner.y) / len)
}

/// One end's cut and arrow head.
struct End {
    /// Where the stroke is cut: the endpoint (Flat) or the head's base (Arrow).
    cut_at: Point,
    /// Outward unit direction at the end.
    dir: Point,
    /// Only pixels this close to the endpoint are cut (keeps the cut from
    /// reaching other parts of a curve that bends back).
    reach: f32,
    /// The endpoint.
    tip: Point,
    /// The arrow head's triangle, if the cap is Arrow.
    head: Option<[Point; 3]>,
}

impl End {
    fn new(cap: LineCap, tip: Point, dir: Point, head_len: f32, half: f32) -> Option<Self> {
        match cap {
            LineCap::Round => None,
            LineCap::Flat => Some(Self {
                cut_at: tip,
                dir,
                reach: half + 2.0,
                tip,
                head: None,
            }),
            LineCap::Arrow => {
                let base = Point::new(tip.x - dir.x * head_len, tip.y - dir.y * head_len);
                let (nx, ny) = (-dir.y * head_len / 2.0, dir.x * head_len / 2.0);
                Some(Self {
                    cut_at: base,
                    dir,
                    reach: head_len + half + 2.0,
                    tip,
                    head: Some([
                        tip,
                        Point::new(base.x + nx, base.y + ny),
                        Point::new(base.x - nx, base.y - ny),
                    ]),
                })
            }
        }
    }

    /// How much of the stroke survives the cut at `(px, py)` (1 = all).
    fn keep(&self, px: f32, py: f32, aa: bool) -> f32 {
        if libm::hypotf(px - self.tip.x, py - self.tip.y) > self.reach {
            return 1.0;
        }
        let beyond = (px - self.cut_at.x) * self.dir.x + (py - self.cut_at.y) * self.dir.y;
        if aa {
            (0.5 - beyond).clamp(0.0, 1.0)
        } else if beyond <= 0.0 {
            1.0
        } else {
            0.0
        }
    }

    fn head_coverage(&self, px: f32, py: f32, aa: bool) -> f32 {
        self.head
            .map_or(0.0, |tri| inside_coverage(sd_polygon(px, py, &tri), aa))
    }
}

/// The Line / Curve tool: rasterizes a [`Curve`] with a [`ShapeStyle`]
/// (stroke only; fill modes act as outline, like [`Shape::Line`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CurveLine {
    /// The curve.
    pub curve: Curve,
    /// Stroke styling: width, color, dash, anti-alias, blend.
    pub style: ShapeStyle,
}

impl CurveLine {
    /// Creates the tool for `curve` drawn with `style`.
    pub fn new(curve: Curve, style: ShapeStyle) -> Self {
        Self { curve, style }
    }

    /// The arrow-head length for this stroke width before any shrink:
    /// max(8, 4 × stroke width) px (AC-08).
    pub fn head_length(&self) -> f32 {
        (4.0 * self.style.stroke_width.clamp(1.0, 500.0)).max(8.0)
    }

    /// Rasterizes the curve into a [`SetPixels`] on `layer_index`, or `None`
    /// when it paints nothing (off canvas, an invalid layer, or all points on
    /// one spot with a Flat or Arrow cap — AC-16). The selection bounds the
    /// write like every pixel tool (AC-14). Straight or curved, it is a
    /// step of the Line tool in History.
    pub fn draw(&self, layer_index: usize, doc: &Document) -> Option<SetPixels> {
        let pts = self.curve.effective_points();
        let round = self.curve.start_cap == LineCap::Round && self.curve.end_cap == LineCap::Round;
        if pts.len() < 2 {
            // All points on one spot: today's zero-length dot, or nothing.
            let p = *pts.first()?;
            if !round {
                return None;
            }
            let cmd = Shapes::new(Shape::Line { a: p, b: p }, self.style).draw(layer_index, doc)?;
            return Some(cmd.with_kind(HistoryKind::Line));
        }
        if pts.len() == 2 && round {
            let cmd = Shapes::new(
                Shape::Line {
                    a: pts[0],
                    b: pts[1],
                },
                self.style,
            )
            .draw(layer_index, doc)?;
            return Some(cmd.with_kind(HistoryKind::Line));
        }
        self.rasterize(&pts, layer_index, doc)
            .map(|cmd| cmd.with_kind(HistoryKind::Line))
    }

    fn rasterize(&self, pts: &[Point], layer_index: usize, doc: &Document) -> Option<SetPixels> {
        let layer = doc.layers.get(layer_index)?;
        let (cw, ch) = (doc.canvas.width(), doc.canvas.height());
        let width = self.style.stroke_width.clamp(1.0, 500.0);
        let half = width / 2.0;
        let aa = self.style.anti_alias;
        let poly = flatten(pts);
        let length = arc_length(&poly);

        // Arrow heads shrink together when they are longer than the curve
        // (AC-10).
        let caps = [self.curve.start_cap, self.curve.end_cap];
        let heads = caps.iter().filter(|&&c| c == LineCap::Arrow).count() as f32;
        let mut head_len = self.head_length();
        if heads > 0.0 && heads * head_len > length {
            head_len = length / heads;
        }
        let ends: Vec<End> = [(caps[0], false), (caps[1], true)]
            .into_iter()
            .filter_map(|(cap, at_end)| {
                let tip = if at_end { pts[pts.len() - 1] } else { pts[0] };
                End::new(cap, tip, outward(pts, at_end), head_len, half)
            })
            .collect();

        let segs: Vec<(Point, Point)> = match self.style.dash.runs(width) {
            Some(runs) => dash_segments(&poly, false, &runs),
            None => poly.windows(2).map(|w| (w[0], w[1])).collect(),
        };

        // The painted region: the polyline and the heads, padded.
        let mut bounds = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut grow = |p: Point, pad: f32| {
            bounds.0 = bounds.0.min(p.x - pad);
            bounds.1 = bounds.1.min(p.y - pad);
            bounds.2 = bounds.2.max(p.x + pad);
            bounds.3 = bounds.3.max(p.y + pad);
        };
        for &p in &poly {
            grow(p, half + 1.0);
        }
        for end in &ends {
            for p in end.head.iter().flatten() {
                grow(*p, 1.0);
            }
        }
        let x0 = (bounds.0.floor() as i32).clamp(0, cw as i32);
        let y0 = (bounds.1.floor() as i32).clamp(0, ch as i32);
        let x1 = (bounds.2.ceil() as i32).clamp(0, cw as i32);
        let y1 = (bounds.3.ceil() as i32).clamp(0, ch as i32);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let region = Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32);

        // The distance of each pixel to the stroke, filled in near the
        // segments only: each segment is walked in short pieces and only the
        // pixels around a piece measure their distance to the (whole)
        // segment. Pixels farther than `reach` can get no band coverage.
        let reach = half + 1.0;
        let (rw, rh) = (region.w as usize, region.h as usize);
        let mut dist = vec![f32::INFINITY; rw * rh];
        let step = (2.0 * reach).max(4.0);
        let clip_x = |v: f32| (v.floor() as i32 - region.x).clamp(0, rw as i32) as usize;
        let clip_y = |v: f32| (v.floor() as i32 - region.y).clamp(0, rh as i32) as usize;
        for &(a, b) in &segs {
            let pieces = (a.distance(b) / step).ceil().max(1.0) as usize;
            for k in 0..pieces {
                let (t0, t1) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
                let (ax, ay) = (a.x + (b.x - a.x) * t0, a.y + (b.y - a.y) * t0);
                let (bx, by) = (a.x + (b.x - a.x) * t1, a.y + (b.y - a.y) * t1);
                let (x0, x1) = (clip_x(ax.min(bx) - reach), clip_x(ax.max(bx) + reach) + 1);
                let (y0, y1) = (clip_y(ay.min(by) - reach), clip_y(ay.max(by) + reach) + 1);
                for ly in y0..y1.min(rh) {
                    let py = (region.y + ly as i32) as f32 + 0.5;
                    for lx in x0..x1.min(rw) {
                        let px = (region.x + lx as i32) as f32 + 0.5;
                        let i = ly * rw + lx;
                        dist[i] = dist[i].min(dist_segment(px, py, a, b));
                    }
                }
            }
        }
        // The pixels an arrow head may cover.
        let mut in_head = vec![false; rw * rh];
        for tri in ends.iter().filter_map(|e| e.head) {
            let (min_x, max_x) = tri
                .iter()
                .fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.x), m.1.max(p.x)));
            let (min_y, max_y) = tri
                .iter()
                .fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
            for ly in clip_y(min_y - 1.0)..(clip_y(max_y + 1.0) + 1).min(rh) {
                for lx in clip_x(min_x - 1.0)..(clip_x(max_x + 1.0) + 1).min(rw) {
                    in_head[ly * rw + lx] = true;
                }
            }
        }

        let selection = doc.selection.as_ref();
        let mut after = layer.pixels.copy_region(region).ok()?;
        let pa = self.style.stroke_color.a as f32 / 255.0;
        for ly in 0..region.h {
            for lx in 0..region.w {
                let i = ly as usize * rw + lx as usize;
                if dist[i] >= reach && !in_head[i] {
                    continue;
                }
                let (cx, cy) = (region.x + lx as i32, region.y + ly as i32);
                let (px, py) = (cx as f32 + 0.5, cy as f32 + 0.5);
                let mut cov = band_coverage(dist[i], half, aa);
                for end in &ends {
                    cov *= end.keep(px, py, aa);
                }
                // The stroke's ramp at a head's base and the head's own edge
                // there are complementary, as are two shrunk heads meeting
                // base to base: their sum joins them without a seam. Clamped
                // to 1, overlapping heads still take the stroke color once.
                for end in &ends {
                    cov += end.head_coverage(px, py, aa);
                }
                cov = cov.min(1.0);
                if cov <= 0.0 {
                    continue;
                }
                let sel =
                    selection.map_or(1.0, |s| s.coverage(cx as u32, cy as u32) as f32 / 255.0);
                if sel <= 0.0 {
                    continue;
                }
                let dst = after.get_pixel(lx, ly).unwrap_or(Color::TRANSPARENT);
                let out = self
                    .style
                    .blend
                    .paint(self.style.stroke_color, pa, cov * sel, dst);
                after.set_pixel(lx, ly, out);
            }
        }
        Some(SetPixels::new(layer_index, region, after))
    }
}
