//! Ink and its connected components.
//!
//! A pixel is ink when the luma in its 3 × 3 neighbourhood spans at least
//! a threshold (a morphological gradient): that marks the edges of text in
//! either polarity and on any background colour, and leaves flat areas,
//! soft gradients and faint separators out. The threshold rises with the
//! page's noise (the median gradient), so a grainy scan does not turn into
//! ink. Ink runs are labelled into 8-connected components band by band, so
//! memory stays proportional to the ink, not to the page.

use super::{REPORT_PIXELS, Reporter, Source};
use crate::PageError;
use crate::page::Progress;

/// The lowest gradient that counts as ink.
const MIN_THRESHOLD: u32 = 40;
/// The highest threshold a noisy page gets.
const MAX_THRESHOLD: u32 = 160;
/// The threshold is this multiple of the median gradient, when higher.
const NOISE_FACTOR: u32 = 3;

/// A horizontal run of ink pixels: row `y`, columns `x0..=x1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Run {
    pub(crate) y: u32,
    pub(crate) x0: u32,
    pub(crate) x1: u32,
}

/// A connected component of ink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comp {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    /// Ink pixels.
    pub(crate) area: u32,
    /// The component's runs: `Ink::runs[first..end]`, ordered by row.
    pub(crate) first: u32,
    pub(crate) end: u32,
}

impl Comp {
    pub(crate) fn width(&self) -> i32 {
        self.right - self.left + 1
    }

    pub(crate) fn height(&self) -> i32 {
        self.bottom - self.top + 1
    }
}

/// The ink of a page.
pub(crate) struct Ink {
    pub(crate) comps: Vec<Comp>,
    pub(crate) runs: Vec<Run>,
}

impl Ink {
    pub(crate) fn runs_of(&self, comp: &Comp) -> &[Run] {
        &self.runs[comp.first as usize..comp.end as usize]
    }
}

/// The gradient of rows `y0..y1` into `out` (`(y1 - y0) × width`).
fn gradient(src: &Source, y0: usize, y1: usize, out: &mut Vec<u8>) {
    let (w, h) = (src.width, src.height);
    let ya = y0.saturating_sub(1);
    let yb = (y1 + 1).min(h);
    let rows = yb - ya;
    let mut hmax = vec![0u8; rows * w];
    let mut hmin = vec![0u8; rows * w];
    let mut line = vec![0u8; w];
    for r in 0..rows {
        for (x, l) in line.iter_mut().enumerate() {
            *l = src.luma(x, ya + r);
        }
        let (mx, mn) = (&mut hmax[r * w..r * w + w], &mut hmin[r * w..r * w + w]);
        for x in 0..w {
            let a = line[x.saturating_sub(1)];
            let b = line[x];
            let c = line[(x + 1).min(w - 1)];
            mx[x] = a.max(b).max(c);
            mn[x] = a.min(b).min(c);
        }
    }
    out.clear();
    out.resize((y1 - y0) * w, 0);
    for y in y0..y1 {
        let r0 = y.saturating_sub(1).max(ya) - ya;
        let r1 = (y + 1).min(h - 1) - ya;
        let row = &mut out[(y - y0) * w..(y - y0 + 1) * w];
        for (x, g) in row.iter_mut().enumerate() {
            let mut mx = 0u8;
            let mut mn = 255u8;
            for r in r0..=r1 {
                mx = mx.max(hmax[r * w + x]);
                mn = mn.min(hmin[r * w + x]);
            }
            *g = mx - mn;
        }
    }
}

/// Union-find over run indices.
struct Forest {
    parent: Vec<u32>,
}

impl Forest {
    fn add(&mut self) -> u32 {
        let id = self.parent.len() as u32;
        self.parent.push(id);
        id
    }

    fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let p = self.parent[x as usize];
            self.parent[x as usize] = self.parent[p as usize];
            x = p;
        }
        x
    }

    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi as usize] = lo;
        }
    }
}

/// Finds the ink of a page and its connected components, reporting
/// progress after every band of at most [`REPORT_PIXELS`] pixels.
pub(crate) fn find(src: &Source, reporter: &mut Reporter) -> Result<Ink, PageError> {
    let (w, h) = (src.width, src.height);
    let band = (REPORT_PIXELS / w).max(1);
    let total = 2 * (w as u64) * (h as u64);
    let mut done = 0u64;
    let mut grad = Vec::new();
    // Pass 1: the gradient histogram, for the noise level.
    let mut histogram = [0u64; 256];
    let mut y0 = 0;
    while y0 < h {
        let y1 = (y0 + band).min(h);
        gradient(src, y0, y1, &mut grad);
        for &g in &grad {
            histogram[g as usize] += 1;
        }
        done += ((y1 - y0) * w) as u64;
        reporter.report(Progress::Layout { done, total })?;
        y0 = y1;
    }
    let half = (w as u64 * h as u64).div_ceil(2);
    let mut seen = 0u64;
    let mut median = 0u32;
    for (v, &c) in histogram.iter().enumerate() {
        seen += c;
        if seen >= half {
            median = v as u32;
            break;
        }
    }
    let threshold = (NOISE_FACTOR * median).clamp(MIN_THRESHOLD, MAX_THRESHOLD) as u8;
    // Pass 2: runs and their components.
    let mut runs: Vec<Run> = Vec::new();
    let mut forest = Forest { parent: Vec::new() };
    let mut prev = 0..0;
    y0 = 0;
    while y0 < h {
        let y1 = (y0 + band).min(h);
        gradient(src, y0, y1, &mut grad);
        for y in y0..y1 {
            let row = &grad[(y - y0) * w..(y - y0 + 1) * w];
            let start = runs.len();
            let mut x = 0;
            while x < w {
                if row[x] < threshold {
                    x += 1;
                    continue;
                }
                let x0 = x;
                while x < w && row[x] >= threshold {
                    x += 1;
                }
                runs.push(Run {
                    y: y as u32,
                    x0: x0 as u32,
                    x1: (x - 1) as u32,
                });
                forest.add();
            }
            // 8-connectivity with the previous row's runs.
            let mut p = prev.start;
            for c in start..runs.len() {
                let cur = runs[c];
                while p < prev.end && runs[p].x1 + 1 < cur.x0 {
                    p += 1;
                }
                let mut q = p;
                while q < prev.end && runs[q].x0 <= cur.x1 + 1 {
                    forest.union(q as u32, c as u32);
                    q += 1;
                }
            }
            prev = start..runs.len();
        }
        done += ((y1 - y0) * w) as u64;
        reporter.report(Progress::Layout { done, total })?;
        y0 = y1;
    }
    Ok(collect(runs, &mut forest))
}

/// Groups the runs by component.
fn collect(runs: Vec<Run>, forest: &mut Forest) -> Ink {
    let n = runs.len();
    let mut comp_of_root = vec![u32::MAX; n];
    let mut comps: Vec<Comp> = Vec::new();
    let mut label = vec![0u32; n];
    for (i, run) in runs.iter().enumerate() {
        let root = forest.find(i as u32) as usize;
        if comp_of_root[root] == u32::MAX {
            comp_of_root[root] = comps.len() as u32;
            comps.push(Comp {
                left: run.x0 as i32,
                top: run.y as i32,
                right: run.x1 as i32,
                bottom: run.y as i32,
                area: 0,
                first: 0,
                end: 0,
            });
        }
        let id = comp_of_root[root];
        label[i] = id;
        let c = &mut comps[id as usize];
        c.left = c.left.min(run.x0 as i32);
        c.right = c.right.max(run.x1 as i32);
        c.top = c.top.min(run.y as i32);
        c.bottom = c.bottom.max(run.y as i32);
        c.area += run.x1 - run.x0 + 1;
    }
    // Counting sort of the runs by component, keeping row order.
    let mut counts = vec![0u32; comps.len() + 1];
    for &l in &label {
        counts[l as usize + 1] += 1;
    }
    for i in 1..counts.len() {
        counts[i] += counts[i - 1];
    }
    for (i, c) in comps.iter_mut().enumerate() {
        c.first = counts[i];
        c.end = counts[i + 1];
    }
    let mut next = counts;
    let mut sorted = vec![Run { y: 0, x0: 0, x1: 0 }; n];
    for (i, run) in runs.into_iter().enumerate() {
        let slot = &mut next[label[i] as usize];
        sorted[*slot as usize] = run;
        *slot += 1;
    }
    Ink {
        comps,
        runs: sorted,
    }
}
