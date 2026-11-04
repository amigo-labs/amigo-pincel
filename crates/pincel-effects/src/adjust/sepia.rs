//! Sepia (docs/specs/adjustments-paintnet.md): tones the image like an old
//! photograph.

use super::luma_x10000;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Blends each pixel toward its sepia tone by `intensity` percent; the tone
/// of a pixel with luma `l` (0–255) is `(min(255, l), 0.86 · l, 0.67 · l)`.
/// Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sepia {
    /// `0..=100`; `0` changes nothing.
    pub intensity: f32,
}

impl Sepia {
    /// Creates a sepia toning of `intensity` percent (non-finite reads as
    /// 0, then clamped to `0..=100`).
    pub fn new(intensity: f32) -> Self {
        let v = if intensity.is_finite() {
            intensity
        } else {
            0.0
        };
        Self {
            intensity: v.clamp(0.0, 100.0),
        }
    }
}

impl Effect for Sepia {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let k = f64::from(self.intensity) / 100.0;
        let mut out = src.data().to_vec();
        for p in out.as_chunks_mut::<4>().0 {
            let l = f64::from(luma_x10000(p[0], p[1], p[2])) / 10_000.0;
            let tone = [l.min(255.0), 0.86 * l, 0.67 * l];
            for (ch, t) in p[..3].iter_mut().zip(tone) {
                // c · (1 − k) + t · k is exactly c at k = 0 and t at k = 1.
                let v = f64::from(*ch) * (1.0 - k) + t * k;
                *ch = v.round().clamp(0.0, 255.0) as u8;
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
    fn half_intensity_lies_between_source_and_tone() {
        // Pure blue: luma 18.4, tone (18, 16, 12).
        let src = EffectImage::from_rgba8(1, 1, vec![0, 0, 255, 255]).unwrap();
        let half = Sepia::new(50.0).apply(&src);
        assert_eq!(half.data(), &[9, 8, 134, 255]);
    }
}
