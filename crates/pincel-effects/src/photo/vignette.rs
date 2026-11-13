//! Vignette (docs/specs/effects-artistic-photo.md): darkens or lightens
//! toward the edges around a centre point.

use crate::blur::param;
use crate::distort::edge::centre;
use crate::effect::Effect;
use crate::image::EffectImage;

/// The colour a vignette fades toward.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VignetteColor {
    /// Darkens toward the edges.
    #[default]
    Black,
    /// Lightens toward the edges.
    White,
}

/// With `r0 = radius % · half the image diagonal` and `d` a pixel's distance
/// from the centre, the pixel moves toward the colour by `density % ·
/// smoothstep((d − r0) / r0)`: unchanged within `r0`, fully faded from
/// `2 · r0`. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vignette {
    /// Centre as a fraction of the width, `0..=1`.
    pub center_x: f32,
    /// Centre as a fraction of the height, `0..=1`.
    pub center_y: f32,
    /// Where the falloff starts, `10..=400` % of half the diagonal.
    pub radius: f32,
    /// Strength, `0..=100` %; 0 leaves the layer unchanged.
    pub density: f32,
    /// Black darkens, White lightens.
    pub color: VignetteColor,
}

impl Vignette {
    /// Creates a vignette (non-finite numbers read as 0, then clamped).
    pub fn new(
        center_x: f32,
        center_y: f32,
        radius: f32,
        density: f32,
        color: VignetteColor,
    ) -> Self {
        Self {
            center_x: param(center_x, 0.0, 1.0),
            center_y: param(center_y, 0.0, 1.0),
            radius: param(radius, 10.0, 400.0),
            density: param(density, 0.0, 100.0),
            color,
        }
    }
}

impl Effect for Vignette {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.density == 0.0 {
            return src.clone();
        }
        let (w, h) = (src.width(), src.height());
        let (cx, cy) = (centre(self.center_x, w), centre(self.center_y, h));
        let half_diagonal = libm::hypotf(w as f32, h as f32) / 2.0;
        let r0 = (self.radius / 100.0 * half_diagonal).max(f32::EPSILON);
        let strength = self.density / 100.0;
        let mut out = src.data().to_vec();
        for (i, px) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
            let t = ((libm::hypotf(x - cx, y - cy) - r0) / r0).clamp(0.0, 1.0);
            let k = strength * t * t * (3.0 - 2.0 * t);
            if k == 0.0 {
                continue;
            }
            for c in &mut px[..3] {
                let v = f32::from(*c);
                let to = match self.color {
                    VignetteColor::Black => 0.0,
                    VignetteColor::White => 255.0,
                };
                *c = (v + (to - v) * k).round().clamp(0.0, 255.0) as u8;
            }
        }
        EffectImage::from_rgba8(w, h, out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // Every distance is relative to the image size.
        *self
    }
}
