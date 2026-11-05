//! Frosted Glass (docs/specs/effects-distort.md): every pixel becomes a
//! seeded random pixel from its neighbourhood, as if seen through frosted
//! glass.

use crate::blur::whole;
use crate::effect::Effect;
use crate::image::EffectImage;
use crate::random::{hash, unit};

/// Replaces each pixel by the pixel at a seeded random offset within
/// `amount` px (Euclidean), positions outside the layer clamped to its
/// edge. Copies whole pixels: no colour mixing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrostedGlass {
    /// Largest offset in pixels, `1..=200`.
    pub amount: u32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
}

impl FrostedGlass {
    /// Creates a frosted glass effect (non-finite numbers read as 0, then
    /// rounded and clamped).
    pub fn new(amount: f32, seed: f32) -> Self {
        Self {
            amount: whole(amount, 1, 200),
            seed: whole(seed, 0, 9999),
        }
    }

    /// The offset of pixel `(x, y)`: `dx` uniform in `-a..=a`, then `dy`
    /// uniform within the disc's chord at `dx`.
    fn offset(&self, x: usize, y: usize) -> (i64, i64) {
        let a = i64::from(self.amount);
        let bits = hash(self.seed, x, y);
        let pick = |t: f32, m: i64| ((t * (2 * m + 1) as f32).floor() as i64).min(2 * m) - m;
        let dx = pick(unit(bits), a);
        let mut m = ((a * a - dx * dx) as f64).sqrt() as i64;
        while m * m > a * a - dx * dx {
            m -= 1;
        }
        (dx, pick(unit(bits >> 24), m))
    }
}

impl Effect for FrostedGlass {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let mut out = vec![0u8; data.len()];
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = self.offset(x, y);
                let sx = (x as i64 + dx).clamp(0, w as i64 - 1) as usize;
                let sy = (y as i64 + dy).clamp(0, h as i64 - 1) as usize;
                let (o, s) = ((y * w + x) * 4, (sy * w + sx) * 4);
                out[o..o + 4].copy_from_slice(&data[s..s + 4]);
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out)
            .expect("same size as the source buffer")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            amount: ((self.amount as f32 * factor).round() as u32).max(1),
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_stays_within_the_disc_and_covers_it() {
        let g = FrostedGlass::new(3.0, 5.0);
        let mut seen = std::collections::HashSet::new();
        for y in 0..60 {
            for x in 0..60 {
                let (dx, dy) = g.offset(x, y);
                assert!(dx * dx + dy * dy <= 9, "({dx}, {dy})");
                seen.insert((dx, dy));
            }
        }
        // Every one of the 29 offsets within radius 3 occurs.
        assert_eq!(seen.len(), 29);
    }
}
