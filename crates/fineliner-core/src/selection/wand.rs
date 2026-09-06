//! The Magic Wand selection (spec §9.3) and Select by Color (spec §8.4).
//!
//! Selects pixels whose color is within `tolerance` of the clicked seed,
//! sampling either the current layer or the flattened composite. Contiguous
//! mode flood-fills the connected region (BFS); non-contiguous mode marks every
//! matching pixel on the canvas — which, with the whole canvas considered, is
//! the "Select by Color" command. Hard edge; anti-aliasing is deferred.

use super::SelectionMask;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Point;
use crate::render::compose;
use crate::tools::SampleSource;

/// Builds a selection mask of pixels similar to the color under `seed`.
///
/// Returns `None` if the seed is off-canvas or the layer index is invalid.
/// `sample` chooses whether the color comparison reads the target layer or the
/// composite (spec §9.3 Magic Wand "Sample"). The returned mask is the raw
/// selection; callers combine it with any existing selection via
/// [`super::apply_mode`].
pub fn magic_wand(
    doc: &Document,
    layer_index: usize,
    seed: Point,
    tolerance: u8,
    contiguous: bool,
    sample: SampleSource,
) -> Option<SelectionMask> {
    let layer = doc.layers().get(layer_index)?;
    let w = doc.canvas.width();
    let h = doc.canvas.height();

    let sx = seed.x.floor() as i32;
    let sy = seed.y.floor() as i32;
    if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
        return None;
    }
    let (sx, sy) = (sx as u32, sy as u32);

    // Buffer the tolerance comparison runs against.
    let composite;
    let buffer: &ImageBuffer = match sample {
        SampleSource::CurrentLayer => &layer.pixels,
        SampleSource::AllLayers => {
            composite = compose(doc.layers());
            &composite
        }
    };
    let seed_color = buffer.get_pixel(sx, sy)?;

    let mut mask = SelectionMask::new_empty(w, h);
    if contiguous {
        let mut visited = vec![false; w as usize * h as usize];
        let idx = |x: u32, y: u32| y as usize * w as usize + x as usize;
        let mut stack = vec![(sx, sy)];
        visited[idx(sx, sy)] = true;
        while let Some((x, y)) = stack.pop() {
            mask.set(x, y, 255);
            let mut visit = |nx: u32, ny: u32, stack: &mut Vec<(u32, u32)>| {
                let i = idx(nx, ny);
                if visited[i] {
                    return;
                }
                if let Some(c) = buffer.get_pixel(nx, ny) {
                    if c.within_tolerance(seed_color, tolerance) {
                        visited[i] = true;
                        stack.push((nx, ny));
                    }
                }
            };
            if x > 0 {
                visit(x - 1, y, &mut stack);
            }
            if x + 1 < w {
                visit(x + 1, y, &mut stack);
            }
            if y > 0 {
                visit(x, y - 1, &mut stack);
            }
            if y + 1 < h {
                visit(x, y + 1, &mut stack);
            }
        }
    } else {
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = buffer.get_pixel(x, y) {
                    if c.within_tolerance(seed_color, tolerance) {
                        mask.set(x, y, 255);
                    }
                }
            }
        }
    }
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn doc_3px_wall() -> Document {
        // White, black wall, white.
        let mut doc = Document::new(3, 1).unwrap();
        doc.layers[0].pixels.set_pixel(0, 0, Color::WHITE);
        doc.layers[0].pixels.set_pixel(1, 0, Color::BLACK);
        doc.layers[0].pixels.set_pixel(2, 0, Color::WHITE);
        doc
    }

    #[test]
    fn magic_wand_contiguous_stops_at_color_boundary() {
        let doc = doc_3px_wall();
        let mask = magic_wand(
            &doc,
            0,
            Point::new(0.0, 0.0),
            0,
            true,
            SampleSource::CurrentLayer,
        )
        .unwrap();
        assert_eq!(mask.coverage(0, 0), 255);
        assert_eq!(mask.coverage(1, 0), 0); // the wall blocks the flood
        assert_eq!(mask.coverage(2, 0), 0); // disconnected white
        assert_eq!(mask.selected_count(), 1);
    }

    #[test]
    fn magic_wand_global_selects_all_matching_pixels() {
        let doc = doc_3px_wall();
        let mask = magic_wand(
            &doc,
            0,
            Point::new(0.0, 0.0),
            0,
            false,
            SampleSource::CurrentLayer,
        )
        .unwrap();
        // Both white pixels select despite the wall; the wall does not.
        assert_eq!(mask.coverage(0, 0), 255);
        assert_eq!(mask.coverage(1, 0), 0);
        assert_eq!(mask.coverage(2, 0), 255);
        assert_eq!(mask.selected_count(), 2);
    }

    #[test]
    fn magic_wand_tolerance_includes_near_colors() {
        let mut doc = Document::new(2, 1).unwrap();
        doc.layers[0].pixels.set_pixel(0, 0, Color::WHITE);
        doc.layers[0]
            .pixels
            .set_pixel(1, 0, Color::rgba(250, 250, 250, 255));
        let mask = magic_wand(
            &doc,
            0,
            Point::new(0.0, 0.0),
            20,
            true,
            SampleSource::CurrentLayer,
        )
        .unwrap();
        assert_eq!(mask.coverage(1, 0), 255);
    }

    #[test]
    fn magic_wand_off_canvas_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        assert!(magic_wand(
            &doc,
            0,
            Point::new(-1.0, 0.0),
            0,
            true,
            SampleSource::CurrentLayer
        )
        .is_none());
    }

    #[test]
    fn magic_wand_all_layers_samples_composite() {
        // Top layer transparent; white comes from the layer below.
        let mut doc = Document::new(2, 1).unwrap();
        for x in 0..2 {
            doc.layers[0].pixels.set_pixel(x, 0, Color::WHITE);
        }
        doc.add_layer("Top").unwrap();
        let top = doc.layer_count() - 1;
        let mask = magic_wand(
            &doc,
            top,
            Point::new(0.0, 0.0),
            0,
            false,
            SampleSource::AllLayers,
        )
        .unwrap();
        assert_eq!(mask.selected_count(), 2);
    }
}
