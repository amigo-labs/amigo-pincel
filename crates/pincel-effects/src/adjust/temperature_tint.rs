//! Temperature and Tint (docs/specs/adjustments-paintnet.md): a white
//! balance shift along the warm–cool and the green–magenta axis.

use crate::effect::Effect;
use crate::image::EffectImage;

/// Gain per unit of `temperature / 100` or `tint / 100` on a channel.
const GAIN: f64 = 0.3;

/// Channel gains in sRGB: temperature `t` multiplies red by `1 + 0.3 t` and
/// blue by `1 − 0.3 t`, tint `n` multiplies green by `1 − 0.3 n` (positive
/// tint toward magenta), with `t, n = value / 100`; clamped. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureTint {
    /// `-100..=100`; positive warms (red up, blue down).
    pub temperature: f32,
    /// `-100..=100`; positive shifts toward magenta (green down).
    pub tint: f32,
}

/// Non-finite reads as 0, then clamped to `-100..=100`.
fn percent(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-100.0, 100.0)
    } else {
        0.0
    }
}

impl TemperatureTint {
    /// Creates the shift (non-finite reads as 0, then each value is
    /// clamped to `-100..=100`).
    pub fn new(temperature: f32, tint: f32) -> Self {
        Self {
            temperature: percent(temperature),
            tint: percent(tint),
        }
    }
}

impl Effect for TemperatureTint {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let t = f64::from(self.temperature) / 100.0;
        let n = f64::from(self.tint) / 100.0;
        let gains = [1.0 + GAIN * t, 1.0 - GAIN * n, 1.0 - GAIN * t];
        let mut out = src.data().to_vec();
        for p in out.as_chunks_mut::<4>().0 {
            for (ch, g) in p[..3].iter_mut().zip(gains) {
                *ch = (f64::from(*ch) * g).round().clamp(0.0, 255.0) as u8;
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
    fn black_stays_black_under_any_shift() {
        let src = EffectImage::from_rgba8(1, 1, vec![0, 0, 0, 255]).unwrap();
        assert_eq!(TemperatureTint::new(100.0, -100.0).apply(&src), src);
    }
}
