//! Pixelate (docs/specs/effects-distort.md): one colour per square of a
//! grid that starts at the canvas origin.

use crate::blur::whole;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Which colour fills a Pixelate square.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PixelateColor {
    /// The premultiplied average of the square's pixels.
    #[default]
    Average,
    /// The square's centre pixel (offset `⌊(w − 1) / 2⌋, ⌊(h − 1) / 2⌋` of
    /// the square clipped to the image): no new colours.
    Centre,
}

/// Fills each `cell_size` square of a grid anchored at the canvas origin
/// with one colour. `origin` is the canvas position of the buffer's
/// top-left pixel (a trimmed Pixel-mode cel's position), so the squares
/// line up with the document's pixel grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pixelate {
    /// Square size in pixels, `2..=200`.
    pub cell_size: u32,
    /// The colour each square takes.
    pub color: PixelateColor,
    /// Canvas position of the buffer's top-left pixel.
    pub origin: (i32, i32),
}

impl Pixelate {
    /// Creates a pixelate effect for a buffer at the canvas origin (the
    /// cell size read as finite, rounded and clamped to `2..=200`).
    pub fn new(cell_size: f32, color: PixelateColor) -> Self {
        Self {
            cell_size: whole(cell_size, 2, 200),
            color,
            origin: (0, 0),
        }
    }

    /// The same effect for a buffer whose top-left pixel sits at canvas
    /// position `origin`.
    pub fn with_origin(self, origin: (i32, i32)) -> Self {
        Self { origin, ..self }
    }
}

/// The `[start, end)` spans of the grid along one axis of `len` pixels
/// whose first pixel sits at canvas coordinate `origin`.
fn spans(origin: i32, len: usize, cell: u32) -> Vec<(usize, usize)> {
    let cell = i64::from(cell);
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < len {
        let canvas = i64::from(origin) + start as i64;
        let end = (start as i64 + cell - canvas.rem_euclid(cell)).min(len as i64) as usize;
        out.push((start, end));
        start = end;
    }
    out
}

impl Effect for Pixelate {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let buf = src.to_premultiplied_f32();
        let mut out = data.to_vec();
        for &(y0, y1) in &spans(self.origin.1, h, self.cell_size) {
            for &(x0, x1) in &spans(self.origin.0, w, self.cell_size) {
                let colour: [u8; 4] = match self.color {
                    PixelateColor::Centre => {
                        let i = ((y0 + (y1 - y0 - 1) / 2) * w + x0 + (x1 - x0 - 1) / 2) * 4;
                        [data[i], data[i + 1], data[i + 2], data[i + 3]]
                    }
                    PixelateColor::Average => {
                        let mut sum = [0.0f64; 4];
                        for y in y0..y1 {
                            for x in x0..x1 {
                                for (c, s) in sum.iter_mut().enumerate() {
                                    *s += f64::from(buf[(y * w + x) * 4 + c]);
                                }
                            }
                        }
                        let n = ((y1 - y0) * (x1 - x0)) as f64;
                        let mean = sum.map(|s| (s / n) as f32);
                        let px = EffectImage::from_premultiplied_f32(1, 1, &mean).into_raw();
                        [px[0], px[1], px[2], px[3]]
                    }
                };
                for y in y0..y1 {
                    for x in x0..x1 {
                        out[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&colour);
                    }
                }
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out)
            .expect("same size as the source buffer")
    }

    fn scaled(&self, factor: f32) -> Self {
        let scale = |v: i32| (v as f32 * factor).round() as i32;
        Self {
            cell_size: ((self.cell_size as f32 * factor).round() as u32).max(2),
            color: self.color,
            origin: (scale(self.origin.0), scale(self.origin.1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_follow_the_canvas_grid() {
        assert_eq!(spans(0, 10, 4), vec![(0, 4), (4, 8), (8, 10)]);
        assert_eq!(spans(3, 6, 4), vec![(0, 1), (1, 5), (5, 6)]);
        assert_eq!(spans(-2, 5, 4), vec![(0, 2), (2, 5)]);
        assert_eq!(spans(0, 3, 200), vec![(0, 3)]);
    }
}
