//! Levels (Fineliner spec §12.4): input black/gamma/white and output black/white, per
//! composite or individual channel, applied through a 256-LUT.

use crate::effect::Effect;
use crate::image::EffectImage;

/// Which channel(s) the levels remap targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelsChannel {
    /// R, G and B together.
    Composite,
    /// Red only.
    Red,
    /// Green only.
    Green,
    /// Blue only.
    Blue,
}

/// Levels adjustment. Points are in `[0,1]`; `gamma` is a midtone exponent
/// (1.0 = linear). Identity is `in_black=0, in_white=1, gamma=1, out_black=0,
/// out_white=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    /// Target channel(s).
    pub channel: LevelsChannel,
    /// Input black point, `[0,1)`.
    pub in_black: f32,
    /// Input white point, `(0,1]`.
    pub in_white: f32,
    /// Midtone gamma (0.01–9.99).
    pub gamma: f32,
    /// Output black point, `[0,1)`.
    pub out_black: f32,
    /// Output white point, `(0,1]`.
    pub out_white: f32,
}

impl Levels {
    /// Creates a levels adjustment.
    pub fn new(
        channel: LevelsChannel,
        in_black: f32,
        in_white: f32,
        gamma: f32,
        out_black: f32,
        out_white: f32,
    ) -> Self {
        Self {
            channel,
            in_black,
            in_white,
            gamma,
            out_black,
            out_white,
        }
    }

    fn build_lut(&self) -> [u8; 256] {
        let span = (self.in_white - self.in_black).max(1e-6);
        let inv_gamma = 1.0 / self.gamma.max(1e-3);
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            let t = (((i as f32 / 255.0) - self.in_black) / span).clamp(0.0, 1.0);
            let g = t.powf(inv_gamma);
            let out = self.out_black + g * (self.out_white - self.out_black);
            *v = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        lut
    }
}

impl Effect for Levels {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let lut = self.build_lut();
        let mut out = src.data().to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            match self.channel {
                LevelsChannel::Composite => {
                    px[0] = lut[px[0] as usize];
                    px[1] = lut[px[1] as usize];
                    px[2] = lut[px[2] as usize];
                }
                LevelsChannel::Red => px[0] = lut[px[0] as usize],
                LevelsChannel::Green => px[1] = lut[px[1] as usize],
                LevelsChannel::Blue => px[2] = lut[px[2] as usize],
            }
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

    fn identity() -> Levels {
        Levels::new(LevelsChannel::Composite, 0.0, 1.0, 1.0, 0.0, 1.0)
    }

    #[test]
    fn levels_identity_is_a_no_op() {
        let src = EffectImage::from_rgba8(2, 1, vec![10, 90, 200, 255, 0, 128, 255, 60]).unwrap();
        assert_eq!(identity().apply(&src), src);
    }

    #[test]
    fn raising_input_black_darkens_shadows() {
        // Pixels below the new black point clip to output black.
        let src = EffectImage::from_rgba8(1, 1, vec![20, 20, 20, 255]).unwrap();
        let out = Levels::new(LevelsChannel::Composite, 0.2, 1.0, 1.0, 0.0, 1.0).apply(&src);
        assert_eq!(&out.data()[0..3], &[0, 0, 0]);
    }

    #[test]
    fn output_white_caps_brightness() {
        let src = EffectImage::from_rgba8(1, 1, vec![255, 255, 255, 255]).unwrap();
        let out = Levels::new(LevelsChannel::Composite, 0.0, 1.0, 1.0, 0.0, 0.5).apply(&src);
        assert_eq!(out.data()[0], 128);
    }
}
