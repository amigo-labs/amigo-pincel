//! Selection modifiers: Expand, Contract, Feather (spec §8.4).
//!
//! All three are separable neighborhood operations over the coverage mask.
//! Expand is a square (Chebyshev) dilation, Contract a square erosion, and
//! Feather a Gaussian-approximating triple box blur. They run as two passes
//! (horizontal then vertical), which is exact for min/max and a good Gaussian
//! approximation for the blur.

use super::SelectionMask;

/// Bounds a caller-supplied radius to the mask extent. Beyond that the window
/// already spans every row/column, and on 32-bit targets (wasm32) an unbounded
/// radius would overflow the `x + r` window arithmetic.
fn clamp_radius(radius: u32, w: usize, h: usize) -> usize {
    (radius as usize).min(w.max(h))
}

/// Maximum over a horizontal window of radius `r`, row by row.
fn horizontal_max(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
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

/// Minimum over a horizontal window of radius `r`, row by row.
fn horizontal_min(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
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

/// Transposes a `w × h` buffer into an `h × w` one so a horizontal pass can
/// serve as a vertical pass.
fn transpose(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            out[x * h + y] = src[y * w + x];
        }
    }
    out
}

/// One horizontal box-blur pass of radius `r` (clamped edges), row by row.
fn horizontal_box(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
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

impl SelectionMask {
    /// Grows the selection by `radius` pixels (square dilation; spec §8.4).
    pub fn expand(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_max(self.data(), w, h, r);
        let t = transpose(&pass, w, h);
        let t = horizontal_max(&t, h, w, r);
        self.data = transpose(&t, h, w);
    }

    /// Shrinks the selection by `radius` pixels (square erosion; spec §8.4).
    pub fn contract(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let r = clamp_radius(radius, w, h);
        let pass = horizontal_min(self.data(), w, h, r);
        let t = transpose(&pass, w, h);
        let t = horizontal_min(&t, h, w, r);
        self.data = transpose(&t, h, w);
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
            let t = transpose(&buf, w, h);
            let t = horizontal_box(&t, h, w, r);
            buf = transpose(&t, h, w);
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
}
