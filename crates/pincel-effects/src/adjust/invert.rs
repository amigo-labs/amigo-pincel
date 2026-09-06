//! Invert (Fineliner spec §12.6): each channel becomes 1 − x; alpha preserved.

use super::map_rgb;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Inverts the RGB channels. No parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Invert;

impl Effect for Invert {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        map_rgb(src, |[r, g, b]| [1.0 - r, 1.0 - g, 1.0 - b])
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_of_invert_is_identity() {
        let src = EffectImage::from_rgba8(2, 1, vec![10, 200, 30, 255, 0, 128, 255, 80]).unwrap();
        assert_eq!(Invert.apply(&Invert.apply(&src)), src);
    }

    #[test]
    fn invert_preserves_alpha() {
        let src = EffectImage::from_rgba8(1, 1, vec![10, 20, 30, 123]).unwrap();
        let out = Invert.apply(&src);
        assert_eq!(out.data(), &[245, 235, 225, 123]);
    }
}
