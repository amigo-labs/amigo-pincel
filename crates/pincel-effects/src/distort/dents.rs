//! Dents (docs/specs/effects-distort.md): displaces each pixel by a seeded,
//! smooth noise field.

use std::f32::consts::TAU;

use super::edge::{EdgeBehavior, Source, remap};
use crate::blur::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;
use crate::random::{hash, unit};

/// Octaves of the noise field; Roughness weights the ones above the first.
const OCTAVES: i32 = 4;

/// Displaces each pixel by two seeded gradient-noise fields (one per
/// axis): features about `scale` px wide, offsets up to about
/// `refraction / 100 · scale` px, finer octaves weighted by `roughness`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dents {
    /// Feature size in pixels, `1..=200`.
    pub scale: f32,
    /// Displacement strength, `0..=200`; 0 leaves the layer unchanged.
    pub refraction: f32,
    /// Weight of the finer noise octaves, `0..=100`.
    pub roughness: f32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
    /// What positions outside the layer read.
    pub edge: EdgeBehavior,
}

impl Dents {
    /// Creates a dents effect (non-finite numbers read as 0, then clamped;
    /// the seed rounded).
    pub fn new(scale: f32, refraction: f32, roughness: f32, seed: f32, edge: EdgeBehavior) -> Self {
        Self {
            scale: param(scale, 1.0, 200.0),
            refraction: param(refraction, 0.0, 200.0),
            roughness: param(roughness, 0.0, 100.0),
            seed: whole(seed, 0, 9999),
            edge,
        }
    }

    /// Fractal noise of field `axis` at `(x, y)` (in feature units),
    /// roughly in `[-1, 1]`.
    fn noise(&self, axis: u32, x: f32, y: f32) -> f32 {
        let p = self.roughness / 100.0;
        let (mut sum, mut weight) = (0.0, 0.0);
        for octave in 0..OCTAVES {
            let amp = p.powi(octave);
            if amp == 0.0 {
                break;
            }
            let f = (1 << octave) as f32;
            let stream = self.seed + 10_000 * (axis + 2 * octave as u32);
            sum += amp * gradient_noise(stream, x * f, y * f);
            weight += amp;
        }
        // Gradient noise peaks near ±0.7; stretch it to about ±1.
        sum / weight * std::f32::consts::SQRT_2
    }
}

/// 2-D gradient (Perlin) noise at a non-negative position, with the
/// lattice gradients drawn from `stream`.
fn gradient_noise(stream: u32, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as usize, fy as usize);
    let (tx, ty) = (x - fx, y - fy);
    let dot = |gx: usize, gy: usize, dx: f32, dy: f32| {
        let (s, c) = crate::math::sin_cos(unit(hash(stream, gx, gy)) * TAU);
        c * dx + s * dy
    };
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v) = (fade(tx), fade(ty));
    let n00 = dot(ix, iy, tx, ty);
    let n10 = dot(ix + 1, iy, tx - 1.0, ty);
    let n01 = dot(ix, iy + 1, tx, ty - 1.0);
    let n11 = dot(ix + 1, iy + 1, tx - 1.0, ty - 1.0);
    let top = n00 + (n10 - n00) * u;
    let bot = n01 + (n11 - n01) * u;
    top + (bot - top) * v
}

impl Effect for Dents {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.refraction == 0.0 {
            return src.clone();
        }
        let strength = self.refraction / 100.0 * self.scale;
        remap(src, self.edge, |x, y| {
            let (nx, ny) = (x / self.scale, y / self.scale);
            Source::At(
                x + strength * self.noise(0, nx, ny),
                y + strength * self.noise(1, nx, ny),
            )
        })
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            scale: (self.scale * factor).max(1.0),
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_noise_is_zero_on_the_lattice_and_varies_between() {
        assert_eq!(gradient_noise(1, 3.0, 4.0), 0.0);
        assert_ne!(gradient_noise(1, 3.5, 4.25), gradient_noise(2, 3.5, 4.25));
    }

    #[test]
    fn noise_stays_roughly_in_unit_range() {
        let d = Dents::new(8.0, 100.0, 100.0, 7.0, EdgeBehavior::Clamp);
        for i in 0..2000 {
            let (x, y) = (i as f32 * 0.37, i as f32 * 0.21);
            let n = d.noise(0, x, y);
            assert!(n.abs() <= 1.5, "{n} at {x}, {y}");
        }
    }
}
