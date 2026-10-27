//! The Move tool (spec §9.2).
//!
//! Translates the contents of a layer by an integer pixel offset. Pixels shifted
//! off the canvas are dropped; the area they vacate becomes transparent. The
//! tool produces one [`SetPixels`] over the whole layer on pointer up.
//!
//! Auto-select (picking the topmost non-transparent layer), the live ghost
//! preview, and arrow-key nudging are UI concerns layered on top of this
//! translation (spec §9.2 Move); the core only computes the moved pixels.

use crate::color::Color;
use crate::command::SetPixels;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;
use crate::tools::src_over;

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

impl Move {
    /// Translates only the selected pixels of `layer_index` by `(dx, dy)`:
    /// the selection's content is lifted, the area it vacates becomes
    /// transparent, and it is composited back at the offset over whatever
    /// was there. Pixels pushed off the canvas are dropped.
    ///
    /// Feathered edges carry their coverage both ways — a half-selected
    /// pixel leaves half of itself behind and arrives half-transparent —
    /// so the two halves still add up to the original.
    ///
    /// Without a selection this is [`Move::translate`]: the whole layer
    /// moves, as paint.net's Move Selected Pixels does. Returns `None` for
    /// a zero offset, an invalid layer, or an empty selection.
    pub fn translate_selection(
        &self,
        layer_index: usize,
        dx: i32,
        dy: i32,
        doc: &Document,
    ) -> Option<SetPixels> {
        let Some(mask) = doc.selection.as_ref() else {
            return self.translate(layer_index, dx, dy, doc);
        };
        if (dx == 0 && dy == 0) || mask.is_empty() {
            return None;
        }
        let src = &doc.layers.get(layer_index)?.pixels;
        let w = src.width();
        let h = src.height();

        // Lift: scale the alpha of every selected pixel down by its coverage.
        let mut after = src.clone();
        for y in 0..h {
            for x in 0..w {
                let cov = u32::from(mask.coverage(x, y));
                if cov == 0 {
                    continue;
                }
                let Some(c) = src.get_pixel(x, y) else {
                    continue;
                };
                let a = (((u32::from(c.a) * (255 - cov)) + 127) / 255) as u8;
                // Normalized to (0,0,0,0) when fully lifted, as the eraser
                // does, so no stale color lingers behind zero alpha.
                let left = if a == 0 {
                    Color::TRANSPARENT
                } else {
                    Color::rgba(c.r, c.g, c.b, a)
                };
                after.set_pixel(x, y, left);
            }
        }

        // Drop: composite what was lifted at the offset.
        for y in 0..h {
            for x in 0..w {
                let cov = mask.coverage(x, y);
                if cov == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let Some(c) = src.get_pixel(x, y) else {
                    continue;
                };
                let sa = f32::from(c.a) / 255.0 * (f32::from(cov) / 255.0);
                if sa <= 0.0 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                let dst = after.get_pixel(nx, ny).unwrap_or(Color::TRANSPARENT);
                after.set_pixel(nx, ny, src_over(c, sa, dst));
            }
        }

        let region = Rect::new(0, 0, w, h);
        Some(SetPixels::new(layer_index, region, after).with_label("Move Selection"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::selection::SelectionMask;

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

    fn mask_rect(w: u32, h: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> SelectionMask {
        let mut m = SelectionMask::new_empty(w, h);
        for y in y0..y1 {
            for x in x0..x1 {
                m.set(x, y, 255);
            }
        }
        m
    }

    #[test]
    fn move_selection_takes_only_the_selected_pixels() {
        let mut doc = doc_with(4, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::BLACK);
        });
        doc.selection = Some(mask_rect(4, 1, 0, 0, 1, 1));
        let mut cmd = Move.translate_selection(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        // The selected pixel moved and left transparency behind …
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
        assert_eq!(
            doc.layers[0].pixels.get_pixel(0, 0),
            Some(Color::TRANSPARENT)
        );
        // … the unselected one stayed put.
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::BLACK));
    }

    #[test]
    fn move_selection_composites_over_what_is_already_there() {
        let mut doc = doc_with(2, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::BLACK);
        });
        doc.selection = Some(mask_rect(2, 1, 0, 0, 1, 1));
        let mut cmd = Move.translate_selection(0, 1, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        // Opaque white lands on black and wins.
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::WHITE));
    }

    #[test]
    fn move_selection_splits_a_half_covered_pixel() {
        let mut doc = doc_with(3, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        let mut mask = SelectionMask::new_empty(3, 1);
        mask.set(0, 0, 128);
        doc.selection = Some(mask);
        let mut cmd = Move.translate_selection(0, 1, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let left = doc.layers[0].pixels.get_pixel(0, 0).unwrap();
        let right = doc.layers[0].pixels.get_pixel(1, 0).unwrap();
        // Half stays, half arrives; together they are the original alpha.
        assert_eq!(u32::from(left.a) + u32::from(right.a), 255);
    }

    #[test]
    fn move_selection_without_a_selection_moves_the_layer() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        let mut cmd = Move.translate_selection(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
    }

    #[test]
    fn move_selection_with_an_empty_selection_returns_none() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        doc.selection = Some(SelectionMask::new_empty(4, 1));
        assert!(Move.translate_selection(0, 2, 0, &doc).is_none());
    }

    #[test]
    fn move_selection_is_undoable_to_original_pixels() {
        let mut doc = doc_with(4, 1, |b| b.set_pixel(0, 0, Color::WHITE));
        doc.selection = Some(mask_rect(4, 1, 0, 0, 1, 1));
        let mut cmd = Move.translate_selection(0, 2, 0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
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
