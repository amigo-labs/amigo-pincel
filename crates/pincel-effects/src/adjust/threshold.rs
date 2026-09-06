//! Threshold (Fineliner spec §12.9): luminance above the cut → white, below → black.

use super::{luma, map_rgb};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Splits pixels to black or white by luminance. Alpha preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Threshold {
    /// Cut point, 0–255.
    pub threshold: u8,
}

impl Threshold {
    /// Creates a threshold adjustment.
    pub fn new(threshold: u8) -> Self {
        Self { threshold }
    }
}

impl Effect for Threshold {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let cut = self.threshold as f32 / 255.0;
        map_rgb(src, |rgb| {
            let v = if luma(rgb) >= cut { 1.0 } else { 0.0 };
            [v, v, v]
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_splits_by_luminance() {
        // Dark grey below the cut → black; light grey above → white.
        let src = EffectImage::from_rgba8(2, 1, vec![40, 40, 40, 255, 220, 220, 220, 255]).unwrap();
        let out = Threshold::new(128).apply(&src);
        assert_eq!(&out.data()[0..4], &[0, 0, 0, 255]);
        assert_eq!(&out.data()[4..8], &[255, 255, 255, 255]);
    }
}
