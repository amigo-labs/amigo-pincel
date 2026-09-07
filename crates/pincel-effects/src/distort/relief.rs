//! Relief (Fineliner spec §11.3 group): a colour-preserving emboss.
//!
//! Unlike [`Emboss`](super::Emboss), which greys the image out, Relief adds the
//! directional luminance derivative back onto the original colour, so edges gain
//! a raised highlight/shadow while the underlying hue survives.

use super::gradient;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Relief: a colour-preserving directional emboss.
///
/// - `angle` is the light direction in degrees (0–360).
/// - `amount` (1–10) scales how strongly the relief highlight/shadow is added.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relief {
    /// Light direction in degrees.
    pub angle: f32,
    /// Relief strength, 1–10.
    pub amount: f32,
}

impl Relief {
    /// Creates a relief effect.
    pub fn new(angle: f32, amount: f32) -> Self {
        Self { angle, amount }
    }
}

impl Effect for Relief {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let az = self.angle.to_radians();
        let (dx, dy) = (az.cos(), az.sin());
        let mut out = data.to_vec();
        for y in 0..h {
            for x in 0..w {
                let (gx, gy) = gradient(data, w, h, x as i32, y as i32, 2.0);
                // Directional derivative of luminance along the light vector.
                let d = (gx * dx + gy * dy) * self.amount;
                let o = (y * w + x) * 4;
                for c in 0..3 {
                    let v = data[o + c] as f32 / 255.0 + d;
                    out[o + c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
                // Alpha (out[o + 3]) is already the copied source value.
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // 3×3 neighbourhood; independent of image scale.
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(cols: &[[u8; 4]], height: u32) -> EffectImage {
        let w = cols.len() as u32;
        let mut data = Vec::new();
        for _ in 0..height {
            for &c in cols {
                data.extend_from_slice(&c);
            }
        }
        EffectImage::from_rgba8(w, height, data).unwrap()
    }

    #[test]
    fn relief_of_solid_color_is_unchanged() {
        let src = columns(&[[40, 120, 200, 255]; 4], 4);
        assert_eq!(Relief::new(0.0, 5.0).apply(&src), src);
    }

    #[test]
    fn relief_preserves_hue_direction_at_edges() {
        // A coloured step: the relief adds highlight/shadow but keeps colour, so
        // the result differs from the source yet alpha is untouched.
        let src = columns(
            &[
                [200, 40, 40, 255],
                [200, 40, 40, 255],
                [40, 40, 200, 255],
                [40, 40, 200, 255],
            ],
            3,
        );
        let out = Relief::new(0.0, 8.0).apply(&src);
        assert_ne!(out, src);
        for px in out.data().as_chunks::<4>().0 {
            assert_eq!(px[3], 255);
        }
    }
}
