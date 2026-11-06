//! The Clone Stamp tool (docs/specs/clone-stamp.md).
//!
//! Paints with pixels copied from elsewhere in the image: every pixel `p`
//! a dab covers receives the source image's pixel at `p + offset`,
//! composited source-over by the dab's effective coverage — exactly the way
//! the Brush lays down its color, so a uniform source paints like a Brush
//! stroke in that color (AC-06).
//!
//! The source image is the state at the stroke's press (AC-08): one drag
//! arrives as several commands sharing a `stroke_id`, so the caller keeps a
//! [`CloneSnapshot`] across them, and the stroke never reads pixels it has
//! written itself.

use crate::color::Color;
use crate::command::{HistoryKind, SetPixels};
use crate::document::{Document, ImageBuffer};
use crate::geometry::Point;
use crate::render::compose;
use crate::tools::{Brush, PenSamples, SampleSource, src_over};

/// The source image of one Clone Stamp stroke, as it was at the press.
///
/// Taken by the stroke's first command and reused by the rest; cleared
/// when a new drag starts.
#[derive(Debug, Default, Clone)]
pub struct CloneSnapshot {
    image: Option<ImageBuffer>,
}

impl CloneSnapshot {
    /// Forget the snapshot; a fresh drag takes its own.
    pub fn clear(&mut self) {
        self.image = None;
    }
}

/// The Clone Stamp tool.
#[derive(Debug, Clone, Copy)]
pub struct CloneStamp {
    /// The brush tip; its color is ignored — the source image supplies it.
    pub brush: Brush,
    /// Source position minus destination position, in whole pixels.
    pub offset: (i32, i32),
    /// Where the copied pixels come from: the painted layer or the
    /// composite of the visible layers.
    pub sample: SampleSource,
}

impl CloneStamp {
    /// Creates a clone stamp copying from `offset` away, out of `sample`.
    pub fn new(brush: Brush, offset: (i32, i32), sample: SampleSource) -> Self {
        Self {
            brush,
            offset,
            sample,
        }
    }

    /// Clones along `points`.
    ///
    /// For a stroke delivered in one piece; a drag split across several
    /// calls passes its own [`CloneSnapshot`] to [`CloneStamp::stroke_with_pen`].
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        self.stroke_with_pen(
            layer_index,
            points,
            PenSamples::default(),
            doc,
            &mut CloneSnapshot::default(),
        )
    }

    /// [`CloneStamp::stroke`] with pen input, reading the source image from
    /// the drag's `snapshot` (taken now if this is the drag's first call).
    ///
    /// Returns `None` when the stroke changes no pixel — it misses the
    /// canvas or the selection, every source position lies outside the
    /// canvas, or every pixel is copied onto an equal one — so such a
    /// stroke adds no undo step.
    pub fn stroke_with_pen(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
        snapshot: &mut CloneSnapshot,
    ) -> Option<SetPixels> {
        let layer = doc.layers.get(layer_index)?;
        let source: &ImageBuffer = snapshot.image.get_or_insert_with(|| match self.sample {
            SampleSource::CurrentLayer => layer.pixels.clone(),
            SampleSource::AllLayers => compose(doc.layers()),
        });
        let (ox, oy) = (i64::from(self.offset.0), i64::from(self.offset.1));
        let mut changed = false;
        let (region, after) =
            self.brush
                .rasterize(layer_index, points, pen, doc, self.brush.opacity, |p| {
                    let sx = i64::from(p.x) + ox;
                    let sy = i64::from(p.y) + oy;
                    let src = u32::try_from(sx)
                        .ok()
                        .zip(u32::try_from(sy).ok())
                        .and_then(|(x, y)| source.get_pixel(x, y));
                    // A source position outside the canvas leaves the pixel
                    // as it is (AC-11).
                    let Some(src) = src else {
                        return p.dst;
                    };
                    let out = clone_over(src, p.eff, p.dst);
                    changed |= out != p.dst;
                    out
                })?;
        changed
            .then(|| SetPixels::new(layer_index, region, after).with_kind(HistoryKind::CloneStamp))
    }
}

/// `src` source-over `dst` with its alpha scaled by the coverage `eff`.
fn clone_over(src: Color, eff: f32, dst: Color) -> Color {
    let sa = f32::from(src.a) / 255.0 * eff;
    if sa <= 0.0 {
        return dst;
    }
    src_over(src, sa, dst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::geometry::Rect;
    use crate::selection::SelectionMask;
    use crate::tools::{BrushShape, Pencil, PressureDynamics};

    const W: u32 = 32;
    const H: u32 = 16;

    /// A distinct, partly translucent color per pixel.
    fn pattern(x: u32, y: u32) -> Color {
        Color::rgba(
            (x * 8) as u8,
            (y * 15) as u8,
            ((x * 7 + y * 11) % 256) as u8,
            (96 + (x * 5 + y * 3) % 160) as u8,
        )
    }

    /// `pattern` made opaque.
    fn opaque(x: u32, y: u32) -> Color {
        let p = pattern(x, y);
        Color::rgba(p.r, p.g, p.b, 255)
    }

    fn doc_with(paint: impl Fn(u32, u32) -> Color) -> Document {
        let mut doc = Document::new(W, H).unwrap();
        for y in 0..H {
            for x in 0..W {
                doc.layers[0].pixels.set_pixel(x, y, paint(x, y));
            }
        }
        doc
    }

    fn pixel(doc: &Document, layer: usize, x: u32, y: u32) -> Color {
        doc.layers[layer].pixels.get_pixel(x, y).unwrap()
    }

    fn soft_brush() -> Brush {
        Brush::new(7, Color::BLACK, 0.7)
            .with_shape(BrushShape::SoftRound)
            .with_hardness(0.3)
    }

    fn hard(size: u32) -> Brush {
        Brush::new(size, Color::BLACK, 1.0)
    }

    fn line(x0: u32, x1: u32, y: f32) -> Vec<Point> {
        (x0..=x1).map(|x| Point::new(x as f32 + 0.5, y)).collect()
    }

    fn apply(cmd: Option<SetPixels>, doc: &mut Document) {
        if let Some(mut cmd) = cmd {
            cmd.apply(doc).unwrap();
        }
    }

    /// Pixel `(x, y)` of layer 0 after a Brush stroke in `color`.
    fn brush_pixel(
        brush: Brush,
        color: Color,
        points: &[Point],
        doc: &Document,
        x: u32,
        y: u32,
    ) -> Color {
        let mut d = doc.clone();
        let mut b = brush;
        b.color = color;
        apply(Pencil::new(b).stroke(0, points, &d), &mut d);
        pixel(&d, 0, x, y)
    }

    #[test]
    fn clone_composites_each_source_pixel_through_the_dabs() {
        // AC-05: at every pixel p the result is what the Brush lays down in
        // the color of the source pixel at p + offset (per-dab source-over
        // by the effective coverage, building up like the Brush), and only
        // the painted layer changes.
        let mut doc = doc_with(pattern);
        doc.add_layer("Other").unwrap();
        doc.layers[1].pixels.set_pixel(20, 7, Color::WHITE);
        let base = doc.clone();
        let points = line(14, 24, 7.5);
        let tool = CloneStamp::new(soft_brush(), (-11, 2), SampleSource::CurrentLayer);
        apply(tool.stroke(0, &points, &doc), &mut doc);
        let mut changed = 0;
        for y in 0..H {
            for x in 0..W {
                let (sx, sy) = (x as i32 - 11, y as i32 + 2);
                let want = if sx < 0 || sy >= H as i32 {
                    pixel(&base, 0, x, y)
                } else {
                    let color = pixel(&base, 0, sx as u32, sy as u32);
                    brush_pixel(soft_brush(), color, &points, &base, x, y)
                };
                assert_eq!(pixel(&doc, 0, x, y), want, "pixel ({x}, {y})");
                changed += usize::from(want != pixel(&base, 0, x, y));
            }
        }
        assert!(changed > 20, "the stroke changed {changed} pixels");
        assert_eq!(
            doc.layers[1].pixels, base.layers[1].pixels,
            "only the painted layer changes"
        );
    }

    /// Left of `edge` one opaque color, right of it a pattern.
    fn uniform_left(c: Color, edge: u32) -> Document {
        doc_with(|x, y| if x < edge { c } else { pattern(x, y) })
    }

    #[test]
    fn a_uniform_source_paints_like_the_brush_in_that_color() {
        // AC-06: same segments, tip settings and pen input give the Brush's
        // pixels — a soft tip, pressure on size and opacity, two segments.
        let c = Color::rgba(200, 40, 90, 255);
        let brush = soft_brush().with_dynamics(PressureDynamics {
            size: true,
            opacity: true,
        });
        let segments = [
            (line(20, 24, 6.2), vec![0.4, 0.9, 0.6, 0.3, 1.0]),
            (line(24, 27, 9.7), vec![0.5, 0.2, 0.7, 1.0]),
        ];
        // The tip reaches pixels 17..=31, which read 1..=15: all `c`.
        let mut cloned = uniform_left(c, 16);
        let mut brushed = cloned.clone();
        let mut snapshot = CloneSnapshot::default();
        let tool = CloneStamp::new(brush, (-16, 1), SampleSource::CurrentLayer);
        let mut b = brush;
        b.color = c;
        for (points, pressure) in &segments {
            let pen = PenSamples {
                pressure,
                azimuth: &[],
            };
            let cmd = tool.stroke_with_pen(0, points, pen, &cloned, &mut snapshot);
            apply(cmd, &mut cloned);
            apply(
                Pencil::new(b).stroke_with_pen(0, points, pen, &brushed),
                &mut brushed,
            );
        }
        assert_ne!(
            cloned.layers[0].pixels,
            uniform_left(c, 16).layers[0].pixels
        );
        assert_eq!(cloned.layers[0].pixels, brushed.layers[0].pixels);
    }

    #[test]
    fn a_hard_full_opacity_tip_copies_the_source_exactly() {
        // AC-06, second sentence: every covered pixel whose source lies on
        // the canvas equals the source pixel.
        let mut doc = doc_with(opaque);
        let base = doc.clone();
        let tool = CloneStamp::new(hard(5), (-13, 3), SampleSource::CurrentLayer);
        apply(tool.stroke(0, &line(15, 27, 6.5), &doc), &mut doc);
        let mut covered = 0;
        for y in 0..H - 3 {
            for x in 13..W {
                if pixel(&doc, 0, x, y) != pixel(&base, 0, x, y) {
                    covered += 1;
                    assert_eq!(pixel(&doc, 0, x, y), pixel(&base, 0, x - 13, y + 3));
                }
            }
        }
        assert!(covered > 40, "covered {covered}");
        // The tip's centre line is covered, so it holds the source.
        for x in 15..=27 {
            assert_eq!(pixel(&doc, 0, x, 6), pixel(&base, 0, x - 13, 9));
        }
    }

    /// A white strip with one red column at `x = 2`.
    fn red_column() -> Document {
        doc_with(|x, _| {
            if x == 2 {
                Color::rgba(255, 0, 0, 255)
            } else {
                Color::WHITE
            }
        })
    }

    #[test]
    fn a_stroke_reads_the_source_as_it_was_at_the_press() {
        // AC-08 / edge case: painting over its own source area copies the
        // press state once instead of smearing the copy along the stroke.
        let mut doc = red_column();
        let tool = CloneStamp::new(hard(3), (-2, 0), SampleSource::CurrentLayer);
        apply(tool.stroke(0, &line(2, 20, 8.5), &doc), &mut doc);
        assert_eq!(pixel(&doc, 0, 4, 8), Color::rgba(255, 0, 0, 255));
        for x in 5..W {
            assert_eq!(pixel(&doc, 0, x, 8), Color::WHITE, "x = {x}");
        }
    }

    /// The three overlapping parts of `line(2, 20)`: consecutive commands
    /// share the point they meet at, as the pointer handler sends them.
    fn parts() -> Vec<Vec<Point>> {
        let pts = line(2, 20, 8.5);
        vec![pts[..6].to_vec(), pts[5..12].to_vec(), pts[11..].to_vec()]
    }

    /// Clones the parts with offset −2 as three commands; `share` keeps one
    /// snapshot across them, as the editor does.
    fn split(share: bool) -> ImageBuffer {
        let mut doc = red_column();
        let tool = CloneStamp::new(soft_brush(), (-2, 0), SampleSource::CurrentLayer);
        let mut snapshot = CloneSnapshot::default();
        for part in parts() {
            if !share {
                snapshot.clear();
            }
            let cmd = tool.stroke_with_pen(0, &part, PenSamples::default(), &doc, &mut snapshot);
            apply(cmd, &mut doc);
        }
        doc.layers[0].pixels.clone()
    }

    #[test]
    fn a_split_stroke_matches_the_same_segments_in_one_command() {
        // AC-08: one command over the segment sequence and three commands
        // of the same stroke give the same pixels; both stamp the points
        // where the parts meet twice (as end and as start of a segment).
        let mut whole = red_column();
        let tool = CloneStamp::new(soft_brush(), (-2, 0), SampleSource::CurrentLayer);
        apply(tool.stroke(0, &line(2, 20, 8.5), &whole), &mut whole);
        assert_eq!(split(true), whole.layers[0].pixels);
    }

    #[test]
    fn a_split_stroke_that_forgets_its_snapshot_differs() {
        // The counter-test: re-reading the canvas per command smears.
        assert_ne!(split(true), split(false));
    }

    #[test]
    fn all_layers_copies_the_visible_composite_into_the_target_layer() {
        // AC-09: the source is the composite of the visible layers; the
        // hidden layer is not part of it, and the stroke writes only into
        // the target layer.
        let red = Color::rgba(255, 0, 0, 255);
        let blue = Color::rgba(0, 0, 255, 255);
        let mut doc = doc_with(|_, _| blue);
        doc.add_layer("Red").unwrap();
        for y in 0..H {
            for x in 0..8 {
                doc.layers[1].pixels.set_pixel(x, y, red);
            }
        }
        doc.add_layer("Hidden").unwrap();
        for y in 0..H {
            for x in 0..W {
                doc.layers[2]
                    .pixels
                    .set_pixel(x, y, Color::rgba(0, 255, 0, 255));
            }
        }
        doc.layers[2].visible = false;
        doc.add_layer("Target").unwrap();
        let base = doc.clone();
        let tool = CloneStamp::new(hard(3), (-16, 0), SampleSource::AllLayers);
        apply(tool.stroke(3, &line(18, 28, 8.5), &doc), &mut doc);
        // Red from the composite where the red layer covers x < 8, blue
        // beyond it; never the hidden green.
        assert_eq!(pixel(&doc, 3, 20, 8), red);
        assert_eq!(pixel(&doc, 3, 26, 8), blue);
        for i in 0..3 {
            assert_eq!(doc.layers[i].pixels, base.layers[i].pixels, "layer {i}");
        }
        // Current Layer reads the (transparent) target layer itself.
        let current = CloneStamp::new(hard(3), (-16, 0), SampleSource::CurrentLayer);
        assert!(current.stroke(3, &line(18, 28, 8.5), &base).is_none());
    }

    #[test]
    fn the_selection_bounds_the_writes_and_feathers_like_the_brush() {
        // AC-10: only selected pixels change, a feathered edge scales the
        // coverage as for the Brush, and the source may lie outside the
        // selection (left of x = 12, the selection is x = 16..26).
        let c = Color::rgba(30, 160, 60, 255);
        let mut mask = SelectionMask::rectangle(W, H, Rect::new(16, 0, 10, H));
        for y in 0..H {
            mask.set(16, y, 128);
            mask.set(25, y, 64);
        }
        let mut cloned = uniform_left(c, 12);
        cloned.selection = Some(mask);
        let mut brushed = cloned.clone();
        let tool = CloneStamp::new(soft_brush(), (-14, 0), SampleSource::CurrentLayer);
        let points = line(14, 29, 8.0);
        apply(tool.stroke(0, &points, &cloned), &mut cloned);
        let mut b = soft_brush();
        b.color = c;
        apply(Pencil::new(b).stroke(0, &points, &brushed), &mut brushed);
        assert_eq!(cloned.layers[0].pixels, brushed.layers[0].pixels);
        let base = uniform_left(c, 12);
        for y in 0..H {
            for x in (0..16).chain(26..W) {
                assert_eq!(pixel(&cloned, 0, x, y), pixel(&base, 0, x, y));
            }
        }
        assert_ne!(pixel(&cloned, 0, 20, 8), pixel(&base, 0, 20, 8));
    }

    #[test]
    fn a_source_outside_the_canvas_leaves_the_pixel_unchanged() {
        // AC-11: the left part of the stroke reads beyond the left edge.
        let mut doc = doc_with(opaque);
        let base = doc.clone();
        let tool = CloneStamp::new(hard(3), (-10, 0), SampleSource::CurrentLayer);
        apply(tool.stroke(0, &line(2, 20, 8.5), &doc), &mut doc);
        for x in 0..10 {
            assert_eq!(pixel(&doc, 0, x, 8), pixel(&base, 0, x, 8), "x = {x}");
        }
        assert_eq!(pixel(&doc, 0, 15, 8), pixel(&base, 0, 5, 8));
        // Every source position off the canvas: no change, no command.
        let far = CloneStamp::new(hard(3), (500, 0), SampleSource::CurrentLayer);
        assert!(far.stroke(0, &line(2, 20, 8.5), &doc).is_none());
    }

    #[test]
    fn transparent_or_identical_sources_change_nothing() {
        // Edge cases: a transparent source leaves the destination as it
        // is, and offset 0 copies opaque pixels onto themselves.
        let doc = doc_with(|x, y| {
            if x < 12 {
                Color::TRANSPARENT
            } else {
                opaque(x, y)
            }
        });
        // Pixels 13..=27 read -7..=7: off the canvas or transparent.
        let clear = CloneStamp::new(soft_brush(), (-20, 0), SampleSource::CurrentLayer);
        assert!(clear.stroke(0, &line(16, 24, 8.0), &doc).is_none());
        let onto_itself = CloneStamp::new(soft_brush(), (0, 0), SampleSource::CurrentLayer);
        assert!(onto_itself.stroke(0, &line(16, 24, 8.0), &doc).is_none());
    }

    #[test]
    fn clone_is_undoable_byte_for_byte() {
        let mut doc = doc_with(pattern);
        let before = doc.layers[0].pixels.clone();
        let tool = CloneStamp::new(soft_brush(), (-9, 2), SampleSource::CurrentLayer);
        let mut cmd = tool.stroke(0, &line(12, 26, 6.0), &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let after = doc.layers[0].pixels.clone();
        assert_ne!(after, before);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels, before);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels, after);
    }

    #[test]
    fn clone_on_an_invalid_layer_returns_none() {
        let doc = doc_with(pattern);
        let tool = CloneStamp::new(soft_brush(), (-9, 2), SampleSource::CurrentLayer);
        assert!(tool.stroke(7, &line(12, 26, 6.0), &doc).is_none());
    }
}
