//! Drawing tools (spec §9).
//!
//! M5–M6 scope: Pencil (hard/soft/flat brush), Eraser, Fill, Eyedropper and
//! Move. Each tool turns a gesture (a polyline of points, or a seed/sample
//! point) into a [`SetPixels`] command — or, for the Eyedropper, a sampled
//! color. The full pointer-event `Tool` trait with modifiers and cursors
//! (spec §9.1) is deferred; tools keep this simpler "gesture → command" shape
//! until a tool needs richer modifier/cursor state.

mod delete;
mod eraser;
mod eyedropper;
mod fill;
mod move_tool;
mod shapes;
mod text;

pub use delete::delete_selection;
pub use eraser::{Eraser, EraserMode};
pub use eyedropper::{Eyedropper, SampleSize};
pub use fill::{Fill, FillOptions, SampleSource};
pub use move_tool::Move;
pub use shapes::{DashPattern, Shape, ShapeMode, ShapeStyle, Shapes};
pub use text::{Text, TextAlign, TextStyle};

use crate::color::Color;
use crate::command::SetPixels;
use crate::document::{Document, ImageBuffer};
use crate::geometry::{Point, Rect};
use crate::selection::SelectionMask;

/// Brush tip shape (spec §9.2 Pencil "Brush shape").
///
/// `Hardness` is only meaningful for `SoftRound` and `Flat`; `HardRound` paints
/// a crisp binary edge. Textured/custom tips are Phase 2 (spec §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushShape {
    /// A crisp circle with a binary edge.
    #[default]
    HardRound,
    /// A circle whose coverage falls off toward the edge per `hardness`.
    SoftRound,
    /// A flat (calligraphic) nib oriented at 45°, softened per `hardness`.
    Flat,
}

/// Minor/major axis ratio of the [`BrushShape::Flat`] nib.
const FLAT_ASPECT: f32 = 0.35;

/// Immutable per-stroke context shared by every dab of one rasterization:
/// the clamped target region, the stroke strength, and the active selection
/// mask (if any). Bundled to keep the stamping helpers' signatures small.
struct StrokeCtx<'a> {
    region: Rect,
    strength: f32,
    selection: Option<&'a SelectionMask>,
}

/// A round/flat brush tip (spec §9.2 Pencil).
#[derive(Debug, Clone, Copy)]
pub struct Brush {
    /// Diameter in pixels, 1–500.
    pub size: u32,
    /// Paint color (alpha is combined with `opacity`).
    pub color: Color,
    /// Stroke opacity in `[0.0, 1.0]`.
    pub opacity: f32,
    /// Tip shape.
    pub shape: BrushShape,
    /// Edge hardness in `[0.0, 1.0]` (soft/flat only); 1.0 is a crisp edge.
    pub hardness: f32,
}

impl Brush {
    /// Creates a hard-round brush, clamping size to 1–500 and opacity to `[0, 1]`.
    pub fn new(size: u32, color: Color, opacity: f32) -> Self {
        Self {
            size: size.clamp(1, 500),
            color,
            opacity: opacity.clamp(0.0, 1.0),
            shape: BrushShape::HardRound,
            hardness: 1.0,
        }
    }

    /// Sets the tip shape.
    pub fn with_shape(mut self, shape: BrushShape) -> Self {
        self.shape = shape;
        self
    }

    /// Sets the edge hardness, clamped to `[0.0, 1.0]`.
    pub fn with_hardness(mut self, hardness: f32) -> Self {
        self.hardness = hardness.clamp(0.0, 1.0);
        self
    }

    fn radius(&self) -> f32 {
        self.size as f32 / 2.0
    }

    /// Coverage in `[0.0, 1.0]` of the tip at offset `(dx, dy)` from its center.
    fn coverage(&self, dx: f32, dy: f32) -> f32 {
        let radius = self.radius();
        match self.shape {
            BrushShape::HardRound => {
                if dx * dx + dy * dy <= radius * radius {
                    1.0
                } else {
                    0.0
                }
            }
            BrushShape::SoftRound => {
                soft_falloff((dx * dx + dy * dy).sqrt(), radius, self.hardness)
            }
            BrushShape::Flat => {
                // Rotate the offset into the nib's 45° frame, then test against a
                // squashed unit ellipse (major axis = radius, minor = aspect).
                let c = std::f32::consts::FRAC_1_SQRT_2;
                let lx = dx * c + dy * c;
                let ly = -dx * c + dy * c;
                let minor = (radius * FLAT_ASPECT).max(0.5);
                let d = ((lx / radius).powi(2) + (ly / minor).powi(2)).sqrt();
                soft_falloff(d, 1.0, self.hardness)
            }
        }
    }

    /// Rasterizes a stroke over `points`, combining the tip coverage with the
    /// existing pixels via `op`. `op` receives the effective source alpha
    /// (`strength * coverage`) and the destination color, and returns the new
    /// color. Returns the painted region and its new pixels, or `None` if the
    /// stroke misses the canvas, the layer is invalid, or `strength <= 0`.
    pub(crate) fn rasterize<F>(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
        strength: f32,
        mut op: F,
    ) -> Option<(Rect, ImageBuffer)>
    where
        F: FnMut(f32, Color) -> Color,
    {
        if strength <= 0.0 {
            return None;
        }
        let layer = doc.layers.get(layer_index)?;
        let region = self.stroke_region(points, doc.canvas.width(), doc.canvas.height())?;
        // The active selection (if any) constrains where the stroke may write.
        let selection = doc.selection.as_ref();
        let mut after = layer.pixels.copy_region(region).ok()?;
        let ctx = StrokeCtx {
            region,
            strength,
            selection,
        };
        if points.len() == 1 {
            self.stamp(&mut after, points[0], &ctx, &mut op);
        } else {
            for pair in points.windows(2) {
                self.stamp_segment(&mut after, pair[0], pair[1], &ctx, &mut op);
            }
        }
        Some((region, after))
    }

    /// Bounding region of the stroke, clamped to the canvas. `None` if off-canvas.
    fn stroke_region(&self, points: &[Point], cw: u32, ch: u32) -> Option<Rect> {
        if points.is_empty() {
            return None;
        }
        let r = self.radius() + 1.0;
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for p in points {
            min_x = min_x.min(p.x - r);
            min_y = min_y.min(p.y - r);
            max_x = max_x.max(p.x + r);
            max_y = max_y.max(p.y + r);
        }
        let x0 = (min_x.floor() as i32).clamp(0, cw as i32);
        let y0 = (min_y.floor() as i32).clamp(0, ch as i32);
        let x1 = (max_x.ceil() as i32).clamp(0, cw as i32);
        let y1 = (max_y.ceil() as i32).clamp(0, ch as i32);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
    }

    /// Stamps one dab centered at canvas-space `center` into `buf` (whose origin
    /// is `ctx.region`'s top-left), applying `op` per covered pixel.
    fn stamp<F>(&self, buf: &mut ImageBuffer, center: Point, ctx: &StrokeCtx, op: &mut F)
    where
        F: FnMut(f32, Color) -> Color,
    {
        let region = ctx.region;
        let radius = self.radius();
        let cx0 = (center.x - radius).floor() as i32;
        let cy0 = (center.y - radius).floor() as i32;
        let cx1 = (center.x + radius).ceil() as i32;
        let cy1 = (center.y + radius).ceil() as i32;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let dx = cx as f32 + 0.5 - center.x;
                let dy = cy as f32 + 0.5 - center.y;
                let cov = self.coverage(dx, dy);
                if cov <= 0.0 {
                    continue;
                }
                let mut eff = (ctx.strength * cov).clamp(0.0, 1.0);
                if eff <= 0.0 {
                    continue;
                }
                let lx = cx - region.x;
                let ly = cy - region.y;
                if lx < 0 || ly < 0 || lx >= region.w as i32 || ly >= region.h as i32 {
                    continue;
                }
                // Constrain to the active selection: scale by its coverage so an
                // unselected pixel (0) is untouched and feathered edges blend.
                if let Some(sel) = ctx.selection {
                    eff *= sel.coverage(cx as u32, cy as u32) as f32 / 255.0;
                    if eff <= 0.0 {
                        continue;
                    }
                }
                let dst = buf
                    .get_pixel(lx as u32, ly as u32)
                    .unwrap_or(Color::TRANSPARENT);
                buf.set_pixel(lx as u32, ly as u32, op(eff, dst));
            }
        }
    }

    /// Stamps dabs evenly along the segment `a`→`b`.
    fn stamp_segment<F>(
        &self,
        buf: &mut ImageBuffer,
        a: Point,
        b: Point,
        ctx: &StrokeCtx,
        op: &mut F,
    ) where
        F: FnMut(f32, Color) -> Color,
    {
        let dist = a.distance(b);
        // Spacing of a quarter brush size keeps strokes solid without overdraw blowup.
        let spacing = (self.size as f32 * 0.25).max(1.0);
        let steps = (dist / spacing).ceil() as i32;
        if steps == 0 {
            self.stamp(buf, a, ctx, op);
            return;
        }
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            self.stamp(buf, p, ctx, op);
        }
    }
}

/// Linear edge falloff: full coverage within `radius * hardness`, ramping to 0
/// at `radius`. Returns 0 at or beyond `radius`.
fn soft_falloff(dist: f32, radius: f32, hardness: f32) -> f32 {
    if dist >= radius {
        return 0.0;
    }
    let inner = radius * hardness;
    if dist <= inner || radius <= inner {
        1.0
    } else {
        1.0 - (dist - inner) / (radius - inner)
    }
}

/// The Pencil tool — freehand hard-round painting (spec §9.2).
#[derive(Debug, Clone, Copy)]
pub struct Pencil {
    /// The active brush.
    pub brush: Brush,
}

impl Pencil {
    /// Creates a pencil with the given brush.
    pub fn new(brush: Brush) -> Self {
        Self { brush }
    }

    /// Rasterizes a stroke over `points` into a [`SetPixels`] command.
    ///
    /// Returns `None` if the stroke does not touch the canvas or `points` is
    /// empty. A single point paints one dab; multiple points paint connected
    /// segments. Painting uses straight-alpha source-over within the stroke's
    /// bounding region; the command captures the prior pixels for undo.
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        let color = self.brush.color;
        let ca = color.a as f32 / 255.0 * self.brush.opacity;
        let (region, after) = self
            .brush
            .rasterize(layer_index, points, doc, ca, |eff, dst| {
                src_over(color, eff, dst)
            })?;
        Some(SetPixels::new(layer_index, region, after).with_label("Pencil Stroke"))
    }
}

/// Straight-alpha source-over of `src` (with effective alpha `sa`) onto `dst`.
fn src_over(src: Color, sa: f32, dst: Color) -> Color {
    let da = dst.a as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let mix = |s: u8, d: u8| -> u8 {
        let s = s as f32 / 255.0;
        let d = d as f32 / 255.0;
        let v = (s * sa + d * da * (1.0 - sa)) / out_a;
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color::rgba(
        mix(src.r, dst.r),
        mix(src.g, dst.g),
        mix(src.b, dst.b),
        (out_a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;

    fn black_pencil(size: u32) -> Pencil {
        Pencil::new(Brush::new(size, Color::BLACK, 1.0))
    }

    #[test]
    fn single_dab_paints_center_pixel() {
        let mut doc = Document::new(20, 20).unwrap();
        let mut cmd = black_pencil(6)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), Some(Color::BLACK));
    }

    #[test]
    fn stroke_off_canvas_returns_none() {
        let doc = Document::new(10, 10).unwrap();
        assert!(black_pencil(4)
            .stroke(0, &[Point::new(-50.0, -50.0)], &doc)
            .is_none());
    }

    #[test]
    fn stroke_paints_a_connected_line() {
        let mut doc = Document::new(40, 40).unwrap();
        let mut cmd = black_pencil(4)
            .stroke(0, &[Point::new(5.0, 5.0), Point::new(35.0, 5.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        // A point midway along the line is painted.
        assert_eq!(doc.layers[0].pixels.get_pixel(20, 5), Some(Color::BLACK));
    }

    #[test]
    fn stroke_is_undoable_to_transparent() {
        let mut doc = Document::new(20, 20).unwrap();
        let mut cmd = black_pencil(8)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(10, 10),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn half_opacity_over_transparent_yields_half_alpha() {
        let mut doc = Document::new(10, 10).unwrap();
        let pencil = Pencil::new(Brush::new(6, Color::BLACK, 0.5));
        let mut cmd = pencil.stroke(0, &[Point::new(5.0, 5.0)], &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let a = doc.layers[0].pixels.get_pixel(5, 5).unwrap().a;
        assert!((126..=130).contains(&a), "got {a}");
    }

    #[test]
    fn soft_round_edge_is_softer_than_center() {
        // A fully-soft brush is opaque at the center and fades toward the rim.
        let mut doc = Document::new(20, 20).unwrap();
        let brush = Brush::new(20, Color::BLACK, 1.0)
            .with_shape(BrushShape::SoftRound)
            .with_hardness(0.0);
        let mut cmd = Pencil::new(brush)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let center = doc.layers[0].pixels.get_pixel(10, 10).unwrap().a;
        let rim = doc.layers[0].pixels.get_pixel(10, 2).unwrap().a;
        assert!(center > 200, "center alpha {center}");
        assert!(
            rim > 0 && rim < center,
            "rim alpha {rim} vs center {center}"
        );
    }

    #[test]
    fn stroke_outside_selection_writes_nothing() {
        // Select only the left half; paint a dab centered in the right half.
        let mut doc = Document::new(20, 20).unwrap();
        doc.selection = Some(SelectionMask::rectangle(20, 20, Rect::new(0, 0, 10, 20)));
        let result = black_pencil(6).stroke(0, &[Point::new(16.0, 10.0)], &doc);
        // The dab lands entirely in the unselected half, so nothing is painted.
        if let Some(mut cmd) = result {
            cmd.apply(&mut doc).unwrap();
        }
        assert_eq!(
            doc.layers[0].pixels.get_pixel(16, 10),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn stroke_straddling_selection_edge_writes_only_selected_side() {
        // Selection covers x < 10. A wide dab on the boundary paints the left
        // side but leaves the right side untouched.
        let mut doc = Document::new(20, 20).unwrap();
        doc.selection = Some(SelectionMask::rectangle(20, 20, Rect::new(0, 0, 10, 20)));
        let mut cmd = black_pencil(12)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(7, 10), Some(Color::BLACK));
        assert_eq!(
            doc.layers[0].pixels.get_pixel(13, 10),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn flat_brush_paints_along_its_45_degree_axis() {
        // The flat nib's major axis runs along the +/+ diagonal; a point on that
        // diagonal is covered, while an equidistant point off-axis is not.
        let mut doc = Document::new(20, 20).unwrap();
        let brush = Brush::new(20, Color::BLACK, 1.0).with_shape(BrushShape::Flat);
        let mut cmd = Pencil::new(brush)
            .stroke(0, &[Point::new(10.0, 10.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(15, 15), Some(Color::BLACK));
        assert_eq!(
            doc.layers[0].pixels.get_pixel(15, 5),
            Some(Color::TRANSPARENT)
        );
    }
}
