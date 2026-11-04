//! Sketch Blur (docs/specs/effects-blurs.md): short smears in random,
//! seeded directions, like hand-drawn hatching.

use super::{sample_bilinear, whole};
use crate::effect::Effect;
use crate::image::EffectImage;
use std::f32::consts::TAU;

/// Smears each pixel along a line centred on it, of a direction and a length
/// up to `radius` drawn per pixel position from `seed`: the same seed and
/// parameters give the same pixels on every run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SketchBlur {
    /// Longest smear in pixels, `1..=100`.
    pub radius: u32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
}

impl SketchBlur {
    /// Creates a sketch blur (non-finite numbers read as 0, then clamped and
    /// rounded: radius to `1..=100`, seed to `0..=9999`).
    pub fn new(radius: f32, seed: f32) -> Self {
        Self {
            radius: whole(radius, 1, 100),
            seed: whole(seed, 0, 9999),
        }
    }
}

/// SplitMix64 of the seed and a pixel position: a stateless per-pixel
/// random number, so the result does not depend on visiting order.
fn hash(seed: u32, x: usize, y: usize) -> u64 {
    let mut z = (u64::from(seed) << 40) ^ ((y as u64) << 20) ^ x as u64;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Effect for SketchBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        if w == 0 || h == 0 {
            return src.clone();
        }
        let buf = src.to_premultiplied_f32();
        let mut out = vec![0.0f32; buf.len()];
        let unit = |bits: u64| (bits & 0xFF_FFFF) as f32 / 0xFF_FFFF as f32;
        for y in 0..h {
            for x in 0..w {
                let bits = hash(self.seed, x, y);
                let theta = unit(bits) * TAU;
                let len = unit(bits >> 24) * self.radius as f32;
                let (dy, dx) = theta.sin_cos();
                let n = len.ceil() as usize + 1;
                let mut acc = [0.0f32; 4];
                for i in 0..n {
                    let t = if n > 1 {
                        len * (i as f32 / (n - 1) as f32 - 0.5)
                    } else {
                        0.0
                    };
                    let s = sample_bilinear(&buf, w, h, x as f32 + dx * t, y as f32 + dy * t);
                    for c in 0..4 {
                        acc[c] += s[c];
                    }
                }
                let o = (y * w + x) * 4;
                for c in 0..4 {
                    out[o + c] = acc[c] / n as f32;
                }
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_differs_per_seed_and_position() {
        assert_ne!(hash(1, 0, 0), hash(2, 0, 0));
        assert_ne!(hash(1, 1, 0), hash(1, 0, 1));
        assert_eq!(hash(7, 3, 4), hash(7, 3, 4));
    }
}
