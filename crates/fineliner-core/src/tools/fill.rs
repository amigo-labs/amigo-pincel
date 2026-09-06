//! The Fill / Paint Bucket tool (spec §9.2).
//!
//! Flood-fills a region of similar color with the active color. Matching uses
//! Euclidean distance in RGBA8 space against the *sample* buffer (the current
//! layer or the flattened composite); the fill is always written into the
//! current layer via straight-alpha source-over.

use super::src_over;
use crate::color::Color;
use crate::command::SetPixels;
use crate::document::{Document, ImageBuffer};
use crate::geometry::{Point, Rect};
use crate::render::compose;

/// Where the Fill tool reads colors from when deciding which pixels match.
///
/// Filling always writes to the current layer; this only chooses the buffer
/// the tolerance comparison runs against (spec §9.2 Fill "Sample").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleSource {
    /// Compare against the target layer's own pixels.
    #[default]
    CurrentLayer,
    /// Compare against the flattened composite of all layers.
    AllLayers,
}

/// Options controlling which pixels a [`Fill`] affects (spec §9.2 Fill).
#[derive(Debug, Clone, Copy)]
pub struct FillOptions {
    /// Color-similarity threshold, 0–255 (Euclidean distance in RGBA8 space).
    pub tolerance: u8,
    /// `true` fills the connected region around the seed (BFS); `false` fills
    /// every matching pixel in the layer.
    pub contiguous: bool,
    /// Which buffer the tolerance comparison samples (spec §9.2 Fill "Sample").
    pub sample: SampleSource,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self {
            tolerance: 0,
            contiguous: true,
            sample: SampleSource::CurrentLayer,
        }
    }
}

/// The Fill / Paint Bucket tool — flood-fills similar pixels (spec §9.2).
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    /// Fill color (alpha is combined with `opacity`).
    pub color: Color,
    /// Fill opacity in `[0.0, 1.0]`.
    pub opacity: f32,
    /// Matching/sampling options.
    pub options: FillOptions,
}

impl Fill {
    /// Creates a fill with the given color, options, and full opacity.
    pub fn new(color: Color, options: FillOptions) -> Self {
        Self {
            color,
            opacity: 1.0,
            options,
        }
    }

    /// Sets the fill opacity, clamped to `[0.0, 1.0]`.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Flood-fills from `seed` and returns the resulting [`SetPixels`] command.
    ///
    /// Returns `None` if the seed is off-canvas, the layer index is invalid, or
    /// the effective alpha is zero (nothing to paint). The command's region is
    /// the bounding box of the filled pixels; the prior pixels are captured for
    /// undo.
    pub fn fill(&self, layer_index: usize, seed: Point, doc: &Document) -> Option<SetPixels> {
        let layer = doc.layers.get(layer_index)?;
        let w = doc.canvas.width();
        let h = doc.canvas.height();

        let sx = seed.x.floor() as i32;
        let sy = seed.y.floor() as i32;
        if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
            return None;
        }
        let (sx, sy) = (sx as u32, sy as u32);

        let ca = self.color.a as f32 / 255.0 * self.opacity;
        if ca <= 0.0 {
            return None;
        }

        // Buffer the tolerance comparison runs against. Borrow the target layer
        // directly; only the composite needs allocating.
        let composite;
        let sample: &ImageBuffer = match self.options.sample {
            SampleSource::CurrentLayer => &layer.pixels,
            SampleSource::AllLayers => {
                composite = compose(doc.layers());
                &composite
            }
        };
        let seed_color = sample.get_pixel(sx, sy)?;
        let tol = self.options.tolerance;

        // Mark every pixel that should be filled.
        let idx = |x: u32, y: u32| y as usize * w as usize + x as usize;
        let mut mask = vec![false; w as usize * h as usize];
        if self.options.contiguous {
            let mut stack = vec![(sx, sy)];
            mask[idx(sx, sy)] = true;
            while let Some((x, y)) = stack.pop() {
                let mut visit = |nx: u32, ny: u32, stack: &mut Vec<(u32, u32)>| {
                    let i = idx(nx, ny);
                    if mask[i] {
                        return;
                    }
                    if let Some(c) = sample.get_pixel(nx, ny) {
                        if c.within_tolerance(seed_color, tol) {
                            mask[i] = true;
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
                    if let Some(c) = sample.get_pixel(x, y) {
                        if c.within_tolerance(seed_color, tol) {
                            mask[idx(x, y)] = true;
                        }
                    }
                }
            }
        }

        // Tight bounding box of the filled pixels.
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                if mask[idx(x, y)] {
                    any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if !any {
            return None;
        }
        let region = Rect::new(
            min_x as i32,
            min_y as i32,
            max_x - min_x + 1,
            max_y - min_y + 1,
        );

        // Paint the fill color over the layer's existing pixels in the region.
        // The active selection (if any) scales the write per pixel, so an
        // unselected pixel is untouched and feathered edges blend.
        let selection = doc.selection.as_ref();
        let mut after = layer.pixels.copy_region(region).ok()?;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if !mask[idx(x, y)] {
                    continue;
                }
                let eff = match selection {
                    Some(sel) => ca * sel.coverage(x, y) as f32 / 255.0,
                    None => ca,
                };
                if eff <= 0.0 {
                    continue;
                }
                let lx = x - min_x;
                let ly = y - min_y;
                let dst = after.get_pixel(lx, ly).unwrap_or(Color::TRANSPARENT);
                after.set_pixel(lx, ly, src_over(self.color, eff, dst));
            }
        }

        Some(SetPixels::new(layer_index, region, after).with_label("Fill"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::document::ImageBuffer;

    /// A document whose single layer is `paint`ed via the closure.
    fn doc_with(w: u32, h: u32, paint: impl Fn(&mut ImageBuffer)) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        paint(&mut doc.layers[0].pixels);
        doc
    }

    fn red_fill(opts: FillOptions) -> Fill {
        Fill::new(Color::rgba(255, 0, 0, 255), opts)
    }

    #[test]
    fn fill_with_zero_tolerance_fills_exact_color_region() {
        // Left half white, right half black; seed in the white half.
        let doc = doc_with(4, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::WHITE);
            b.set_pixel(2, 0, Color::BLACK);
            b.set_pixel(3, 0, Color::BLACK);
        });
        let mut doc = doc;
        let mut cmd = red_fill(FillOptions::default())
            .fill(0, Point::new(0.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(red));
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(red));
        // The black pixels are a different color and stay untouched.
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::BLACK));
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 0), Some(Color::BLACK));
    }

    #[test]
    fn fill_contiguous_stops_at_color_boundary() {
        // A vertical black wall at x=1 separates two white columns.
        let doc = doc_with(3, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::BLACK);
            b.set_pixel(2, 0, Color::WHITE);
        });
        let mut doc = doc;
        let mut cmd = red_fill(FillOptions::default())
            .fill(0, Point::new(0.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(red));
        // The far white pixel is disconnected by the wall.
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
    }

    #[test]
    fn fill_non_contiguous_fills_all_matching_pixels() {
        let doc = doc_with(3, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::BLACK);
            b.set_pixel(2, 0, Color::WHITE);
        });
        let mut doc = doc;
        let opts = FillOptions {
            contiguous: false,
            ..FillOptions::default()
        };
        let mut cmd = red_fill(opts).fill(0, Point::new(0.0, 0.0), &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        // Both white pixels fill despite the wall; the wall itself does not.
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(red));
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::BLACK));
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(red));
    }

    #[test]
    fn fill_tolerance_includes_near_colors() {
        // A near-white pixel within tolerance of the pure-white seed.
        let doc = doc_with(2, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::rgba(250, 250, 250, 255));
        });
        let mut doc = doc;
        let opts = FillOptions {
            tolerance: 20,
            ..FillOptions::default()
        };
        let mut cmd = red_fill(opts).fill(0, Point::new(0.0, 0.0), &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(red));
    }

    #[test]
    fn fill_seed_off_canvas_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        assert!(red_fill(FillOptions::default())
            .fill(0, Point::new(-1.0, 0.0), &doc)
            .is_none());
        assert!(red_fill(FillOptions::default())
            .fill(0, Point::new(4.0, 0.0), &doc)
            .is_none());
    }

    #[test]
    fn fill_is_undoable_to_original_pixels() {
        let doc = doc_with(2, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::WHITE);
        });
        let mut doc = doc;
        let mut cmd = red_fill(FillOptions::default())
            .fill(0, Point::new(0.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::WHITE));
    }

    #[test]
    fn fill_respects_active_selection() {
        use crate::geometry::Rect;
        use crate::selection::SelectionMask;
        // All white; select only the left two pixels of a 4×1 canvas.
        let mut doc = doc_with(4, 1, |b| {
            for x in 0..4 {
                b.set_pixel(x, 0, Color::WHITE);
            }
        });
        doc.selection = Some(SelectionMask::rectangle(4, 1, Rect::new(0, 0, 2, 1)));
        let mut cmd = red_fill(FillOptions::default())
            .fill(0, Point::new(0.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        // Inside the selection the fill applies; outside it is left untouched.
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(red));
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(red));
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 0), Some(Color::WHITE));
    }

    #[test]
    fn fill_all_layers_sample_matches_composite_not_layer() {
        // Target (top) layer is transparent everywhere; the opaque white comes
        // from a lower layer, so only the composite reveals the seed color.
        let mut doc = Document::new(2, 1).unwrap();
        for x in 0..2 {
            doc.layers[0].pixels.set_pixel(x, 0, Color::WHITE);
        }
        doc.add_layer("Top").unwrap();
        let top = doc.layer_count() - 1;
        doc.set_active_layer(top).unwrap();

        let opts = FillOptions {
            sample: SampleSource::AllLayers,
            ..FillOptions::default()
        };
        let mut cmd = red_fill(opts)
            .fill(top, Point::new(0.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let red = Color::rgba(255, 0, 0, 255);
        // Fill landed on the top layer even though it was transparent there.
        assert_eq!(doc.layers[top].pixels.get_pixel(0, 0), Some(red));
        assert_eq!(doc.layers[top].pixels.get_pixel(1, 0), Some(red));
    }
}
