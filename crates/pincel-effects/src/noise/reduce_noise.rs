//! Reduce Noise (Fineliner spec §11.4): a median filter of a given radius.

use crate::effect::Effect;
use crate::image::EffectImage;

/// Reduce Noise: replaces each RGB channel with the median of its
/// `(2·radius+1)²` neighbourhood, which removes impulse (salt-and-pepper) noise
/// while keeping edges sharp. Alpha is preserved. `radius` is 1–10 px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReduceNoise {
    /// Median window radius in pixels.
    pub radius: u32,
}

impl ReduceNoise {
    /// Creates a Reduce Noise effect.
    pub fn new(radius: u32) -> Self {
        Self { radius }
    }
}

/// Median of a slice of bytes (mutates order). Returns the lower-middle element.
fn median(values: &mut [u8]) -> u8 {
    values.sort_unstable();
    values[values.len() / 2]
}

impl Effect for ReduceNoise {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let r = self.radius as i32;
        if r <= 0 {
            return src.clone();
        }
        let (w, h) = (src.width() as i32, src.height() as i32);
        let data = src.data();
        let mut out = data.to_vec();
        let mut window: Vec<u8> = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    window.clear();
                    for dy in -r..=r {
                        let sy = (y + dy).clamp(0, h - 1);
                        for dx in -r..=r {
                            let sx = (x + dx).clamp(0, w - 1);
                            window.push(data[((sy * w + sx) * 4 + c) as usize]);
                        }
                    }
                    out[((y * w + x) * 4 + c) as usize] = median(&mut window);
                }
                // Alpha is copied through unchanged.
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_solid_is_unchanged() {
        let data: Vec<u8> = std::iter::repeat_n([50, 60, 70, 255], 25)
            .flatten()
            .collect();
        let src = EffectImage::from_rgba8(5, 5, data).unwrap();
        assert_eq!(ReduceNoise::new(1).apply(&src), src);
    }

    #[test]
    fn median_removes_a_single_pixel_spike() {
        // A flat grey field with one white outlier in the centre.
        let mut data: Vec<u8> = std::iter::repeat_n([100, 100, 100, 255], 25)
            .flatten()
            .collect();
        let center = (2 * 5 + 2) * 4;
        data[center..center + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(5, 5, data).unwrap();
        let out = ReduceNoise::new(1).apply(&src);
        // The spike is gone — the centre is back to the surrounding grey.
        assert_eq!(&out.data()[center..center + 3], &[100, 100, 100]);
    }

    #[test]
    fn scaled_rounds_radius_min_one() {
        assert_eq!(ReduceNoise::new(8).scaled(0.5).radius, 4);
        assert_eq!(ReduceNoise::new(1).scaled(0.1).radius, 1);
    }
}
