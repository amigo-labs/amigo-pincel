//! Gradient parameter math shared by both cores: Image mode's blending
//! Gradient (`pincel-image-core`) and Pixel mode's dithered Gradient
//! (`pincel-core`) map a pixel to the same `t` (`docs/specs/pincel.md` §15,
//! 2026-10-29).
//!
//! Every function takes the pixel's offset `(px, py)` from the drag's start
//! and returns the position along the ramp: `0` at the start, `1` at the end.

/// A gradient drag, pre-computed once per gradient instead of per pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientLine {
    dx: f32,
    dy: f32,
    len: f32,
    len_sq: f32,
}

impl GradientLine {
    /// The drag that runs `(dx, dy)` from its start. `None` for a drag
    /// whose squared length is at most `f32::EPSILON`: it has no direction
    /// to ramp along.
    pub fn new(dx: f32, dy: f32) -> Option<Self> {
        let len_sq = dx * dx + dy * dy;
        if len_sq <= f32::EPSILON {
            return None;
        }
        Some(Self {
            dx,
            dy,
            len: len_sq.sqrt(),
            len_sq,
        })
    }

    /// The drag's x extent.
    pub fn dx(&self) -> f32 {
        self.dx
    }

    /// The drag's y extent.
    pub fn dy(&self) -> f32 {
        self.dy
    }

    /// The drag's length.
    pub fn len(&self) -> f32 {
        self.len
    }

    /// Where the offset `(px, py)` projects onto the drag, unclamped:
    /// `0` at the start, `1` at the end, negative before the start.
    pub fn projection(&self, px: f32, py: f32) -> f32 {
        (px * self.dx + py * self.dy) / self.len_sq
    }

    /// Linear `t`: the projection clamped to `[0, 1]`, so everything before
    /// the start is `0` and everything past the end is `1`.
    pub fn linear(&self, px: f32, py: f32) -> f32 {
        self.projection(px, py).clamp(0.0, 1.0)
    }

    /// Radial `t`: the distance from the start over the drag's length,
    /// clamped to `[0, 1]`.
    pub fn radial(&self, px: f32, py: f32) -> f32 {
        ((px * px + py * py).sqrt() / self.len).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero_length_drag_returns_none() {
        assert_eq!(GradientLine::new(0.0, 0.0), None);
        assert_eq!(GradientLine::new(f32::EPSILON / 4.0, 0.0), None);
    }

    #[test]
    fn linear_is_zero_at_the_start_and_one_at_the_end() {
        let line = GradientLine::new(4.0, 3.0).unwrap();
        assert_eq!(line.linear(0.0, 0.0), 0.0);
        assert_eq!(line.linear(4.0, 3.0), 1.0);
        assert!((line.linear(2.0, 1.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn linear_clamps_before_the_start_and_past_the_end() {
        let line = GradientLine::new(10.0, 0.0).unwrap();
        assert_eq!(line.linear(-5.0, 7.0), 0.0);
        assert_eq!(line.linear(25.0, -3.0), 1.0);
        assert!(line.projection(-5.0, 0.0) < 0.0);
    }

    #[test]
    fn linear_ignores_the_perpendicular_offset() {
        let line = GradientLine::new(8.0, 0.0).unwrap();
        assert_eq!(line.linear(3.0, -9.0), line.linear(3.0, 4.0));
    }

    #[test]
    fn radial_grows_with_the_distance_from_the_start() {
        let line = GradientLine::new(0.0, 5.0).unwrap();
        assert_eq!(line.radial(0.0, 0.0), 0.0);
        assert_eq!(line.radial(3.0, 4.0), 1.0);
        assert_eq!(line.radial(-30.0, 0.0), 1.0);
        assert!((line.radial(-1.5, -2.0) - 0.5).abs() < 1e-6);
    }
}
