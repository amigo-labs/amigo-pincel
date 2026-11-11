//! Fragment (docs/specs/effects-blurs.md): the average of shifted copies.

use super::{angle, param, sample_bilinear, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Averages `fragments` copies of the layer, each shifted by `distance` px
/// in directions spaced `360° / fragments` apart, the first at `rotation`
/// (0° = +x, 90° = +y, as Motion Blur). Distance 0 is the identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fragment {
    /// Number of copies, `2..=50`.
    pub fragments: u32,
    /// Shift of each copy in pixels, `0..=100`.
    pub distance: f32,
    /// Direction of the first copy in degrees, `0..360`.
    pub rotation: f32,
}

impl Fragment {
    /// Creates a fragment blur (non-finite numbers read as 0, then clamped:
    /// fragments to `2..=50` rounded, distance to `0..=100`; the rotation
    /// wraps into `0..360`).
    pub fn new(fragments: f32, distance: f32, rotation: f32) -> Self {
        Self {
            fragments: whole(fragments, 2, 50),
            distance: param(distance, 0.0, 100.0),
            rotation: angle(rotation),
        }
    }
}

impl Effect for Fragment {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.distance <= 0.0 || src.width() == 0 || src.height() == 0 {
            return src.clone();
        }
        let (w, h) = (src.width() as usize, src.height() as usize);
        let n = self.fragments.max(1);
        let shifts: Vec<(f32, f32)> = (0..n)
            .map(|k| {
                let deg = self.rotation + k as f32 * 360.0 / n as f32;
                let (s, c) = crate::math::sin_cos(deg.to_radians());
                (c * self.distance, s * self.distance)
            })
            .collect();
        let buf = src.to_premultiplied_f32();
        let mut out = vec![0.0f32; buf.len()];
        let inv_n = 1.0 / n as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                // A copy shifted by s shows at p the pixel at p - s.
                for &(sx, sy) in &shifts {
                    let s = sample_bilinear(&buf, w, h, x as f32 - sx, y as f32 - sy);
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
        Self {
            distance: self.distance * factor,
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_two_copies_split_a_pixel_to_both_sides() {
        let mut data = vec![0u8; 9 * 4];
        data[4 * 4..4 * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(9, 1, data).expect("9x1");
        let out = Fragment::new(2.0, 2.0, 0.0).apply(&src);
        let alpha = |x: usize| out.data()[x * 4 + 3];
        assert_eq!((alpha(2), alpha(4), alpha(6)), (128, 0, 128));
    }
}
