//! Crystalize (docs/specs/effects-distort.md): random convex cells, each
//! filled with the average colour of its pixels.

use crate::blur::whole;
use crate::effect::Effect;
use crate::image::EffectImage;
use crate::random::{hash, unit};

/// Splits the image into the Voronoi cells of seeded points about
/// `cell_size` apart (one per grid square, jittered within its middle
/// half) and fills each cell with the premultiplied average of its pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crystalize {
    /// Point spacing in pixels, `2..=250`.
    pub cell_size: u32,
    /// Random seed, `0..=9999`.
    pub seed: u32,
}

impl Crystalize {
    /// Creates a crystalize effect (non-finite numbers read as 0, then
    /// rounded and clamped).
    pub fn new(cell_size: f32, seed: f32) -> Self {
        Self {
            cell_size: whole(cell_size, 2, 250),
            seed: whole(seed, 0, 9999),
        }
    }

    /// The cell index of every pixel, row-major.
    fn labels(&self, w: usize, h: usize) -> Vec<usize> {
        let c = self.cell_size as usize;
        let (gw, gh) = (w.div_ceil(c), h.div_ceil(c));
        let points: Vec<(f32, f32)> = (0..gw * gh)
            .map(|i| {
                let (gx, gy) = (i % gw, i / gw);
                let bits = hash(self.seed, gx, gy);
                (
                    (gx as f32 + 0.25 + 0.5 * unit(bits)) * c as f32,
                    (gy as f32 + 0.25 + 0.5 * unit(bits >> 24)) * c as f32,
                )
            })
            .collect();
        let mut labels = vec![0usize; w * h];
        for y in 0..h {
            let gy = y / c;
            for x in 0..w {
                let gx = x / c;
                // Points two squares away can still be the nearest; three
                // squares away cannot.
                let mut best = (f32::INFINITY, 0);
                for ny in gy.saturating_sub(2)..(gy + 3).min(gh) {
                    for nx in gx.saturating_sub(2)..(gx + 3).min(gw) {
                        let (px, py) = points[ny * gw + nx];
                        let d = (px - x as f32).powi(2) + (py - y as f32).powi(2);
                        if d < best.0 {
                            best = (d, ny * gw + nx);
                        }
                    }
                }
                labels[y * w + x] = best.1;
            }
        }
        join_lone_pixels(&mut labels, w, h);
        labels
    }
}

/// Gives every pixel that shares its cell with none of its 4-neighbours
/// the cell of its first neighbour (left, up, right, down), so no cell
/// shows as a lone pixel. Joining a lone pixel never leaves another one
/// alone: no neighbour shared its cell.
fn join_lone_pixels(labels: &mut [usize], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let neighbours = [
                (x > 0).then(|| i - 1),
                (y > 0).then(|| i - w),
                (x + 1 < w).then(|| i + 1),
                (y + 1 < h).then(|| i + w),
            ];
            let mut present = neighbours.iter().flatten();
            if present.clone().all(|&n| labels[n] != labels[i])
                && let Some(&first) = present.next()
            {
                labels[i] = labels[first];
            }
        }
    }
}

impl Effect for Crystalize {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        if w == 0 || h == 0 {
            return src.clone();
        }
        let labels = self.labels(w, h);
        let buf = src.to_premultiplied_f32();
        let cells = labels.iter().max().map_or(0, |m| m + 1);
        let mut sums = vec![[0.0f64; 4]; cells];
        let mut counts = vec![0u32; cells];
        for (i, &l) in labels.iter().enumerate() {
            for c in 0..4 {
                sums[l][c] += f64::from(buf[i * 4 + c]);
            }
            counts[l] += 1;
        }
        let mean: Vec<[f32; 4]> = sums
            .iter()
            .zip(&counts)
            .map(|(s, &n)| {
                let n = f64::from(n.max(1));
                [s[0], s[1], s[2], s[3]].map(|v| (v / n) as f32)
            })
            .collect();
        let out: Vec<f32> = labels.iter().flat_map(|&l| mean[l]).collect();
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            cell_size: ((self.cell_size as f32 * factor).round() as u32).max(2),
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_lone_pixels_joins_a_lone_pixel_to_its_first_neighbour() {
        let mut labels = vec![0, 0, 0, 0, 1, 0, 0, 0, 0];
        join_lone_pixels(&mut labels, 3, 3);
        assert_eq!(labels, vec![0; 9]);
    }

    #[test]
    fn labels_cover_about_one_cell_per_grid_square() {
        let labels = Crystalize::new(8.0, 3.0).labels(64, 64);
        let cells: std::collections::HashSet<usize> = labels.into_iter().collect();
        assert!((50..=64).contains(&cells.len()), "{}", cells.len());
    }
}
