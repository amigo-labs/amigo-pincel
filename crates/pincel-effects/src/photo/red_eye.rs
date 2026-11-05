//! Red Eye Removal (docs/specs/effects-artistic-photo.md): lowers the red
//! channel of strongly red pixels.

use crate::blur::param;
use crate::effect::Effect;
use crate::image::EffectImage;

/// A pixel is strongly red when `(R − (G + B) / 2) / 255 · 100` is above
/// `100 − tolerance`; its red channel then moves toward `(G + B) / 2` by
/// `saturation` %. Green, blue and alpha are kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RedEyeRemoval {
    /// How weakly red a pixel may be and still count, `0..=100`.
    pub tolerance: f32,
    /// How far red moves toward the green / blue average, `0..=100` %;
    /// 0 leaves the layer unchanged.
    pub saturation: f32,
}

impl RedEyeRemoval {
    /// Creates a red-eye removal (non-finite numbers read as 0, then
    /// clamped).
    pub fn new(tolerance: f32, saturation: f32) -> Self {
        Self {
            tolerance: param(tolerance, 0.0, 100.0),
            saturation: param(saturation, 0.0, 100.0),
        }
    }
}

impl Effect for RedEyeRemoval {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.saturation == 0.0 {
            return src.clone();
        }
        let limit = 100.0 - self.tolerance;
        let k = self.saturation / 100.0;
        let mut out = src.data().to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            let avg = (f32::from(px[1]) + f32::from(px[2])) / 2.0;
            let excess = f32::from(px[0]) - avg;
            if excess / 255.0 * 100.0 > limit {
                px[0] = (f32::from(px[0]) - excess * k).round().clamp(0.0, 255.0) as u8;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}
