//! Tesseract's `TBOX` (`src/ccstruct/rect.h`, `rect.cpp`).

/// `INT16_MAX`, the corner of a null box.
const NULL_EDGE: i32 = i16::MAX as i32;

/// An axis-aligned box in Tesseract coordinates: `x` grows to the right,
/// `y` upwards from the bottom of the image. A box made with
/// [`TBox::default`] is Tesseract's null box, which a union replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TBox {
    pub(crate) left: i32,
    pub(crate) bottom: i32,
    pub(crate) right: i32,
    pub(crate) top: i32,
}

impl Default for TBox {
    /// `TBOX()`: the null box.
    fn default() -> Self {
        Self {
            left: NULL_EDGE,
            bottom: NULL_EDGE,
            right: -NULL_EDGE,
            top: -NULL_EDGE,
        }
    }
}

impl TBox {
    /// `TBOX(left, bottom, right, top)`.
    pub(crate) const fn new(left: i32, bottom: i32, right: i32, top: i32) -> Self {
        Self {
            left,
            bottom,
            right,
            top,
        }
    }

    /// `TBOX(ICOORD, ICOORD)`: the box spanned by two corners in any order.
    pub(crate) fn from_corners(a: (i32, i32), b: (i32, i32)) -> Self {
        Self::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
    }

    /// `TBOX::width`: 0 for a null box.
    pub(crate) fn width(&self) -> i32 {
        if self.is_null() {
            0
        } else {
            self.right - self.left
        }
    }

    /// `TBOX::height`: 0 for a null box.
    pub(crate) fn height(&self) -> i32 {
        if self.is_null() {
            0
        } else {
            self.top - self.bottom
        }
    }

    /// `TBOX::null_box`.
    pub(crate) fn is_null(&self) -> bool {
        self.left >= self.right || self.top <= self.bottom
    }

    /// `TBOX::area`.
    pub(crate) fn area(&self) -> i32 {
        if self.is_null() {
            0
        } else {
            self.width() * self.height()
        }
    }

    /// `TBOX::x_middle`.
    pub(crate) fn x_middle(&self) -> i32 {
        (self.left + self.right) / 2
    }

    /// `operator+=`: the union, without a null-box check.
    pub(crate) fn union(&self, o: &TBox) -> TBox {
        TBox {
            left: self.left.min(o.left),
            bottom: self.bottom.min(o.bottom),
            right: self.right.max(o.right),
            top: self.top.max(o.top),
        }
    }

    /// `TBOX::contains(ICOORD)`, edges included.
    pub(crate) fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.bottom && y <= self.top
    }

    /// `TBOX::contains(TBOX)`.
    pub(crate) fn contains(&self, b: &TBox) -> bool {
        self.contains_point(b.left, b.bottom) && self.contains_point(b.right, b.top)
    }

    /// `TBOX::overlap`, touching edges included.
    pub(crate) fn overlap(&self, b: &TBox) -> bool {
        b.left <= self.right && b.right >= self.left && b.bottom <= self.top && b.top >= self.bottom
    }

    /// `TBOX::major_overlap`.
    pub(crate) fn major_overlap(&self, b: &TBox) -> bool {
        let mut overlap = b.right.min(self.right) - b.left.max(self.left);
        overlap += overlap;
        if overlap < b.width().min(self.width()) {
            return false;
        }
        let mut overlap = b.top.min(self.top) - b.bottom.max(self.bottom);
        overlap += overlap;
        overlap >= b.height().min(self.height())
    }

    /// `TBOX::major_x_overlap`.
    pub(crate) fn major_x_overlap(&self, b: &TBox) -> bool {
        let mut overlap = b.width();
        if self.left > b.left {
            overlap -= self.left - b.left;
        }
        if self.right < b.right {
            overlap -= b.right - self.right;
        }
        overlap >= b.width() / 2 || overlap >= self.width() / 2
    }

    /// `TBOX::intersection`.
    pub(crate) fn intersection(&self, b: &TBox) -> TBox {
        if self.overlap(b) {
            TBox::new(
                self.left.max(b.left),
                self.bottom.max(b.bottom),
                self.right.min(b.right),
                self.top.min(b.top),
            )
        } else {
            TBox::new(NULL_EDGE, NULL_EDGE, -NULL_EDGE, -NULL_EDGE)
        }
    }
}
