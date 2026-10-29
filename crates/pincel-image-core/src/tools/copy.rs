//! Copy / Copy Merged — what Edit ▸ Copy puts on the clipboard (spec §5.4
//! Paste, Decision Log 2026-10-29). Read-only: the document is unchanged;
//! Cut is this plus [`super::delete_selection`].

use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;
use crate::render::compose;

/// The active layer's pixels — with `merged`, the composite of the visible
/// layers — cropped to the selection's bounding box (the whole canvas with
/// no selection). Each pixel's alpha is scaled by the selection's coverage
/// there, so feathered edges copy soft and unselected pixels transparent.
/// `None` when the selection is empty.
pub fn copy_selection(doc: &Document, merged: bool) -> Option<(Rect, ImageBuffer)> {
    let composite;
    let src = if merged {
        composite = compose(doc.layers());
        &composite
    } else {
        &doc.active_layer().pixels
    };
    let Some(sel) = doc.selection.as_ref() else {
        return Some((Rect::new(0, 0, src.width(), src.height()), src.clone()));
    };
    let region = sel.bounding_box()?;
    let mut out = src.copy_region(region).ok()?;
    let w = region.w;
    let data = out.data_mut();
    for ly in 0..region.h {
        for lx in 0..w {
            let cov = u32::from(sel.coverage(region.x as u32 + lx, region.y as u32 + ly));
            let i = ((ly * w + lx) * 4) as usize;
            let a = (u32::from(data[i + 3]) * cov + 127) / 255;
            if a == 0 {
                data[i..i + 4].copy_from_slice(&[0; 4]);
            } else {
                data[i + 3] = a as u8;
            }
        }
    }
    Some((region, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::selection::SelectionMask;

    fn doc() -> Document {
        let mut doc = Document::new(4, 4).unwrap();
        doc.layers[0]
            .pixels
            .set_pixel(1, 1, Color::rgba(255, 0, 0, 255));
        doc.layers[0]
            .pixels
            .set_pixel(3, 3, Color::rgba(255, 0, 0, 255));
        doc.add_layer("top").unwrap();
        doc.layers[1]
            .pixels
            .set_pixel(2, 1, Color::rgba(0, 0, 255, 200));
        doc
    }

    #[test]
    fn copy_selection_crops_to_the_bounding_box_of_the_active_layer() {
        let mut d = doc();
        d.set_active_layer(0).unwrap();
        d.selection = Some(SelectionMask::rectangle(4, 4, Rect::new(1, 1, 2, 2)));
        let (r, img) = copy_selection(&d, false).unwrap();
        assert_eq!(r, Rect::new(1, 1, 2, 2));
        assert_eq!((img.width(), img.height()), (2, 2));
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(img.get_pixel(1, 0), Some(Color::TRANSPARENT));
    }

    #[test]
    fn copy_selection_scales_alpha_by_feathered_coverage() {
        let mut d = doc();
        d.set_active_layer(0).unwrap();
        let mut sel = SelectionMask::new_empty(4, 4);
        sel.set(1, 1, 128);
        sel.set(3, 3, 255);
        sel.set(2, 2, 64);
        d.selection = Some(sel);
        let (r, img) = copy_selection(&d, false).unwrap();
        assert_eq!(r, Rect::new(1, 1, 3, 3));
        // 255 * 128 / 255 = 128, colour kept.
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(255, 0, 0, 128)));
        assert_eq!(img.get_pixel(2, 2), Some(Color::rgba(255, 0, 0, 255)));
        // Selected but transparent on the layer, and unselected: transparent.
        assert_eq!(img.get_pixel(1, 1), Some(Color::TRANSPARENT));
        assert_eq!(img.get_pixel(2, 0), Some(Color::TRANSPARENT));
    }

    #[test]
    fn copy_selection_partial_alpha_times_coverage() {
        let mut d = doc();
        let mut sel = SelectionMask::new_empty(4, 4);
        sel.set(2, 1, 128);
        d.selection = Some(sel);
        let (_, img) = copy_selection(&d, false).unwrap();
        // 200 * 128 / 255 = 100.39 -> 100.
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(0, 0, 255, 100)));
    }

    #[test]
    fn copy_selection_without_selection_is_the_whole_layer() {
        let d = doc();
        let (r, img) = copy_selection(&d, false).unwrap();
        assert_eq!(r, Rect::new(0, 0, 4, 4));
        assert_eq!(img, d.layers[1].pixels);
    }

    #[test]
    fn copy_merged_takes_the_composite_of_the_same_region() {
        let mut d = doc();
        d.layers[0]
            .pixels
            .set_pixel(2, 1, Color::rgba(0, 255, 0, 255));
        d.selection = Some(SelectionMask::rectangle(4, 4, Rect::new(1, 1, 2, 1)));
        let (r, img) = copy_selection(&d, true).unwrap();
        let (r1, _) = copy_selection(&d, false).unwrap();
        assert_eq!(r, r1);
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(255, 0, 0, 255)));
        let mixed = img.get_pixel(1, 0).unwrap();
        assert_eq!(mixed.a, 255, "blue over green is opaque");
        assert!(
            mixed.b > 0 && mixed.g > 0,
            "both layers contribute: {mixed:?}"
        );
        d.layers[1].visible = false;
        let (_, hidden) = copy_selection(&d, true).unwrap();
        assert_eq!(hidden.get_pixel(1, 0), Some(Color::rgba(0, 255, 0, 255)));
    }

    #[test]
    fn copy_selection_empty_selection_is_none() {
        let mut d = doc();
        d.selection = Some(SelectionMask::new_empty(4, 4));
        assert!(copy_selection(&d, false).is_none());
    }
}
