//! The Move tool (spec §9.2).
//!
//! Translates the contents of a layer by an integer pixel offset. Pixels shifted
//! off the canvas are dropped; the area they vacate becomes transparent. The
//! tool produces one [`SetPixels`] over the whole layer on pointer up.
//!
//! Auto-select (picking the topmost non-transparent layer), the live ghost
//! preview, and arrow-key nudging are UI concerns layered on top of this
//! translation (spec §9.2 Move); the core only computes the moved pixels.

use crate::command::SetPixels;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;

/// The Move tool — translates a layer's pixel contents (spec §9.2).
#[derive(Debug, Clone, Copy, Default)]
pub struct Move;

impl Move {
    /// Translates layer `layer_index` by `(dx, dy)` pixels.
    ///
    /// Returns `None` if the offset is zero or the layer index is invalid. The
    /// command rewrites the entire layer, so undo restores the original content.
    pub fn translate(
        &self,
        layer_index: usize,
        dx: i32,
        dy: i32,
        doc: &Document,
    ) -> Option<SetPixels> {
        if dx == 0 && dy == 0 {
            return None;
        }
        let src = &doc.layers.get(layer_index)?.pixels;
        let w = src.width();
        let h = src.height();

        let mut after = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                if let Some(c) = src.get_pixel(x, y) {
                    after.set_pixel(nx as u32, ny as u32, c);
                }
            }
        }

        let region = Rect::new(0, 0, w, h);
        Some(SetPixels::new(layer_index, region, after).with_label("Move Layer"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::command::Command;

    fn doc_with(w: u32, h: u32, paint: impl Fn(&mut ImageBuffer)) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        paint(&mut doc.layers[0].pixels);
        doc
    }

    #[test]
    fn move_right_shifts_pixels() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        let mut cmd = Move.translate(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
    }

    #[test]
    fn move_clears_the_vacated_pixel() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        let mut cmd = Move.translate(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(0, 0),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn move_drops_pixels_shifted_off_canvas() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(3, 0, Color::WHITE));
        // Shifting the only painted pixel one past the edge loses it entirely.
        let mut cmd = Move.translate(0, 1, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        for x in 0..4 {
            assert_eq!(
                doc.layers[0].pixels.get_pixel(x, 0),
                Some(Color::TRANSPARENT)
            );
        }
    }

    #[test]
    fn move_zero_offset_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        assert!(Move.translate(0, 0, 0, &doc).is_none());
    }

    #[test]
    fn move_invalid_layer_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        assert!(Move.translate(9, 1, 0, &doc).is_none());
    }

    #[test]
    fn move_is_undoable_to_original_pixels() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        let mut cmd = Move.translate(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(
            doc.layers[0].pixels.get_pixel(2, 0),
            Some(Color::TRANSPARENT)
        );
    }
}
