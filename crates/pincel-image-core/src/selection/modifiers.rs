//! Selection modifiers: Expand, Contract, Feather (spec §8.4).
//!
//! All three are separable neighborhood operations over the coverage mask.
//! Expand is a square (Chebyshev) dilation, Contract a square erosion, and
//! Feather a Gaussian-approximating triple box blur. They run as two passes
//! (horizontal then vertical), which is exact for min/max and a good Gaussian
//! approximation for the blur. Every pass costs O(1) per pixel whatever the
//! radius: running sums for the box blur, van Herk / Gil-Werman block
//! extremes for min/max.

use super::SelectionMask;

/// Bounds a caller-supplied radius to the mask extent. Beyond that the window
/// already spans every row/column, and on 32-bit targets (wasm32) an unbounded
/// radius would overflow the `x + r` window arithmetic.
fn clamp_radius(radius: u32, w: usize, h: usize) -> usize {
    (radius as usize).min(w.max(h))
}

/// Maximum (`pick = u8::max`, `identity = 0`) or minimum (`u8::min`, `255`)
/// over a horizontal window of radius `r`, row by row. Pixels past the row
/// ends are left out of the window.
///
/// van Herk / Gil-Werman: the row, padded with `r` identity pixels on each
/// side, is cut into blocks of the window length `2r + 1`; a running
/// extreme from each block's start (`fwd`) and from its end (`bwd`) give
/// any window, which spans at most two blocks, as `bwd[x] ⊕ fwd[x + 2r]`.
fn horizontal_extreme(
    src: &[u8],
    w: usize,
    h: usize,
    r: usize,
    identity: u8,
    pick: impl Fn(u8, u8) -> u8 + Copy,
) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    if w == 0 || h == 0 {
        return out;
    }
    // A window wider than the row reaches no further pixels.
    let r = r.min(w - 1);
    let k = 2 * r + 1;
    let mut padded = vec![identity; w + 2 * r];
    let mut fwd = vec![identity; padded.len()];
    let mut bwd = vec![identity; padded.len()];
    for (row, out_row) in src.chunks_exact(w).zip(out.chunks_exact_mut(w)) {
        padded[r..r + w].copy_from_slice(row);
        for ((f, b), p) in fwd
            .chunks_mut(k)
            .zip(bwd.chunks_mut(k))
            .zip(padded.chunks(k))
        {
            let mut m = identity;
            for (f, &p) in f.iter_mut().zip(p) {
                m = pick(m, p);
                *f = m;
            }
            let mut m = identity;
            for (b, &p) in b.iter_mut().zip(p).rev() {
                m = pick(m, p);
                *b = m;
            }
        }
        for ((o, &b), &f) in out_row.iter_mut().zip(&bwd).zip(&fwd[2 * r..]) {
            *o = pick(b, f);
        }
    }
    out
}

/// [`horizontal_extreme`] down the columns: the window of radius `r` spans
/// rows, and rows past the top or bottom edge are left out of it. The
/// block extremes run over whole rows at once, so no transpose is needed.
fn vertical_extreme(
    src: &[u8],
    w: usize,
    h: usize,
    r: usize,
    identity: u8,
    pick: impl Fn(u8, u8) -> u8 + Copy,
) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    if w == 0 || h == 0 {
        return out;
    }
    let r = r.min(h - 1);
    let k = 2 * r + 1;
    let n = h + 2 * r;
    let identity_row = vec![identity; w];
    // Padded row `i`: an identity row above and below the mask.
    let row = |i: usize| -> &[u8] {
        if i < r || i >= r + h {
            &identity_row
        } else {
            &src[(i - r) * w..][..w]
        }
    };
    let combine = |dst: &mut [u8], a: &[u8], b: &[u8]| {
        for ((d, &a), &b) in dst.iter_mut().zip(a).zip(b) {
            *d = pick(a, b);
        }
    };
    let mut fwd = vec![identity; n * w];
    for i in 0..n {
        let (done, rest) = fwd.split_at_mut(i * w);
        let cur = &mut rest[..w];
        if i % k == 0 {
            cur.copy_from_slice(row(i));
        } else {
            combine(cur, &done[(i - 1) * w..], row(i));
        }
    }
    let mut bwd = vec![identity; n * w];
    for i in (0..n).rev() {
        let (head, done) = bwd.split_at_mut((i + 1) * w);
        let cur = &mut head[i * w..];
        if (i + 1) % k == 0 || i + 1 == n {
            cur.copy_from_slice(row(i));
        } else {
            combine(cur, &done[..w], row(i));
        }
    }
    for (y, out_row) in out.chunks_exact_mut(w).enumerate() {
        combine(out_row, &bwd[y * w..][..w], &fwd[(y + 2 * r) * w..][..w]);
    }
    out
}

/// The size of the clamped window of radius `r` around index `i` of `n`.
fn window_count(i: usize, n: usize, r: usize) -> u32 {
    let lo = i.saturating_sub(r);
    let hi = (i + r).min(n - 1);
    (hi - lo + 1) as u32
}

/// One horizontal box-blur pass of radius `r` (clamped edges), row by row:
/// the rounded mean of the pixels inside the row, from a running sum.
fn horizontal_box(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    if w == 0 || h == 0 {
        return out;
    }
    let r = r.min(w - 1);
    for (row, out_row) in src.chunks_exact(w).zip(out.chunks_exact_mut(w)) {
        let mut sum: u32 = row[..=r].iter().map(|&v| u32::from(v)).sum();
        for (x, o) in out_row.iter_mut().enumerate() {
            let count = window_count(x, w, r);
            *o = ((sum + count / 2) / count) as u8;
            // Slide the window from `x` to `x + 1`.
            if let Some(&v) = row.get(x + 1 + r) {
                sum += u32::from(v);
            }
            if x >= r {
                sum -= u32::from(row[x - r]);
            }
        }
    }
    out
}

/// One vertical box-blur pass of radius `r` (clamped edges), column by
/// column, with the same rounding as [`horizontal_box`]: a running sum per
/// column, slid one row at a time.
fn vertical_box(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    if w == 0 || h == 0 {
        return out;
    }
    let r = r.min(h - 1);
    let row = |y: usize| &src[y * w..][..w];
    let mut sums = vec![0u32; w];
    for y in 0..=r {
        for (s, &v) in sums.iter_mut().zip(row(y)) {
            *s += u32::from(v);
        }
    }
    for (y, out_row) in out.chunks_exact_mut(w).enumerate() {
        let count = window_count(y, h, r);
        for (o, &s) in out_row.iter_mut().zip(&sums) {
            *o = ((s + count / 2) / count) as u8;
        }
        // Slide the windows from row `y` to row `y + 1`.
        if y + 1 + r < h {
            for (s, &v) in sums.iter_mut().zip(row(y + 1 + r)) {
                *s += u32::from(v);
            }
        }
        if y >= r {
            for (s, &v) in sums.iter_mut().zip(row(y - r)) {
                *s -= u32::from(v);
            }
        }
    }
    out
}

impl SelectionMask {
    /// Grows the selection by `radius` pixels (square dilation; spec §8.4).
    pub fn expand(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_extreme(self.data(), w, h, r, 0, u8::max);
        self.data = vertical_extreme(&pass, w, h, r, 0, u8::max);
    }

    /// Shrinks the selection by `radius` pixels (square erosion; spec §8.4).
    pub fn contract(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_extreme(self.data(), w, h, r, 255, u8::min);
        self.data = vertical_extreme(&pass, w, h, r, 255, u8::min);
    }

    /// Softens the selection edges by blurring the mask (spec §8.4 Feather).
    ///
    /// Approximates a Gaussian with three separable box-blur passes of the given
    /// `radius`; edge pixels gain partial coverage (`1..=254`).
    pub fn feather(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let r = clamp_radius(radius, w, h);
        let mut buf = self.data().to_vec();
        for _ in 0..3 {
            buf = horizontal_box(&buf, w, h, r);
            buf = vertical_box(&buf, w, h, r);
        }
        self.data = buf;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    #[test]
    fn expand_grows_selection() {
        let mut m = SelectionMask::rectangle(20, 20, Rect::new(8, 8, 4, 4));
        let before = m.selected_count();
        m.expand(2);
        assert!(m.selected_count() > before);
        // A pixel two to the left of the original edge is now selected.
        assert_eq!(m.coverage(6, 9), 255);
    }

    #[test]
    fn contract_shrinks_selection() {
        let mut m = SelectionMask::rectangle(20, 20, Rect::new(8, 8, 6, 6));
        let before = m.selected_count();
        m.contract(1);
        assert!(m.selected_count() < before);
        // The original border pixel is eroded away.
        assert_eq!(m.coverage(8, 8), 0);
        // The interior survives.
        assert_eq!(m.coverage(10, 10), 255);
    }

    #[test]
    fn expand_then_contract_is_near_identity_for_solid_block() {
        let mut m = SelectionMask::rectangle(20, 20, Rect::new(6, 6, 8, 8));
        let original = m.clone();
        m.expand(2);
        m.contract(2);
        // A morphological close of a solid block restores it exactly.
        assert_eq!(m.selected_count(), original.selected_count());
    }

    #[test]
    fn expand_with_huge_radius_selects_everything_without_overflow() {
        let mut m = SelectionMask::rectangle(20, 20, Rect::new(8, 8, 4, 4));
        m.expand(u32::MAX);
        assert_eq!(m.selected_count(), 20 * 20);
    }

    #[test]
    fn feather_introduces_partial_coverage_at_edges() {
        let mut m = SelectionMask::rectangle(20, 20, Rect::new(6, 6, 8, 8));
        m.feather(2);
        // Some pixel now has partial (non-binary) coverage.
        let mut partial = false;
        for y in 0..20 {
            for x in 0..20 {
                let c = m.coverage(x, y);
                if c > 0 && c < 255 {
                    partial = true;
                }
            }
        }
        assert!(partial);
    }

    #[test]
    fn feather_keeps_deep_interior_fully_selected() {
        let mut m = SelectionMask::rectangle(40, 40, Rect::new(10, 10, 20, 20));
        m.feather(2);
        // Well inside the block, coverage stays at the maximum.
        assert_eq!(m.coverage(20, 20), 255);
    }

    // ---- The O(r)-per-pixel passes before the running sums and the van
    // Herk / Gil-Werman extremes, kept as the reference the fast passes must
    // reproduce byte for byte.

    fn horizontal_max_reference(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let lo = x.saturating_sub(r);
                let hi = (x + r).min(w - 1);
                let mut m = 0u8;
                for sx in lo..=hi {
                    m = m.max(src[row + sx]);
                }
                out[row + x] = m;
            }
        }
        out
    }

    fn horizontal_min_reference(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let lo = x.saturating_sub(r);
                let hi = (x + r).min(w - 1);
                let mut m = 255u8;
                for sx in lo..=hi {
                    m = m.min(src[row + sx]);
                }
                out[row + x] = m;
            }
        }
        out
    }

    fn transpose_reference(src: &[u8], w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for y in 0..h {
            for x in 0..w {
                out[x * h + y] = src[y * w + x];
            }
        }
        out
    }

    fn horizontal_box_reference(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let lo = x.saturating_sub(r);
                let hi = (x + r).min(w - 1);
                let mut sum = 0u32;
                for sx in lo..=hi {
                    sum += src[row + sx] as u32;
                }
                let count = (hi - lo + 1) as u32;
                out[row + x] = ((sum + count / 2) / count) as u8;
            }
        }
        out
    }

    fn expand_reference(m: &mut SelectionMask, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (m.width() as usize, m.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_max_reference(m.data(), w, h, r);
        let t = transpose_reference(&pass, w, h);
        let t = horizontal_max_reference(&t, h, w, r);
        m.data = transpose_reference(&t, h, w);
    }

    fn contract_reference(m: &mut SelectionMask, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (m.width() as usize, m.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_min_reference(m.data(), w, h, r);
        let t = transpose_reference(&pass, w, h);
        let t = horizontal_min_reference(&t, h, w, r);
        m.data = transpose_reference(&t, h, w);
    }

    fn feather_reference(m: &mut SelectionMask, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (m.width() as usize, m.height() as usize);
        let r = clamp_radius(radius, w, h);
        let mut buf = m.data().to_vec();
        for _ in 0..3 {
            buf = horizontal_box_reference(&buf, w, h, r);
            let t = transpose_reference(&buf, w, h);
            let t = horizontal_box_reference(&t, h, w, r);
            buf = transpose_reference(&t, h, w);
        }
        m.data = buf;
    }

    /// Deterministic pseudo-random masks: one with arbitrary coverage, one
    /// binary, one sparse, so min, max and the rounding all see ties,
    /// plateaus and partial values.
    fn random_masks(w: u32, h: u32, seed: u64) -> Vec<SelectionMask> {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        };
        let n = w as usize * h as usize;
        let any: Vec<u8> = (0..n).map(|_| next()).collect();
        let binary: Vec<u8> = (0..n).map(|_| if next() < 128 { 0 } else { 255 }).collect();
        let sparse: Vec<u8> = (0..n)
            .map(|_| if next() < 20 { next() } else { 0 })
            .collect();
        [any, binary, sparse]
            .into_iter()
            .map(|d| SelectionMask::from_raw(w, h, d).expect("mask size"))
            .collect()
    }

    /// Mask sizes from 1×1 to wider and taller than radius 40, with sides
    /// below, around and above every radius the test runs.
    const SIZES: [(u32, u32); 10] = [
        (1, 1),
        (1, 9),
        (9, 1),
        (2, 3),
        (5, 5),
        (13, 9),
        (37, 23),
        (64, 3),
        (3, 64),
        (90, 41),
    ];

    type Modifier = fn(&mut SelectionMask, u32);

    #[test]
    fn modifiers_match_the_window_scan_for_every_radius_up_to_40() {
        let ops: [(&str, Modifier, Modifier); 3] = [
            ("expand", SelectionMask::expand, expand_reference),
            ("contract", SelectionMask::contract, contract_reference),
            ("feather", SelectionMask::feather, feather_reference),
        ];
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            for mask in random_masks(w, h, i as u64 + 1) {
                for radius in (0..=40).chain([1000, u32::MAX]) {
                    for (name, fast, reference) in ops {
                        let (mut a, mut b) = (mask.clone(), mask.clone());
                        fast(&mut a, radius);
                        reference(&mut b, radius);
                        assert!(
                            a.data() == b.data(),
                            "{name} r={radius} on {w}x{h} mask {i}"
                        );
                    }
                }
            }
        }
    }
}
