//! Stylize ▸ Outline (docs/specs/effects-object-stylize-color-noise.md):
//! paint.net's edge effect, darkening pixels near colour edges.

use crate::blur::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Blends each pixel toward the darkest-luma pixel of the square of
/// `thickness` px around it, by `intensity` % times the square's luma
/// range / 255: flat areas (range 0) stay as they are, a pixel near an edge
/// darkens. Luma is BT.709; the square ends at the layer's edge.
///
/// Fully transparent pixels take no part — neither their colour nor their
/// luma counts — and stay as they are; alpha is preserved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylizeOutline {
    /// Half the square's side in pixels, `1..=200`.
    pub thickness: u32,
    /// Strength in percent, `0..=100`.
    pub intensity: f32,
}

impl StylizeOutline {
    /// Creates the effect (non-finite numbers read as 0, then clamped:
    /// thickness to `1..=200` rounded, intensity to `0..=100`).
    pub fn new(thickness: f32, intensity: f32) -> Self {
        Self {
            thickness: whole(thickness, 1, 200),
            intensity: param(intensity, 0.0, 100.0),
        }
    }
}

/// BT.709 luma scaled to an integer (`0..=2_550_000`), so comparisons are
/// exact.
fn luma_key(p: &[u8; 4]) -> u32 {
    2126 * u32::from(p[0]) + 7152 * u32::from(p[1]) + 722 * u32::from(p[2])
}

/// For every position `i` of a line of `n` keys (read through `key(i)`),
/// the position of the smallest key within `i - r ..= i + r` (cut at the
/// line's ends): a monotonic-deque sliding minimum, O(n).
fn sliding_argmin(n: usize, r: usize, key: impl Fn(usize) -> u64) -> Vec<usize> {
    let mut out = Vec::with_capacity(n);
    let mut deque = std::collections::VecDeque::new();
    let mut next = 0;
    for i in 0..n {
        let hi = (i + r).min(n - 1);
        while next <= hi {
            let k = key(next);
            while deque.back().is_some_and(|&b| key(b) > k) {
                deque.pop_back();
            }
            deque.push_back(next);
            next += 1;
        }
        let lo = i.saturating_sub(r);
        while deque.front().is_some_and(|&f| f < lo) {
            deque.pop_front();
        }
        out.push(*deque.front().expect("the window holds position i"));
    }
    out
}

/// For every pixel, the index of the pixel with the smallest `key` in the
/// `(2r + 1)²` square around it: a row pass, then a column pass.
fn square_argmin(w: usize, h: usize, r: usize, key: &[u64]) -> Vec<usize> {
    let mut rows = vec![0; w * h];
    for y in 0..h {
        let line = sliding_argmin(w, r, |x| key[y * w + x]);
        for (x, i) in line.into_iter().enumerate() {
            rows[y * w + x] = y * w + i;
        }
    }
    let mut out = vec![0; w * h];
    for x in 0..w {
        let line = sliding_argmin(h, r, |y| key[rows[y * w + x]]);
        for (y, j) in line.into_iter().enumerate() {
            out[y * w + x] = rows[j * w + x];
        }
    }
    out
}

impl Effect for StylizeOutline {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        if w == 0 || h == 0 || self.intensity <= 0.0 {
            return src.clone();
        }
        let px = src.data().as_chunks::<4>().0;
        let r = self.thickness as usize;
        // Transparent pixels sort last in both searches.
        let dark: Vec<u64> = px
            .iter()
            .map(|p| {
                if p[3] > 0 {
                    u64::from(luma_key(p))
                } else {
                    u64::MAX
                }
            })
            .collect();
        let light: Vec<u64> = px
            .iter()
            .map(|p| {
                if p[3] > 0 {
                    u64::from(u32::MAX - luma_key(p))
                } else {
                    u64::MAX
                }
            })
            .collect();
        let darkest = square_argmin(w, h, r, &dark);
        let lightest = square_argmin(w, h, r, &light);
        let k = self.intensity / 100.0;
        let mut out = src.data().to_vec();
        for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let (d, l) = (&px[darkest[i]], &px[lightest[i]]);
            let range = luma_key(l).saturating_sub(luma_key(d)) as f32 / 2_550_000.0;
            let t = k * range;
            if t <= 0.0 {
                continue;
            }
            for c in 0..3 {
                let (v, target) = (f32::from(p[c]), f32::from(d[c]));
                p[c] = (v + (target - v) * t).round().clamp(0.0, 255.0) as u8;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            thickness: ((self.thickness as f32 * factor).round() as u32).max(1),
            intensity: self.intensity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliding_argmin_matches_a_scan() {
        let keys = [5u64, 3, 9, 1, 7, 7, 2, 8, 6];
        for r in 0..5 {
            let got = sliding_argmin(keys.len(), r, |i| keys[i]);
            for (i, &g) in got.iter().enumerate() {
                let lo = i.saturating_sub(r);
                let hi = (i + r).min(keys.len() - 1);
                let min = keys[lo..=hi].iter().min().expect("non-empty");
                assert_eq!(keys[g], *min, "r {r} i {i}");
                assert!((lo..=hi).contains(&g));
            }
        }
    }

    #[test]
    fn scaled_keeps_at_least_one_pixel() {
        assert_eq!(StylizeOutline::new(8.0, 50.0).scaled(0.5).thickness, 4);
        assert_eq!(StylizeOutline::new(1.0, 50.0).scaled(0.1).thickness, 1);
    }
}
