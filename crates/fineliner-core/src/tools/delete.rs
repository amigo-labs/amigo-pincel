//! Delete / Clear — erases the selected pixels of a layer (Edit ▸ Clear,
//! Delete key; spec §8.1 "no prior selection = everything selected").
//!
//! Emits one [`SetPixels`] over the whole layer, like the Move tool: undo
//! restores the original content exactly. Partial (feathered) selection
//! coverage reduces alpha proportionally instead of cutting a hard edge.

use crate::color::Color;
use crate::command::SetPixels;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;

/// Builds a command erasing the selected pixels of layer `layer_index` to
/// transparent.
///
/// With no active selection the whole layer is cleared. Returns `None` if the
/// layer index is invalid or the selection covers nothing (avoiding a no-op
/// undo step).
pub fn delete_selection(layer_index: usize, doc: &Document) -> Option<SetPixels> {
    let src = &doc.layers.get(layer_index)?.pixels;
    let (w, h) = (src.width(), src.height());

    let after = match doc.selection.as_ref() {
        None => ImageBuffer::new_transparent(w, h),
        Some(sel) if sel.is_empty() => return None,
        Some(sel) => {
            let mut out = src.clone();
            for y in 0..h {
                for x in 0..w {
                    let cov = sel.coverage(x, y);
                    if cov == 0 {
                        continue;
                    }
                    let Some(mut c) = out.get_pixel(x, y) else {
                        continue;
                    };
                    let keep = 1.0 - cov as f32 / 255.0;
                    c.a = (c.a as f32 * keep).round() as u8;
                    // Fully erased pixels are normalized to (0,0,0,0) so no
                    // stale color data lingers behind zero alpha.
                    if c.a == 0 {
                        c = Color::TRANSPARENT;
                    }
                    out.set_pixel(x, y, c);
                }
            }
            out
        }
    };

    let region = Rect::new(0, 0, w, h);
    Some(SetPixels::new(layer_index, region, after).with_label("Delete Selection"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::command::Command;
    use crate::selection::SelectionMask;

    fn solid_doc(w: u32, h: u32, c: Color) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                doc.layers[0].pixels.set_pixel(x, y, c);
            }
        }
        doc
    }

    #[test]
    fn delete_without_selection_clears_whole_layer_and_undoes() {
        let mut doc = solid_doc(4, 4, Color::WHITE);
        let mut cmd = delete_selection(0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(2, 2),
            Some(Color::TRANSPARENT)
        );
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 2), Some(Color::WHITE));
    }

    #[test]
    fn delete_clears_inside_selection_only() {
        let mut doc = solid_doc(8, 8, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(8, 8, Rect::new(0, 0, 4, 8)));
        let mut cmd = delete_selection(0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(1, 1),
            Some(Color::TRANSPARENT)
        );
        assert_eq!(doc.layers[0].pixels.get_pixel(6, 1), Some(Color::WHITE));
    }

    #[test]
    fn delete_with_partial_coverage_reduces_alpha_proportionally() {
        let mut doc = solid_doc(2, 2, Color::WHITE);
        let mut mask = SelectionMask::new_empty(2, 2);
        mask.set(0, 0, 128); // ~50% feathered coverage
        doc.selection = Some(mask);
        let mut cmd = delete_selection(0, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let a = doc.layers[0].pixels.get_pixel(0, 0).unwrap().a;
        assert!((126..=129).contains(&a), "got alpha {a}");
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 1), Some(Color::WHITE));
    }

    #[test]
    fn delete_with_empty_selection_returns_none() {
        let mut doc = solid_doc(4, 4, Color::WHITE);
        doc.selection = Some(SelectionMask::new_empty(4, 4));
        assert!(delete_selection(0, &doc).is_none());
    }

    #[test]
    fn delete_invalid_layer_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        assert!(delete_selection(5, &doc).is_none());
    }
}
