//! Glow (docs/specs/effects-artistic-photo.md): a blurred, brightness- and
//! contrast-adjusted copy screen-blended over the image.

use crate::adjust::BrightnessContrast;
use crate::blur::{GaussianBlur, param};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Screen-blends `1 − (1 − a)(1 − b)` a Gaussian blur (`radius`) of the
/// image, adjusted by Brightness / Contrast (legacy, linear), over the
/// image's straight colours. Screen never darkens; alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    /// Blur radius in pixels, `1..=20`.
    pub radius: f32,
    /// Brightness of the glow copy, `-100..=100`.
    pub brightness: f32,
    /// Contrast of the glow copy, `-100..=100`.
    pub contrast: f32,
}

impl Glow {
    /// Creates a glow (non-finite numbers read as 0, then clamped).
    pub fn new(radius: f32, brightness: f32, contrast: f32) -> Self {
        Self {
            radius: param(radius, 1.0, 20.0),
            brightness: param(brightness, -100.0, 100.0),
            contrast: param(contrast, -100.0, 100.0),
        }
    }
}

impl Effect for Glow {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let blurred = GaussianBlur::new(self.radius).apply(src);
        let glow = BrightnessContrast::new(self.brightness, self.contrast).apply(&blurred);
        let mut out = src.data().to_vec();
        for (px, g) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(glow.data().as_chunks::<4>().0)
        {
            for (c, &b) in px[..3].iter_mut().zip(&g[..3]) {
                let (a, b) = (f32::from(*c), f32::from(b));
                *c = (255.0 - (255.0 - a) * (255.0 - b) / 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: self.radius * factor,
            ..*self
        }
    }
}
