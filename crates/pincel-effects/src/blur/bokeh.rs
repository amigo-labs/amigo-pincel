//! Bokeh (docs/specs/effects-blurs.md): a camera-lens blur over a filled
//! disc, with bright pixels weighted up so highlights spread into discs.

use super::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Weight gain of a fully bright pixel at Highlights 100: its weight is
/// `1 + HIGHLIGHT_GAIN · (h / 100) · l⁴` for premultiplied luma `l` in 0..1.
const HIGHLIGHT_GAIN: f64 = 100.0;

/// Averages each pixel over the filled disc of `radius` (premultiplied,
/// clamped edge pixels); `highlights` > 0 weights bright pixels more.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bokeh {
    /// Disc radius in pixels, `1..=100`.
    pub radius: u32,
    /// Highlight weighting, `0..=100`.
    pub highlights: f32,
}

impl Bokeh {
    /// Creates a bokeh blur (non-finite numbers read as 0, then clamped:
    /// radius to `1..=100` px rounded, highlights to `0..=100`).
    pub fn new(radius: f32, highlights: f32) -> Self {
        Self {
            radius: whole(radius, 1, 100),
            highlights: param(highlights, 0.0, 100.0),
        }
    }
}

/// Half-widths of the disc's rows: for every `dy` in `-r..=r` the largest
/// `dx` with `dx² + dy² <= r²`.
fn disc_rows(r: i64) -> Vec<i64> {
    (-r..=r)
        .map(|dy| {
            let mut hx = 0;
            while (hx + 1) * (hx + 1) + dy * dy <= r * r {
                hx += 1;
            }
            hx
        })
        .collect()
}

impl Effect for Bokeh {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        if w == 0 || h == 0 {
            return src.clone();
        }
        let buf = src.to_premultiplied_f32();
        let k = f64::from(self.highlights) / 100.0 * HIGHLIGHT_GAIN;
        // Per pixel: weight · premultiplied RGBA, and the weight.
        let weighted: Vec<[f64; 5]> = buf
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let [r, g, b, a] = p.map(f64::from);
                let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let wt = 1.0 + k * l.powi(4);
                [wt * r, wt * g, wt * b, wt * a, wt]
            })
            .collect();
        // Row prefix sums turn each disc row into one subtraction, so the
        // cost per pixel is O(radius), not O(radius²). Rows are built on
        // demand and dropped once no later output row reads them.
        let row_prefix = |y: usize| -> Vec<[f64; 5]> {
            let mut out = Vec::with_capacity(w + 1);
            let mut acc = [0.0f64; 5];
            out.push(acc);
            for v in &weighted[y * w..(y + 1) * w] {
                for c in 0..5 {
                    acc[c] += v[c];
                }
                out.push(acc);
            }
            out
        };
        let r = i64::from(self.radius);
        let rows = disc_rows(r);
        let (wi, hi) = (w as i64, h as i64);
        let mut cache: Vec<Option<Vec<[f64; 5]>>> = vec![None; h];
        let mut out = vec![0.0f32; buf.len()];
        for y in 0..hi {
            let lo = (y - r).max(0) as usize;
            let top = (y + r).min(hi - 1) as usize;
            if lo > 0 {
                cache[lo - 1] = None;
            }
            for (sy, slot) in cache.iter_mut().enumerate().take(top + 1).skip(lo) {
                if slot.is_none() {
                    *slot = Some(row_prefix(sy));
                }
            }
            for x in 0..wi {
                let mut acc = [0.0f64; 5];
                for (dy, &hx) in (-r..=r).zip(&rows) {
                    let sy = (y + dy).clamp(0, hi - 1) as usize;
                    let Some(prefix) = cache[sy].as_ref() else {
                        continue;
                    };
                    let (a, b) = (x - hx, x + hx);
                    let left = (-a).max(0) as f64;
                    let right = (b - (wi - 1)).max(0) as f64;
                    let inner_lo = a.max(0) as usize;
                    let inner_hi = b.min(wi - 1) as usize;
                    let first = &weighted[sy * w];
                    let last = &weighted[sy * w + w - 1];
                    for c in 0..5 {
                        acc[c] += prefix[inner_hi + 1][c] - prefix[inner_lo][c]
                            + left * first[c]
                            + right * last[c];
                    }
                }
                let o = (y as usize * w + x as usize) * 4;
                for c in 0..4 {
                    out[o + c] = (acc[c] / acc[4]) as f32;
                }
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            highlights: self.highlights,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_rows_radius_two_is_a_rounded_square() {
        assert_eq!(disc_rows(2), vec![0, 1, 2, 1, 0]);
    }

    #[test]
    fn bokeh_spreads_a_bright_pixel_more_with_highlights() {
        let mut data = [[10u8, 10, 10, 255]; 81].concat();
        data[40 * 4..40 * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(9, 9, data).expect("9x9");
        let at = |img: &EffectImage| img.data()[(4 * 9 + 2) * 4];
        let plain = Bokeh::new(3.0, 0.0).apply(&src);
        let bright = Bokeh::new(3.0, 100.0).apply(&src);
        assert!(
            at(&bright) > at(&plain),
            "{} vs {}",
            at(&bright),
            at(&plain)
        );
    }
}
