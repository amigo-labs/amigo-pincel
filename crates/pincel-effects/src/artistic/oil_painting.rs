//! Oil Painting (docs/specs/effects-artistic-photo.md): each pixel takes the
//! average colour of the most frequent luma bin in its neighbourhood.

use crate::adjust::luma_x10000;
use crate::blur::whole;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Flat, brush-like patches: over the `(2 · brush_size + 1)²` square around
/// each pixel (clipped to the layer), opaque pixels are sorted into
/// `coarseness` luma bins; the pixel takes the alpha-weighted average colour
/// of the fullest bin (ties: the darker bin). Its alpha is kept, and a pixel
/// with no opaque neighbour keeps its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OilPainting {
    /// Neighbourhood radius in pixels, `1..=8`.
    pub brush_size: u32,
    /// Number of luma bins, `3..=255`.
    pub coarseness: u32,
}

impl OilPainting {
    /// Creates an oil painting (non-finite numbers read as 0, then rounded
    /// and clamped).
    pub fn new(brush_size: f32, coarseness: f32) -> Self {
        Self {
            brush_size: whole(brush_size, 1, 8),
            coarseness: whole(coarseness, 3, 255),
        }
    }
}

/// Per-bin sums: pixel count, alpha, and alpha-weighted R, G, B.
#[derive(Clone, Copy, Default)]
struct Bin {
    count: u32,
    alpha: u64,
    rgb: [u64; 3],
}

impl Effect for OilPainting {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let bins = self.coarseness as usize;
        // The bin of every pixel: luma 0..=255 · 10 000 spread over `bins`.
        let bin_of: Vec<usize> = data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let l = u64::from(luma_x10000(p[0], p[1], p[2]));
                ((l * bins as u64) / (256 * 10_000)) as usize
            })
            .collect();
        let r = self.brush_size as usize;
        let mut hist = vec![Bin::default(); bins];
        let mut out = data.to_vec();
        for y in 0..h {
            for x in 0..w {
                hist.fill(Bin::default());
                for ny in y.saturating_sub(r)..(y + r + 1).min(h) {
                    for nx in x.saturating_sub(r)..(x + r + 1).min(w) {
                        let i = ny * w + nx;
                        let a = u64::from(data[i * 4 + 3]);
                        if a == 0 {
                            continue;
                        }
                        let bin = &mut hist[bin_of[i]];
                        bin.count += 1;
                        bin.alpha += a;
                        for (sum, &c) in bin.rgb.iter_mut().zip(&data[i * 4..i * 4 + 3]) {
                            *sum += u64::from(c) * a;
                        }
                    }
                }
                // The first fullest bin: `max_by_key` would take the last.
                let mut best = &hist[0];
                for bin in &hist[1..] {
                    if bin.count > best.count {
                        best = bin;
                    }
                }
                if best.count == 0 {
                    continue;
                }
                let o = (y * w + x) * 4;
                for (c, sum) in out[o..o + 3].iter_mut().zip(best.rgb) {
                    *c = ((sum + best.alpha / 2) / best.alpha) as u8;
                }
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            brush_size: ((self.brush_size as f32 * factor).round() as u32).max(1),
            ..*self
        }
    }
}
