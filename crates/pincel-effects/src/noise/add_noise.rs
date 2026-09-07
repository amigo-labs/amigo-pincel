//! Add Noise (Fineliner spec §11.4): amount, type, channels, seed.

use crate::effect::Effect;
use crate::image::EffectImage;
use std::f32::consts::TAU;

/// Noise distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    /// Flat distribution in `[-amount, amount]`.
    Uniform,
    /// Normal distribution with standard deviation `amount`.
    Gaussian,
}

/// Whether channels get independent noise or share one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseChannels {
    /// Independent noise per R/G/B channel (chromatic).
    Rgb,
    /// One shared value added to R, G and B (monochromatic).
    Monochromatic,
}

/// Add Noise: perturbs each pixel by a seeded pseudo-random amount. Alpha is
/// preserved. `amount` is 0–100%; `seed` makes the result reproducible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddNoise {
    /// Noise strength as a percentage (0–100).
    pub amount: u32,
    /// Distribution.
    pub noise_type: NoiseType,
    /// Chromatic or monochromatic.
    pub channels: NoiseChannels,
    /// Seed for reproducibility.
    pub seed: u64,
}

impl AddNoise {
    /// Creates an Add Noise effect.
    pub fn new(amount: u32, noise_type: NoiseType, channels: NoiseChannels, seed: u64) -> Self {
        Self {
            amount,
            noise_type,
            channels,
            seed,
        }
    }
}

/// A tiny deterministic PRNG (xorshift64*) — avoids a `rand` dependency while
/// keeping seeded noise reproducible.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        // Mix the seed and force a nonzero state (xorshift requires it).
        Self {
            state: (seed ^ 0x9E37_79B9_7F4A_7C15) | 1,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform `f32` in `[0, 1)`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Standard-normal `f32` via Box–Muller.
    fn next_gaussian(&mut self) -> f32 {
        let u1 = self.next_f32().max(1e-7);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

impl Effect for AddNoise {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.amount == 0 {
            return src.clone();
        }
        let strength = self.amount as f32 / 100.0;
        let mut rng = Rng::new(self.seed);
        let mut out = src.data().to_vec();
        // One noise sample scaled to [0,1] channel units.
        let sample = |rng: &mut Rng| match self.noise_type {
            NoiseType::Uniform => (rng.next_f32() * 2.0 - 1.0) * strength,
            NoiseType::Gaussian => rng.next_gaussian() * strength,
        };
        for px in out.as_chunks_mut::<4>().0 {
            match self.channels {
                NoiseChannels::Monochromatic => {
                    let d = sample(&mut rng);
                    for c in px.iter_mut().take(3) {
                        *c = perturb(*c, d);
                    }
                }
                NoiseChannels::Rgb => {
                    for c in px.iter_mut().take(3) {
                        let d = sample(&mut rng);
                        *c = perturb(*c, d);
                    }
                }
            }
            // px[3] (alpha) is left untouched.
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // Per-pixel noise is scale-independent.
        *self
    }
}

/// Adds `delta` (in `[0,1]` units) to an RGBA8 channel, clamping.
fn perturb(value: u8, delta: f32) -> u8 {
    ((value as f32 / 255.0 + delta).clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> EffectImage {
        let data: Vec<u8> = std::iter::repeat_n(c, (w * h) as usize).flatten().collect();
        EffectImage::from_rgba8(w, h, data).unwrap()
    }

    #[test]
    fn add_noise_amount_zero_is_identity() {
        let src = solid(4, 4, [100, 120, 140, 255]);
        let fx = AddNoise::new(0, NoiseType::Gaussian, NoiseChannels::Rgb, 42);
        assert_eq!(fx.apply(&src), src);
    }

    #[test]
    fn add_noise_is_reproducible_for_a_fixed_seed() {
        let src = solid(8, 8, [128, 128, 128, 255]);
        let fx = AddNoise::new(40, NoiseType::Gaussian, NoiseChannels::Rgb, 12345);
        assert_eq!(fx.apply(&src), fx.apply(&src));
    }

    #[test]
    fn different_seeds_differ() {
        let src = solid(8, 8, [128, 128, 128, 255]);
        let a = AddNoise::new(40, NoiseType::Uniform, NoiseChannels::Rgb, 1).apply(&src);
        let b = AddNoise::new(40, NoiseType::Uniform, NoiseChannels::Rgb, 2).apply(&src);
        assert_ne!(a, b);
    }

    #[test]
    fn add_noise_changes_pixels_and_preserves_alpha() {
        let src = solid(8, 8, [128, 128, 128, 200]);
        let out = AddNoise::new(50, NoiseType::Uniform, NoiseChannels::Rgb, 7).apply(&src);
        assert_ne!(out, src);
        assert!(out.data().as_chunks::<4>().0.iter().all(|p| p[3] == 200));
    }

    #[test]
    fn monochromatic_keeps_channels_equal_on_gray() {
        // Starting from a neutral grey, mono noise moves R=G=B together.
        let src = solid(8, 8, [128, 128, 128, 255]);
        let out =
            AddNoise::new(30, NoiseType::Uniform, NoiseChannels::Monochromatic, 99).apply(&src);
        for px in out.data().as_chunks::<4>().0 {
            assert_eq!(px[0], px[1]);
            assert_eq!(px[1], px[2]);
        }
    }
}
