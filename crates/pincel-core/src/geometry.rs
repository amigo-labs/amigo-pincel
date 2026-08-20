//! Basic geometry primitives used throughout the document model.

/// A 2D point on the sprite grid. Signed to allow positions outside the
/// canvas (e.g. cel offsets, slice keys after a translation).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ORIGIN: Self = Self::new(0, 0);

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle in sprite coordinates. The rectangle covers
/// `[x, x + width)` × `[y, y + height)` — top-left inclusive, bottom-right
/// exclusive.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(&self, p: Point) -> bool {
        let x = i64::from(p.x);
        let y = i64::from(p.y);
        x >= i64::from(self.x)
            && y >= i64::from(self.y)
            && x < i64::from(self.x) + i64::from(self.width)
            && y < i64::from(self.y) + i64::from(self.height)
    }

    /// Intersection of `self` and `other` as an axis-aligned rectangle.
    /// Returns an empty rect (zero width or height) when they do not
    /// overlap; the empty rect's `x` / `y` are clamped to the overlap's
    /// nominal origin so callers can still address it.
    pub fn intersect(&self, other: Rect) -> Rect {
        let ax = i64::from(self.x);
        let ay = i64::from(self.y);
        let aw = i64::from(self.width);
        let ah = i64::from(self.height);
        let bx = i64::from(other.x);
        let by = i64::from(other.y);
        let bw = i64::from(other.width);
        let bh = i64::from(other.height);

        let x0 = ax.max(bx);
        let y0 = ay.max(by);
        let x1 = (ax + aw).min(bx + bw);
        let y1 = (ay + ah).min(by + bh);

        let w = (x1 - x0).max(0) as u32;
        let h = (y1 - y0).max(0) as u32;
        Rect::new(x0 as i32, y0 as i32, w, h)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn rect_contains_includes_top_left_excludes_bottom_right() {
        let r = Rect::new(2, 3, 4, 5);
        assert!(r.contains(Point::new(2, 3)));
        assert!(r.contains(Point::new(5, 7)));
        assert!(!r.contains(Point::new(6, 7)));
        assert!(!r.contains(Point::new(5, 8)));
        assert!(!r.contains(Point::new(1, 3)));
    }

    #[test]
    fn rect_is_empty_when_either_dimension_is_zero() {
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        assert!(Rect::new(0, 0, 5, 0).is_empty());
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
    }

    #[test]
    fn point_origin_is_zero_zero() {
        assert_eq!(Point::ORIGIN, Point::new(0, 0));
    }

    #[test]
    fn rect_intersect_fully_inside_returns_inner() {
        let outer = Rect::new(0, 0, 10, 10);
        let inner = Rect::new(2, 3, 4, 5);
        assert_eq!(outer.intersect(inner), inner);
        assert_eq!(inner.intersect(outer), inner);
    }

    #[test]
    fn rect_intersect_partial_overlap_returns_overlap() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(b), Rect::new(5, 5, 5, 5));
    }

    #[test]
    fn rect_intersect_no_overlap_returns_empty() {
        let a = Rect::new(0, 0, 4, 4);
        let b = Rect::new(10, 10, 4, 4);
        assert!(a.intersect(b).is_empty());
    }

    #[test]
    fn rect_intersect_touching_edges_returns_empty() {
        let a = Rect::new(0, 0, 4, 4);
        // b starts exactly where a ends (exclusive upper bound).
        let b = Rect::new(4, 0, 4, 4);
        assert!(a.intersect(b).is_empty());
    }

    #[test]
    fn rect_intersect_negative_coordinates_clamp_correctly() {
        let a = Rect::new(-5, -5, 10, 10);
        let b = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(b), Rect::new(0, 0, 5, 5));
    }
}
