//! Soften Portrait (docs/specs/effects-artistic-photo.md): a blurred copy
//! for smooth skin, a brightness shift and a warm white balance.

use crate::adjust::TemperatureTint;
use crate::blur::{GaussianBlur, param};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Blur radius in pixels per unit of Softness.
const BLUR_PER_SOFTNESS: f32 = 2.0;
/// Share of the blurred copy per unit of Softness (half at Softness 10).
const MIX_PER_SOFTNESS: f32 = 0.05;
/// Channel offset (0–255) per unit of Lighting.
const LIGHT_PER_UNIT: f32 = 2.5;
/// Temperature (`adjustments-paintnet`) per unit of Warmth.
const TEMPERATURE_PER_WARMTH: f32 = 5.0;

/// In order: mixes `softness · 5 %` of a Gaussian blur (radius
/// `2 · softness`) into the straight colours, adds `2.5 · lighting` to every
/// channel, and warms like Temperature `5 · warmth`. Alpha is kept; all
/// three at 0 leave the layer unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoftenPortrait {
    /// Skin smoothing, `0..=10`.
    pub softness: f32,
    /// Brightness shift, `-20..=20`.
    pub lighting: f32,
    /// Warming, `0..=20`.
    pub warmth: f32,
}

impl SoftenPortrait {
    /// Creates the effect (non-finite numbers read as 0, then clamped).
    pub fn new(softness: f32, lighting: f32, warmth: f32) -> Self {
        Self {
            softness: param(softness, 0.0, 10.0),
            lighting: param(lighting, -20.0, 20.0),
            warmth: param(warmth, 0.0, 20.0),
        }
    }
}

impl Effect for SoftenPortrait {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let mut out = src.data().to_vec();
        if self.softness > 0.0 {
            let blurred = GaussianBlur::new(self.softness * BLUR_PER_SOFTNESS).apply(src);
            let k = self.softness * MIX_PER_SOFTNESS;
            for (px, b) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(blurred.data().as_chunks::<4>().0)
            {
                for (c, &v) in px[..3].iter_mut().zip(&b[..3]) {
                    let (a, v) = (f32::from(*c), f32::from(v));
                    *c = (a + (v - a) * k).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        if self.lighting != 0.0 {
            let offset = self.lighting * LIGHT_PER_UNIT;
            for px in out.as_chunks_mut::<4>().0 {
                for c in &mut px[..3] {
                    *c = (f32::from(*c) + offset).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        let out = EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions");
        if self.warmth > 0.0 {
            TemperatureTint::new(self.warmth * TEMPERATURE_PER_WARMTH, 0.0).apply(&out)
        } else {
            out
        }
    }

    fn scaled(&self, _factor: f32) -> Self {
        // Softness also sets the mix; a preview keeps the full look.
        *self
    }
}
