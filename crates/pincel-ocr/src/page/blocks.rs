//! Text blocks and reading order (docs/specs/ocr-page.md, "Text block",
//! "Column gap", "Reading order").
//!
//! A block is a chain of lines, each the only line directly below the one
//! before and overlapping it horizontally, with a vertical gap of at most
//! one line height. Blocks are then ordered by the recursive cut: a region
//! with a column gap splits at it into left and right; otherwise at its
//! widest full-width horizontal gap into top and bottom.

use super::lines::{DBox, Line};

/// A block under construction: its lines in order and its box.
struct Block {
    lines: Vec<usize>,
    bbox: DBox,
}

fn same_row(a: &DBox, b: &DBox) -> bool {
    a.v1.min(b.v1) - a.v0.max(b.v0) >= 0.5 * a.height().min(b.height())
}

fn overlap_u(a: &DBox, b: &DBox) -> bool {
    a.u0 < b.u1 && b.u0 < a.u1
}

/// The line height of the block rule: the box height, at least two
/// x-heights (spec Decisions).
fn line_height(l: &Line) -> f64 {
    l.bbox.height().max(2.0 * l.xh)
}

/// Lines sorted for the neighbour search: by top edge ascending and by
/// bottom edge descending.
struct Sorted {
    by_top: Vec<usize>,
    by_bottom: Vec<usize>,
    max_height: f64,
}

impl Sorted {
    fn new(lines: &[Line]) -> Self {
        let mut by_top: Vec<usize> = (0..lines.len()).collect();
        by_top.sort_by(|&p, &q| lines[p].bbox.v0.total_cmp(&lines[q].bbox.v0));
        let mut by_bottom: Vec<usize> = (0..lines.len()).collect();
        by_bottom.sort_by(|&p, &q| lines[q].bbox.v1.total_cmp(&lines[p].bbox.v1));
        let max_height = lines.iter().map(|l| l.bbox.height()).fold(0.0, f64::max);
        Self {
            by_top,
            by_bottom,
            max_height,
        }
    }
}

/// The lines directly below (`down`) or above line `i`: the nearest line
/// overlapping it horizontally and the lines on that line's row.
fn neighbours(lines: &[Line], sorted: &Sorted, i: usize, down: bool) -> Vec<usize> {
    let a = lines[i].bbox;
    let center = a.v0 + a.v1;
    let valid = |j: usize| {
        let b = lines[j].bbox;
        j != i
            && overlap_u(&a, &b)
            && !same_row(&a, &b)
            && if down {
                b.v0 + b.v1 > center
            } else {
                b.v0 + b.v1 < center
            }
    };
    let mut out: Vec<usize> = Vec::new();
    let mut near: Option<DBox> = None;
    if down {
        // A line whose centre is below this one's starts below
        // `a.v0 - max_height`.
        let order = &sorted.by_top;
        let from = order.partition_point(|&j| lines[j].bbox.v0 < a.v0 - sorted.max_height);
        for &j in &order[from..] {
            let b = lines[j].bbox;
            if near.is_some_and(|n| b.v0 >= n.v1) {
                break;
            }
            if valid(j) && near.is_none_or(|n| same_row(&n, &b)) {
                near.get_or_insert(b);
                out.push(j);
            }
        }
    } else {
        let order = &sorted.by_bottom;
        let from = order.partition_point(|&j| lines[j].bbox.v1 > a.v1 + sorted.max_height);
        for &j in &order[from..] {
            let b = lines[j].bbox;
            if near.is_some_and(|n| b.v1 <= n.v0) {
                break;
            }
            if valid(j) && near.is_none_or(|n| same_row(&n, &b)) {
                near.get_or_insert(b);
                out.push(j);
            }
        }
    }
    out.sort_unstable();
    out
}

fn chains(lines: &[Line]) -> Vec<Block> {
    let n = lines.len();
    let sorted = Sorted::new(lines);
    let mut next = vec![usize::MAX; n];
    let mut has_prev = vec![false; n];
    for i in 0..n {
        let below = neighbours(lines, &sorted, i, true);
        if below.len() != 1 {
            continue;
        }
        let j = below[0];
        if neighbours(lines, &sorted, j, false) != [i] {
            continue;
        }
        let gap = lines[j].bbox.v0 - lines[i].bbox.v1;
        if gap <= line_height(&lines[i]).max(line_height(&lines[j])) {
            next[i] = j;
            has_prev[j] = true;
        }
    }
    let mut blocks = Vec::new();
    for (i, _) in has_prev.iter().enumerate().filter(|(_, p)| !**p) {
        let mut chain = vec![i];
        let mut k = i;
        while next[k] != usize::MAX && chain.len() <= n {
            k = next[k];
            chain.push(k);
        }
        let bbox = chain
            .iter()
            .map(|&k| lines[k].bbox)
            .reduce(|a, b| DBox {
                u0: a.u0.min(b.u0),
                u1: a.u1.max(b.u1),
                v0: a.v0.min(b.v0),
                v1: a.v1.max(b.v1),
            })
            .expect("a chain has a line");
        blocks.push(Block { lines: chain, bbox });
    }
    blocks
}

/// Gaps between the union of intervals `[lo, hi)`: `(start, end)`.
fn gaps(mut intervals: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let Some(&(_, first)) = intervals.first() else {
        return out;
    };
    let mut reach = first;
    for &(lo, hi) in &intervals[1..] {
        if lo > reach {
            out.push((reach, lo));
        }
        reach = reach.max(hi);
    }
    out
}

fn median_xh(lines: &[Line], blocks: &[&Block]) -> f64 {
    let mut xs: Vec<f64> = blocks
        .iter()
        .flat_map(|b| b.lines.iter().map(|&l| lines[l].xh))
        .collect();
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_by(f64::total_cmp);
    let m = xs.len() / 2;
    if xs.len() % 2 == 1 {
        xs[m]
    } else {
        (xs[m - 1] + xs[m]) / 2.0
    }
}

fn cut<'a>(lines: &[Line], blocks: Vec<&'a Block>, out: &mut Vec<&'a Block>) {
    if blocks.len() <= 1 {
        out.extend(blocks);
        return;
    }
    for (lo, hi) in gaps(blocks.iter().map(|b| (b.bbox.u0, b.bbox.u1)).collect()) {
        let (left, right): (Vec<&Block>, Vec<&Block>) =
            blocks.iter().partition(|b| b.bbox.u1 <= lo);
        let both: Vec<&Block> = left.iter().chain(&right).copied().collect();
        if hi - lo >= 2.0 * median_xh(lines, &both) {
            cut(lines, left, out);
            cut(lines, right, out);
            return;
        }
    }
    let rows = gaps(blocks.iter().map(|b| (b.bbox.v0, b.bbox.v1)).collect());
    if let Some(&(lo, _)) = rows
        .iter()
        .reduce(|best, g| if g.1 - g.0 > best.1 - best.0 { g } else { best })
    {
        let (top, bottom): (Vec<&Block>, Vec<&Block>) =
            blocks.iter().partition(|b| b.bbox.v1 <= lo);
        cut(lines, top, out);
        cut(lines, bottom, out);
        return;
    }
    let mut rest = blocks;
    rest.sort_by(|a, b| {
        a.bbox
            .v0
            .total_cmp(&b.bbox.v0)
            .then(a.bbox.u0.total_cmp(&b.bbox.u0))
    });
    out.extend(rest);
}

/// The text blocks in reading order, each as its line indices top to
/// bottom.
pub(crate) fn order(lines: &[Line]) -> Vec<Vec<usize>> {
    let blocks = chains(lines);
    let mut out = Vec::new();
    cut(lines, blocks.iter().collect(), &mut out);
    out.into_iter().map(|b| b.lines.clone()).collect()
}
