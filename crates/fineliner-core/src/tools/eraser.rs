//! The Eraser tool (spec §9.2).
//!
//! Clears pixels to transparent or paints the background color, using the same
//! brush rasterizer as the [`Pencil`](super::Pencil). Erasing to transparent
//! reduces the layer's alpha (respecting existing alpha); erasing to the
//! background color composites that color over the pixels.

use super::{src_over, Brush};
use crate::color::Color;
use crate::command::SetPixels;
use crate::document::Document;
use crate::geometry::Point;

/// What the eraser leaves behind (spec §9.2 Eraser "Mode").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EraserMode {
    /// Reduce alpha toward 0, revealing transparency.
    #[default]
    ToTransparent,
    /// Composite the background color over the erased pixels.
    ToBackground,
}

/// The Eraser tool — clears or repaints pixels under the brush (spec §9.2).
#[derive(Debug, Clone, Copy)]
pub struct Eraser {
    /// The brush tip (size, shape, hardness, opacity). The brush color is
    /// ignored except in [`EraserMode::ToBackground`], which uses `background`.
    pub brush: Brush,
    /// Erase behavior.
    pub mode: EraserMode,
    /// Background color used by [`EraserMode::ToBackground`].
    pub background: Color,
}

impl Eraser {
    /// Creates an eraser with the given brush and mode; background defaults to
    /// opaque white.
    pub fn new(brush: Brush, mode: EraserMode) -> Self {
        Self {
            brush,
            mode,
            background: Color::WHITE,
        }
    }

    /// Sets the background color used by [`EraserMode::ToBackground`].
    pub fn with_background(mut self, background: Color) -> Self {
        self.background = background;
        self
    }

    /// Rasterizes an erase stroke over `points` into a [`SetPixels`] command.
    ///
    /// Returns `None` if the stroke misses the canvas, the layer is invalid, or
    /// the effective strength is zero. The command captures the prior pixels for
    /// undo.
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        let (region, after) = match self.mode {
            EraserMode::ToTransparent => {
                let strength = self.brush.opacity;
                self.brush
                    .rasterize(layer_index, points, doc, strength, |eff, dst| {
                        erase_to_transparent(dst, eff)
                    })?
            }
            EraserMode::ToBackground => {
                let bg = self.background;
                let strength = self.brush.opacity * (bg.a as f32 / 255.0);
                self.brush
                    .rasterize(layer_index, points, doc, strength, |eff, dst| {
                        src_over(bg, eff, dst)
                    })?
            }
        };
        Some(SetPixels::new(layer_index, region, after).with_label("Erase"))
    }
}

/// Reduces `dst`'s alpha by the effective coverage `eff`, leaving RGB intact
/// (straight-alpha). `eff == 1.0` fully clears the pixel.
fn erase_to_transparent(dst: Color, eff: f32) -> Color {
    let a = dst.a as f32 / 255.0 * (1.0 - eff);
    Color::rgba(
        dst.r,
        dst.g,
        dst.b,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;

    /// A document whose single layer is filled opaque black.
    fn opaque_doc(w: u32, h: u32) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                doc.layers[0].pixels.set_pixel(x, y, Color::BLACK);
            }
        }
        doc
    }

    fn eraser(size: u32, opacity: f32, mode: EraserMode) -> Eraser {
        Eraser::new(Brush::new(size, Color::TRANSPARENT, opacity), mode)
    }

    #[test]
    fn erase_to_transparent_clears_opaque_pixel() {
        let mut doc = opaque_doc(20, 20);
        let mut cmd = eraser(8, 1.0, EraserMode::ToTransparent)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10).unwrap().a, 0);
    }

    #[test]
    fn erase_partial_opacity_halves_alpha() {
        let mut doc = opaque_doc(20, 20);
        let mut cmd = eraser(8, 0.5, EraserMode::ToTransparent)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let a = doc.layers[0].pixels.get_pixel(10, 10).unwrap().a;
        assert!((126..=129).contains(&a), "got {a}");
    }

    #[test]
    fn erase_to_background_paints_background_color() {
        let mut doc = opaque_doc(20, 20);
        let mut cmd = eraser(8, 1.0, EraserMode::ToBackground)
            .with_background(Color::WHITE)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), Some(Color::WHITE));
    }

    #[test]
    fn erase_is_undoable_to_original_pixels() {
        let mut doc = opaque_doc(20, 20);
        let mut cmd = eraser(8, 1.0, EraserMode::ToTransparent)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), Some(Color::BLACK));
    }

    #[test]
    fn erase_off_canvas_returns_none() {
        let doc = opaque_doc(10, 10);
        assert!(eraser(4, 1.0, EraserMode::ToTransparent)
            .stroke(0, &[Point::new(-50.0, -50.0)], &doc)
            .is_none());
    }
}
