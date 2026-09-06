//! Shaped selections (spec §13.1, Fineliner parity): a canvas-sized
//! coverage mask plus the builders and set operations behind the
//! ellipse / lasso / polygon / magic-wand tools.
//!
//! The mask complements — not replaces — [`Sprite::selection`]: the rect
//! stays the bounding box every existing consumer reads, and a
//! [`SelectionMask`] is only attached when the shape is not a full
//! rectangle. Selection state remains transient editor state (not in the
//! file, not on the undo stack).
//!
//! [`Sprite::selection`]: crate::document::Sprite::selection

use std::collections::VecDeque;

use crate::document::{PixelBuffer, Rgba};
use crate::geometry::Rect;

/// How a new shape combines with the existing selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl SelectionMode {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
        }
    }

    /// Inverse of [`SelectionMode::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Replace, Self::Add, Self::Subtract, Self::Intersect]
            .into_iter()
            .find(|m| m.name() == name)
    }
}

/// Canvas-sized per-pixel selection coverage (`0` = unselected, `255` =
/// selected). Coordinates are sprite space; anything off-canvas is
/// unselected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionMask {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl SelectionMask {
    /// Empty (nothing selected) mask for a `width × height` canvas.
    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize],
        }
    }

    /// Everything selected.
    pub fn full(width: u32, height: u32) -> Self {
        let mut m = Self::empty(width, height);
        m.data.fill(255);
        m
    }

    /// Wrap raw coverage bytes (length must be `width * height`).
    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        (data.len() == width as usize * height as usize).then_some(Self {
            width,
            height,
            data,
        })
    }

    /// Axis-aligned rectangle (clipped to the canvas).
    pub fn rect(width: u32, height: u32, rect: Rect) -> Self {
        let mut m = Self::empty(width, height);
        let r = rect.intersect(Rect::new(0, 0, width, height));
        for y in 0..r.height {
            for x in 0..r.width {
                m.set(r.x as u32 + x, r.y as u32 + y, true);
            }
        }
        m
    }

    /// Filled ellipse inscribed in `rect` (pixel-centre test, so a 1×1
    /// rect selects exactly one pixel).
    pub fn ellipse(width: u32, height: u32, rect: Rect) -> Self {
        let mut m = Self::empty(width, height);
        if rect.is_empty() {
            return m;
        }
        let rx = f64::from(rect.width) / 2.0;
        let ry = f64::from(rect.height) / 2.0;
        let cx = f64::from(rect.x) + rx;
        let cy = f64::from(rect.y) + ry;
        let clip = rect.intersect(Rect::new(0, 0, width, height));
        for y in 0..clip.height {
            for x in 0..clip.width {
                let px = clip.x as u32 + x;
                let py = clip.y as u32 + y;
                let dx = (f64::from(px) + 0.5 - cx) / rx;
                let dy = (f64::from(py) + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    m.set(px, py, true);
                }
            }
        }
        m
    }

    /// Filled polygon through `points` (sprite space, implicitly closed),
    /// even-odd rule on pixel centres. Fewer than three points selects
    /// nothing.
    pub fn polygon(width: u32, height: u32, points: &[(i32, i32)]) -> Self {
        let mut m = Self::empty(width, height);
        if points.len() < 3 {
            return m;
        }
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let max_y = points
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(0)
            .min(height.saturating_sub(1) as i32);
        let mut xs: Vec<f64> = Vec::new();
        for py in min_y..=max_y {
            let yc = f64::from(py) + 0.5;
            xs.clear();
            for i in 0..points.len() {
                let (x0, y0) = points[i];
                let (x1, y1) = points[(i + 1) % points.len()];
                let (fx0, fy0, fx1, fy1) = (
                    f64::from(x0) + 0.5,
                    f64::from(y0) + 0.5,
                    f64::from(x1) + 0.5,
                    f64::from(y1) + 0.5,
                );
                if (fy0 <= yc && fy1 > yc) || (fy1 <= yc && fy0 > yc) {
                    let t = (yc - fy0) / (fy1 - fy0);
                    xs.push(fx0 + t * (fx1 - fx0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let start = (pair[0] - 0.5).ceil().max(0.0) as i64;
                let end = (pair[1] - 0.5).floor().min(f64::from(width) - 1.0) as i64;
                for px in start..=end {
                    m.set(px as u32, py as u32, true);
                }
            }
        }
        m
    }

    /// Magic wand: pixels of `buffer` (canvas-sized RGBA8) whose colour is
    /// within `tolerance` (0–255, max per-channel difference) of the seed
    /// pixel — the 4-connected region around the seed when `contiguous`,
    /// every matching pixel otherwise. An off-canvas seed selects nothing.
    pub fn wand(buffer: &PixelBuffer, seed: (i32, i32), tolerance: u8, contiguous: bool) -> Self {
        let (w, h) = (buffer.width, buffer.height);
        let mut m = Self::empty(w, h);
        if seed.0 < 0 || seed.1 < 0 || seed.0 >= w as i32 || seed.1 >= h as i32 {
            return m;
        }
        let px = |x: u32, y: u32| -> Rgba {
            let i = ((y * w + x) * 4) as usize;
            Rgba::new(
                buffer.data[i],
                buffer.data[i + 1],
                buffer.data[i + 2],
                buffer.data[i + 3],
            )
        };
        let target = px(seed.0 as u32, seed.1 as u32);
        let matches = |c: Rgba| -> bool {
            let d = |a: u8, b: u8| a.abs_diff(b);
            // Fully transparent pixels match each other regardless of RGB.
            if target.a == 0 && c.a == 0 {
                return true;
            }
            d(c.r, target.r) <= tolerance
                && d(c.g, target.g) <= tolerance
                && d(c.b, target.b) <= tolerance
                && d(c.a, target.a) <= tolerance
        };
        if !contiguous {
            for y in 0..h {
                for x in 0..w {
                    if matches(px(x, y)) {
                        m.set(x, y, true);
                    }
                }
            }
            return m;
        }
        let mut queue = VecDeque::new();
        queue.push_back((seed.0 as u32, seed.1 as u32));
        m.set(seed.0 as u32, seed.1 as u32, true);
        while let Some((x, y)) = queue.pop_front() {
            let neighbours = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in neighbours {
                if nx < w && ny < h && !m.contains(nx as i32, ny as i32) && matches(px(nx, ny)) {
                    m.set(nx, ny, true);
                    queue.push_back((nx, ny));
                }
            }
        }
        m
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw coverage bytes, row-major.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// `true` when the pixel at sprite `(x, y)` is selected.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        self.data[y as usize * self.width as usize + x as usize] != 0
    }

    fn set(&mut self, x: u32, y: u32, on: bool) {
        if x < self.width && y < self.height {
            self.data[y as usize * self.width as usize + x as usize] = if on { 255 } else { 0 };
        }
    }

    /// `true` when no pixel is selected.
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&v| v == 0)
    }

    /// Bounding box of the selected pixels, `None` when empty.
    pub fn bounds(&self) -> Option<Rect> {
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut any = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.data[y as usize * self.width as usize + x as usize] != 0 {
                    any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        any.then(|| {
            Rect::new(
                min_x as i32,
                min_y as i32,
                max_x - min_x + 1,
                max_y - min_y + 1,
            )
        })
    }

    /// `true` when every pixel inside [`bounds`](Self::bounds) is
    /// selected — the mask is a plain rectangle.
    pub fn is_rect(&self) -> bool {
        match self.bounds() {
            None => true,
            Some(b) => {
                for y in 0..b.height {
                    for x in 0..b.width {
                        if !self.contains(b.x + x as i32, b.y + y as i32) {
                            return false;
                        }
                    }
                }
                true
            }
        }
    }

    /// Combine `other` into `self` per `mode` (both canvas-sized).
    pub fn combine(&mut self, other: &SelectionMask, mode: SelectionMode) {
        debug_assert_eq!((self.width, self.height), (other.width, other.height));
        for (a, &b) in self.data.iter_mut().zip(other.data.iter()) {
            *a = match mode {
                SelectionMode::Replace => b,
                SelectionMode::Add => (*a).max(b),
                SelectionMode::Subtract => {
                    if b != 0 {
                        0
                    } else {
                        *a
                    }
                }
                SelectionMode::Intersect => (*a).min(b),
            };
        }
    }

    /// Select everything that was unselected and vice versa.
    pub fn invert(&mut self) {
        for v in &mut self.data {
            *v = if *v == 0 { 255 } else { 0 };
        }
    }

    /// Grow the selection by `radius` pixels (square structuring element,
    /// clipped to the canvas).
    pub fn expand(&self, radius: u32) -> Self {
        self.morph(radius, true)
    }

    /// Shrink the selection by `radius` pixels; the canvas edge counts as
    /// outside, so a full-canvas selection contracts inward.
    pub fn contract(&self, radius: u32) -> Self {
        self.morph(radius, false)
    }

    fn morph(&self, radius: u32, grow: bool) -> Self {
        if radius == 0 {
            return self.clone();
        }
        let r = radius as i32;
        let mut out = Self::empty(self.width, self.height);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let mut hit = !grow;
                'scan: for dy in -r..=r {
                    for dx in -r..=r {
                        let inside = self.contains(x + dx, y + dy);
                        if grow && inside {
                            hit = true;
                            break 'scan;
                        }
                        if !grow && !inside {
                            hit = false;
                            break 'scan;
                        }
                    }
                }
                out.set(x as u32, y as u32, hit);
            }
        }
        out
    }

    /// Copy shifted by `(dx, dy)`; pixels leaving the canvas are dropped.
    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        let mut out = Self::empty(self.width, self.height);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                if self.contains(x, y) {
                    let (nx, ny) = (i64::from(x) + i64::from(dx), i64::from(y) + i64::from(dy));
                    if nx >= 0
                        && ny >= 0
                        && nx < i64::from(self.width)
                        && ny < i64::from(self.height)
                    {
                        out.set(nx as u32, ny as u32, true);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ColorMode;

    fn selected(m: &SelectionMask) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for y in 0..m.height as i32 {
            for x in 0..m.width as i32 {
                if m.contains(x, y) {
                    v.push((x, y));
                }
            }
        }
        v
    }

    #[test]
    fn rect_mask_is_a_rect_with_matching_bounds() {
        let m = SelectionMask::rect(4, 4, Rect::new(1, 1, 2, 2));
        assert!(m.is_rect());
        assert_eq!(m.bounds(), Some(Rect::new(1, 1, 2, 2)));
        assert_eq!(selected(&m).len(), 4);
        assert!(!m.contains(0, 0));
        assert!(!m.contains(9, 9));
    }

    #[test]
    fn ellipse_drops_the_corners() {
        let m = SelectionMask::ellipse(4, 4, Rect::new(0, 0, 4, 4));
        assert!(!m.contains(0, 0));
        assert!(m.contains(1, 0));
        assert!(m.contains(0, 1));
        assert!(m.contains(1, 1));
        assert!(!m.is_rect());
        assert_eq!(m.bounds(), Some(Rect::new(0, 0, 4, 4)));
        let one = SelectionMask::ellipse(4, 4, Rect::new(2, 2, 1, 1));
        assert_eq!(selected(&one), vec![(2, 2)]);
    }

    #[test]
    fn polygon_fills_a_triangle_and_ignores_degenerate_input() {
        let m = SelectionMask::polygon(4, 4, &[(0, 0), (3, 0), (0, 3)]);
        assert!(m.contains(0, 0));
        assert!(m.contains(1, 1));
        assert!(!m.contains(3, 3));
        assert!(!m.contains(3, 1));
        assert!(SelectionMask::polygon(4, 4, &[(0, 0), (1, 1)]).is_empty());
    }

    fn buffer(w: u32, h: u32, px: &[[u8; 4]]) -> PixelBuffer {
        let mut b = PixelBuffer::empty(w, h, ColorMode::Rgba);
        b.data = px.iter().flatten().copied().collect();
        b
    }

    #[test]
    fn wand_contiguous_vs_global_and_tolerance() {
        const R: [u8; 4] = [255, 0, 0, 255];
        const B: [u8; 4] = [0, 0, 255, 255];
        const R2: [u8; 4] = [250, 0, 0, 255];
        // R B R2 / B R B
        let buf = buffer(3, 2, &[R, B, R2, B, R, B]);
        let c = SelectionMask::wand(&buf, (0, 0), 0, true);
        assert_eq!(selected(&c), vec![(0, 0)]);
        let g = SelectionMask::wand(&buf, (0, 0), 0, false);
        assert_eq!(selected(&g), vec![(0, 0), (1, 1)]);
        let t = SelectionMask::wand(&buf, (0, 0), 10, false);
        assert_eq!(selected(&t), vec![(0, 0), (2, 0), (1, 1)]);
        assert!(SelectionMask::wand(&buf, (7, 7), 0, true).is_empty());
    }

    #[test]
    fn wand_treats_all_transparent_pixels_as_one_colour() {
        let buf = buffer(2, 1, &[[0, 0, 0, 0], [9, 9, 9, 0]]);
        let m = SelectionMask::wand(&buf, (0, 0), 0, true);
        assert_eq!(selected(&m).len(), 2);
    }

    #[test]
    fn combine_modes() {
        let a = SelectionMask::rect(4, 1, Rect::new(0, 0, 2, 1));
        let b = SelectionMask::rect(4, 1, Rect::new(1, 0, 2, 1));
        let mut m = a.clone();
        m.combine(&b, SelectionMode::Add);
        assert_eq!(selected(&m), vec![(0, 0), (1, 0), (2, 0)]);
        let mut m = a.clone();
        m.combine(&b, SelectionMode::Subtract);
        assert_eq!(selected(&m), vec![(0, 0)]);
        let mut m = a.clone();
        m.combine(&b, SelectionMode::Intersect);
        assert_eq!(selected(&m), vec![(1, 0)]);
        let mut m = a;
        m.combine(&b, SelectionMode::Replace);
        assert_eq!(selected(&m), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn invert_expand_contract_translate() {
        let mut m = SelectionMask::rect(5, 5, Rect::new(2, 2, 1, 1));
        let e = m.expand(1);
        assert_eq!(e.bounds(), Some(Rect::new(1, 1, 3, 3)));
        assert!(e.is_rect());
        assert_eq!(e.contract(1), m);
        assert!(SelectionMask::full(3, 3).contract(1).bounds() == Some(Rect::new(1, 1, 1, 1)));
        m.invert();
        assert_eq!(selected(&m).len(), 24);
        assert!(!m.contains(2, 2));
        let t = SelectionMask::rect(3, 1, Rect::new(0, 0, 2, 1)).translated(2, 0);
        assert_eq!(selected(&t), vec![(2, 0)]);
    }

    #[test]
    fn mode_names_round_trip() {
        for m in [
            SelectionMode::Replace,
            SelectionMode::Add,
            SelectionMode::Subtract,
            SelectionMode::Intersect,
        ] {
            assert_eq!(SelectionMode::from_name(m.name()), Some(m));
        }
        assert_eq!(SelectionMode::from_name("xor"), None);
    }
}
