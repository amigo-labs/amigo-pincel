//! Layer ▸ Rotate / Zoom (docs/specs/image-transform-handles-rotate-zoom.md
//! AC-04): turns the layer about the canvas centre, zooms it about that
//! centre, then pans it, resampled with [`Quality`].

use super::{Affine, Quality, resample};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Rotate / Zoom's parameters; [`RotateZoom::new`] clamps them to the
/// dialog's ranges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateZoom {
    /// Degrees, `-180..=180`, clockwise on screen for a positive angle.
    pub angle: f64,
    /// Shift of the layer's centre to the right, in percent of the canvas
    /// width, `-100..=100`.
    pub pan_x: f64,
    /// Shift of the layer's centre down, in percent of the canvas height,
    /// `-100..=100`.
    pub pan_y: f64,
    /// Scale factor, `0.1..=10`.
    pub zoom: f64,
    /// Resampling taps.
    pub quality: Quality,
}

impl RotateZoom {
    /// Creates the transform; each value is clamped to its range, and a
    /// non-finite value reads as 0 first (the crate's wire convention).
    pub fn new(angle: f64, pan_x: f64, pan_y: f64, zoom: f64, quality: Quality) -> Self {
        let clamp = |v: f64, lo: f64, hi: f64| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                0.0f64.clamp(lo, hi)
            }
        };
        Self {
            angle: clamp(angle, -180.0, 180.0),
            pan_x: clamp(pan_x, -100.0, 100.0),
            pan_y: clamp(pan_y, -100.0, 100.0),
            zoom: clamp(zoom, 0.1, 10.0),
            quality,
        }
    }

    /// Whether the transform leaves every pixel where it is.
    pub fn is_identity(&self) -> bool {
        self.angle.rem_euclid(360.0) == 0.0
            && self.pan_x == 0.0
            && self.pan_y == 0.0
            && self.zoom == 1.0
    }

    /// The forward map on a `width` × `height` canvas: about the centre
    /// `(width / 2, height / 2)`, the pan in pixels.
    pub fn forward(&self, width: u32, height: u32) -> Affine {
        let (w, h) = (f64::from(width), f64::from(height));
        Affine::rotate_zoom(
            (w / 2.0, h / 2.0),
            self.angle,
            self.zoom,
            (self.pan_x * w / 100.0, self.pan_y * h / 100.0),
        )
    }
}

impl Effect for RotateZoom {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.is_identity() {
            return src.clone();
        }
        let (w, h) = (src.width(), src.height());
        match self.forward(w, h).inverse() {
            Some(inverse) => resample(src, w, h, &inverse, self.quality),
            None => EffectImage::new_transparent(w, h),
        }
    }

    fn scaled(&self, _factor: f32) -> Self {
        // The pan is a share of the canvas, the angle and zoom have no
        // extent: a downscaled preview uses the same values.
        *self
    }
}
