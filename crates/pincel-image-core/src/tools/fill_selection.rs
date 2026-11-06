//! Fill Selection — paints the selected pixels of a layer with one color
//! (Edit ▸ Fill Selection; docs/specs/image-edit-extras.md).
//!
//! Emits one [`SetPixels`] over the whole layer, like Delete Selection: undo
//! restores the original content exactly. Partial (feathered) selection
//! coverage moves a pixel toward the color by its coverage instead of
//! cutting a hard edge.

use crate::color::Color;
use crate::command::{HistoryKind, SetPixels};
use crate::document::Document;
use crate::geometry::Rect;

/// Builds a command filling the selected pixels of layer `layer_index` with
/// `color`.
///
/// With no active selection the whole layer is filled. Returns `None` if the
/// layer index is invalid, the selection covers nothing, or the fill would
/// leave every pixel as it was (no no-op undo step).
pub fn fill_selection(layer_index: usize, color: Color, doc: &Document) -> Option<SetPixels> {
    let src = &doc.layers.get(layer_index)?.pixels;
    let (w, h) = (src.width(), src.height());
    let selection = doc.selection.as_ref();
    if selection.is_some_and(|sel| sel.is_empty()) {
        return None;
    }

    let mut out = src.clone();
    let mut changed = false;
    for y in 0..h {
        for x in 0..w {
            let cov = selection.map_or(255, |sel| sel.coverage(x, y));
            if cov == 0 {
                continue;
            }
            let Some(prev) = out.get_pixel(x, y) else {
                continue;
            };
            let next = toward(prev, color, cov);
            if next != prev {
                out.set_pixel(x, y, next);
                changed = true;
            }
        }
    }
    if !changed {
        return None;
    }

    let region = Rect::new(0, 0, w, h);
    Some(SetPixels::new(layer_index, region, out).with_kind(HistoryKind::FillSelection))
}

/// Moves `prev` toward `paint` by `cov / 255`, alpha included: alpha is a
/// rounded lerp, color a lerp of the alpha-weighted (premultiplied) colors,
/// so a transparent `paint` keeps the pixel's color and only lowers its
/// alpha, as Delete Selection does. `cov == 255` gives `paint` exactly.
fn toward(prev: Color, paint: Color, cov: u8) -> Color {
    if cov == 255 {
        return paint;
    }
    let t = u32::from(cov);
    let lerp = |a: u8, b: u8| ((u32::from(a) * (255 - t) + u32::from(b) * t + 127) / 255) as u8;
    let alpha = lerp(prev.a, paint.a);
    // Weights of the two colors, each scaled by 255.
    let wp = u32::from(prev.a) * (255 - t);
    let wn = u32::from(paint.a) * t;
    let sum = wp + wn;
    // Both weights zero (two transparent pixels): a plain lerp instead.
    let mix = |a: u8, b: u8| {
        (u32::from(a) * wp + u32::from(b) * wn + sum / 2)
            .checked_div(sum)
            .map_or_else(|| lerp(a, b), |v| v as u8)
    };
    Color::rgba(
        mix(prev.r, paint.r),
        mix(prev.g, paint.g),
        mix(prev.b, paint.b),
        alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::selection::SelectionMask;

    const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };

    fn solid_doc(w: u32, h: u32, c: Color) -> Document {
        let mut doc = Document::new(w, h).expect("doc");
        for y in 0..h {
            for x in 0..w {
                doc.layers[0].pixels.set_pixel(x, y, c);
            }
        }
        doc
    }

    fn px(doc: &Document, layer: usize, x: u32, y: u32) -> Color {
        doc.layers[layer].pixels.get_pixel(x, y).expect("in bounds")
    }

    #[test]
    fn fill_selection_full_coverage_takes_the_color_and_leaves_unselected_pixels() {
        let mut doc = solid_doc(8, 8, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(8, 8, Rect::new(2, 1, 3, 4)));
        let fg = Color::rgba(10, 200, 30, 77);
        let mut cmd = fill_selection(0, fg, &doc).expect("changes");
        cmd.apply(&mut doc).expect("apply");
        for y in 0..8 {
            for x in 0..8 {
                let inside = (2..5).contains(&x) && (1..5).contains(&y);
                let want = if inside { fg } else { Color::WHITE };
                assert_eq!(px(&doc, 0, x, y), want, "({x}, {y})");
            }
        }
    }

    #[test]
    fn fill_selection_full_coverage_over_transparent_takes_the_exact_color() {
        let mut doc = Document::new(4, 4).expect("doc");
        doc.layers[0].pixels = crate::document::ImageBuffer::new_transparent(4, 4);
        doc.selection = Some(SelectionMask::rectangle(4, 4, Rect::new(0, 0, 2, 2)));
        // A transparent color keeps its red, green and blue at coverage 255.
        let fg = Color::rgba(200, 100, 50, 0);
        doc.layers[0].pixels.set_pixel(0, 0, RED);
        let mut cmd = fill_selection(0, fg, &doc).expect("changes");
        cmd.apply(&mut doc).expect("apply");
        assert_eq!(px(&doc, 0, 0, 0), fg);
        assert_eq!(px(&doc, 0, 1, 1), fg);
        assert_eq!(px(&doc, 0, 3, 3), Color::TRANSPARENT);
    }

    #[test]
    fn fill_selection_partial_coverage_interpolates_opaque_channels() {
        // Opaque foreground over opaque pixels: each channel moves by c/255.
        let prev = Color::rgba(20, 240, 100, 255);
        let fg = Color::rgba(250, 10, 100, 255);
        for cov in [1u8, 64, 128, 200, 254] {
            let mut doc = solid_doc(2, 1, prev);
            let mut mask = SelectionMask::new_empty(2, 1);
            mask.set(0, 0, cov);
            doc.selection = Some(mask);
            let mut cmd = fill_selection(0, fg, &doc).expect("changes");
            cmd.apply(&mut doc).expect("apply");
            let got = px(&doc, 0, 0, 0);
            let t = f32::from(cov) / 255.0;
            for (g, p, f) in [
                (got.r, prev.r, fg.r),
                (got.g, prev.g, fg.g),
                (got.b, prev.b, fg.b),
            ] {
                let want = f32::from(p) + (f32::from(f) - f32::from(p)) * t;
                assert!(
                    (f32::from(g) - want).abs() <= 1.0,
                    "cov {cov}: {g} vs {want}"
                );
            }
            assert_eq!(got.a, 255, "cov {cov}");
            assert_eq!(px(&doc, 0, 1, 0), prev, "coverage 0 is unchanged");
        }
    }

    #[test]
    fn fill_selection_partial_coverage_moves_alpha_and_keeps_color_between() {
        // Translucent color over a translucent pixel.
        let prev = Color::rgba(0, 0, 255, 60);
        let fg = Color::rgba(255, 0, 0, 200);
        for cov in [1u8, 50, 128, 254] {
            let mut doc = solid_doc(1, 1, prev);
            let mut mask = SelectionMask::new_empty(1, 1);
            mask.set(0, 0, cov);
            doc.selection = Some(mask);
            let mut cmd = fill_selection(0, fg, &doc).expect("changes");
            cmd.apply(&mut doc).expect("apply");
            let got = px(&doc, 0, 0, 0);
            let t = f32::from(cov) / 255.0;
            let want_a = f32::from(prev.a) + (f32::from(fg.a) - f32::from(prev.a)) * t;
            assert!(
                (f32::from(got.a) - want_a).abs() <= 1.0,
                "cov {cov}: alpha {}",
                got.a
            );
            for (g, p, f) in [
                (got.r, prev.r, fg.r),
                (got.g, prev.g, fg.g),
                (got.b, prev.b, fg.b),
            ] {
                assert!(
                    g >= p.min(f) && g <= p.max(f),
                    "cov {cov}: {g} outside {p}..{f}"
                );
            }
        }
    }

    #[test]
    fn fill_selection_transparent_color_scales_alpha_like_delete() {
        let mut doc = solid_doc(2, 1, RED);
        let mut mask = SelectionMask::new_empty(2, 1);
        mask.set(0, 0, 128);
        doc.selection = Some(mask);
        let deleted = {
            let mut d = doc.clone();
            let mut cmd = crate::tools::delete_selection(0, &d).expect("delete");
            cmd.apply(&mut d).expect("apply");
            px(&d, 0, 0, 0)
        };
        let mut cmd = fill_selection(0, Color::rgba(0, 0, 0, 0), &doc).expect("changes");
        cmd.apply(&mut doc).expect("apply");
        let got = px(&doc, 0, 0, 0);
        assert!(got.a.abs_diff(deleted.a) <= 1, "{got:?} vs {deleted:?}");
        assert_eq!((got.r, got.g, got.b), (255, 0, 0), "the color stays");
    }

    #[test]
    fn fill_selection_without_selection_fills_the_whole_layer_and_undoes() {
        let mut doc = solid_doc(3, 3, Color::WHITE);
        doc.layers[0]
            .pixels
            .set_pixel(1, 1, Color::rgba(1, 2, 3, 4));
        let before = doc.layers[0].pixels.clone();
        let fg = Color::rgba(9, 8, 7, 255);
        let mut cmd = fill_selection(0, fg, &doc).expect("changes");
        cmd.apply(&mut doc).expect("apply");
        assert!(
            (0..3).all(|y| (0..3).all(|x| px(&doc, 0, x, y) == fg)),
            "every pixel"
        );
        cmd.revert(&mut doc).expect("revert");
        assert_eq!(doc.layers[0].pixels, before);
        assert_eq!(cmd.kind(), HistoryKind::FillSelection);
    }

    #[test]
    fn fill_selection_that_changes_nothing_returns_none() {
        let mut doc = solid_doc(4, 4, RED);
        assert!(fill_selection(0, RED, &doc).is_none(), "no selection");
        doc.layers[0].pixels.set_pixel(3, 3, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(4, 4, Rect::new(0, 0, 2, 2)));
        assert!(
            fill_selection(0, RED, &doc).is_none(),
            "selected pixels already red"
        );
    }

    #[test]
    fn fill_selection_with_empty_selection_returns_none() {
        let mut doc = solid_doc(4, 4, Color::WHITE);
        doc.selection = Some(SelectionMask::new_empty(4, 4));
        assert!(fill_selection(0, RED, &doc).is_none());
    }

    #[test]
    fn fill_selection_invalid_layer_returns_none() {
        let doc = Document::new(4, 4).expect("doc");
        assert!(fill_selection(5, RED, &doc).is_none());
    }
}
