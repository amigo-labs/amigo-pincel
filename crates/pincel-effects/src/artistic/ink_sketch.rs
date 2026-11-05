//! Ink Sketch (docs/specs/effects-artistic-photo.md): dark ink lines along
//! the luma edges over a desaturated copy of the image.

use super::luma;
use crate::blur::param;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Edge magnitude (normalised Sobel, roughly `0..=1.4`) times Ink outline
/// that turns a pixel fully black.
const INK_GAIN: f32 = 1.0 / 25.0;

/// Ink lines from a Sobel edge-detect of the premultiplied luma (so a
/// silhouette against transparency is inked too), darkening the image whose
/// saturation is scaled to `coloring` %. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkSketch {
    /// Line strength, `0..=99`; 0 draws no lines.
    pub ink_outline: f32,
    /// Colour saturation kept, `0..=100` %; 0 gives a gray image.
    pub coloring: f32,
}

impl InkSketch {
    /// Creates an ink sketch (non-finite numbers read as 0, then clamped).
    pub fn new(ink_outline: f32, coloring: f32) -> Self {
        Self {
            ink_outline: param(ink_outline, 0.0, 99.0),
            coloring: param(coloring, 0.0, 100.0),
        }
    }
}

impl Effect for InkSketch {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        // Premultiplied luma 0..=1: transparent pixels read as black.
        let height: Vec<f32> = data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| luma(p) / 255.0 * f32::from(p[3]) / 255.0)
            .collect();
        let at = |x: isize, y: isize| {
            let cx = x.clamp(0, w as isize - 1) as usize;
            let cy = y.clamp(0, h as isize - 1) as usize;
            height[cy * w + cx]
        };
        let keep = self.coloring / 100.0;
        let gain = self.ink_outline * INK_GAIN;
        let mut out = data.to_vec();
        for y in 0..h {
            for x in 0..w {
                let (xi, yi) = (x as isize, y as isize);
                let gx = (at(xi + 1, yi - 1) + 2.0 * at(xi + 1, yi) + at(xi + 1, yi + 1)
                    - at(xi - 1, yi - 1)
                    - 2.0 * at(xi - 1, yi)
                    - at(xi - 1, yi + 1))
                    / 4.0;
                let gy = (at(xi - 1, yi + 1) + 2.0 * at(xi, yi + 1) + at(xi + 1, yi + 1)
                    - at(xi - 1, yi - 1)
                    - 2.0 * at(xi, yi - 1)
                    - at(xi + 1, yi - 1))
                    / 4.0;
                let ink = ((gx * gx + gy * gy).sqrt() * gain).min(1.0);
                let o = (y * w + x) * 4;
                let px = &mut out[o..o + 4];
                let l = luma(px);
                for c in &mut px[..3] {
                    let tinted = l + (f32::from(*c) - l) * keep;
                    *c = (tinted * (1.0 - ink)).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // A 3 x 3 neighbourhood; independent of image scale.
        *self
    }
}
