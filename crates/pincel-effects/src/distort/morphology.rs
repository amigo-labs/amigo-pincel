//! Morphology (docs/specs/effects-distort.md): Dilate, Erode, Open and
//! Close by luma rank. It picks whole existing pixels and never mixes
//! colours, so it keeps a Pixel-mode palette.

use crate::blur::whole;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Which morphological operation to run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MorphologyOp {
    /// Each pixel becomes the brightest opaque pixel of its neighbourhood.
    #[default]
    Dilate,
    /// Each pixel becomes the darkest pixel of its neighbourhood, or
    /// transparent when the neighbourhood holds a transparent pixel.
    Erode,
    /// Erode, then Dilate.
    Open,
    /// Dilate, then Erode.
    Close,
}

/// The neighbourhood's shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MorphologyShape {
    /// The `(2r + 1)²` square.
    #[default]
    Square,
    /// The disc `dx² + dy² ≤ r²` (radius 1 is a plus).
    Circle,
}

/// Morphology by luma (`0.2126 R + 0.7152 G + 0.0722 B`) over a square or
/// circular neighbourhood. Ties go to the first pixel in row-major order;
/// pixels beyond the layer's edge count as transparent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Morphology {
    /// The operation.
    pub op: MorphologyOp,
    /// The neighbourhood's shape.
    pub shape: MorphologyShape,
    /// Neighbourhood radius in pixels, `1..=100`.
    pub radius: u32,
}

impl Morphology {
    /// Creates a morphology effect (the radius read as finite, rounded and
    /// clamped to `1..=100`).
    pub fn new(op: MorphologyOp, shape: MorphologyShape, radius: f32) -> Self {
        Self {
            op,
            shape,
            radius: whole(radius, 1, 100),
        }
    }

    /// Half-width of the neighbourhood's row at vertical offset `dy`.
    fn half_width(&self, dy: i64) -> usize {
        let r = i64::from(self.radius);
        match self.shape {
            MorphologyShape::Square => r as usize,
            MorphologyShape::Circle => {
                let mut k = ((r * r - dy * dy) as f64).sqrt() as i64;
                while k * k > r * r - dy * dy {
                    k -= 1;
                }
                k as usize
            }
        }
    }

    /// One pass: each pixel becomes the pixel with the best rank key of
    /// its neighbourhood (`dilate`: highest, else lowest). Key 0 stands
    /// for transparent and outside.
    fn pass(&self, src: &EffectImage, dilate: bool) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let keys: Vec<u64> = data
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if p[3] == 0 {
                    return 0;
                }
                let luma = 2126 * u64::from(p[0]) + 7152 * u64::from(p[1]) + 722 * u64::from(p[2]);
                // Ties to the lower index: ascending for min, descending
                // for max.
                let tie = if dilate {
                    u64::from(u32::MAX) - i as u64
                } else {
                    i as u64
                };
                ((luma + 1) << 32) | tie
            })
            .collect();
        let better = |a: u64, b: u64| if dilate { a > b } else { a < b };
        let r = i64::from(self.radius);
        let mut best = vec![None::<u64>; w * h];
        let mut widths: Vec<usize> = (-r..=r).map(|dy| self.half_width(dy)).collect();
        widths.sort_unstable();
        widths.dedup();
        for k in widths {
            let rows = row_extremes(&keys, w, h, k, &better);
            for dy in (-r..=r).filter(|&dy| self.half_width(dy) == k) {
                for y in 0..h {
                    let sy = y as i64 + dy;
                    for x in 0..w {
                        // Outside the layer: transparent, key 0.
                        let key = if (0..h as i64).contains(&sy) {
                            rows[sy as usize * w + x]
                        } else {
                            0
                        };
                        let slot = &mut best[y * w + x];
                        if slot.is_none_or(|b| better(key, b)) {
                            *slot = Some(key);
                        }
                    }
                }
            }
        }
        let mut out = vec![0u8; data.len()];
        for (o, key) in out.as_chunks_mut::<4>().0.iter_mut().zip(best) {
            let key = key.unwrap_or(0);
            if key != 0 {
                let low = key & u64::from(u32::MAX);
                let i = if dilate {
                    u64::from(u32::MAX) - low
                } else {
                    low
                } as usize;
                o.copy_from_slice(&data[i * 4..i * 4 + 4]);
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out)
            .expect("same size as the source buffer")
    }
}

/// For every pixel, the best key over the row window `x - k..=x + k`; a
/// window reaching past the row's ends also holds key 0 (outside).
fn row_extremes(
    keys: &[u64],
    w: usize,
    h: usize,
    k: usize,
    better: &impl Fn(u64, u64) -> bool,
) -> Vec<u64> {
    let mut out = vec![0u64; w * h];
    let mut deque = std::collections::VecDeque::with_capacity(2 * k + 1);
    for y in 0..h {
        let row = &keys[y * w..(y + 1) * w];
        deque.clear();
        let mut next = 0;
        for x in 0..w {
            let hi = (x + k).min(w - 1);
            while next <= hi {
                // Keep the deque's keys strictly decreasing in quality so
                // its front is the window's best.
                while deque
                    .back()
                    .is_some_and(|&j: &usize| !better(row[j], row[next]))
                {
                    deque.pop_back();
                }
                deque.push_back(next);
                next += 1;
            }
            while deque.front().is_some_and(|&j| j + k < x) {
                deque.pop_front();
            }
            let front = deque.front().map_or(0, |&j| row[j]);
            let outside = x < k || x + k >= w;
            out[y * w + x] = if outside && better(0, front) {
                0
            } else {
                front
            };
        }
    }
    out
}

impl Effect for Morphology {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if src.width() == 0 || src.height() == 0 {
            return src.clone();
        }
        match self.op {
            MorphologyOp::Dilate => self.pass(src, true),
            MorphologyOp::Erode => self.pass(src, false),
            MorphologyOp::Open => self.pass(&self.pass(src, false), true),
            MorphologyOp::Close => self.pass(&self.pass(src, true), false),
        }
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_extremes_match_a_naive_window() {
        let keys: Vec<u64> = [5, 1, 9, 3, 3, 7, 0, 2, 8, 4].map(|v| v as u64).to_vec();
        for k in 0..4 {
            for dilate in [true, false] {
                let better = |a: u64, b: u64| if dilate { a > b } else { a < b };
                let got = row_extremes(&keys, 10, 1, k, &better);
                for (x, &g) in got.iter().enumerate() {
                    let lo = x as i64 - k as i64;
                    let hi = x as i64 + k as i64;
                    let naive = (lo..=hi)
                        .map(|i| {
                            if (0..10).contains(&i) {
                                keys[i as usize]
                            } else {
                                0
                            }
                        })
                        .reduce(|a, b| if better(b, a) { b } else { a })
                        .expect("window");
                    assert_eq!(g, naive, "k {k} dilate {dilate} x {x}");
                }
            }
        }
    }

    #[test]
    fn circle_half_widths_form_a_disc() {
        let m = Morphology::new(MorphologyOp::Dilate, MorphologyShape::Circle, 3.0);
        let widths: Vec<usize> = (-3..=3).map(|dy| m.half_width(dy)).collect();
        assert_eq!(widths, vec![0, 2, 2, 3, 2, 2, 0]);
    }
}
