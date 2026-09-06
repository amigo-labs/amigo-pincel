//! Selection masks (spec §8).
//!
//! A selection is a grayscale coverage mask the size of the canvas: `255` =
//! fully selected, `0` = not selected, `1..=254` = partial (feather / anti-
//! alias). A document with no selection (`None`) treats every pixel as fully
//! selected. This module owns the mask type, the four combine modes, and the
//! basic shape rasterizers (rectangle, ellipse). Lasso, magic wand, and the
//! expand/contract/feather modifiers build on these in later tasks.

mod modifiers;
mod wand;

pub use wand::magic_wand;

use crate::geometry::{Point, Rect};
use serde::{Deserialize, Serialize};

/// How a newly drawn selection combines with the existing one (spec §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Replace the existing selection.
    #[default]
    Replace,
    /// Union with the existing selection (Shift).
    Add,
    /// Remove from the existing selection (Alt).
    Subtract,
    /// Keep only the overlap (Shift+Alt).
    Intersect,
}

/// A canvas-sized grayscale coverage mask (spec §8.1).
///
/// One byte per pixel in row-major order. Coverage is `0..=255`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionMask {
    width: u32,
    height: u32,
    /// `width * height` coverage bytes, row-major.
    data: Vec<u8>,
}

impl SelectionMask {
    /// Creates a mask with nothing selected (all zero).
    pub fn new_empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width as usize) * (height as usize)],
        }
    }

    /// Creates a mask with everything selected (all 255) — i.e. Select All.
    pub fn new_full(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![255; (width as usize) * (height as usize)],
        }
    }

    /// Mask width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Mask height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw coverage bytes, row-major.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Coverage at `(x, y)`, or `0` if out of bounds.
    pub fn coverage(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }

    /// Sets the coverage at `(x, y)`; out-of-bounds writes are ignored.
    pub fn set(&mut self, x: u32, y: u32, value: u8) {
        if x < self.width && y < self.height {
            self.data[(y as usize) * (self.width as usize) + (x as usize)] = value;
        }
    }

    /// Number of pixels with non-zero coverage.
    pub fn selected_count(&self) -> usize {
        self.data.iter().filter(|&&v| v > 0).count()
    }

    /// Whether nothing is selected (all coverage is zero).
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&v| v == 0)
    }

    /// Tight bounding box of the selected (non-zero) pixels, or `None` when the
    /// selection is empty. Used to position the marching-ants overlay (spec §8.5).
    pub fn bounding_box(&self) -> Option<Rect> {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (self.width, self.height, 0u32, 0u32);
        let mut any = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.coverage(x, y) > 0 {
                    any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        any.then(|| {
            Rect::new(
                min_x as i32,
                min_y as i32,
                max_x - min_x + 1,
                max_y - min_y + 1,
            )
        })
    }

    /// Inverts coverage in place: `255 - x` for every pixel (spec §8.4 Invert).
    pub fn invert(&mut self) {
        for v in &mut self.data {
            *v = 255 - *v;
        }
    }

    /// A rectangular selection (255 inside `rect`, clamped to the mask).
    pub fn rectangle(width: u32, height: u32, rect: Rect) -> Self {
        let mut mask = Self::new_empty(width, height);
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.right().max(0) as u32).min(width);
        let y1 = (rect.bottom().max(0) as u32).min(height);
        for y in y0..y1 {
            for x in x0..x1 {
                mask.set(x, y, 255);
            }
        }
        mask
    }

    /// An elliptical selection inscribed in `rect` (255 for pixel centers inside
    /// the ellipse, hard edge). Anti-aliasing is a later concern (spec §9.3).
    pub fn ellipse(width: u32, height: u32, rect: Rect) -> Self {
        let mut mask = Self::new_empty(width, height);
        if rect.w == 0 || rect.h == 0 {
            return mask;
        }
        // Ellipse center and radii in pixel space.
        let cx = rect.x as f32 + rect.w as f32 / 2.0;
        let cy = rect.y as f32 + rect.h as f32 / 2.0;
        let rx = rect.w as f32 / 2.0;
        let ry = rect.h as f32 / 2.0;
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.right().max(0) as u32).min(width);
        let y1 = (rect.bottom().max(0) as u32).min(height);
        for y in y0..y1 {
            for x in x0..x1 {
                // Test the pixel center.
                let nx = (x as f32 + 0.5 - cx) / rx;
                let ny = (y as f32 + 0.5 - cy) / ry;
                if nx * nx + ny * ny <= 1.0 {
                    mask.set(x, y, 255);
                }
            }
        }
        mask
    }

    /// A polygonal selection filled by the even-odd rule (Lasso / Polygonal
    /// Lasso, spec §9.3). `points` are polygon vertices in canvas space; the
    /// path is implicitly closed (last vertex back to the first). Fewer than
    /// three vertices select nothing. Hard edge; anti-aliasing is deferred.
    pub fn polygon(width: u32, height: u32, points: &[Point]) -> Self {
        let mut mask = Self::new_empty(width, height);
        let n = points.len();
        if n < 3 {
            return mask;
        }
        for y in 0..height {
            let yc = y as f32 + 0.5;
            // X coordinates where polygon edges cross this scanline.
            let mut crossings: Vec<f32> = Vec::new();
            for i in 0..n {
                let a = points[i];
                let b = points[(i + 1) % n];
                // Half-open test avoids double-counting shared vertices.
                if (a.y <= yc && b.y > yc) || (b.y <= yc && a.y > yc) {
                    let t = (yc - a.y) / (b.y - a.y);
                    crossings.push(a.x + t * (b.x - a.x));
                }
            }
            crossings.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
            // Fill spans between consecutive crossing pairs.
            for pair in crossings.as_chunks::<2>().0 {
                let x_lo = (pair[0] - 0.5).ceil().max(0.0) as i64;
                let x_hi = ((pair[1] - 0.5).ceil() as i64).min(width as i64);
                for x in x_lo..x_hi {
                    mask.set(x as u32, y, 255);
                }
            }
        }
        mask
    }

    /// Combines `incoming` into `self` per `mode`, mutating `self` in place.
    ///
    /// Masks must share dimensions; mismatched sizes are a no-op. Coverage math:
    /// Add = `max`, Subtract = `saturating_sub`, Intersect = `min`, Replace
    /// copies `incoming`.
    pub fn combine(&mut self, incoming: &SelectionMask, mode: SelectionMode) {
        if self.width != incoming.width || self.height != incoming.height {
            return;
        }
        match mode {
            SelectionMode::Replace => self.data.copy_from_slice(&incoming.data),
            SelectionMode::Add => {
                for (a, b) in self.data.iter_mut().zip(&incoming.data) {
                    *a = (*a).max(*b);
                }
            }
            SelectionMode::Subtract => {
                for (a, b) in self.data.iter_mut().zip(&incoming.data) {
                    *a = a.saturating_sub(*b);
                }
            }
            SelectionMode::Intersect => {
                for (a, b) in self.data.iter_mut().zip(&incoming.data) {
                    *a = (*a).min(*b);
                }
            }
        }
    }
}

/// Applies a freshly drawn selection to the prior selection per `mode`.
///
/// Replace yields the new mask; the other modes start from the existing mask
/// (or, when there is none, treat the canvas as fully selected for Subtract /
/// Intersect and empty for Add). Always returns a concrete mask — never `None` —
/// even when the result selects nothing. The caller decides whether to store it
/// or clear the document's selection; note that a document's `None` selection
/// means "everything selected", which is *not* the same as an all-zero mask
/// (see [`SelectionMask::is_empty`]).
pub fn apply_mode(
    existing: Option<&SelectionMask>,
    incoming: SelectionMask,
    mode: SelectionMode,
) -> SelectionMask {
    match mode {
        SelectionMode::Replace => incoming,
        SelectionMode::Add => {
            let mut base = existing
                .cloned()
                .unwrap_or_else(|| SelectionMask::new_empty(incoming.width, incoming.height));
            base.combine(&incoming, SelectionMode::Add);
            base
        }
        SelectionMode::Subtract | SelectionMode::Intersect => {
            // No prior selection means "everything selected" (spec §8.1).
            let mut base = existing
                .cloned()
                .unwrap_or_else(|| SelectionMask::new_full(incoming.width, incoming.height));
            base.combine(&incoming, mode);
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_full_selects_everything() {
        let m = SelectionMask::new_full(4, 4);
        assert_eq!(m.selected_count(), 16);
        assert_eq!(m.coverage(0, 0), 255);
        assert!(!m.is_empty());
    }

    #[test]
    fn new_empty_selects_nothing() {
        let m = SelectionMask::new_empty(4, 4);
        assert_eq!(m.selected_count(), 0);
        assert!(m.is_empty());
    }

    #[test]
    fn rectangle_selects_only_inside() {
        let m = SelectionMask::rectangle(10, 10, Rect::new(2, 3, 4, 2));
        assert_eq!(m.coverage(2, 3), 255);
        assert_eq!(m.coverage(5, 4), 255);
        assert_eq!(m.coverage(1, 3), 0); // left of rect
        assert_eq!(m.coverage(6, 3), 0); // right edge is exclusive
        assert_eq!(m.selected_count(), 8); // 4 × 2
    }

    #[test]
    fn rectangle_clamps_to_canvas() {
        let m = SelectionMask::rectangle(4, 4, Rect::new(-2, -2, 4, 4));
        // Only the lower-right quadrant lands on the canvas.
        assert_eq!(m.coverage(0, 0), 255);
        assert_eq!(m.coverage(1, 1), 255);
        assert_eq!(m.selected_count(), 4);
    }

    #[test]
    fn ellipse_selects_center_not_corner() {
        let m = SelectionMask::ellipse(20, 20, Rect::new(0, 0, 20, 20));
        assert_eq!(m.coverage(10, 10), 255); // center
        assert_eq!(m.coverage(0, 0), 0); // corner is outside the inscribed ellipse
        assert!(m.selected_count() > 0);
        assert!(m.selected_count() < 400); // strictly fewer than the bounding box
    }

    #[test]
    fn invert_swaps_full_and_empty() {
        let mut m = SelectionMask::new_full(4, 4);
        m.invert();
        assert!(m.is_empty());
        m.invert();
        assert_eq!(m.selected_count(), 16);
    }

    #[test]
    fn invert_complements_pixel_coverage() {
        // Spec §8.4: Invert = 255 − x. A rectangle's complement covers the rest.
        let mut m = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 4, 4));
        let before = m.selected_count();
        m.invert();
        assert_eq!(m.selected_count(), 100 - before);
        assert_eq!(m.coverage(0, 0), 0); // was selected → now clear
        assert_eq!(m.coverage(9, 9), 255); // was clear → now selected
    }

    #[test]
    fn combine_add_is_union() {
        let mut a = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 4, 4));
        let b = SelectionMask::rectangle(10, 10, Rect::new(4, 0, 4, 4));
        a.combine(&b, SelectionMode::Add);
        assert_eq!(a.selected_count(), 32);
    }

    #[test]
    fn combine_subtract_removes_overlap() {
        let mut a = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 6, 4));
        let b = SelectionMask::rectangle(10, 10, Rect::new(4, 0, 6, 4));
        a.combine(&b, SelectionMode::Subtract);
        assert_eq!(a.selected_count(), 16); // 6×4 minus the 2×4 overlap
    }

    #[test]
    fn combine_intersect_keeps_only_overlap() {
        let mut a = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 6, 4));
        let b = SelectionMask::rectangle(10, 10, Rect::new(4, 0, 6, 4));
        a.combine(&b, SelectionMode::Intersect);
        assert_eq!(a.selected_count(), 8); // 2×4 overlap
    }

    #[test]
    fn apply_mode_subtract_without_prior_starts_full() {
        // No prior selection means everything is selected (spec §8.1), so
        // subtracting a rectangle leaves the complement.
        let rect = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 4, 4));
        let result = apply_mode(None, rect, SelectionMode::Subtract);
        assert_eq!(result.selected_count(), 100 - 16);
    }

    #[test]
    fn bounding_box_is_tight_around_selection() {
        let m = SelectionMask::rectangle(20, 20, Rect::new(3, 5, 4, 6));
        let bb = m.bounding_box().unwrap();
        assert_eq!((bb.x, bb.y, bb.w, bb.h), (3, 5, 4, 6));
    }

    #[test]
    fn bounding_box_of_empty_is_none() {
        assert!(SelectionMask::new_empty(8, 8).bounding_box().is_none());
    }

    #[test]
    fn polygon_fills_triangle_interior() {
        // A right triangle with vertices (0,0), (10,0), (0,10).
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
        ];
        let m = SelectionMask::polygon(12, 12, &pts);
        assert_eq!(m.coverage(1, 1), 255); // inside
        assert_eq!(m.coverage(8, 8), 0); // beyond the hypotenuse
        assert!(m.selected_count() > 0);
        assert!(m.selected_count() < 100);
    }

    #[test]
    fn polygon_matches_rectangle_for_axis_aligned_box() {
        let pts = [
            Point::new(2.0, 2.0),
            Point::new(6.0, 2.0),
            Point::new(6.0, 6.0),
            Point::new(2.0, 6.0),
        ];
        let poly = SelectionMask::polygon(10, 10, &pts);
        let rect = SelectionMask::rectangle(10, 10, Rect::new(2, 2, 4, 4));
        assert_eq!(poly.selected_count(), rect.selected_count());
        assert_eq!(poly.data(), rect.data());
    }

    #[test]
    fn polygon_with_too_few_points_selects_nothing() {
        let pts = [Point::new(0.0, 0.0), Point::new(5.0, 5.0)];
        assert!(SelectionMask::polygon(8, 8, &pts).is_empty());
    }

    #[test]
    fn apply_mode_replace_returns_incoming() {
        let rect = SelectionMask::rectangle(10, 10, Rect::new(0, 0, 4, 4));
        let result = apply_mode(None, rect, SelectionMode::Replace);
        assert_eq!(result.selected_count(), 16);
    }
}
