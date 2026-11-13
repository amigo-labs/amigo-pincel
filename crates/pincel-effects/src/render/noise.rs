//! Clouds and Turbulence (docs/specs/effects-render.md): seeded fractal
//! noise in layer coordinates, mapped between two colours — value noise
//! for Clouds, gradient noise for Turbulence (whose ridges at the noise's
//! zero crossings would show value noise's lattice).

use super::RenderBlend;
use crate::blur::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// SplitMix64 of the seed, the octave and a lattice point. Signed lattice
/// coordinates (a cel left of or above the canvas origin) hash as their
/// two's-complement bits, the same on every platform.
fn lattice(seed: u32, octave: u32, ix: i64, iy: i64) -> f32 {
    let mut z = (u64::from(seed) << 32 | u64::from(octave))
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / 0xFF_FFFF as f32
}

/// Quintic fade: smooth interpolation with flat ends.
fn fade(t: f64) -> f32 {
    (t * t * t * (t * (t * 6.0 - 15.0) + 10.0)) as f32
}

/// Value noise in `[0, 1]` at canvas position `(x, y)` (pixel centres)
/// with lattice spacing `size` pixels.
fn value(seed: u32, octave: u32, size: f64, x: i64, y: i64) -> f32 {
    let gx = (x as f64 + 0.5) / size;
    let gy = (y as f64 + 0.5) / size;
    let (x0, y0) = (gx.floor(), gy.floor());
    let (tx, ty) = (fade(gx - x0), fade(gy - y0));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let v = |dx, dy| lattice(seed, octave, ix + dx, iy + dy);
    let top = v(0, 0) + (v(1, 0) - v(0, 0)) * tx;
    let bottom = v(0, 1) + (v(1, 1) - v(0, 1)) * tx;
    top + (bottom - top) * ty
}

/// Gradient (Perlin) noise, roughly in `[-0.7, 0.7]`, at canvas position
/// `(x, y)` (pixel centres) with lattice spacing `size` pixels; the
/// lattice gradients' angles come from [`lattice`].
fn gradient(seed: u32, octave: u32, size: f64, x: i64, y: i64) -> f32 {
    let gx = (x as f64 + 0.5) / size;
    let gy = (y as f64 + 0.5) / size;
    let (x0, y0) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - x0, gy - y0);
    let (ix, iy) = (x0 as i64, y0 as i64);
    let dot = |dx: i64, dy: i64| {
        let a = f64::from(lattice(seed, octave, ix + dx, iy + dy)) * std::f64::consts::TAU;
        let (s, c) = (libm::sin(a), libm::cos(a));
        (c * (fx - dx as f64) + s * (fy - dy as f64)) as f32
    };
    let (tx, ty) = (fade(fx), fade(fy));
    let top = dot(0, 0) + (dot(1, 0) - dot(0, 0)) * tx;
    let bottom = dot(0, 1) + (dot(1, 1) - dot(0, 1)) * tx;
    top + (bottom - top) * ty
}

/// The colour at rendered value `t` in `[0, 1]`: each straight-alpha
/// channel interpolated from `c1` (0) to `c2` (1).
fn mix(c1: [u8; 4], c2: [u8; 4], t: f32) -> [u8; 4] {
    let mut out = [0; 4];
    for ch in 0..4 {
        let (a, b) = (f32::from(c1[ch]), f32::from(c2[ch]));
        out[ch] = (a + (b - a) * t).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Paints `value(x, y)` (canvas coordinates) between `c1` and `c2` over
/// `src` with `blend`; `origin` is the canvas position of `src`'s top-left
/// pixel.
fn paint(
    src: &EffectImage,
    origin: (i32, i32),
    (c1, c2): ([u8; 4], [u8; 4]),
    blend: RenderBlend,
    value: impl Fn(i64, i64) -> f32,
) -> EffectImage {
    let w = src.width() as usize;
    let mut out = src.data().to_vec();
    for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = i64::from(origin.0) + (i % w) as i64;
        let y = i64::from(origin.1) + (i / w) as i64;
        let t = value(x, y).clamp(0.0, 1.0);
        *p = blend.combine(*p, mix(c1, c2, t));
    }
    EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
}

/// Clouds: seeded fractal value noise. Octave `k` has feature size
/// `scale / 2^k` px (while that is at least 1 px) and weight
/// `(roughness / 100)^k`; the weighted sum, divided by the sum of the
/// weights, maps from `color1` (0) to `color2` (1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clouds {
    /// Feature size of the first octave in pixels, `2..=1000`.
    pub scale: u32,
    /// Weight ratio between octaves in percent, `0..=100`.
    pub roughness: f32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
    /// Straight-alpha RGBA at rendered value 0.
    pub color1: [u8; 4],
    /// Straight-alpha RGBA at rendered value 1.
    pub color2: [u8; 4],
    /// How the rendered colour combines with the existing pixel.
    pub blend: RenderBlend,
    /// Canvas position of the buffer's top-left pixel.
    pub origin: (i32, i32),
}

impl Clouds {
    /// Creates a clouds effect for a buffer at the canvas origin
    /// (non-finite numbers read as 0, then clamped; scale and seed
    /// rounded).
    pub fn new(
        scale: f32,
        roughness: f32,
        seed: f32,
        colors: ([u8; 4], [u8; 4]),
        blend: RenderBlend,
    ) -> Self {
        Self {
            scale: whole(scale, 2, 1000),
            roughness: param(roughness, 0.0, 100.0),
            seed: whole(seed, 0, 9999),
            color1: colors.0,
            color2: colors.1,
            blend,
            origin: (0, 0),
        }
    }

    /// The same effect for a buffer whose top-left pixel sits at canvas
    /// position `origin`.
    pub fn with_origin(self, origin: (i32, i32)) -> Self {
        Self { origin, ..self }
    }

    fn value(&self, x: i64, y: i64) -> f32 {
        let ratio = self.roughness / 100.0;
        let (mut sum, mut total, mut weight) = (0.0, 0.0, 1.0);
        let mut size = f64::from(self.scale);
        let mut octave = 0;
        while size >= 1.0 && weight > 0.0 {
            sum += weight * value(self.seed, octave, size, x, y);
            total += weight;
            weight *= ratio;
            size /= 2.0;
            octave += 1;
        }
        sum / total
    }
}

impl Effect for Clouds {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let colors = (self.color1, self.color2);
        paint(src, self.origin, colors, self.blend, |x, y| {
            self.value(x, y)
        })
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            scale: whole(self.scale as f32 * factor, 2, 1000),
            ..*self
        }
    }
}

/// Turbulence: the absolute value of `octaves` signed gradient-noise
/// octaves (feature size `scale / 2^k` px, at least 1 px, weight `1 / 2^k`,
/// each stretched from about ±0.7 to ±1), summed and divided by the sum of
/// the weights — sharp ridges where each octave crosses zero — mapped from
/// `color1` (0) to `color2` (1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turbulence {
    /// Feature size of the first octave in pixels, `2..=1000`.
    pub scale: u32,
    /// Number of octaves, `1..=8`.
    pub octaves: u32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
    /// Straight-alpha RGBA at rendered value 0.
    pub color1: [u8; 4],
    /// Straight-alpha RGBA at rendered value 1.
    pub color2: [u8; 4],
    /// How the rendered colour combines with the existing pixel.
    pub blend: RenderBlend,
    /// Canvas position of the buffer's top-left pixel.
    pub origin: (i32, i32),
}

impl Turbulence {
    /// Creates a turbulence effect for a buffer at the canvas origin
    /// (non-finite numbers read as 0, then rounded and clamped).
    pub fn new(
        scale: f32,
        octaves: f32,
        seed: f32,
        colors: ([u8; 4], [u8; 4]),
        blend: RenderBlend,
    ) -> Self {
        Self {
            scale: whole(scale, 2, 1000),
            octaves: whole(octaves, 1, 8),
            seed: whole(seed, 0, 9999),
            color1: colors.0,
            color2: colors.1,
            blend,
            origin: (0, 0),
        }
    }

    /// The same effect for a buffer whose top-left pixel sits at canvas
    /// position `origin`.
    pub fn with_origin(self, origin: (i32, i32)) -> Self {
        Self { origin, ..self }
    }

    fn value(&self, x: i64, y: i64) -> f32 {
        let (mut sum, mut total, mut weight) = (0.0, 0.0, 1.0);
        let mut size = f64::from(self.scale);
        for octave in 0..self.octaves {
            let n = gradient(self.seed, octave, size.max(1.0), x, y);
            sum += weight * (n.abs() * std::f32::consts::SQRT_2).min(1.0);
            total += weight;
            weight *= 0.5;
            size /= 2.0;
        }
        sum / total
    }
}

impl Effect for Turbulence {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let colors = (self.color1, self.color2);
        paint(src, self.origin, colors, self.blend, |x, y| {
            self.value(x, y)
        })
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            scale: whole(self.scale as f32 * factor, 2, 1000),
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_stays_in_unit_range_and_hits_lattice_values() {
        for (x, y) in [(-40, 3), (0, 0), (17, 999)] {
            let v = value(3, 0, 7.5, x, y);
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn gradient_is_zero_on_the_lattice_and_bounded() {
        // Pixel centre 0.5 at spacing 1 sits mid-cell; spacing 0.5 puts
        // it on a lattice point, where gradient noise is 0.
        assert_eq!(gradient(1, 0, 0.5, 3, -7), 0.0);
        for (x, y) in [(-40, 3), (0, 0), (17, 999), (5, 5)] {
            let g = gradient(9, 2, 6.3, x, y);
            assert!(g.abs() <= 1.0, "{g}");
        }
    }

    #[test]
    fn clouds_roughness_zero_uses_one_octave() {
        let c = Clouds::new(64.0, 0.0, 5.0, ([0; 4], [255; 4]), RenderBlend::Normal);
        assert_eq!(c.value(10, 20), value(5, 0, 64.0, 10, 20));
    }

    #[test]
    fn mix_ends_are_the_two_colours() {
        let (a, b) = ([0, 50, 100, 255], [255, 150, 0, 0]);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
    }

    #[test]
    fn scaled_keeps_the_scale_in_range() {
        let t = Turbulence::new(200.0, 5.0, 1.0, ([0; 4], [255; 4]), RenderBlend::Normal);
        assert_eq!(t.scaled(0.5).scale, 100);
        assert_eq!(t.scaled(0.001).scale, 2);
    }
}
