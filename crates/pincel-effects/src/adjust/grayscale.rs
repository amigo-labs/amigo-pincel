//! Grayscale (Fineliner spec §12.7): desaturate via a chosen weighting.

use super::map_rgb;
use crate::effect::Effect;
use crate::image::EffectImage;

/// How RGB is collapsed to grey.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrayscaleMethod {
    /// Rec.601 luma (0.299, 0.587, 0.114).
    Luminosity,
    /// Simple mean of the three channels.
    Average,
    /// Rec.709 luma (0.2126, 0.7152, 0.0722).
    Bt709,
    /// User-supplied channel weights (see [`Grayscale::mixer`]).
    ChannelMixer,
}

/// Converts a layer to grey. `mixer` is used only by [`GrayscaleMethod::ChannelMixer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grayscale {
    /// Which weighting to use.
    pub method: GrayscaleMethod,
    /// Channel-mixer weights (should sum to ~1.0).
    pub mixer: [f32; 3],
}

impl Grayscale {
    /// Creates a grayscale adjustment.
    pub fn new(method: GrayscaleMethod) -> Self {
        Self {
            method,
            mixer: [0.299, 0.587, 0.114],
        }
    }

    /// Creates a channel-mixer grayscale with explicit weights.
    pub fn mixer(weights: [f32; 3]) -> Self {
        Self {
            method: GrayscaleMethod::ChannelMixer,
            mixer: weights,
        }
    }

    fn weights(&self) -> [f32; 3] {
        match self.method {
            GrayscaleMethod::Luminosity => [0.299, 0.587, 0.114],
            GrayscaleMethod::Average => [1.0 / 3.0; 3],
            GrayscaleMethod::Bt709 => [0.2126, 0.7152, 0.0722],
            GrayscaleMethod::ChannelMixer => self.mixer,
        }
    }
}

impl Effect for Grayscale {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let w = self.weights();
        map_rgb(src, |[r, g, b]| {
            let gray = r * w[0] + g * w[1] + b * w[2];
            [gray, gray, gray]
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
    fn grayscale_output_channels_are_equal() {
        let src = EffectImage::from_rgba8(1, 1, vec![200, 100, 50, 255]).unwrap();
        let out = Grayscale::new(GrayscaleMethod::Luminosity).apply(&src);
        let px = out.data();
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
        assert_eq!(px[3], 255);
    }

    #[test]
    fn luminosity_preserves_luma() {
        // The grey value equals the Rec.601 luma of the source.
        let src = EffectImage::from_rgba8(1, 1, vec![200, 100, 50, 255]).unwrap();
        let out = Grayscale::new(GrayscaleMethod::Luminosity).apply(&src);
        let expected = (0.299_f32 * 200.0 + 0.587 * 100.0 + 0.114 * 50.0).round() as i32;
        assert!((out.data()[0] as i32 - expected).abs() <= 1);
    }

    #[test]
    fn average_is_the_mean() {
        let src = EffectImage::from_rgba8(1, 1, vec![30, 60, 90, 255]).unwrap();
        let out = Grayscale::new(GrayscaleMethod::Average).apply(&src);
        assert_eq!(out.data()[0], 60);
    }
}
