//! Median (docs/specs/effects-blurs.md): a percentile filter over a square
//! neighbourhood, kept O(radius) per pixel by a sliding histogram.

use super::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Replaces each colour channel by the `percentile` of that channel over
/// the opaque pixels (alpha > 0) of the `(2·radius + 1)²` neighbourhood —
/// transparent pixels never tint — and alpha by the percentile of every
/// alpha there. A neighbourhood without an opaque pixel keeps the pixel's
/// colour. Percentile 0 is the minimum, 100 the maximum, 50 the median.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Median {
    /// Neighbourhood radius in pixels, `1..=200`.
    pub radius: u32,
    /// Percentile, `0..=100`.
    pub percentile: f32,
}

impl Median {
    /// Creates a median filter (non-finite numbers read as 0, then clamped:
    /// radius to `1..=200` rounded, percentile to `0..=100`).
    pub fn new(radius: f32, percentile: f32) -> Self {
        Self {
            radius: whole(radius, 1, 200),
            percentile: param(percentile, 0.0, 100.0),
        }
    }
}

/// Histograms of a window: R, G, B over its opaque pixels, alpha over all.
struct Window {
    hist: [[u32; 256]; 4],
    opaque: u32,
    total: u32,
}

impl Window {
    fn new() -> Self {
        Self {
            hist: [[0; 256]; 4],
            opaque: 0,
            total: 0,
        }
    }

    fn add(&mut self, p: &[u8; 4]) {
        self.hist[3][p[3] as usize] += 1;
        self.total += 1;
        if p[3] > 0 {
            for (hist, &v) in self.hist.iter_mut().zip(&p[..3]) {
                hist[v as usize] += 1;
            }
            self.opaque += 1;
        }
    }

    fn remove(&mut self, p: &[u8; 4]) {
        self.hist[3][p[3] as usize] -= 1;
        self.total -= 1;
        if p[3] > 0 {
            for (hist, &v) in self.hist.iter_mut().zip(&p[..3]) {
                hist[v as usize] -= 1;
            }
            self.opaque -= 1;
        }
    }
}

/// The value at rank `round(p · (n − 1))` of a histogram holding `n` values.
fn percentile(hist: &[u32; 256], n: u32, p: f32) -> u8 {
    let rank = (f64::from(p) / 100.0 * f64::from(n.saturating_sub(1))).round() as u32;
    let mut seen = 0;
    for (v, &count) in hist.iter().enumerate() {
        seen += count;
        if seen > rank {
            return v as u8;
        }
    }
    255
}

impl Effect for Median {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as i64, src.height() as i64);
        if w == 0 || h == 0 {
            return src.clone();
        }
        let px = src.data().as_chunks::<4>().0;
        let r = i64::from(self.radius);
        let at = |x: i64, y: i64| &px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize];
        let mut out = src.data().to_vec();
        for y in 0..h {
            let mut win = Window::new();
            for dx in -r..=r {
                for dy in -r..=r {
                    win.add(at(dx, y + dy));
                }
            }
            for x in 0..w {
                if x > 0 {
                    // Window of x is the clamped columns x − r ..= x + r.
                    for dy in -r..=r {
                        win.remove(at(x - 1 - r, y + dy));
                        win.add(at(x + r, y + dy));
                    }
                }
                let o = ((y * w + x) * 4) as usize;
                if win.opaque > 0 {
                    for c in 0..3 {
                        out[o + c] = percentile(&win.hist[c], win.opaque, self.percentile);
                    }
                }
                out[o + 3] = percentile(&win.hist[3], win.total, self.percentile);
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            percentile: self.percentile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same filter by sorting every neighbourhood.
    fn naive(src: &EffectImage, r: i64, p: f32) -> Vec<u8> {
        let (w, h) = (src.width() as i64, src.height() as i64);
        let px = src.data().as_chunks::<4>().0;
        let mut out = src.data().to_vec();
        for y in 0..h {
            for x in 0..w {
                let mut near = Vec::new();
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (sx, sy) = ((x + dx).clamp(0, w - 1), (y + dy).clamp(0, h - 1));
                        near.push(px[(sy * w + sx) as usize]);
                    }
                }
                let pick = |mut v: Vec<u8>| {
                    v.sort_unstable();
                    let rank = (f64::from(p) / 100.0 * (v.len() - 1) as f64).round() as usize;
                    v[rank]
                };
                let o = ((y * w + x) * 4) as usize;
                let opaque: Vec<[u8; 4]> = near.iter().copied().filter(|q| q[3] > 0).collect();
                if !opaque.is_empty() {
                    for c in 0..3 {
                        out[o + c] = pick(opaque.iter().map(|q| q[c]).collect());
                    }
                }
                out[o + 3] = pick(near.iter().map(|q| q[3]).collect());
            }
        }
        out
    }

    #[test]
    fn median_sliding_histogram_matches_sorting() {
        let data: Vec<u8> = (0..11u32 * 7 * 4)
            .map(|i| match i % 4 {
                3 if i % 12 == 3 => 0,
                _ => (i * 89 % 256) as u8,
            })
            .collect();
        let src = EffectImage::from_rgba8(11, 7, data).expect("11x7");
        for (r, p) in [(1, 50.0), (2, 0.0), (3, 100.0), (2, 30.0), (9, 70.0)] {
            let out = Median::new(r as f32, p).apply(&src);
            assert_eq!(out.data(), naive(&src, r, p).as_slice(), "r {r} p {p}");
        }
    }
}
