//! Exposure (docs/specs/adjustments-paintnet.md): scales light in linear
//! space by a number of stops.

use super::{linear_to_srgb, srgb_to_linear};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Multiplies each channel in linear light by `2^ev`, converted back to
/// sRGB and clamped. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    /// Stops, `-5..=5`; `0` changes nothing.
    pub ev: f32,
}

impl Exposure {
    /// Creates an exposure change of `ev` stops (non-finite reads as 0,
    /// then clamped to `-5..=5`).
    pub fn new(ev: f32) -> Self {
        let ev = if ev.is_finite() { ev } else { 0.0 };
        Self {
            ev: ev.clamp(-5.0, 5.0),
        }
    }

    fn lut(&self) -> [u8; 256] {
        let gain = 2f64.powf(f64::from(self.ev));
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            let lin = srgb_to_linear(i as f64 / 255.0) * gain;
            *v = (linear_to_srgb(lin.min(1.0)) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        lut
    }
}

impl Effect for Exposure {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let lut = self.lut();
        let mut out = src.data().to_vec();
        for p in out.as_chunks_mut::<4>().0 {
            for c in &mut p[..3] {
                *c = lut[usize::from(*c)];
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

    #[test]
    fn exposure_zero_is_the_identity_lut() {
        let lut = Exposure::new(0.0).lut();
        assert!(lut.iter().enumerate().all(|(i, &v)| usize::from(v) == i));
    }

    #[test]
    fn exposure_one_stop_doubles_linear_light() {
        // sRGB 128 is ~0.2159 linear; doubled ~0.4317 is sRGB ~175.55.
        assert_eq!(Exposure::new(1.0).lut()[128], 176);
    }
}
