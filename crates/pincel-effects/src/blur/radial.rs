//! Radial blur (Fineliner spec §11.1): spin or zoom around a centre point.

use super::sample_bilinear;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Radial blur variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialKind {
    /// Rotate samples about the centre (circular streaks).
    Spin,
    /// Scale samples toward/away from the centre (zoom streaks).
    Zoom,
}

/// Peak spin sweep in radians at `amount = 100` (≈ 20°).
const SPIN_MAX_RAD: f32 = 0.35;
/// Peak zoom sweep (fractional scale span) at `amount = 100` (40%).
const ZOOM_MAX: f32 = 0.4;

/// Radial blur about `(center_x, center_y)`.
///
/// `amount` (1–100, Fineliner spec §11.1) drives both the sweep magnitude and the sample
/// count. The centre pixel is always unchanged. The centre defaults to the
/// canvas centre in the UI but is explicit here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialBlur {
    /// Blur strength, 1–100.
    pub amount: f32,
    /// Centre x in pixels.
    pub center_x: f32,
    /// Centre y in pixels.
    pub center_y: f32,
    /// Spin or zoom.
    pub kind: RadialKind,
}

impl RadialBlur {
    /// Creates a radial blur of the given amount, centre and kind.
    pub fn new(amount: f32, center_x: f32, center_y: f32, kind: RadialKind) -> Self {
        Self {
            amount,
            center_x,
            center_y,
            kind,
        }
    }
}

impl Effect for RadialBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.amount <= 0.0 {
            return src.clone();
        }
        let (w, h) = (src.width() as usize, src.height() as usize);
        let n = (self.amount.round() as i32).clamp(2, 64);
        let strength = (self.amount / 100.0).clamp(0.0, 1.0);
        let buf = src.to_premultiplied_f32();
        let mut out = vec![0.0f32; buf.len()];
        let inv_n = 1.0 / n as f32;
        for y in 0..h {
            for x in 0..w {
                let vx = x as f32 - self.center_x;
                let vy = y as f32 - self.center_y;
                let mut acc = [0.0f32; 4];
                for i in 0..n {
                    // Centred sweep in [-0.5, 0.5].
                    let frac = i as f32 / (n - 1) as f32 - 0.5;
                    let (sx, sy) = match self.kind {
                        RadialKind::Spin => {
                            let theta = frac * strength * SPIN_MAX_RAD;
                            let (s, c) = theta.sin_cos();
                            (
                                self.center_x + vx * c - vy * s,
                                self.center_y + vx * s + vy * c,
                            )
                        }
                        RadialKind::Zoom => {
                            let scale = 1.0 + frac * strength * ZOOM_MAX;
                            (self.center_x + vx * scale, self.center_y + vy * scale)
                        }
                    };
                    let s = sample_bilinear(&buf, w, h, sx, sy);
                    for c in 0..4 {
                        acc[c] += s[c];
                    }
                }
                let o = (y * w + x) * 4;
                for c in 0..4 {
                    out[o + c] = acc[c] * inv_n;
                }
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        // amount is scale-independent; only the centre moves with the image.
        Self {
            amount: self.amount,
            center_x: self.center_x * factor,
            center_y: self.center_y * factor,
            kind: self.kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> EffectImage {
        let data: Vec<u8> = std::iter::repeat_n(c, (w * h) as usize).flatten().collect();
        EffectImage::from_rgba8(w, h, data).unwrap()
    }

    #[test]
    fn radial_spin_of_solid_color_is_unchanged() {
        let src = solid(9, 9, [40, 120, 200, 255]);
        let out = RadialBlur::new(100.0, 4.0, 4.0, RadialKind::Spin).apply(&src);
        assert_eq!(out, src);
    }

    #[test]
    fn radial_zoom_of_solid_color_is_unchanged() {
        let src = solid(9, 9, [200, 60, 90, 255]);
        let out = RadialBlur::new(100.0, 4.0, 4.0, RadialKind::Zoom).apply(&src);
        assert_eq!(out, src);
    }

    #[test]
    fn radial_leaves_the_center_pixel_untouched() {
        // A two-tone field: only the exact centre must survive verbatim.
        let mut data = Vec::new();
        for y in 0..9u32 {
            for _ in 0..9u32 {
                let v = if y < 4 { 0 } else { 255 };
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let src = EffectImage::from_rgba8(9, 9, data).unwrap();
        let out = RadialBlur::new(80.0, 4.0, 4.0, RadialKind::Spin).apply(&src);
        let center = (4 * 9 + 4) * 4;
        assert_eq!(
            &out.data()[center..center + 4],
            &src.data()[center..center + 4]
        );
    }

    #[test]
    fn radial_spin_blurs_a_two_tone_field() {
        // A vertical seam: spinning must mix black and white somewhere.
        let mut data = Vec::new();
        for _ in 0..9u32 {
            for x in 0..9u32 {
                let v = if x < 4 { 0 } else { 255 };
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let src = EffectImage::from_rgba8(9, 9, data).unwrap();
        let out = RadialBlur::new(100.0, 4.0, 4.0, RadialKind::Spin).apply(&src);
        assert_ne!(out, src, "spin should mix samples across the seam");
    }
}
