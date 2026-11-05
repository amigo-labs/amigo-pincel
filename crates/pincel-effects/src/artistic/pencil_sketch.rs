//! Pencil Sketch (docs/specs/effects-artistic-photo.md): a gray colour-dodge
//! of the luma by its blurred negative.

use super::luma;
use crate::blur::{GaussianBlur, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Gray pencil lines: with `l` the luma and `b` the Gaussian blur (radius =
/// `tip_size`) of `255 − l`, each pixel becomes `255` when `b = 255`, else
/// `min(255, l · 255 / (255 − b))`, plus `color_range · 5`, clamped. Alpha
/// is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PencilSketch {
    /// Blur radius of the negative in pixels, `1..=20`.
    pub tip_size: u32,
    /// Brightness offset in steps of 5, `-20..=20`.
    pub color_range: i32,
}

impl PencilSketch {
    /// Creates a pencil sketch (non-finite numbers read as 0, then rounded
    /// and clamped).
    pub fn new(tip_size: f32, color_range: f32) -> Self {
        let range = if color_range.is_finite() {
            color_range.round().clamp(-20.0, 20.0) as i32
        } else {
            0
        };
        Self {
            tip_size: whole(tip_size, 1, 20),
            color_range: range,
        }
    }
}

impl Effect for PencilSketch {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        // Whole-number lumas, so a flat area's blurred negative is exactly
        // `255 − l` and dodges to white.
        let lumas: Vec<f32> = src
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| luma(p).round())
            .collect();
        // The negative as an opaque gray image, so alpha plays no part in
        // the blur.
        let negative: Vec<u8> = lumas
            .iter()
            .flat_map(|&l| {
                let v = (255.0 - l) as u8;
                [v, v, v, 255]
            })
            .collect();
        let negative =
            EffectImage::from_rgba8(src.width(), src.height(), negative).expect("same dimensions");
        let blurred = GaussianBlur::new(self.tip_size as f32).apply(&negative);
        let offset = self.color_range as f32 * 5.0;
        let mut out = src.data().to_vec();
        for ((px, &l), b) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&lumas)
            .zip(blurred.data().as_chunks::<4>().0)
        {
            let b = f32::from(b[0]);
            let dodge = if b >= 255.0 {
                255.0
            } else {
                (l * 255.0 / (255.0 - b)).min(255.0)
            };
            let v = (dodge + offset).round().clamp(0.0, 255.0) as u8;
            px[..3].fill(v);
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            tip_size: ((self.tip_size as f32 * factor).round() as u32).max(1),
            ..*self
        }
    }
}
