//! Quantize (docs/specs/effects-object-stylize-color-noise.md): reduce the
//! target's colours to a palette of at most N, by Median Cut or Octree,
//! optionally with Floyd–Steinberg dithering.

use std::collections::{BTreeMap, HashMap};

use crate::blur::{param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// How Quantize chooses its palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizeAlgorithm {
    /// Splits the colour box with the widest channel at its weighted median.
    MedianCut,
    /// Merges the deepest, least used branches of an RGB octree.
    Octree,
}

/// Maps every opaque pixel (alpha > 0) of the target to the nearest colour
/// (RGB distance, ties to the earlier palette entry) of a palette of at
/// most `colors` colours that `algorithm` builds from the target's
/// distinct colours. A target with no more distinct colours than `colors`
/// keeps its own colours as the palette. `dithering` > 0 diffuses each
/// pixel's error to its target neighbours (Floyd–Steinberg, scaled by the
/// percentage). Alpha and transparent pixels are kept; the result is
/// deterministic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantize {
    /// Largest palette size, `2..=256`.
    pub colors: u32,
    /// Palette algorithm.
    pub algorithm: QuantizeAlgorithm,
    /// Error diffusion in percent, `0..=100`.
    pub dithering: f32,
}

type Rgb = [u8; 3];

impl Quantize {
    /// Creates the effect (non-finite numbers read as 0, then clamped:
    /// colours to `2..=256` rounded, dithering to `0..=100`).
    pub fn new(colors: f32, algorithm: QuantizeAlgorithm, dithering: f32) -> Self {
        Self {
            colors: whole(colors, 2, 256),
            algorithm,
            dithering: param(dithering, 0.0, 100.0),
        }
    }

    /// Like [`Effect::apply`], for a target: `in_target(x, y)` tells
    /// whether the pixel at `(x, y)` belongs to it. The palette comes from
    /// the target's colours and error diffuses among target pixels only.
    pub fn apply_in(&self, src: &EffectImage, in_target: impl Fn(u32, u32) -> bool) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let px = src.data().as_chunks::<4>().0;
        let mask: Vec<bool> = (0..w * h)
            .map(|i| px[i][3] > 0 && in_target((i % w) as u32, (i / w) as u32))
            .collect();
        let mut hist: BTreeMap<Rgb, u64> = BTreeMap::new();
        for (p, _) in px.iter().zip(&mask).filter(|(_, m)| **m) {
            *hist.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
        if hist.len() <= self.colors as usize {
            // Its own colours: without dithering nothing moves, with it
            // every error is zero.
            return src.clone();
        }
        let colours: Vec<(Rgb, u64)> = hist.into_iter().collect();
        let palette = match self.algorithm {
            QuantizeAlgorithm::MedianCut => median_cut(&colours, self.colors as usize),
            QuantizeAlgorithm::Octree => octree(&colours, self.colors as usize),
        };
        let mut nearest = Nearest::new(&palette);
        let mut out = src.data().to_vec();
        let pixels = out.as_chunks_mut::<4>().0;
        if self.dithering <= 0.0 {
            for (p, _) in pixels.iter_mut().zip(&mask).filter(|(_, m)| **m) {
                let c = nearest.get([p[0], p[1], p[2]]);
                p[..3].copy_from_slice(&c);
            }
        } else {
            let k = self.dithering / 100.0;
            let mut err = vec![[0.0f32; 3]; w * h];
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    if !mask[i] {
                        continue;
                    }
                    let p = &mut pixels[i];
                    let want: [f32; 3] =
                        std::array::from_fn(|c| (f32::from(p[c]) + err[i][c]).clamp(0.0, 255.0));
                    let c = nearest.get(want.map(|v| v.round() as u8));
                    p[..3].copy_from_slice(&c);
                    let e: [f32; 3] = std::array::from_fn(|ch| (want[ch] - f32::from(c[ch])) * k);
                    for (dx, dy, wgt) in [(1, 0, 7.0), (-1, 1, 3.0), (0, 1, 5.0), (1, 1, 1.0)] {
                        let (nx, ny) = (x as isize + dx, y + dy);
                        if nx < 0 || nx as usize >= w || ny >= h {
                            continue;
                        }
                        let j = ny * w + nx as usize;
                        if mask[j] {
                            for ch in 0..3 {
                                err[j][ch] += e[ch] * wgt / 16.0;
                            }
                        }
                    }
                }
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }
}

impl Effect for Quantize {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        self.apply_in(src, |_, _| true)
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

/// Nearest palette colour per RGB value, cached.
struct Nearest<'a> {
    palette: &'a [Rgb],
    cache: HashMap<Rgb, Rgb>,
}

impl<'a> Nearest<'a> {
    fn new(palette: &'a [Rgb]) -> Self {
        Self {
            palette,
            cache: HashMap::new(),
        }
    }

    fn get(&mut self, c: Rgb) -> Rgb {
        let palette = self.palette;
        *self.cache.entry(c).or_insert_with(|| {
            let dist = |p: &Rgb| -> i32 {
                (0..3)
                    .map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2))
                    .sum()
            };
            // `min_by_key` keeps the first of equal keys.
            *palette
                .iter()
                .min_by_key(|p| dist(p))
                .expect("palette is not empty")
        })
    }
}

/// The count-weighted average of `colours`, rounded.
fn average(colours: &[(Rgb, u64)]) -> Rgb {
    let n: u64 = colours.iter().map(|(_, k)| k).sum();
    std::array::from_fn(|c| {
        let sum: u64 = colours.iter().map(|(p, k)| u64::from(p[c]) * k).sum();
        ((sum + n / 2) / n) as u8
    })
}

/// A palette sorted and without duplicates.
fn finish(mut palette: Vec<Rgb>) -> Vec<Rgb> {
    palette.sort_unstable();
    palette.dedup();
    palette
}

/// Median Cut: start with one box of every colour; until there are `n`
/// boxes, split the box whose widest channel spans the most (the first on
/// a tie) at its count-weighted median along that channel.
fn median_cut(colours: &[(Rgb, u64)], n: usize) -> Vec<Rgb> {
    let widest = |b: &[(Rgb, u64)]| -> (u8, usize) {
        (0..3)
            .map(|c| {
                let lo = b.iter().map(|(p, _)| p[c]).min().unwrap_or(0);
                let hi = b.iter().map(|(p, _)| p[c]).max().unwrap_or(0);
                (hi - lo, c)
            })
            // Largest span; on a tie the earlier channel (R, G, B).
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
            .unwrap_or((0, 0))
    };
    let mut boxes: Vec<Vec<(Rgb, u64)>> = vec![colours.to_vec()];
    while boxes.len() < n {
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| (widest(b).0, i))
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        let Some((_, i)) = pick else { break };
        let mut b = boxes.swap_remove(i);
        let channel = widest(&b).1;
        b.sort_unstable_by_key(|(p, _)| (p[channel], *p));
        let total: u64 = b.iter().map(|(_, k)| k).sum();
        let mut seen = 0;
        let mut split = b.len() / 2;
        for (j, (_, k)) in b.iter().enumerate() {
            seen += k;
            if seen * 2 >= total {
                split = j + 1;
                break;
            }
        }
        let split = split.clamp(1, b.len() - 1);
        let upper = b.split_off(split);
        // Keep the box order stable: the halves take the old slot and the end.
        boxes.push(b);
        let last = boxes.len() - 1;
        boxes.swap(i, last);
        boxes.push(upper);
    }
    finish(boxes.iter().map(|b| average(b)).collect())
}

/// One octree node: the colour sum and pixel count of its leaves.
struct Node {
    sum: [u64; 3],
    count: u64,
    children: [Option<usize>; 8],
    leaf: bool,
    level: u8,
}

/// Octree: insert every colour down to level 8; while there are more than
/// `n` leaves, merge the children of the internal node on the deepest
/// level that has the fewest pixels (the first one created on a tie).
fn octree(colours: &[(Rgb, u64)], n: usize) -> Vec<Rgb> {
    let mut nodes = vec![Node {
        sum: [0; 3],
        count: 0,
        children: [None; 8],
        leaf: false,
        level: 0,
    }];
    let mut leaves = 0usize;
    for &(p, k) in colours {
        let mut at = 0;
        for level in 0..8u8 {
            let bit = 7 - level;
            let slot = usize::from((p[0] >> bit) & 1) << 2
                | usize::from((p[1] >> bit) & 1) << 1
                | usize::from((p[2] >> bit) & 1);
            nodes[at].count += k;
            at = match nodes[at].children[slot] {
                Some(c) => c,
                None => {
                    nodes.push(Node {
                        sum: [0; 3],
                        count: 0,
                        children: [None; 8],
                        leaf: level == 7,
                        level: level + 1,
                    });
                    let c = nodes.len() - 1;
                    nodes[at].children[slot] = Some(c);
                    if level == 7 {
                        leaves += 1;
                    }
                    c
                }
            };
        }
        let leaf = &mut nodes[at];
        leaf.count += k;
        for (sum, &v) in leaf.sum.iter_mut().zip(&p) {
            *sum += u64::from(v) * k;
        }
    }
    // Internal nodes, deepest level first, then fewest pixels, then
    // creation order. Merging never changes a node's count, so one sort
    // gives the whole merge order; merging a level's nodes only after every
    // deeper one means their children are leaves.
    let mut order: Vec<usize> = (0..nodes.len()).filter(|&i| !nodes[i].leaf).collect();
    order.sort_unstable_by_key(|&i| (std::cmp::Reverse(nodes[i].level), nodes[i].count, i));
    for i in order {
        if leaves <= n {
            break;
        }
        let children: Vec<usize> = nodes[i].children.iter().flatten().copied().collect();
        for c in &children {
            for ch in 0..3 {
                nodes[i].sum[ch] += nodes[*c].sum[ch];
            }
        }
        nodes[i].children = [None; 8];
        nodes[i].leaf = true;
        leaves = leaves + 1 - children.len();
    }
    let mut palette = Vec::new();
    let mut stack = vec![0usize];
    while let Some(i) = stack.pop() {
        let node = &nodes[i];
        if node.leaf {
            let k = node.count.max(1);
            palette.push(std::array::from_fn(|c| ((node.sum[c] + k / 2) / k) as u8));
        } else {
            stack.extend(node.children.iter().flatten());
        }
    }
    finish(palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours(n: u8) -> Vec<(Rgb, u64)> {
        (0..n)
            .map(|i| ([i * 3, 255 - i * 2, i], 1 + u64::from(i % 3)))
            .collect()
    }

    #[test]
    fn median_cut_and_octree_return_at_most_n_colours() {
        let c = colours(80);
        for n in [2, 3, 7, 16, 64] {
            assert!(median_cut(&c, n).len() <= n, "median cut {n}");
            assert!(octree(&c, n).len() <= n, "octree {n}");
            assert!(!median_cut(&c, n).is_empty());
            assert!(!octree(&c, n).is_empty());
        }
    }

    #[test]
    fn median_cut_splits_into_exactly_n_boxes_when_it_can() {
        assert_eq!(median_cut(&colours(80), 16).len(), 16);
    }

    #[test]
    fn new_clamps_the_colour_count() {
        let q = |n| Quantize::new(n, QuantizeAlgorithm::Octree, 0.0).colors;
        assert_eq!((q(0.0), q(2.0), q(256.0), q(1000.0)), (2, 2, 256, 256));
    }
}
