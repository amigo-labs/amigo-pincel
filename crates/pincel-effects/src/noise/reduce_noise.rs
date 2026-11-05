//! Reduce Noise (docs/specs/effects-object-stylize-color-noise.md):
//! paint.net's version, blending toward the neighbourhood median.

use crate::blur::{Median, param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Blends each pixel toward the median of its `(2·radius + 1)²`
/// neighbourhood by `strength` %. The median is Blurs ▸ Median's at
/// percentile 50: the colour channels read only opaque pixels, so a
/// transparent pixel's colour never tints; the blend runs premultiplied.
/// Strength 0 keeps the layer, Strength 100 is the median exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReduceNoise {
    /// Median window radius in pixels, `1..=200`.
    pub radius: u32,
    /// Blend toward the median in percent, `0..=100`.
    pub strength: f32,
}

impl ReduceNoise {
    /// Creates a Reduce Noise effect (non-finite numbers read as 0, then
    /// clamped: radius to `1..=200` rounded, strength to `0..=100`).
    pub fn new(radius: f32, strength: f32) -> Self {
        Self {
            radius: whole(radius, 1, 200),
            strength: param(strength, 0.0, 100.0),
        }
    }
}

impl Effect for ReduceNoise {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.strength <= 0.0 {
            return src.clone();
        }
        let median = Median::new(self.radius as f32, 50.0).apply(src);
        if self.strength >= 100.0 {
            return median;
        }
        let t = self.strength / 100.0;
        let a = src.to_premultiplied_f32();
        let b = median.to_premultiplied_f32();
        let mixed: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + (y - x) * t).collect();
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &mixed)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            strength: self.strength,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_noise_of_solid_is_unchanged() {
        let data: Vec<u8> = std::iter::repeat_n([50, 60, 70, 255], 25)
            .flatten()
            .collect();
        let src = EffectImage::from_rgba8(5, 5, data).unwrap();
        assert_eq!(ReduceNoise::new(1.0, 40.0).apply(&src), src);
    }

    #[test]
    fn reduce_noise_full_strength_removes_a_single_pixel_spike() {
        // A flat grey field with one white outlier in the centre.
        let mut data: Vec<u8> = std::iter::repeat_n([100, 100, 100, 255], 25)
            .flatten()
            .collect();
        let center = (2 * 5 + 2) * 4;
        data[center..center + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(5, 5, data).unwrap();
        let out = ReduceNoise::new(1.0, 100.0).apply(&src);
        // The spike is gone — the centre is back to the surrounding grey.
        assert_eq!(&out.data()[center..center + 3], &[100, 100, 100]);
    }

    #[test]
    fn scaled_rounds_radius_min_one() {
        assert_eq!(ReduceNoise::new(8.0, 40.0).scaled(0.5).radius, 4);
        assert_eq!(ReduceNoise::new(1.0, 40.0).scaled(0.1).radius, 1);
    }
}
