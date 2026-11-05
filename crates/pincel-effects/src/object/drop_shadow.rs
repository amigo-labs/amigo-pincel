//! Drop Shadow (docs/specs/effects-object-stylize-color-noise.md): a
//! shifted, blurred copy of the layer's shape painted behind its pixels.

use crate::blur::{gaussian_kernel, param};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Paints the shape (the alpha channel), filled with `color` at
/// `color` alpha × `opacity` %, shifted by the offset and blurred by
/// `blur` px, behind the existing pixels: an opaque pixel keeps its colour,
/// a partly transparent or transparent one shows the shadow beneath it.
///
/// Pixels beyond the layer's edge count as transparent for the shadow, and
/// the layer never grows: a shadow past the edge is clipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadow {
    /// Horizontal offset in pixels, `-100..=100`.
    pub offset_x: i32,
    /// Vertical offset in pixels, `-100..=100`.
    pub offset_y: i32,
    /// Gaussian blur radius in pixels, `0..=50` (0 keeps the edge hard).
    pub blur: f32,
    /// Straight-alpha RGBA of the shadow.
    pub color: [u8; 4],
    /// Opacity in percent, `0..=100`, multiplied with the colour's alpha.
    pub opacity: f32,
}

impl DropShadow {
    /// Largest offset either way.
    pub const MAX_OFFSET: f32 = 100.0;
    /// Largest blur radius.
    pub const MAX_BLUR: f32 = 50.0;

    /// Creates a drop shadow (non-finite numbers read as 0, then clamped:
    /// the offsets to `-100..=100` rounded, blur to `0..=50`, opacity to
    /// `0..=100`).
    pub fn new(offset_x: f32, offset_y: f32, blur: f32, color: [u8; 4], opacity: f32) -> Self {
        let offset = |v: f32| param(v, -Self::MAX_OFFSET, Self::MAX_OFFSET).round() as i32;
        Self {
            offset_x: offset(offset_x),
            offset_y: offset(offset_y),
            blur: param(blur, 0.0, Self::MAX_BLUR),
            color,
            opacity: param(opacity, 0.0, 100.0),
        }
    }

    /// The shape shifted by the offset, `0.0..=1.0` per pixel.
    fn shifted_mask(&self, src: &EffectImage) -> Vec<f32> {
        let (w, h) = (src.width() as i64, src.height() as i64);
        let alpha = src.data().as_chunks::<4>().0;
        let mut mask = vec![0.0f32; (w * h) as usize];
        for y in 0..h {
            let sy = y - i64::from(self.offset_y);
            if sy < 0 || sy >= h {
                continue;
            }
            for x in 0..w {
                let sx = x - i64::from(self.offset_x);
                if sx >= 0 && sx < w {
                    mask[(y * w + x) as usize] =
                        f32::from(alpha[(sy * w + sx) as usize][3]) / 255.0;
                }
            }
        }
        mask
    }
}

/// Separable Gaussian blur of a one-channel buffer, zero beyond the edge.
fn blur_mask(mask: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    let sigma = radius / 3.0;
    if sigma <= 0.0 {
        return mask.to_vec();
    }
    let kernel = gaussian_kernel(sigma);
    let c = (kernel.len() / 2) as isize;
    let pass = |src: &[f32], horizontal: bool| {
        let mut out = vec![0.0f32; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0.0;
                for (k, &weight) in kernel.iter().enumerate() {
                    let d = k as isize - c;
                    let (sx, sy) = if horizontal {
                        (x as isize + d, y as isize)
                    } else {
                        (x as isize, y as isize + d)
                    };
                    if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                        acc += src[sy as usize * w + sx as usize] * weight;
                    }
                }
                out[y * w + x] = acc;
            }
        }
        out
    };
    pass(&pass(mask, true), false)
}

impl Effect for DropShadow {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let strength = f32::from(self.color[3]) * self.opacity / 100.0;
        if strength <= 0.0 || w == 0 || h == 0 {
            return src.clone();
        }
        let mask = blur_mask(&self.shifted_mask(src), w, h, self.blur);
        let rgb = [self.color[0], self.color[1], self.color[2]].map(f32::from);
        let mut out = src.data().to_vec();
        for (p, &m) in out.as_chunks_mut::<4>().0.iter_mut().zip(&mask) {
            // Shadow alpha in 0..=255; nothing beneath an opaque pixel shows.
            let d = (m.clamp(0.0, 1.0) * strength).round();
            if p[3] == 255 || d <= 0.0 {
                continue;
            }
            let (sa, da) = (f32::from(p[3]) / 255.0, d / 255.0);
            // `p` over the shadow, straight alpha.
            let a = sa + da * (1.0 - sa);
            for c in 0..3 {
                let v = (f32::from(p[c]) * sa + rgb[c] * da * (1.0 - sa)) / a;
                p[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            p[3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            offset_x: (self.offset_x as f32 * factor).round() as i32,
            offset_y: (self.offset_y as f32 * factor).round() as i32,
            blur: self.blur * factor,
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clamps_and_rounds_the_offsets() {
        let s = DropShadow::new(250.4, -3.6, 80.0, [0; 4], -1.0);
        assert_eq!(
            (s.offset_x, s.offset_y, s.blur, s.opacity),
            (100, -4, 50.0, 0.0)
        );
    }

    #[test]
    fn scaled_shrinks_offset_and_blur() {
        let s = DropShadow::new(8.0, -4.0, 6.0, [0, 0, 0, 255], 50.0).scaled(0.5);
        assert_eq!((s.offset_x, s.offset_y, s.blur), (4, -2, 3.0));
    }
}
