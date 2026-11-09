//! The box-fitted shapes of the Shapes group — Triangle, Diamond, Star,
//! Block Arrow, Heart and Callout (docs/specs/image-shapes-curves-shapes.md).
//!
//! Each is built as one closed polygon (curves flattened finely), so the
//! Shapes tool fills, strokes, dashes and anti-aliases it with the polygon
//! SDF it uses for Polygon. A shape symmetric about its box's vertical (or
//! horizontal) center line reports that axis, and the rasterizer evaluates
//! the SDF on one side of it only: the pixels come out exactly mirror
//! symmetric (AC-10).
use super::Shape;
use crate::geometry::Point;

/// Default star points (AC-04).
pub const STAR_POINTS: u32 = 5;
/// Default star inner radius as a share of the outer radius (AC-04).
pub const STAR_INNER_RATIO: f32 = 0.38;

/// A box shape's outline: one closed polygon, and the axes it is mirror
/// symmetric about (x of a vertical axis, y of a horizontal one).
pub(super) struct BoxOutline {
    pub verts: Vec<Point>,
    pub mirror_x: Option<f32>,
    pub mirror_y: Option<f32>,
}

/// The outline of a box shape, or `None` for the other shapes and for a
/// box (or star radius) that encloses no area (AC-14).
pub(super) fn outline(shape: &Shape) -> Option<BoxOutline> {
    match *shape {
        Shape::Triangle { a, b } => {
            let (x0, y0, x1, y1) = boxed(a, b)?;
            let cx = (x0 + x1) / 2.0;
            Some(BoxOutline {
                verts: vec![p(x0, y1), p(cx, y0), p(x1, y1)],
                mirror_x: Some(cx),
                mirror_y: None,
            })
        }
        Shape::Diamond { a, b } => {
            let (x0, y0, x1, y1) = boxed(a, b)?;
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            Some(BoxOutline {
                verts: vec![p(cx, y0), p(x1, cy), p(cx, y1), p(x0, cy)],
                mirror_x: Some(cx),
                mirror_y: Some(cy),
            })
        }
        Shape::Star {
            center,
            radius,
            points,
            inner_ratio,
            rotation,
        } => {
            if radius.is_nan() || radius <= 0.0 {
                return None;
            }
            let n = points.clamp(3, 50);
            let inner = radius * inner_ratio.clamp(0.1, 0.9);
            let verts = (0..2 * n)
                .map(|k| {
                    let r = if k % 2 == 0 { radius } else { inner };
                    let ang = rotation - std::f32::consts::FRAC_PI_2
                        + std::f32::consts::PI * k as f32 / n as f32;
                    p(center.x + r * ang.cos(), center.y + r * ang.sin())
                })
                .collect();
            Some(BoxOutline {
                verts,
                // Point up (rotation 0): symmetric about the vertical axis.
                mirror_x: (rotation == 0.0).then_some(center.x),
                mirror_y: None,
            })
        }
        Shape::BlockArrow { start, end } => block_arrow(start, end),
        Shape::Heart { a, b } => heart(a, b),
        Shape::Callout { start, end } => callout(start, end),
        _ => None,
    }
}

/// The outline polygon of a box shape (Triangle, Diamond, Star, Block
/// Arrow, Heart, Callout) exactly as the Shapes tool draws it, for the
/// editor's drag preview (docs/specs/image-shapes-curves-shapes.md AC-12);
/// `None` for another shape or a box without area.
pub fn box_outline(shape: &Shape) -> Option<Vec<Point>> {
    outline(shape).map(|o| o.verts)
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// The normalized box `(x0, y0, x1, y1)`, or `None` when it has no area.
fn boxed(a: Point, b: Point) -> Option<(f32, f32, f32, f32)> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// An arrow filling the box, from `start` toward `end` along the dominant
/// axis (horizontal on a tie): head length min(A / 2, T), head spanning
/// the thickness T, shaft T / 2 thick (AC-05).
fn block_arrow(start: Point, end: Point) -> Option<BoxOutline> {
    let (x0, y0, x1, y1) = boxed(start, end)?;
    let (w, h) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    if w >= h {
        let head = (w / 2.0).min(h);
        let q = h / 4.0;
        // Pointing right; a left arrow is its mirror image.
        let right = end.x >= start.x;
        let x = |t: f32| if right { x0 + t } else { x1 - t };
        Some(BoxOutline {
            verts: vec![
                p(x(0.0), cy - q),
                p(x(w - head), cy - q),
                p(x(w - head), y0),
                p(x(w), cy),
                p(x(w - head), y1),
                p(x(w - head), cy + q),
                p(x(0.0), cy + q),
            ],
            mirror_x: None,
            mirror_y: Some(cy),
        })
    } else {
        let head = (h / 2.0).min(w);
        let q = w / 4.0;
        let down = end.y >= start.y;
        let y = |t: f32| if down { y0 + t } else { y1 - t };
        Some(BoxOutline {
            verts: vec![
                p(cx - q, y(0.0)),
                p(cx - q, y(h - head)),
                p(x0, y(h - head)),
                p(cx, y(h)),
                p(x1, y(h - head)),
                p(cx + q, y(h - head)),
                p(cx + q, y(0.0)),
            ],
            mirror_x: Some(cx),
            mirror_y: None,
        })
    }
}

/// Points of the cubic Bézier `a`, `c1`, `c2`, `b`, excluding `a`, finely
/// enough for the flattening to stay within 0.1 px at any box size.
fn cubic(out: &mut Vec<Point>, a: Point, c1: Point, c2: Point, b: Point, steps: usize) {
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push(p(
            w0 * a.x + w1 * c1.x + w2 * c2.x + w3 * b.x,
            w0 * a.y + w1 * c1.y + w2 * c2.y + w3 * b.y,
        ));
    }
}

/// Segments per Bézier for a box whose larger side is `size` px.
fn steps_for(size: f32) -> usize {
    (size.sqrt() * 4.0).clamp(12.0, 96.0) as usize
}

/// A heart touching all four edges of the box, lobes at the top, its point
/// at the middle of the bottom edge (AC-06): two mirrored halves of two
/// cubic Béziers each, scaled so their extent fills the box.
fn heart(a: Point, b: Point) -> Option<BoxOutline> {
    let (x0, y0, x1, y1) = boxed(a, b)?;
    let steps = steps_for((x1 - x0).max(y1 - y0));
    // The right half in a unit template: notch (0.5, 0.3) → lobe → point
    // (0.5, 1).
    let mut right = vec![p(0.5, 0.3)];
    cubic(
        &mut right,
        p(0.5, 0.3),
        p(0.5, -0.05),
        p(1.05, -0.05),
        p(1.0, 0.32),
        steps,
    );
    cubic(
        &mut right,
        p(1.0, 0.32),
        p(0.97, 0.62),
        p(0.62, 0.8),
        p(0.5, 1.0),
        steps,
    );
    // Fit the template's extent to the box.
    let min_y = right.iter().map(|q| q.y).fold(f32::MAX, f32::min);
    let max_x = right.iter().map(|q| q.x).fold(f32::MIN, f32::max);
    let (w, h) = (x1 - x0, y1 - y0);
    let cx = (x0 + x1) / 2.0;
    let map = |q: &Point, side: f32| {
        let u = (q.x - 0.5) / (max_x - 0.5) * 0.5; // 0 at the center, 0.5 at the edge
        p(cx + side * u * w, y0 + (q.y - min_y) / (1.0 - min_y) * h)
    };
    // Right half top to point, then the left half back up, mirrored.
    let mut verts: Vec<Point> = right.iter().map(|q| map(q, 1.0)).collect();
    verts.extend(
        right
            .iter()
            .rev()
            .skip(1)
            .take(right.len() - 2)
            .map(|q| map(q, -1.0)),
    );
    Some(BoxOutline {
        verts,
        mirror_x: Some(cx),
        mirror_y: None,
    })
}

/// A speech bubble: a rounded body over the 80 % of the box away from
/// `start`, corner radius 15 % of the body's shorter side, and a tail from
/// the body edge facing `start` (15 % to 35 % of the width from `start`'s
/// side) to its tip on `start` — one closed contour (AC-07).
fn callout(start: Point, end: Point) -> Option<BoxOutline> {
    let (x0, y0, x1, y1) = boxed(start, end)?;
    let (w, h) = (x1 - x0, y1 - y0);
    // Built with the start corner at the top left, then mirrored into place.
    let body_top = 0.2 * h;
    let r = 0.15 * w.min(0.8 * h);
    let arc = |cx: f32, cy: f32, from: f32, out: &mut Vec<Point>| {
        let n = steps_for(r.max(1.0)).max(4);
        for i in 0..=n {
            let t = from + std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            out.push(p(cx + r * t.cos(), cy + r * t.sin()));
        }
    };
    let mut c = Vec::new();
    // The tail first: base start on the top edge, tip, base end.
    c.push(p(0.15 * w, body_top));
    c.push(p(0.0, 0.0));
    c.push(p(0.35 * w, body_top));
    // Clockwise (y down) around the body: top-right, bottom-right,
    // bottom-left, top-left corners.
    arc(w - r, body_top + r, -std::f32::consts::FRAC_PI_2, &mut c);
    arc(w - r, h - r, 0.0, &mut c);
    arc(r, h - r, std::f32::consts::FRAC_PI_2, &mut c);
    arc(r, body_top + r, std::f32::consts::PI, &mut c);
    let flip_x = start.x > end.x;
    let flip_y = start.y > end.y;
    let verts = c
        .into_iter()
        .map(|q| {
            let x = if flip_x { x1 - q.x } else { x0 + q.x };
            let y = if flip_y { y1 - q.y } else { y0 + q.y };
            p(x, y)
        })
        .collect();
    Some(BoxOutline {
        verts,
        mirror_x: None,
        mirror_y: None,
    })
}
