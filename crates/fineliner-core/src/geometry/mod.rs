//! Basic geometry primitives shared across the core.
//!
//! See `docs/specs/fineliner.md` §6.3 for the coordinate system: origin is
//! top-left, X increases rightward, Y increases downward.

use serde::{Deserialize, Serialize};

/// A 2D point with sub-pixel (f32) precision.
///
/// Tool input coordinates are f32 to support anti-aliasing; canvas pixel
/// coordinates are integers (see spec §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal coordinate, increasing rightward.
    pub x: f32,
    /// Vertical coordinate, increasing downward.
    pub y: f32,
}

impl Point {
    /// Creates a new point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance(&self, other: Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// An integer size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl Size {
    /// Creates a new size.
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }

    /// Total pixel count (`w * h`) as `u64` to avoid overflow on large canvases.
    pub fn area(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

/// An axis-aligned integer rectangle.
///
/// The rectangle covers pixels in `[x, x + w)` × `[y, y + h)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge (inclusive).
    pub x: i32,
    /// Top edge (inclusive).
    pub y: i32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl Rect {
    /// Creates a new rectangle.
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Right edge (exclusive): `x + w`.
    pub fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    /// Bottom edge (exclusive): `y + h`.
    pub fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }

    /// Pixel area (`w * h`).
    pub fn area(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    /// Returns `true` if the point `(px, py)` lies inside the rectangle.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Returns the overlap of two rectangles, or `None` if they are disjoint.
    pub fn intersect(&self, other: Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 > x0 && y1 > y0 {
            Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_distance_pythagorean_triple_is_exact() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert_eq!(a.distance(b), 5.0);
    }

    #[test]
    fn size_area_does_not_overflow_u32() {
        let s = Size::new(32767, 32767);
        assert_eq!(s.area(), 32767u64 * 32767u64);
    }

    #[test]
    fn rect_contains_respects_exclusive_edges() {
        let r = Rect::new(1, 2, 3, 4);
        assert!(r.contains(1, 2));
        assert!(r.contains(3, 5));
        assert!(!r.contains(4, 2)); // right edge exclusive
        assert!(!r.contains(1, 6)); // bottom edge exclusive
        assert!(!r.contains(0, 2));
    }

    #[test]
    fn rect_intersect_overlapping_returns_overlap() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(b), Some(Rect::new(5, 5, 5, 5)));
    }

    #[test]
    fn rect_intersect_disjoint_returns_none() {
        let a = Rect::new(0, 0, 5, 5);
        let b = Rect::new(10, 10, 5, 5);
        assert_eq!(a.intersect(b), None);
    }
}
