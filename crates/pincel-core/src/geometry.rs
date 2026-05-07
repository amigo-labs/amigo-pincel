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
}
