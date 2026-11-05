//! Invert Alpha (docs/specs/adjustments-paintnet.md): opacity becomes
//! transparency and back.

use crate::effect::Effect;
use crate::image::EffectImage;

/// Sets every pixel's alpha to `255 − alpha` and keeps its colour
/// channels. No parameters; applying it twice restores the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InvertAlpha;

impl Effect for InvertAlpha {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let mut out = src.data().to_vec();
        for p in out.as_chunks_mut::<4>().0 {
            p[3] = 255 - p[3];
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_alpha_flips_only_alpha() {
        let src = EffectImage::from_rgba8(1, 1, vec![10, 20, 30, 200]).unwrap();
        assert_eq!(InvertAlpha.apply(&src).data(), &[10, 20, 30, 55]);
    }
}
