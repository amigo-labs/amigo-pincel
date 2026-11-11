//! Drawing tools (spec §9).
//!
//! M5–M6 scope: Pencil (hard/soft/flat brush), Eraser, Fill, Eyedropper and
//! Move. Each tool turns a gesture (a polyline of points, or a seed/sample
//! point) into a [`SetPixels`] command — or, for the Eyedropper, a sampled
//! color. The full pointer-event `Tool` trait with modifiers and cursors
//! (spec §9.1) is deferred; tools keep this simpler "gesture → command" shape
//! until a tool needs richer modifier/cursor state.

mod box_shapes;
mod clone_stamp;
mod copy;
mod curve;
mod delete;
mod eraser;
mod eyedropper;
mod fill;
mod fill_selection;
mod gradient;
mod move_tool;
mod recolor;
mod shapes;
mod smudge;
mod text;

pub use box_shapes::{STAR_INNER_RATIO, STAR_POINTS, box_outline};
pub use clone_stamp::{CloneSnapshot, CloneStamp};
pub use copy::copy_selection;
pub use curve::{Curve, CurveLine, LineCap, MAX_LINE_POINTS};
pub use delete::delete_selection;
pub use eraser::{Eraser, EraserMode};
pub use eyedropper::{Eyedropper, SampleSize};
pub use fill::{Fill, FillOptions, SampleSource};
pub use fill_selection::fill_selection;
pub use gradient::{Gradient, GradientChannel, GradientKind};
pub use move_tool::Move;
pub use recolor::{Recolor, RecolorOrigins};
pub use shapes::{DashPattern, Shape, ShapeMode, ShapeStyle, Shapes};
pub use smudge::{Smudge, SmudgeCarry};
pub use text::{FontFace, LineLayout, Text, TextAlign, TextLayout, TextStyle};

use crate::color::Color;
use crate::command::HistoryKind;
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
    /// A flat (calligraphic) nib along [`Brush::angle`] (or the pen's
    /// azimuth, see [`Brush::follow_tilt`]), softened per `hardness`.
    Flat,
}

/// What pen pressure varies along a stroke (spec §17.4). Both off means the
/// brush ignores pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PressureDynamics {
    /// Pressure scales the dab diameter.
    pub size: bool,
    /// Pressure scales the dab opacity.
    pub opacity: bool,
}

/// Smallest dab radius pressure can shrink a brush to, in pixels.
const MIN_PRESSURE_RADIUS: f32 = 0.5;

/// Minor/major axis ratio of the [`BrushShape::Flat`] nib.
const FLAT_ASPECT: f32 = 0.35;

/// Default [`Brush::angle`]: the flat nib's historical 45° diagonal.
pub const DEFAULT_BRUSH_ANGLE: f32 = 45.0;

/// Per-point pen input for one stroke (spec §17.4). Entry `i` belongs to
/// `points[i]`; a missing entry means "no reading" (full pressure, the
/// brush's fixed angle).
#[derive(Debug, Clone, Copy, Default)]
pub struct PenSamples<'a> {
    /// Pressure in `[0, 1]`.
    pub pressure: &'a [f32],
    /// Pen azimuth in radians, screen space (0 = +x, clockwise toward +y);
    /// a non-finite value means the pen reported no tilt.
    pub azimuth: &'a [f32],
}

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
    /// What pen pressure varies (see [`Pencil::stroke_with_pressure`]).
    pub dynamics: PressureDynamics,
    /// Orientation of the [`BrushShape::Flat`] nib's long axis, in degrees
    /// clockwise from +x.
    pub angle: f32,
    /// The flat nib follows the pen's azimuth instead of `angle` wherever
    /// the pen reports tilt.
    pub follow_tilt: bool,
    /// The hard round tip blends its edge pixels by how much of them it
    /// covers; off, a pixel is painted exactly when its centre lies inside
    /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03). Soft and
    /// flat tips are unaffected (AC-04).
    pub anti_alias: bool,
    /// Normal composites the paint over the layer; Overwrite replaces the
    /// pixels, alpha included (docs/specs/image-edit-extras-tool-options.md).
    /// Only the Pencil (Brush) reads it.
    pub blend: Blend,
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
            dynamics: PressureDynamics::default(),
            angle: DEFAULT_BRUSH_ANGLE,
            follow_tilt: false,
            anti_alias: false,
            blend: Blend::Normal,
        }
    }

    /// Sets the flat nib's angle in degrees.
    pub fn with_angle(mut self, degrees: f32) -> Self {
        self.angle = if degrees.is_finite() {
            degrees
        } else {
            DEFAULT_BRUSH_ANGLE
        };
        self
    }

    /// Lets the flat nib follow the pen's azimuth.
    pub fn with_follow_tilt(mut self, follow: bool) -> Self {
        self.follow_tilt = follow;
        self
    }

    /// Returns this brush with its hard round edge anti-aliased or hard.
    pub fn with_anti_alias(mut self, anti_alias: bool) -> Self {
        self.anti_alias = anti_alias;
        self
    }

    /// Sets how the Brush combines its paint with the layer.
    pub fn with_blend(mut self, blend: Blend) -> Self {
        self.blend = blend;
        self
    }

    /// Nib angle in radians for a pen `azimuth` reading (if any).
    fn dab_angle(&self, azimuth: Option<f32>) -> f32 {
        match azimuth {
            Some(a) if self.follow_tilt && a.is_finite() => a,
            _ => self.angle.to_radians(),
        }
    }

    /// Sets what pen pressure varies.
    pub fn with_dynamics(mut self, dynamics: PressureDynamics) -> Self {
        self.dynamics = dynamics;
        self
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

    /// Dab radius at pressure `p` in `[0, 1]`.
    fn dab_radius(&self, p: f32) -> f32 {
        if self.dynamics.size {
            (self.radius() * p).max(MIN_PRESSURE_RADIUS)
        } else {
            self.radius()
        }
    }

    /// Dab strength factor at pressure `p` in `[0, 1]`.
    fn dab_strength(&self, p: f32) -> f32 {
        if self.dynamics.opacity { p } else { 1.0 }
    }

    /// Coverage in `[0.0, 1.0]` of a tip of `radius`, nib angle `(cos, sin)`,
    /// at offset `(dx, dy)` from its center.
    fn coverage(&self, dx: f32, dy: f32, radius: f32, (cos, sin): (f32, f32)) -> f32 {
        match self.shape {
            BrushShape::HardRound if self.anti_alias => {
                // The share of the pixel inside the circle, approximated by
                // the centre's distance to the edge.
                (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
            }
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
                // Rotate the offset into the nib's frame, then test against a
                // squashed unit ellipse (major axis = radius, minor = aspect).
                let lx = dx * cos + dy * sin;
                let ly = -dx * sin + dy * cos;
                let minor = (radius * FLAT_ASPECT).max(0.5);
                let d = ((lx / radius).powi(2) + (ly / minor).powi(2)).sqrt();
                soft_falloff(d, 1.0, self.hardness)
            }
        }
    }

    /// Rasterizes a stroke over `points`, combining the tip coverage with the
    /// existing pixels via `op`. `op` receives the effective source alpha
    /// (`strength * coverage`) and the destination color, and returns the new
    /// color. `pen.pressure[i]` (clamped to `[0, 1]`, missing entries = 1) is
    /// the pen pressure at `points[i]`, applied per [`Brush::dynamics`];
    /// `pen.azimuth[i]` turns the flat nib when [`Brush::follow_tilt`] is on.
    /// Returns the painted region and its new pixels, or `None` if the stroke
    /// misses the canvas, the layer is invalid, `strength <= 0`, or no dab
    /// reaches a pixel (all of them outside the selection).
    pub(crate) fn rasterize<F>(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
        strength: f32,
        mut op: F,
    ) -> Option<(Rect, ImageBuffer)>
    where
        F: FnMut(DabPixel) -> Color,
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
        let at = |i: usize| Dab {
            p: pen.pressure.get(i).copied().unwrap_or(1.0).clamp(0.0, 1.0),
            angle: self.dab_angle(pen.azimuth.get(i).copied()),
        };
        // Whether any dab reached a pixel: a stroke that reaches none is no
        // change at all and must not become an (empty) undo step.
        let mut touched = false;
        let mut op = |p: DabPixel| {
            touched = true;
            op(p)
        };
        if points.len() == 1 {
            self.stamp(&mut after, points[0], at(0), &ctx, &mut op);
        } else {
            for (i, pair) in points.windows(2).enumerate() {
                let (a, b) = ((pair[0], at(i)), (pair[1], at(i + 1)));
                self.stamp_segment(&mut after, a, b, &ctx, &mut op);
            }
        }
        touched.then_some((region, after))
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

    /// Stamps one dab centered at canvas-space `center` into `buf` (whose
    /// origin is `ctx.region`'s top-left), applying `op` per covered pixel.
    fn stamp<F>(&self, buf: &mut ImageBuffer, center: Point, dab: Dab, ctx: &StrokeCtx, op: &mut F)
    where
        F: FnMut(DabPixel) -> Color,
    {
        let region = ctx.region;
        let radius = self.dab_radius(dab.p);
        let strength = ctx.strength * self.dab_strength(dab.p);
        let rot = (dab.angle.cos(), dab.angle.sin());
        let cx0 = (center.x - radius).floor() as i32;
        let cy0 = (center.y - radius).floor() as i32;
        let cx1 = (center.x + radius).ceil() as i32;
        let cy1 = (center.y + radius).ceil() as i32;
        // A dab narrower than a pixel diagonal can fall between pixel
        // centers and miss every one; it still paints the pixel it lies in.
        let own = (radius < std::f32::consts::FRAC_1_SQRT_2)
            .then(|| (center.x.floor() as i32, center.y.floor() as i32));
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let dx = cx as f32 + 0.5 - center.x;
                let dy = cy as f32 + 0.5 - center.y;
                let cov = if own == Some((cx, cy)) {
                    self.coverage(0.0, 0.0, radius, rot)
                } else {
                    self.coverage(dx, dy, radius, rot)
                };
                if cov <= 0.0 {
                    continue;
                }
                let mut eff = (strength * cov).clamp(0.0, 1.0);
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
                let pixel = DabPixel {
                    eff,
                    dst,
                    // Both are inside the region, so inside the canvas.
                    x: cx as u32,
                    y: cy as u32,
                    dx: cx - center.x.round() as i32,
                    dy: cy - center.y.round() as i32,
                };
                buf.set_pixel(lx as u32, ly as u32, op(pixel));
            }
        }
    }

    /// Stamps dabs evenly along the segment `a`→`b`, each end a point and
    /// its dab; pressure is interpolated linearly, the nib angle along the
    /// shorter way round (a flat nib looks the same turned by 180°).
    fn stamp_segment<F>(
        &self,
        buf: &mut ImageBuffer,
        (a, da): (Point, Dab),
        (b, db): (Point, Dab),
        ctx: &StrokeCtx,
        op: &mut F,
    ) where
        F: FnMut(DabPixel) -> Color,
    {
        let dist = a.distance(b);
        // Spacing of a quarter dab diameter keeps strokes solid without
        // overdraw blowup; the thinner end sets it so light pressure stays solid.
        let spacing = (self.dab_radius(da.p.min(db.p)) * 0.5).max(1.0);
        let steps = (dist / spacing).ceil() as i32;
        if steps == 0 {
            self.stamp(buf, a, da, ctx, op);
            return;
        }
        let turn = nib_turn(da.angle, db.angle);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let dab = Dab {
                p: da.p + (db.p - da.p) * t,
                angle: da.angle + turn * t,
            };
            self.stamp(buf, p, dab, ctx, op);
        }
    }
}

/// One pixel under a dab, handed to the per-tool operation.
pub(crate) struct DabPixel {
    /// Coverage after brush falloff, pressure and the selection, 0.0–1.0.
    pub eff: f32,
    /// The pixel as it stands before this dab.
    pub dst: Color,
    /// Canvas coordinates. Tools that carry state across the dabs of a
    /// drag key it by this; the plain brushes ignore it.
    pub x: u32,
    pub y: u32,
    /// Offset from the dab's centre, in whole pixels. The smudge reads its
    /// carried sample by this; every other tool ignores it.
    pub dx: i32,
    pub dy: i32,
}

/// Pressure and nib angle (radians) of one dab.
#[derive(Debug, Clone, Copy)]
struct Dab {
    p: f32,
    angle: f32,
}

/// Signed turn from nib angle `a` to `b`, in `(-π/2, π/2]`: the nib is
/// symmetric under a half turn, so it never rotates further than 90°.
fn nib_turn(a: f32, b: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let d = (b - a + FRAC_PI_2).rem_euclid(PI) - FRAC_PI_2;
    if d <= -FRAC_PI_2 { d + PI } else { d }
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
    /// Returns `None` if the stroke does not touch the canvas or the
    /// selection, or `points` is empty, so such a stroke adds no undo step.
    /// A single point paints one dab; multiple points paint connected
    /// segments. Painting uses straight-alpha source-over within the stroke's
    /// bounding region; the command captures the prior pixels for undo.
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        self.stroke_with_pen(layer_index, points, PenSamples::default(), doc)
    }

    /// [`Pencil::stroke`] with pen pressure: `pressure[i]` in `[0, 1]` belongs
    /// to `points[i]` (missing entries count as full pressure) and varies the
    /// dabs per the brush's [`PressureDynamics`] (spec §17.4).
    pub fn stroke_with_pressure(
        &self,
        layer_index: usize,
        points: &[Point],
        pressure: &[f32],
        doc: &Document,
    ) -> Option<SetPixels> {
        let pen = PenSamples {
            pressure,
            azimuth: &[],
        };
        self.stroke_with_pen(layer_index, points, pen, doc)
    }

    /// [`Pencil::stroke`] with full pen input: pressure and azimuth per
    /// point (spec §17.4).
    pub fn stroke_with_pen(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
    ) -> Option<SetPixels> {
        self.stroke_with_memory(
            layer_index,
            points,
            pen,
            doc,
            &mut OverwriteMemory::default(),
        )
    }

    /// [`Pencil::stroke_with_pen`] for a drag delivered in several calls:
    /// under [`Blend::Overwrite`] `memory` keeps each pixel's value before
    /// the drag and the highest coverage any dab gave it, so overlapping
    /// dabs never build up (docs/specs/image-edit-extras-tool-options.md
    /// AC-11, AC-12). Normal blending ignores it.
    pub fn stroke_with_memory(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
        memory: &mut OverwriteMemory,
    ) -> Option<SetPixels> {
        let color = self.brush.color;
        let ca = color.a as f32 / 255.0 * self.brush.opacity;
        let (region, after) = match self.brush.blend {
            Blend::Normal => self
                .brush
                .rasterize(layer_index, points, pen, doc, ca, |p| {
                    src_over(color, p.eff, p.dst)
                })?,
            // Coverage alone drives the dabs; the paint alpha is the target.
            Blend::Overwrite => self
                .brush
                .rasterize(layer_index, points, pen, doc, 1.0, |p| {
                    let (before, c) = memory.reach(p.x, p.y, p.dst, p.eff);
                    overwrite(before, color, ca, c)
                })?,
        };
        Some(SetPixels::new(layer_index, region, after).with_kind(HistoryKind::Brush))
    }
}

/// How a painting tool combines its paint with the layer
/// (docs/specs/image-edit-extras-tool-options.md).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Blend {
    /// Source-over of the paint at coverage × paint alpha.
    #[default]
    Normal,
    /// Each pixel moves toward the paint, alpha included, by its coverage.
    Overwrite,
}

impl Blend {
    /// The paint at `pa` (0–1) over `dst` at `coverage` (0–1).
    pub(crate) fn paint(self, paint: Color, pa: f32, coverage: f32, dst: Color) -> Color {
        match self {
            Self::Normal => src_over(paint, pa * coverage, dst),
            Self::Overwrite => overwrite(dst, paint, pa, coverage),
        }
    }
}

/// What an Overwrite Brush drag remembers per pixel: its value before the
/// drag and the highest coverage the drag has given it.
#[derive(Debug, Default, Clone)]
pub struct OverwriteMemory {
    seen: std::collections::HashMap<(u32, u32), (Color, f32)>,
}

impl OverwriteMemory {
    /// Forget everything; a fresh drag starts over.
    pub fn clear(&mut self) {
        self.seen.clear();
    }

    /// Records coverage `c` at `(x, y)` (whose current value is `now`) and
    /// returns the pixel before the drag with the drag's highest coverage.
    fn reach(&mut self, x: u32, y: u32, now: Color, c: f32) -> (Color, f32) {
        let entry = self.seen.entry((x, y)).or_insert((now, 0.0));
        entry.1 = entry.1.max(c);
        *entry
    }
}

/// `prev` moved toward `paint` with alpha `pa` (0–1) by coverage `c` (0–1),
/// in premultiplied space so a transparent `prev` takes the paint's own hue.
pub(crate) fn overwrite(prev: Color, paint: Color, pa: f32, c: f32) -> Color {
    let c = c.clamp(0.0, 1.0);
    let pa = pa.clamp(0.0, 1.0);
    let da = f32::from(prev.a) / 255.0;
    let a = da + (pa - da) * c;
    if a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let ch = |d: u8, s: u8| -> u8 {
        let premul = f32::from(d) * da * (1.0 - c) + f32::from(s) * pa * c;
        (premul / a).round().clamp(0.0, 255.0) as u8
    };
    Color::rgba(
        ch(prev.r, paint.r),
        ch(prev.g, paint.g),
        ch(prev.b, paint.b),
        (a * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Straight-alpha source-over of `src` (with effective alpha `sa`) onto `dst`.
pub(crate) fn src_over(src: Color, sa: f32, dst: Color) -> Color {
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
        assert!(
            black_pencil(4)
                .stroke(0, &[Point::new(-50.0, -50.0)], &doc)
                .is_none()
        );
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

    fn painted(doc: &Document) -> usize {
        (0..doc.canvas.height())
            .flat_map(|y| (0..doc.canvas.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                doc.layers[0]
                    .pixels
                    .get_pixel(x, y)
                    .is_some_and(|c| c.a > 0)
            })
            .count()
    }

    fn with_dynamics(size: bool, opacity: bool) -> Pencil {
        Pencil::new(
            Brush::new(10, Color::BLACK, 1.0).with_dynamics(PressureDynamics { size, opacity }),
        )
    }

    #[test]
    fn stroke_with_full_pressure_matches_plain_stroke() {
        let pts = [Point::new(5.0, 10.0), Point::new(30.0, 12.0)];
        let mut a = Document::new(40, 24).unwrap();
        let mut b = Document::new(40, 24).unwrap();
        with_dynamics(true, true)
            .stroke(0, &pts, &a)
            .unwrap()
            .apply(&mut a)
            .unwrap();
        with_dynamics(true, true)
            .stroke_with_pressure(0, &pts, &[1.0, 1.0], &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
        assert_eq!(a.layers[0].pixels, b.layers[0].pixels);
    }

    #[test]
    fn stroke_with_light_pressure_paints_a_smaller_dab() {
        let pt = [Point::new(12.0, 12.0)];
        let mut full = Document::new(24, 24).unwrap();
        let mut light = Document::new(24, 24).unwrap();
        let pencil = with_dynamics(true, false);
        pencil
            .stroke(0, &pt, &full)
            .unwrap()
            .apply(&mut full)
            .unwrap();
        pencil
            .stroke_with_pressure(0, &pt, &[0.3], &light)
            .unwrap()
            .apply(&mut light)
            .unwrap();
        assert!(painted(&light) * 4 < painted(&full));
    }

    #[test]
    fn stroke_of_one_pixel_brush_on_grid_lines_paints() {
        // Every dab sits on a pixel corner, half a pixel diagonally from the
        // nearest pixel center: still a visible 1 px line.
        let mut doc = Document::new(20, 20).unwrap();
        black_pencil(1)
            .stroke(0, &[Point::new(4.0, 10.0), Point::new(16.0, 10.0)], &doc)
            .unwrap()
            .apply(&mut doc)
            .unwrap();
        assert!(painted(&doc) >= 12, "painted {}", painted(&doc));
    }

    #[test]
    fn stroke_with_lightest_pressure_on_a_pixel_edge_paints() {
        let mut doc = Document::new(48, 48).unwrap();
        let pts = [Point::new(8.0, 24.0), Point::new(40.0, 24.0)];
        Pencil::new(
            Brush::new(8, Color::BLACK, 1.0).with_dynamics(PressureDynamics {
                size: true,
                opacity: false,
            }),
        )
        .stroke_with_pressure(0, &pts, &[0.1, 0.1], &doc)
        .unwrap()
        .apply(&mut doc)
        .unwrap();
        assert!(painted(&doc) >= 32, "painted {}", painted(&doc));
    }

    #[test]
    fn stroke_with_pressure_opacity_lowers_alpha_only_when_enabled() {
        let pt = [Point::new(12.0, 12.0)];
        let alpha = |pencil: Pencil| {
            let mut doc = Document::new(24, 24).unwrap();
            pencil
                .stroke_with_pressure(0, &pt, &[0.5], &doc)
                .unwrap()
                .apply(&mut doc)
                .unwrap();
            doc.layers[0].pixels.get_pixel(12, 12).unwrap().a
        };
        assert!((126..=130).contains(&alpha(with_dynamics(false, true))));
        assert_eq!(alpha(with_dynamics(false, false)), 255);
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
    fn stroke_outside_selection_returns_none() {
        // No command, so the stroke adds no empty undo step.
        let mut doc = Document::new(20, 20).unwrap();
        doc.selection = Some(SelectionMask::rectangle(20, 20, Rect::new(0, 0, 10, 20)));
        let line = [Point::new(14.0, 4.0), Point::new(16.0, 16.0)];
        assert!(black_pencil(6).stroke(0, &line, &doc).is_none());
        let eraser = Eraser::new(Brush::new(6, Color::BLACK, 1.0), EraserMode::ToTransparent);
        assert!(eraser.stroke(0, &line, &doc).is_none());
        // The same stroke inside the selection paints.
        doc.selection = None;
        assert!(black_pencil(6).stroke(0, &line, &doc).is_some());
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

    fn flat_dab(brush: Brush, pen: PenSamples) -> Document {
        let mut doc = Document::new(20, 20).unwrap();
        let mut cmd = Pencil::new(brush)
            .stroke_with_pen(0, &[Point::new(10.0, 10.0)], pen, &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        doc
    }

    #[test]
    fn flat_brush_at_135_degrees_covers_the_other_diagonal() {
        let brush = Brush::new(20, Color::BLACK, 1.0)
            .with_shape(BrushShape::Flat)
            .with_angle(135.0);
        let doc = flat_dab(brush, PenSamples::default());
        assert_eq!(doc.layers[0].pixels.get_pixel(5, 15), Some(Color::BLACK));
        assert_eq!(
            doc.layers[0].pixels.get_pixel(15, 15),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn flat_brush_follows_azimuth_only_when_enabled() {
        let azimuth = [135f32.to_radians()];
        let pen = PenSamples {
            pressure: &[],
            azimuth: &azimuth,
        };
        let brush = Brush::new(20, Color::BLACK, 1.0).with_shape(BrushShape::Flat);
        let fixed = flat_dab(brush, pen);
        assert_eq!(fixed.layers[0].pixels.get_pixel(15, 15), Some(Color::BLACK));
        let tilted = flat_dab(brush.with_follow_tilt(true), pen);
        assert_eq!(tilted.layers[0].pixels.get_pixel(5, 15), Some(Color::BLACK));
        assert_eq!(
            tilted.layers[0].pixels.get_pixel(15, 15),
            Some(Color::TRANSPARENT)
        );
        // A pen without tilt (NaN) falls back to the brush angle.
        let none = [f32::NAN];
        let untilted = flat_dab(
            brush.with_follow_tilt(true),
            PenSamples {
                pressure: &[],
                azimuth: &none,
            },
        );
        assert_eq!(
            untilted.layers[0].pixels.get_pixel(15, 15),
            Some(Color::BLACK)
        );
    }

    #[test]
    fn nib_turn_takes_the_short_way_across_zero() {
        let a = 350f32.to_radians();
        let b = 10f32.to_radians();
        assert!((nib_turn(a, b) - 20f32.to_radians()).abs() < 1e-4);
        // A half turn apart is the same nib: no rotation at all.
        assert!(nib_turn(0.0, std::f32::consts::PI).abs() < 1e-4);
        // 100° one way is 80° the other way round.
        assert!((nib_turn(0.0, 100f32.to_radians()) + 80f32.to_radians()).abs() < 1e-4);
    }
}
