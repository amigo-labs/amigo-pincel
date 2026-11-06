//! `PencilStroke` command — one press-drag-release Pixel-mode Pencil or
//! Eraser stroke with a square / round brush of size 1–8 and optional
//! pixel-perfect corner pruning. See `docs/specs/pincel.md` §5.2 and the
//! Decision Log entries of 2026-10-29. Pen pressure scales each stamp to
//! `max(1, round(N × p))` (`docs/specs/pixel-pressure.md`).
//!
//! The command records its input samples and replays them on every
//! [`Command::apply`], so redo reproduces the stroke exactly. While the
//! gesture is live the bus extends the command on its undo top
//! ([`super::Bus::continue_stroke`]) instead of merging per-pixel writes:
//! pixel-perfect needs the whole stroke in one command. Out-of-cel pixels
//! are skipped silently, like [`super::DrawLine`], unless they lie on the
//! canvas: then the stroke grows the cel to cover them and its undo record
//! restores the cel's old bounds.
//!
//! Symmetry ([`Mirror`]) and tiled mode ([`Wrap`]) act on the stamped
//! pixels (`docs/specs/symmetry-tiled-mode.md`): every stamped pixel is
//! also stamped at its mirror images, then each one is wrapped into the
//! canvas along the wrapped axes. Both happen here so the stroke stays one
//! undo entry and Bresenham segments stay continuous across a seam.

use std::collections::HashMap;

use crate::document::{
    Cel, CelData, CelMap, ColorMode, FrameIndex, LayerId, PixelBuffer, Rgba, Sprite,
};
use crate::geometry::Point;

use super::Command;
use super::dirty::DirtyRegion;
use super::draw_line::bresenham;
use super::error::CommandError;
use super::history::HistoryKind;

/// Smallest Pixel-mode brush size.
pub const MIN_BRUSH_SIZE: u8 = 1;
/// Largest Pixel-mode brush size.
pub const MAX_BRUSH_SIZE: u8 = 8;

/// Footprint shape of a [`PixelBrush`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushShape {
    /// Filled N×N square.
    #[default]
    Square,
    /// N×N disc; from size 3 on the corners of the bounding square stay
    /// unpainted (size 3 is the 5-pixel plus).
    Round,
}

/// Pixel-mode brush: a size in `MIN_BRUSH_SIZE..=MAX_BRUSH_SIZE` and a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBrush {
    size: u8,
    shape: BrushShape,
}

impl PixelBrush {
    /// Build a brush; `size` is clamped to `MIN_BRUSH_SIZE..=MAX_BRUSH_SIZE`.
    pub fn new(size: u8, shape: BrushShape) -> Self {
        Self {
            size: size.clamp(MIN_BRUSH_SIZE, MAX_BRUSH_SIZE),
            shape,
        }
    }

    /// Brush size (edge length of the bounding square).
    pub fn size(&self) -> u8 {
        self.size
    }

    /// Brush shape.
    pub fn shape(&self) -> BrushShape {
        self.shape
    }

    /// Offsets of the stamp's pixels relative to the stamped pixel. The
    /// bounding square spans `-N/2 ..= N - 1 - N/2` on both axes (integer
    /// division), so odd sizes centre on the pixel and even sizes put the
    /// extra row / column on the top and left — Aseprite's brush origin.
    pub fn offsets(&self) -> Vec<(i32, i32)> {
        let n = i32::from(self.size);
        let origin = -(n / 2);
        // Round mask in doubled coordinates: cell (i, j) of the bounding
        // square sits at (2i - (n-1), 2j - (n-1)) from the centre and is
        // kept while its squared distance is below r² + r with
        // r = (n-1)/2 (both sides ×4). n ≤ 2 is a full square.
        let r2 = n - 1;
        let limit = r2 * r2 + 2 * r2;
        let mut out = Vec::with_capacity((n * n) as usize);
        for j in 0..n {
            for i in 0..n {
                let keep = match self.shape {
                    BrushShape::Square => true,
                    BrushShape::Round => {
                        let dx = 2 * i - r2;
                        let dy = 2 * j - r2;
                        n <= 2 || dx * dx + dy * dy < limit
                    }
                };
                if keep {
                    out.push((origin + i, origin + j));
                }
            }
        }
        out
    }
}

impl Default for PixelBrush {
    fn default() -> Self {
        Self::new(MIN_BRUSH_SIZE, BrushShape::Square)
    }
}

/// How a Pencil stroke changes the pixels it touches (`docs/specs/pincel.md`
/// §5.2, `docs/specs/shading-ink.md`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Ink {
    /// Paint the stroke color.
    #[default]
    Simple,
    /// Step each touched pixel `step` entries along `ramp`, judged against
    /// its value from before the stroke: a pixel matching ramp entry `i`
    /// (its first occurrence) becomes entry `i + step`, clamped to the
    /// ramp; a pixel whose exact RGBA is not in the ramp is left alone.
    Shading {
        /// The ramp, in palette order.
        ramp: Vec<Rgba>,
        /// `1` toward the ramp's end (left button), `-1` toward its start.
        step: i8,
    },
}

impl Ink {
    /// The color a pixel that was `before` the stroke takes, or `None` to
    /// leave it alone.
    fn color_for(&self, paint: Rgba, before: Rgba) -> Option<Rgba> {
        match self {
            Self::Simple => Some(paint),
            Self::Shading { ramp, step } => {
                let i = ramp.iter().position(|&c| c == before)?;
                let j = (i as i64 + i64::from(*step)).clamp(0, ramp.len() as i64 - 1);
                ramp.get(j as usize).copied()
            }
        }
    }
}

/// Symmetry axes of a stroke, in half pixels from the left / top canvas
/// edge: with the vertical axis at `A`, column `x` mirrors to `A − 1 − x`
/// (an even `A` lies on a pixel edge, an odd one through a pixel centre);
/// rows mirror across `axis_y2` the same way. `None` turns that axis off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mirror {
    /// Vertical axis (left–right symmetry), or `None`.
    pub axis_x2: Option<i32>,
    /// Horizontal axis (top–bottom symmetry), or `None`.
    pub axis_y2: Option<i32>,
}

/// Canvas edges a stroke wraps across (tiled mode): along a wrapped axis
/// the stamped coordinates are taken modulo the canvas size instead of
/// being skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Wrap {
    /// Wrap across the left and right edges.
    pub x: bool,
    /// Wrap across the top and bottom edges.
    pub y: bool,
}

/// One Pixel-mode Pencil / Eraser stroke (one undo entry).
#[derive(Debug, Clone, PartialEq)]
pub struct PencilStroke {
    layer: LayerId,
    frame: FrameIndex,
    /// Paint color; the Eraser passes [`Rgba::TRANSPARENT`].
    color: Rgba,
    brush: PixelBrush,
    pixel_perfect: bool,
    /// What the stroke does to the pixels it touches.
    ink: Ink,
    /// Symmetry axes, if any.
    mirror: Option<Mirror>,
    /// Tiled-mode wrapping.
    wrap: Wrap,
    /// Input samples in order. Never empty; `apply` replays them.
    samples: Vec<Point>,
    /// Pen pressure per sample in `[0, 1]`, parallel to `samples`; a
    /// missing entry means full pressure.
    pressure: Vec<f32>,
    /// Replay state: the current run of size-1 path points after
    /// pixel-perfect pruning (pixel-perfect strokes only); a larger stamp
    /// ends the run.
    path: Vec<Point>,
    /// Replay state: how many kept stamps cover each pixel (pixel-perfect
    /// strokes only). A pruned corner is restored only at zero.
    coverage: HashMap<(i32, i32), u32>,
    /// Replay state: first-touch value of every pixel the stroke wrote,
    /// in sprite coordinates. `revert` writes these back.
    prior: HashMap<(i32, i32), Rgba>,
    /// Replay state: sprite-space bbox of `prior`'s keys.
    bounds: Bounds,
    /// Replay state: the canvas size, read from the sprite on apply.
    canvas: (i32, i32),
    /// Replay state: the cel's position and size before the stroke first
    /// grew it; `revert` crops the cel back to it.
    grown_from: Option<CelRect>,
}

/// A cel's sprite-space position and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CelRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

/// Inclusive sprite-space bbox, empty until the first pixel is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Bounds(Option<(i32, i32, i32, i32)>);

impl Bounds {
    fn add(&mut self, x: i32, y: i32) {
        self.0 = Some(match self.0 {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }

    fn region(&self, layer: LayerId, frame: FrameIndex) -> DirtyRegion {
        match self.0 {
            None => DirtyRegion::None,
            Some((x0, y0, x1, y1)) => DirtyRegion::bbox(layer, frame, x0, y0, x1, y1),
        }
    }
}

/// The image buffer a stroke paints into, plus its sprite-space origin.
struct Target<'a> {
    buffer: &'a mut PixelBuffer,
    position: &'a mut (i32, i32),
}

impl Target<'_> {
    fn rect(&self) -> CelRect {
        CelRect {
            x: self.position.0,
            y: self.position.1,
            w: self.buffer.width,
            h: self.buffer.height,
        }
    }

    /// Move the buffer to cover exactly `r`, keeping the pixels both
    /// rects share; new pixels are transparent.
    fn reshape(&mut self, r: CelRect) {
        let old = self.rect();
        let mut out = PixelBuffer::empty(r.w, r.h, self.buffer.color_mode);
        let x0 = old.x.max(r.x);
        let x1 = (old.x + old.w as i32).min(r.x + r.w as i32);
        if x0 < x1 {
            let len = (x1 - x0) as usize * 4;
            for y in old.y.max(r.y)..(old.y + old.h as i32).min(r.y + r.h as i32) {
                let src = ((y - old.y) as usize * old.w as usize + (x0 - old.x) as usize) * 4;
                let dst = ((y - r.y) as usize * r.w as usize + (x0 - r.x) as usize) * 4;
                out.data[dst..dst + len].copy_from_slice(&self.buffer.data[src..src + len]);
            }
        }
        *self.buffer = out;
        *self.position = (r.x, r.y);
    }

    /// Byte offset of sprite pixel `(x, y)`, or `None` outside the buffer.
    fn offset(&self, x: i32, y: i32) -> Option<usize> {
        let lx = i64::from(x) - i64::from(self.position.0);
        let ly = i64::from(y) - i64::from(self.position.1);
        if lx < 0
            || ly < 0
            || lx >= i64::from(self.buffer.width)
            || ly >= i64::from(self.buffer.height)
        {
            return None;
        }
        Some(((ly as usize) * (self.buffer.width as usize) + lx as usize) * 4)
    }

    fn read(&self, offset: usize) -> Rgba {
        let d = &self.buffer.data[offset..offset + 4];
        Rgba::new(d[0], d[1], d[2], d[3])
    }

    fn write(&mut self, offset: usize, c: Rgba) {
        self.buffer.data[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
    }
}

fn target(
    cels: &mut CelMap,
    layer: LayerId,
    frame: FrameIndex,
) -> Result<Target<'_>, CommandError> {
    let cel = cels
        .get_mut(layer, frame)
        .ok_or(CommandError::MissingCel { layer, frame })?;
    let Cel { position, data, .. } = cel;
    let CelData::Image(buffer) = data else {
        return Err(CommandError::NotAnImageCel { layer, frame });
    };
    if !matches!(buffer.color_mode, ColorMode::Rgba) {
        return Err(CommandError::UnsupportedColorMode);
    }
    Ok(Target { buffer, position })
}

/// `b` is an L-corner between `a` and `c`: `a`→`c` is one diagonal step
/// and `b` is orthogonally adjacent to both.
fn is_l_corner(a: Point, b: Point, c: Point) -> bool {
    (a.x - c.x).abs() == 1
        && (a.y - c.y).abs() == 1
        && ((b.x == a.x && b.y == c.y) || (b.x == c.x && b.y == a.y))
}

/// Pen pressure clamped to `[0, 1]`; NaN is no reading (full pressure).
fn clamp_pressure(p: f32) -> f32 {
    if p.is_nan() { 1.0 } else { p.clamp(0.0, 1.0) }
}

/// Stamp size at pressure `p` in `[0, 1]`: `max(1, round(size × p))`,
/// halves rounding up.
fn pressure_size(size: u8, p: f32) -> u8 {
    let scaled = (f32::from(size) * p + 0.5).floor();
    // `scaled` lies in `0.0..=size`, so the cast is exact.
    (scaled as u8).max(MIN_BRUSH_SIZE)
}

impl PencilStroke {
    /// The history kind: Shading for the Shading ink, Eraser for a stroke
    /// that paints transparent (what the Eraser passes), Pencil otherwise.
    pub(crate) fn history_kind(&self) -> HistoryKind {
        match self.ink {
            Ink::Shading { .. } => HistoryKind::Shading,
            Ink::Simple if self.color == Rgba::TRANSPARENT => HistoryKind::Eraser,
            Ink::Simple => HistoryKind::Pencil,
        }
    }

    /// Start a stroke at `start` on the cel at `(layer, frame)`. Applying
    /// it stamps `brush` at `start`; [`Self::extend_to`] (via
    /// [`super::Bus::continue_stroke`]) adds the following samples.
    /// `pixel_perfect` only takes effect where the stamp size is 1.
    /// `pressure` is the pen pressure at `start` (`1.0` for mouse and
    /// touch); it is clamped to `[0, 1]`, NaN counts as `1.0`, and the
    /// stamp there has size `max(1, round(N × pressure))`.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        color: Rgba,
        brush: PixelBrush,
        pixel_perfect: bool,
        start: Point,
        pressure: f32,
    ) -> Self {
        Self {
            layer,
            frame,
            color,
            brush,
            pixel_perfect,
            ink: Ink::Simple,
            mirror: None,
            wrap: Wrap::default(),
            samples: vec![start],
            pressure: vec![clamp_pressure(pressure)],
            path: Vec::new(),
            coverage: HashMap::new(),
            prior: HashMap::new(),
            bounds: Bounds::default(),
            canvas: (0, 0),
            grown_from: None,
        }
    }

    /// The stroke with `ink` instead of [`Ink::Simple`]; set it before the
    /// stroke is applied.
    pub fn with_ink(mut self, ink: Ink) -> Self {
        self.ink = ink;
        self
    }

    /// The stroke mirrored across `mirror`'s axes (symmetry); set it
    /// before the stroke is applied. Pixel-perfect pruning runs on the
    /// primary path and its mirror images follow it.
    pub fn with_mirror(mut self, mirror: Mirror) -> Self {
        self.mirror = Some(mirror);
        self
    }

    /// The stroke wrapping across the canvas edges `wrap` names (tiled
    /// mode); set it before the stroke is applied. Samples may then lie
    /// outside the canvas: the line between them is traced unwrapped and
    /// each stamped pixel is wrapped.
    pub fn with_wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Extend an applied stroke to `to` with pen pressure `pressure`
    /// (clamped as in [`Self::new`]): stamp the brush at every pixel of
    /// the Bresenham line from the last sample to `to`, each at the
    /// pressure interpolated linearly between the two samples. Returns
    /// the region this segment changed.
    pub fn extend_to(
        &mut self,
        to: Point,
        pressure: f32,
        cels: &mut CelMap,
    ) -> Result<DirtyRegion, CommandError> {
        let last = self.samples.len() - 1;
        let from = (self.samples[last], self.pressure_at(last));
        let to = (to, clamp_pressure(pressure));
        let mut t = target(cels, self.layer, self.frame)?;
        self.samples.push(to.0);
        self.pressure.push(to.1);
        let mut seg = Bounds::default();
        self.trace(&mut t, from, to, &mut seg);
        Ok(seg.region(self.layer, self.frame))
    }

    /// Pressure of sample `i`; a missing entry is full pressure.
    fn pressure_at(&self, i: usize) -> f32 {
        self.pressure.get(i).copied().unwrap_or(1.0)
    }

    /// The stroke path after pixel-perfect pruning (empty unless the
    /// stroke is pixel-perfect).
    #[cfg(test)]
    fn kept_path(&self) -> &[Point] {
        &self.path
    }

    /// Visit every pixel of the line `from → to` except `from` (already
    /// visited as the previous segment's end), each at the pressure
    /// interpolated linearly between the two samples.
    fn trace(&mut self, t: &mut Target, from: (Point, f32), to: (Point, f32), seg: &mut Bounds) {
        let ((a, pa), (b, pb)) = (from, to);
        let steps = (i64::from(b.x) - i64::from(a.x))
            .abs()
            .max((i64::from(b.y) - i64::from(a.y)).abs());
        for (k, (x, y)) in bresenham(a.x, a.y, b.x, b.y).enumerate().skip(1) {
            let f = k as f32 / steps as f32;
            self.step(t, Point::new(x, y), pa * (1.0 - f) + pb * f, seg);
        }
    }

    /// Advance the stroke by one path pixel at pressure `pressure`.
    fn step(&mut self, t: &mut Target, p: Point, pressure: f32, seg: &mut Bounds) {
        let size = pressure_size(self.brush.size(), pressure);
        self.stamp(t, p, size, seg);
        if !self.pixel_perfect {
            return;
        }
        if size != 1 {
            // Pixel-perfect acts only within runs of size-1 points.
            self.path.clear();
            return;
        }
        // Pixel-perfect (Aseprite): when the last three path pixels form
        // an L, drop the middle one and restore it — unless another kept
        // stamp of the stroke also covers it.
        self.path.push(p);
        let n = self.path.len();
        if n < 3 || !is_l_corner(self.path[n - 3], self.path[n - 2], p) {
            return;
        }
        let b = self.path.remove(n - 2);
        for (x, y) in self.images(b.x, b.y) {
            let count = self.coverage.entry((x, y)).or_insert(1);
            *count -= 1;
            if *count > 0 {
                continue;
            }
            self.coverage.remove(&(x, y));
            if let (Some(off), Some(&before)) = (t.offset(x, y), self.prior.get(&(x, y))) {
                t.write(off, before);
                seg.add(x, y);
            }
        }
    }

    /// Where stamping pixel `(x, y)` writes: the pixel and its mirror
    /// images, each wrapped along the wrapped axes, without duplicates.
    fn images(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        let m = self.mirror.unwrap_or_default();
        let xs = [Some(x), m.axis_x2.map(|a| a - 1 - x)];
        let ys = [Some(y), m.axis_y2.map(|b| b - 1 - y)];
        let (w, h) = self.canvas;
        let mut out = Vec::with_capacity(4);
        for &mx in xs.iter().flatten() {
            for &my in ys.iter().flatten() {
                let wx = if self.wrap.x && w > 0 {
                    mx.rem_euclid(w)
                } else {
                    mx
                };
                let wy = if self.wrap.y && h > 0 {
                    my.rem_euclid(h)
                } else {
                    my
                };
                if !out.contains(&(wx, wy)) {
                    out.push((wx, wy));
                }
            }
        }
        out
    }

    /// Grow the cel so it covers every pixel of `pixels` that lies on the
    /// canvas — along a wrapped axis to the full canvas extent — and
    /// remember its bounds from before the stroke's first growth.
    fn cover(&mut self, t: &mut Target, pixels: &[(i32, i32)]) {
        let (w, h) = self.canvas;
        let old = t.rect();
        let (mut x0, mut y0) = (old.x, old.y);
        let (mut x1, mut y1) = (old.x + old.w as i32, old.y + old.h as i32);
        let mut grow = false;
        for &(x, y) in pixels {
            let on_canvas = x >= 0 && y >= 0 && x < w && y < h;
            if !on_canvas || t.offset(x, y).is_some() {
                continue;
            }
            grow = true;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
        if !grow {
            return;
        }
        if self.wrap.x {
            x0 = x0.min(0);
            x1 = x1.max(w);
        }
        if self.wrap.y {
            y0 = y0.min(0);
            y1 = y1.max(h);
        }
        self.grown_from.get_or_insert(old);
        t.reshape(CelRect {
            x: x0,
            y: y0,
            w: (x1 - x0) as u32,
            h: (y1 - y0) as u32,
        });
    }

    /// Stamp the brush shape at size `size` centred on `p`. The one
    /// stamping routine every stroke variant goes through.
    fn stamp(&mut self, t: &mut Target, p: Point, size: u8, seg: &mut Bounds) {
        let mut pixels: Vec<(i32, i32)> = Vec::new();
        for (dx, dy) in PixelBrush::new(size, self.brush.shape()).offsets() {
            for px in self.images(p.x + dx, p.y + dy) {
                if !pixels.contains(&px) {
                    pixels.push(px);
                }
            }
        }
        self.cover(t, &pixels);
        for (x, y) in pixels {
            if self.pixel_perfect {
                *self.coverage.entry((x, y)).or_insert(0) += 1;
            }
            self.paint(t, x, y, seg);
        }
    }

    /// Write the ink's color to one pixel, recording its first-touch
    /// value. The ink judges the pixel by that first-touch value, so a
    /// pixel the stroke passes over again gets the same color.
    fn paint(&mut self, t: &mut Target, x: i32, y: i32, seg: &mut Bounds) {
        let Some(off) = t.offset(x, y) else {
            return;
        };
        let before = t.read(off);
        let first = *self.prior.entry((x, y)).or_insert(before);
        let Some(color) = self.ink.color_for(self.color, first) else {
            return;
        };
        t.write(off, color);
        self.bounds.add(x, y);
        seg.add(x, y);
    }
}

impl Command for PencilStroke {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        self.path.clear();
        self.coverage.clear();
        self.prior.clear();
        self.bounds = Bounds::default();
        self.grown_from = None;
        self.canvas = (
            i32::try_from(doc.width).unwrap_or(i32::MAX),
            i32::try_from(doc.height).unwrap_or(i32::MAX),
        );
        let mut t = target(cels, self.layer, self.frame)?;
        let samples = std::mem::take(&mut self.samples);
        let mut seg = Bounds::default();
        self.step(&mut t, samples[0], self.pressure_at(0), &mut seg);
        for i in 1..samples.len() {
            let from = (samples[i - 1], self.pressure_at(i - 1));
            let to = (samples[i], self.pressure_at(i));
            self.trace(&mut t, from, to, &mut seg);
        }
        self.samples = samples;
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Ok(mut t) = target(cels, self.layer, self.frame) else {
            return;
        };
        for (&(x, y), &c) in &self.prior {
            if let Some(off) = t.offset(x, y) {
                t.write(off, c);
            }
        }
        if let Some(r) = self.grown_from {
            t.reshape(r);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        self.bounds.region(self.layer, self.frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AnyCommand, Bus};
    use crate::document::{Cel, Frame, Layer};
    use std::collections::HashSet;

    const W: u32 = 8;
    const RED: Rgba = Rgba::new(255, 0, 0, 255);
    const BLUE: Rgba = Rgba::new(0, 0, 255, 255);

    fn fixture() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(W, W)
            .add_layer(Layer::image(LayerId::new(1), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBuffer::empty(W, W, ColorMode::Rgba),
        ));
        (sprite, cels)
    }

    fn fill(cels: &mut CelMap, c: Rgba) {
        let mut t = target(cels, LayerId::new(1), FrameIndex::new(0)).expect("cel");
        for y in 0..W as i32 {
            for x in 0..W as i32 {
                let off = t.offset(x, y).expect("in bounds");
                t.write(off, c);
            }
        }
    }

    fn pixel(cels: &mut CelMap, x: i32, y: i32) -> Rgba {
        let t = target(cels, LayerId::new(1), FrameIndex::new(0)).expect("cel");
        let off = t.offset(x, y).expect("in bounds");
        t.read(off)
    }

    fn snapshot(cels: &mut CelMap) -> Vec<u8> {
        target(cels, LayerId::new(1), FrameIndex::new(0))
            .expect("cel")
            .buffer
            .data
            .clone()
    }

    /// Every pixel of the cel equal to `c`.
    fn painted(cels: &mut CelMap, c: Rgba) -> HashSet<(i32, i32)> {
        let mut out = HashSet::new();
        for y in 0..W as i32 {
            for x in 0..W as i32 {
                if pixel(cels, x, y) == c {
                    out.insert((x, y));
                }
            }
        }
        out
    }

    fn stroke(
        color: Rgba,
        size: u8,
        shape: BrushShape,
        pp: bool,
        pts: &[(i32, i32)],
    ) -> PencilStroke {
        stroke_p(color, size, shape, pp, &with_full_pressure(pts))
    }

    /// [`stroke`] with a pen pressure per sample.
    fn stroke_p(
        color: Rgba,
        size: u8,
        shape: BrushShape,
        pp: bool,
        pts: &[(i32, i32, f32)],
    ) -> PencilStroke {
        let (x, y, p) = pts[0];
        PencilStroke::new(
            LayerId::new(1),
            FrameIndex::new(0),
            color,
            PixelBrush::new(size, shape),
            pp,
            Point::new(x, y),
            p,
        )
    }

    fn with_full_pressure(pts: &[(i32, i32)]) -> Vec<(i32, i32, f32)> {
        pts.iter().map(|&(x, y)| (x, y, 1.0)).collect()
    }

    /// Run a whole stroke through a bus, the way the wasm gesture API does.
    fn run(
        bus: &mut Bus,
        sprite: &mut Sprite,
        cels: &mut CelMap,
        s: PencilStroke,
        rest: &[(i32, i32)],
    ) {
        run_p(bus, sprite, cels, s, &with_full_pressure(rest));
    }

    /// [`run`] with a pen pressure per sample.
    fn run_p(
        bus: &mut Bus,
        sprite: &mut Sprite,
        cels: &mut CelMap,
        mut s: PencilStroke,
        rest: &[(i32, i32, f32)],
    ) {
        bus.seal();
        s.samples.truncate(1);
        s.pressure.truncate(1);
        bus.execute(s.into(), sprite, cels).expect("stroke applies");
        for &(x, y, p) in rest {
            assert!(
                bus.continue_stroke(Point::new(x, y), p, cels)
                    .expect("segment applies"),
                "stroke stays live on the bus"
            );
        }
        bus.seal();
    }

    fn paint_stroke(
        color: Rgba,
        size: u8,
        shape: BrushShape,
        pp: bool,
        pts: &[(i32, i32)],
    ) -> (Sprite, CelMap, Bus) {
        paint_stroke_p(color, size, shape, pp, &with_full_pressure(pts))
    }

    /// [`paint_stroke`] with a pen pressure per sample.
    fn paint_stroke_p(
        color: Rgba,
        size: u8,
        shape: BrushShape,
        pp: bool,
        pts: &[(i32, i32, f32)],
    ) -> (Sprite, CelMap, Bus) {
        let (mut sprite, mut cels) = fixture();
        let mut bus = Bus::new();
        let s = stroke_p(color, size, shape, pp, pts);
        run_p(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
        (sprite, cels, bus)
    }

    fn expected_stamp(size: u8, shape: BrushShape, cx: i32, cy: i32) -> HashSet<(i32, i32)> {
        PixelBrush::new(size, shape)
            .offsets()
            .into_iter()
            .map(|(dx, dy)| (cx + dx, cy + dy))
            .filter(|&(x, y)| x >= 0 && y >= 0 && x < W as i32 && y < W as i32)
            .collect()
    }

    #[test]
    fn pencil_stroke_square_click_paints_n_by_n_block() {
        for size in 1..=8u8 {
            let (_, mut cels, _) = paint_stroke(RED, size, BrushShape::Square, false, &[(4, 4)]);
            let n = i32::from(size);
            let start = 4 - n / 2;
            let want: HashSet<_> = (start..start + n)
                .flat_map(|y| (start..start + n).map(move |x| (x, y)))
                .filter(|&(x, y)| x < W as i32 && y < W as i32)
                .collect();
            assert_eq!(painted(&mut cels, RED), want, "size {size}");
        }
    }

    #[test]
    fn pencil_stroke_size_3_square_stamps_3x3() {
        let (_, mut cels, _) = paint_stroke(RED, 3, BrushShape::Square, false, &[(2, 2)]);
        let want: HashSet<_> = (1..=3).flat_map(|y| (1..=3).map(move |x| (x, y))).collect();
        assert_eq!(painted(&mut cels, RED), want);
    }

    #[test]
    fn pencil_stroke_even_size_extends_top_left() {
        let (_, mut cels, _) = paint_stroke(RED, 2, BrushShape::Square, false, &[(3, 3)]);
        let want: HashSet<_> = [(2, 2), (3, 2), (2, 3), (3, 3)].into_iter().collect();
        assert_eq!(painted(&mut cels, RED), want);
    }

    #[test]
    fn pencil_stroke_round_shape_skips_corners() {
        let (_, mut cels, _) = paint_stroke(RED, 3, BrushShape::Round, false, &[(3, 3)]);
        let want: HashSet<_> = [(3, 3), (2, 3), (4, 3), (3, 2), (3, 4)]
            .into_iter()
            .collect();
        assert_eq!(painted(&mut cels, RED), want);
        for size in 3..=8u8 {
            let offs: HashSet<_> = PixelBrush::new(size, BrushShape::Round)
                .offsets()
                .into_iter()
                .collect();
            let n = i32::from(size);
            let (lo, hi) = (-(n / 2), n - 1 - n / 2);
            for corner in [(lo, lo), (hi, lo), (lo, hi), (hi, hi)] {
                assert!(!offs.contains(&corner), "size {size} corner {corner:?}");
            }
            assert!(offs.contains(&(0, 0)), "size {size} keeps its centre");
        }
    }

    #[test]
    fn pencil_stroke_size_1_matches_bresenham() {
        let (_, mut cels, _) = paint_stroke(RED, 1, BrushShape::Square, false, &[(0, 0), (7, 3)]);
        let want: HashSet<_> = bresenham(0, 0, 7, 3).collect();
        assert_eq!(painted(&mut cels, RED), want);
    }

    #[test]
    fn pencil_stroke_jump_stamps_every_line_pixel_at_every_size() {
        for size in 1..=8u8 {
            for shape in [BrushShape::Square, BrushShape::Round] {
                let (_, mut cels, _) = paint_stroke(RED, size, shape, false, &[(1, 6), (6, 1)]);
                let want: HashSet<_> = bresenham(1, 6, 6, 1)
                    .flat_map(|(x, y)| expected_stamp(size, shape, x, y))
                    .collect();
                assert_eq!(painted(&mut cels, RED), want, "size {size} {shape:?}");
            }
        }
    }

    #[test]
    fn pencil_stroke_revert_restores_first_touch_values() {
        let (mut sprite, mut cels) = fixture();
        fill(&mut cels, BLUE);
        let before = snapshot(&mut cels);
        let mut bus = Bus::new();
        // A size-5 stroke that crosses the same pixels several times.
        let pts = [(1, 1), (6, 6), (1, 6), (6, 1), (3, 3)];
        let s = stroke(RED, 5, BrushShape::Square, false, &pts);
        run(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
        let after = snapshot(&mut cels);
        assert_eq!(bus.undo_depth(), 1, "one stroke, one undo entry");
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(snapshot(&mut cels), before);
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!(snapshot(&mut cels), after);
    }

    #[test]
    fn pencil_stroke_two_strokes_add_two_undo_entries() {
        let (mut sprite, mut cels) = fixture();
        let mut bus = Bus::new();
        let a = stroke(RED, 1, BrushShape::Square, false, &[(0, 0)]);
        run(&mut bus, &mut sprite, &mut cels, a, &[(3, 0)]);
        let b = stroke(BLUE, 1, BrushShape::Square, false, &[(0, 2)]);
        run(&mut bus, &mut sprite, &mut cels, b, &[(3, 2)]);
        assert_eq!(bus.undo_depth(), 2);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(pixel(&mut cels, 1, 2), Rgba::TRANSPARENT);
        assert_eq!(pixel(&mut cels, 1, 0), RED);
    }

    #[test]
    fn pencil_stroke_out_of_bounds_points_are_skipped() {
        let (_, mut cels, _) = paint_stroke(RED, 8, BrushShape::Square, false, &[(0, 0)]);
        let want: HashSet<_> = (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).collect();
        assert_eq!(painted(&mut cels, RED), want);
        // A stroke that runs entirely outside the canvas is not an error.
        let (_, mut cels, bus) = paint_stroke(
            RED,
            3,
            BrushShape::Round,
            false,
            &[(-10, -10), (-20, 30), (40, 40)],
        );
        assert!(painted(&mut cels, RED).is_empty());
        assert_eq!(bus.undo_depth(), 1);
        // A stroke that crosses the edge paints its in-bounds part.
        let (_, mut cels, _) = paint_stroke(RED, 1, BrushShape::Square, false, &[(-5, 2), (12, 2)]);
        let want: HashSet<_> = (0..W as i32).map(|x| (x, 2)).collect();
        assert_eq!(painted(&mut cels, RED), want);
    }

    #[test]
    fn pencil_stroke_transparent_clears_pencil_footprint() {
        let pts = [(1, 1), (6, 2), (2, 6)];
        for size in [1u8, 2, 3, 6] {
            for shape in [BrushShape::Square, BrushShape::Round] {
                for pp in [false, true] {
                    let (_, mut drawn, _) = paint_stroke(RED, size, shape, pp, &pts);
                    let footprint = painted(&mut drawn, RED);
                    let (mut sprite, mut cels) = fixture();
                    fill(&mut cels, BLUE);
                    let mut bus = Bus::new();
                    let s = stroke(Rgba::TRANSPARENT, size, shape, pp, &pts);
                    run(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
                    assert_eq!(
                        painted(&mut cels, Rgba::TRANSPARENT),
                        footprint,
                        "size {size} {shape:?} pp {pp}"
                    );
                }
            }
        }
    }

    #[test]
    fn pixel_perfect_removes_l_corner() {
        let (mut sprite, mut cels) = fixture();
        fill(&mut cels, BLUE);
        let mut bus = Bus::new();
        let pts = [(0, 0), (1, 0), (1, 1), (2, 1), (2, 2)];
        let s = stroke(RED, 1, BrushShape::Square, true, &pts);
        run(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
        let want: HashSet<_> = [(0, 0), (1, 1), (2, 2)].into_iter().collect();
        assert_eq!(painted(&mut cels, RED), want);
        // Removed corners show their value from before the stroke.
        assert_eq!(pixel(&mut cels, 1, 0), BLUE);
        assert_eq!(pixel(&mut cels, 2, 1), BLUE);
    }

    #[test]
    fn pixel_perfect_keeps_corner_painted_earlier_in_stroke() {
        // (1, 0) is the start; the stroke comes back through it as the
        // L-corner of (1, 1) → (1, 0) → (2, 0).
        let pts = [(1, 0), (0, 1), (1, 2), (1, 0), (2, 0)];
        let (_, mut cels, _) = paint_stroke(RED, 1, BrushShape::Square, true, &pts);
        assert_eq!(pixel(&mut cels, 1, 0), RED);
        // (1, 1) is no corner: (1, 2) → (1, 1) → (1, 0) is straight.
        assert_eq!(pixel(&mut cels, 1, 1), RED);
    }

    #[test]
    fn pixel_perfect_has_no_effect_above_size_1() {
        let pts = [(1, 1), (2, 1), (2, 2), (3, 2), (3, 3)];
        for size in 2..=8u8 {
            let (_, mut on, _) = paint_stroke(RED, size, BrushShape::Square, true, &pts);
            let (_, mut off, _) = paint_stroke(RED, size, BrushShape::Square, false, &pts);
            assert_eq!(snapshot(&mut on), snapshot(&mut off), "size {size}");
        }
    }

    /// Deterministic pseudo-random strokes (a proptest stand-in without a
    /// new dev-dependency): revert always restores the original cel, redo
    /// reproduces the stroke, and a pixel-perfect size-1 stroke keeps no
    /// L-corner and paints exactly its kept path.
    #[test]
    fn pencil_stroke_random_strokes_hold_invariants() {
        let mut seed: u64 = 0x5eed_1234_abcd_ef01;
        let mut next = move |m: i32| -> i32 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) % m as u64) as i32
        };
        for case in 0..400 {
            let size = 1 + next(8) as u8;
            let shape = if next(2) == 0 {
                BrushShape::Square
            } else {
                BrushShape::Round
            };
            let pp = case % 2 == 0;
            let len = 1 + next(8) as usize;
            let pts: Vec<(i32, i32)> = (0..len).map(|_| (next(12) - 2, next(12) - 2)).collect();
            let (mut sprite, mut cels) = fixture();
            fill(&mut cels, BLUE);
            let before = snapshot(&mut cels);
            let mut bus = Bus::new();
            let s = stroke(RED, size, shape, pp, &pts);
            run(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
            let after = snapshot(&mut cels);
            if pp && size == 1 {
                let Some(AnyCommand::PencilStroke(cmd)) = bus.top() else {
                    panic!("stroke on bus");
                };
                let path = cmd.kept_path().to_vec();
                for w in path.windows(3) {
                    assert!(
                        !is_l_corner(w[0], w[1], w[2]),
                        "case {case}: corner in {path:?}"
                    );
                }
                let want: HashSet<_> = path
                    .iter()
                    .map(|p| (p.x, p.y))
                    .filter(|&(x, y)| x >= 0 && y >= 0 && x < W as i32 && y < W as i32)
                    .collect();
                assert_eq!(painted(&mut cels, RED), want, "case {case}");
            }
            assert!(bus.undo(&mut sprite, &mut cels));
            assert_eq!(snapshot(&mut cels), before, "case {case}: revert");
            assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
            assert_eq!(snapshot(&mut cels), after, "case {case}: redo");
        }
    }

    // --- Pen pressure (docs/specs/pixel-pressure.md) ---

    #[test]
    fn pencil_stroke_pressure_scales_stamp_size() {
        // max(1, round(N × p)), halves rounding up.
        let cases: [(u8, f32, u8); 9] = [
            (8, 1.0, 8),
            (8, 0.5, 4),
            (8, 0.25, 2),
            (8, 0.1, 1),
            (5, 0.5, 3),
            (3, 0.5, 2),
            (4, 0.3, 1),
            (1, 0.3, 1),
            (1, 1.0, 1),
        ];
        for (n, p, want) in cases {
            for shape in [BrushShape::Square, BrushShape::Round] {
                let (_, mut cels, _) = paint_stroke_p(RED, n, shape, false, &[(4, 4, p)]);
                assert_eq!(
                    painted(&mut cels, RED),
                    expected_stamp(want, shape, 4, 4),
                    "size {n} pressure {p} {shape:?}"
                );
            }
        }
    }

    #[test]
    fn pencil_stroke_pressure_interpolates_along_segment() {
        // Size 4 from pressure 0 at (0, 3) to 1 at (6, 3): step k has
        // p = k / 6 and size max(1, round(4k / 6)) = 1 1 1 2 3 3 4.
        let (_, mut cels, _) = paint_stroke_p(
            RED,
            4,
            BrushShape::Square,
            false,
            &[(0, 3, 0.0), (6, 3, 1.0)],
        );
        let sizes = [1u8, 1, 1, 2, 3, 3, 4];
        let want: HashSet<_> = sizes
            .iter()
            .enumerate()
            .flat_map(|(k, &n)| expected_stamp(n, BrushShape::Square, k as i32, 3))
            .collect();
        assert_eq!(painted(&mut cels, RED), want);
    }

    #[test]
    fn pencil_stroke_full_pressure_matches_size_n_stamps() {
        for size in 1..=8u8 {
            let pts = [(1, 6, 1.0), (6, 1, 1.0), (2, 2, 1.0)];
            let (_, mut cels, _) = paint_stroke_p(RED, size, BrushShape::Round, false, &pts);
            let want: HashSet<_> = bresenham(1, 6, 6, 1)
                .chain(bresenham(6, 1, 2, 2))
                .flat_map(|(x, y)| expected_stamp(size, BrushShape::Round, x, y))
                .collect();
            assert_eq!(painted(&mut cels, RED), want, "size {size}");
        }
    }

    #[test]
    fn pencil_stroke_light_pressure_paints_fewer_pixels() {
        let at = |p: f32| [(1, 4, p), (6, 4, p)];
        let (_, mut light, _) = paint_stroke_p(RED, 8, BrushShape::Square, false, &at(0.2));
        let (_, mut firm, _) = paint_stroke_p(RED, 8, BrushShape::Square, false, &at(1.0));
        let (light, firm) = (painted(&mut light, RED), painted(&mut firm, RED));
        assert!(
            light.len() < firm.len(),
            "{} vs {}",
            light.len(),
            firm.len()
        );
        assert!(light.is_subset(&firm));
    }

    #[test]
    fn pencil_stroke_pressure_is_clamped_to_unit_range() {
        // Below 0 counts as 0 (still a size-1 stamp), above 1 as 1, and
        // NaN as no reading (full pressure).
        let cases: [(f32, u8); 5] = [
            (-0.5, 1),
            (0.0, 1),
            (1.7, 6),
            (f32::INFINITY, 6),
            (f32::NAN, 6),
        ];
        for (p, want) in cases {
            let (_, mut cels, _) = paint_stroke_p(RED, 6, BrushShape::Square, false, &[(4, 4, p)]);
            assert_eq!(
                painted(&mut cels, RED),
                expected_stamp(want, BrushShape::Square, 4, 4),
                "pressure {p}"
            );
        }
    }

    #[test]
    fn pixel_perfect_applies_only_to_size_1_runs() {
        let (mut sprite, mut cels) = fixture();
        fill(&mut cels, BLUE);
        let mut bus = Bus::new();
        // Firm start (size 3), one size-2 sample, then a light staircase.
        let pts = [
            (1, 1, 1.0),
            (2, 1, 0.5),
            (3, 1, 0.0),
            (3, 2, 0.0),
            (4, 2, 0.0),
            (4, 3, 0.0),
            (5, 3, 0.0),
        ];
        let s = stroke_p(RED, 3, BrushShape::Square, true, &pts);
        run_p(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
        // (3, 1) would be the corner of (2, 1) → (3, 1) → (3, 2), but
        // (2, 1) has size 2, so it is outside the size-1 run and stays.
        assert_eq!(pixel(&mut cels, 3, 1), RED);
        // Inside the run the corners go and show the pixel from before.
        assert_eq!(pixel(&mut cels, 3, 2), BLUE);
        assert_eq!(pixel(&mut cels, 4, 3), BLUE);
        assert_eq!(pixel(&mut cels, 4, 2), RED);
        assert_eq!(pixel(&mut cels, 5, 3), RED);
        // The size-3 stamp is untouched.
        for (x, y) in expected_stamp(3, BrushShape::Square, 1, 1) {
            assert_eq!(pixel(&mut cels, x, y), RED, "({x}, {y})");
        }
    }

    #[test]
    fn pixel_perfect_keeps_corner_covered_by_a_larger_stamp() {
        // The size-3 stamp at (1, 1) covers (2, 2); the light run
        // (2, 1) → (2, 2) → (3, 2) makes it a corner, but it stays.
        let pts = [(1, 1, 1.0), (2, 1, 0.0), (2, 2, 0.0), (3, 2, 0.0)];
        let (_, mut cels, _) = paint_stroke_p(RED, 3, BrushShape::Square, true, &pts);
        assert_eq!(pixel(&mut cels, 2, 2), RED);
    }

    /// Deterministic pseudo-random pressure strokes: every changed pixel
    /// has exactly the stroke color, the Eraser clears exactly what the
    /// Pencil paints, revert restores the original cel, redo reproduces
    /// the stroke, and a pixel-perfect run keeps no L-corner.
    #[test]
    fn pencil_stroke_random_pressure_strokes_hold_invariants() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move |m: i32| -> i32 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) % m as u64) as i32
        };
        for case in 0..400 {
            let size = 1 + next(8) as u8;
            let shape = if next(2) == 0 {
                BrushShape::Square
            } else {
                BrushShape::Round
            };
            let pp = case % 2 == 0;
            let len = 1 + next(8) as usize;
            // Pressures from -0.1 to 1.1, so clamping is exercised too.
            let pts: Vec<(i32, i32, f32)> = (0..len)
                .map(|_| (next(12) - 2, next(12) - 2, next(13) as f32 / 10.0 - 0.1))
                .collect();
            let (mut sprite, mut cels) = fixture();
            fill(&mut cels, BLUE);
            let before = snapshot(&mut cels);
            let mut bus = Bus::new();
            let s = stroke_p(RED, size, shape, pp, &pts);
            run_p(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
            let after = snapshot(&mut cels);
            let footprint = painted(&mut cels, RED);
            assert_eq!(
                footprint.len() + painted(&mut cels, BLUE).len(),
                (W * W) as usize,
                "case {case}: only the stroke color is painted"
            );
            if pp {
                let Some(AnyCommand::PencilStroke(cmd)) = bus.top() else {
                    panic!("stroke on bus");
                };
                let path = cmd.kept_path().to_vec();
                for w in path.windows(3) {
                    assert!(
                        !is_l_corner(w[0], w[1], w[2]),
                        "case {case}: corner in {path:?}"
                    );
                }
            }
            assert!(bus.undo(&mut sprite, &mut cels));
            assert_eq!(snapshot(&mut cels), before, "case {case}: revert");
            assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
            assert_eq!(snapshot(&mut cels), after, "case {case}: redo");

            // The Eraser clears exactly the Pencil's footprint.
            let (mut sprite, mut cels) = fixture();
            fill(&mut cels, BLUE);
            let mut bus = Bus::new();
            let s = stroke_p(Rgba::TRANSPARENT, size, shape, pp, &pts);
            run_p(&mut bus, &mut sprite, &mut cels, s, &pts[1..]);
            assert_eq!(
                painted(&mut cels, Rgba::TRANSPARENT),
                footprint,
                "case {case}: eraser"
            );
        }
    }

    // Shading ink (docs/specs/shading-ink.md).

    const R0: Rgba = Rgba::new(10, 0, 0, 255);
    const R1: Rgba = Rgba::new(20, 0, 0, 255);
    const R2: Rgba = Rgba::new(30, 0, 0, 255);
    const OFF: Rgba = Rgba::new(21, 0, 0, 255);

    fn ramp() -> Vec<Rgba> {
        vec![R0, R1, R2]
    }

    fn shade(step: i8, size: u8, pp: bool, pts: &[(i32, i32)]) -> PencilStroke {
        stroke(RED, size, BrushShape::Square, pp, pts).with_ink(Ink::Shading { ramp: ramp(), step })
    }

    /// A cel whose row `y` is R0 R1 R2 OFF R0 R1 R2 OFF and the rest
    /// transparent.
    fn ramp_row(cels: &mut CelMap, y: i32) {
        let mut t = target(cels, LayerId::new(1), FrameIndex::new(0)).expect("cel");
        for x in 0..W as i32 {
            let off = t.offset(x, y).expect("in bounds");
            t.write(off, [R0, R1, R2, OFF][x as usize % 4]);
        }
    }

    fn row(cels: &mut CelMap, y: i32) -> Vec<Rgba> {
        (0..W as i32).map(|x| pixel(cels, x, y)).collect()
    }

    #[test]
    fn shading_steps_one_entry_toward_end() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 1, false, &[(0, 2)]),
            &[(7, 2)],
        );
        assert_eq!(row(&mut cels, 2), [R1, R2, R2, OFF, R1, R2, R2, OFF]);
    }

    #[test]
    fn shading_right_button_steps_toward_start() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(-1, 1, false, &[(0, 2)]),
            &[(7, 2)],
        );
        assert_eq!(row(&mut cels, 2), [R0, R0, R1, OFF, R0, R0, R1, OFF]);
    }

    #[test]
    fn shading_ignores_colors_outside_ramp() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let mut bus = Bus::new();
        // Row 3 is transparent, which is not in the ramp; OFF is one off R1.
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 1, false, &[(3, 2)]),
            &[(3, 3), (0, 3)],
        );
        assert_eq!(pixel(&mut cels, 3, 2), OFF);
        assert!(painted(&mut cels, Rgba::TRANSPARENT).contains(&(0, 3)));
        assert_eq!(row(&mut cels, 3), vec![Rgba::TRANSPARENT; W as usize]);
    }

    #[test]
    fn shading_clamps_at_ramp_end() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 1, false, &[(2, 2)]),
            &[],
        );
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(-1, 1, false, &[(0, 2)]),
            &[],
        );
        assert_eq!(pixel(&mut cels, 2, 2), R2);
        assert_eq!(pixel(&mut cels, 0, 2), R0);
        // A one-entry ramp leaves its color alone both ways.
        for step in [1, -1] {
            let s = stroke(RED, 1, BrushShape::Square, false, &[(1, 2)]).with_ink(Ink::Shading {
                ramp: vec![R1],
                step,
            });
            run(&mut bus, &mut sprite, &mut cels, s, &[]);
            assert_eq!(pixel(&mut cels, 1, 2), R1);
        }
    }

    #[test]
    fn shading_same_pixel_twice_in_one_stroke_steps_once() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 1, false, &[(0, 2)]),
            &[(1, 2), (0, 2), (1, 2), (0, 2)],
        );
        assert_eq!(pixel(&mut cels, 0, 2), R1);
        assert_eq!(pixel(&mut cels, 1, 2), R2);
        // Replaying the stroke (redo) gives the same result.
        let after = snapshot(&mut cels);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!(snapshot(&mut cels), after);
    }

    #[test]
    fn shading_size_3_shades_every_stamped_pixel() {
        let (mut sprite, mut cels) = fixture();
        for y in 1..=3 {
            ramp_row(&mut cels, y);
        }
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 3, false, &[(1, 2)]),
            &[],
        );
        for y in 1..=3 {
            assert_eq!(&row(&mut cels, y)[..4], [R1, R2, R2, OFF], "row {y}");
        }
        assert_eq!(pixel(&mut cels, 4, 2), R0, "outside the stamp");
    }

    #[test]
    fn shading_stroke_is_one_undo_entry_that_restores_every_pixel() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let before = snapshot(&mut cels);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 2, false, &[(0, 1)]),
            &[(7, 2)],
        );
        assert_eq!(bus.undo_depth(), 1);
        assert_ne!(snapshot(&mut cels), before);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(snapshot(&mut cels), before);
    }

    #[test]
    fn shading_pixel_perfect_restores_a_pruned_corner_to_its_prior_value() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        ramp_row(&mut cels, 3);
        let mut bus = Bus::new();
        // (0, 2) → (1, 2) → (1, 3): (1, 2) is an L-corner and is dropped.
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            shade(1, 1, true, &[(0, 2)]),
            &[(1, 2), (1, 3)],
        );
        assert_eq!(pixel(&mut cels, 0, 2), R1);
        assert_eq!(
            pixel(&mut cels, 1, 2),
            R1,
            "pruned corner keeps its prior value"
        );
        assert_eq!(pixel(&mut cels, 1, 3), R2);
    }

    #[test]
    fn simple_ink_paints_like_a_plain_stroke() {
        let (mut sprite, mut cels) = fixture();
        ramp_row(&mut cels, 2);
        let (mut sprite2, mut cels2) = fixture();
        ramp_row(&mut cels2, 2);
        let mut bus = Bus::new();
        let mut bus2 = Bus::new();
        let pts = [(0, 2)];
        let rest = [(5, 2), (5, 5)];
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            stroke(BLUE, 2, BrushShape::Round, false, &pts),
            &rest,
        );
        let s = stroke(BLUE, 2, BrushShape::Round, false, &pts).with_ink(Ink::Simple);
        run(&mut bus2, &mut sprite2, &mut cels2, s, &rest);
        assert_eq!(snapshot(&mut cels), snapshot(&mut cels2));
    }
}

#[cfg(test)]
mod aid_tests {
    //! Symmetry and tiled-mode wrapping (`docs/specs/symmetry-tiled-mode.md`).

    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, Frame, Layer};
    use std::collections::BTreeSet;

    const RED: Rgba = Rgba::new(255, 0, 0, 255);
    const BLUE: Rgba = Rgba::new(0, 0, 255, 255);
    const L: LayerId = LayerId::new(1);
    const F: FrameIndex = FrameIndex::new(0);

    /// Stroke options under test.
    #[derive(Clone, Copy)]
    struct Aid {
        size: u8,
        pp: bool,
        mirror: Option<Mirror>,
        wrap: Wrap,
    }

    const PLAIN: Aid = Aid {
        size: 1,
        pp: false,
        mirror: None,
        wrap: Wrap { x: false, y: false },
    };

    fn mirror(axis_x2: Option<i32>, axis_y2: Option<i32>) -> Aid {
        Aid {
            mirror: Some(Mirror { axis_x2, axis_y2 }),
            ..PLAIN
        }
    }

    fn wrap(x: bool, y: bool) -> Aid {
        Aid {
            wrap: Wrap { x, y },
            ..PLAIN
        }
    }

    /// A `w`×`h` sprite whose one cel spans `cel` (`x, y, w, h`), filled
    /// with `fill`.
    fn doc(w: u32, h: u32, cel: (i32, i32, u32, u32), fill: Rgba) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(L, "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut buffer = PixelBuffer::empty(cel.2, cel.3, ColorMode::Rgba);
        for px in buffer.data.as_chunks_mut::<4>().0 {
            *px = [fill.r, fill.g, fill.b, fill.a];
        }
        let mut c = Cel::image(L, F, buffer);
        c.position = (cel.0, cel.1);
        let mut cels = CelMap::new();
        cels.insert(c);
        (sprite, cels)
    }

    /// A `w`×`h` sprite with a transparent canvas-sized cel.
    fn canvas(w: u32, h: u32) -> (Sprite, CelMap) {
        doc(w, h, (0, 0, w, h), Rgba::TRANSPARENT)
    }

    /// The cel's position, size and bytes.
    fn cel_state(cels: &CelMap) -> ((i32, i32), u32, u32, Vec<u8>) {
        let cel = cels.get(L, F).expect("cel");
        let CelData::Image(b) = &cel.data else {
            panic!("image cel");
        };
        (cel.position, b.width, b.height, b.data.clone())
    }

    /// Every sprite pixel inside the cel equal to `c`.
    fn painted(cels: &CelMap, c: Rgba) -> BTreeSet<(i32, i32)> {
        let ((px, py), w, h, data) = cel_state(cels);
        let mut out = BTreeSet::new();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let i = ((y as usize) * (w as usize) + x as usize) * 4;
                if data[i..i + 4] == [c.r, c.g, c.b, c.a] {
                    out.insert((px + x, py + y));
                }
            }
        }
        out
    }

    fn set(pts: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
        pts.iter().copied().collect()
    }

    /// Run one stroke through a bus like the wasm gesture API does.
    fn run(bus: &mut Bus, sprite: &mut Sprite, cels: &mut CelMap, aid: Aid, pts: &[(i32, i32)]) {
        let (x, y) = pts[0];
        let mut s = PencilStroke::new(
            L,
            F,
            RED,
            PixelBrush::new(aid.size, BrushShape::Square),
            aid.pp,
            Point::new(x, y),
            1.0,
        )
        .with_wrap(aid.wrap);
        if let Some(m) = aid.mirror {
            s = s.with_mirror(m);
        }
        bus.seal();
        bus.execute(s.into(), sprite, cels).expect("stroke applies");
        for &(x, y) in &pts[1..] {
            assert!(
                bus.continue_stroke(Point::new(x, y), 1.0, cels)
                    .expect("segment applies"),
                "stroke stays live on the bus"
            );
        }
        bus.seal();
    }

    /// The red pixels of one stroke on a fresh transparent `w`×`h` canvas.
    fn stroke_on(w: u32, h: u32, aid: Aid, pts: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
        let (mut sprite, mut cels) = canvas(w, h);
        run(&mut Bus::new(), &mut sprite, &mut cels, aid, pts);
        painted(&cels, RED)
    }

    // --- Symmetry (AC-01 – AC-07) ---

    #[test]
    fn mirror_x_even_width_axis_on_edge() {
        // Width 8, default axis A = 8 half pixels: column x mirrors to 7 − x.
        assert_eq!(
            stroke_on(8, 4, mirror(Some(8), None), &[(1, 2)]),
            set(&[(1, 2), (6, 2)])
        );
        // A pixel next to the edge axis lands on the neighbouring column.
        assert_eq!(
            stroke_on(8, 4, mirror(Some(8), None), &[(3, 0)]),
            set(&[(3, 0), (4, 0)])
        );
    }

    #[test]
    fn mirror_x_odd_width_axis_through_centre_pixel() {
        // Width 15, A = 15: column 7 mirrors onto itself, 2 onto 12.
        assert_eq!(
            stroke_on(15, 2, mirror(Some(15), None), &[(7, 0)]),
            set(&[(7, 0)])
        );
        assert_eq!(
            stroke_on(15, 2, mirror(Some(15), None), &[(2, 1)]),
            set(&[(2, 1), (12, 1)])
        );
    }

    #[test]
    fn mirror_y_stamps_across_horizontal_axis() {
        assert_eq!(
            stroke_on(4, 8, mirror(None, Some(8)), &[(3, 1)]),
            set(&[(3, 1), (3, 6)])
        );
        // An off-centre axis (B = 5, through the centre of row 2).
        assert_eq!(
            stroke_on(4, 8, mirror(None, Some(5)), &[(0, 0), (0, 1)]),
            set(&[(0, 0), (0, 1), (0, 4), (0, 3)])
        );
    }

    #[test]
    fn mirror_xy_stamps_four_points() {
        assert_eq!(
            stroke_on(8, 6, mirror(Some(8), Some(6)), &[(1, 0)]),
            set(&[(1, 0), (6, 0), (1, 5), (6, 5)])
        );
    }

    #[test]
    fn mirror_image_outside_canvas_is_skipped() {
        // Axis at the right edge region (A = 14 on width 8): (1, 0)
        // mirrors to 12, beyond the canvas; (6, 0) mirrors to 7.
        assert_eq!(
            stroke_on(8, 2, mirror(Some(14), None), &[(1, 0)]),
            set(&[(1, 0)])
        );
        assert_eq!(
            stroke_on(8, 2, mirror(Some(14), None), &[(6, 0)]),
            set(&[(6, 0), (7, 0)])
        );
    }

    #[test]
    fn mirror_stroke_is_one_undo_step() {
        let (mut sprite, mut cels) = doc(8, 8, (0, 0, 8, 8), BLUE);
        let before = cel_state(&cels);
        let mut bus = Bus::new();
        let aid = Aid {
            size: 3,
            ..mirror(Some(8), Some(8))
        };
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            aid,
            &[(1, 1), (3, 2), (2, 5)],
        );
        let after = cel_state(&cels);
        assert_ne!(after, before);
        assert_eq!(bus.undo_depth(), 1, "one stroke, one entry");
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(cel_state(&cels), before, "every touched pixel restored");
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!(cel_state(&cels), after, "redo reproduces the stroke");
    }

    #[test]
    fn mirror_with_pixel_perfect_follows_primary_path() {
        // A staircase: the L-corners (1, 0) and (2, 1) are dropped, and
        // so are their mirror images.
        let pts = [(0, 0), (1, 0), (1, 1), (2, 1), (2, 2)];
        let aid = Aid {
            pp: true,
            ..mirror(Some(8), None)
        };
        assert_eq!(
            stroke_on(8, 4, aid, &pts),
            set(&[(0, 0), (1, 1), (2, 2), (7, 0), (6, 1), (5, 2)])
        );
    }

    /// The primary stroke alone, painted into a cel far larger than the
    /// canvas so nothing it stamps is clipped.
    fn primary(aid: Aid, pts: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
        let (mut sprite, mut cels) = doc(10, 9, (-30, -30, 70, 70), Rgba::TRANSPARENT);
        let aid = Aid {
            mirror: None,
            ..aid
        };
        run(&mut Bus::new(), &mut sprite, &mut cels, aid, pts);
        painted(&cels, RED)
    }

    #[test]
    fn mirror_random_strokes_paint_primary_plus_mirror_images() {
        let mut seed = 0x5eed_u64;
        let mut next = |m: i32| -> i32 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) % m as u64) as i32
        };
        for case in 0..300 {
            let ax = (next(3) > 0).then(|| next(21));
            let ay = (next(3) > 0).then(|| next(19));
            let aid = Aid {
                size: 1 + next(3) as u8,
                pp: case % 2 == 0,
                ..mirror(ax, ay)
            };
            let len = 1 + next(7) as usize;
            let pts: Vec<(i32, i32)> = (0..len).map(|_| (next(14) - 2, next(13) - 2)).collect();
            let k = primary(aid, &pts);
            let mut want = BTreeSet::new();
            for &(x, y) in &k {
                let xs = [Some(x), ax.map(|a| a - 1 - x)];
                let ys = [Some(y), ay.map(|b| b - 1 - y)];
                for &mx in xs.iter().flatten() {
                    for &my in ys.iter().flatten() {
                        if (0..10).contains(&mx) && (0..9).contains(&my) {
                            want.insert((mx, my));
                        }
                    }
                }
            }
            assert_eq!(
                stroke_on(10, 9, aid, &pts),
                want,
                "case {case}: {aid_x:?} {aid_y:?} {pts:?}",
                aid_x = ax,
                aid_y = ay
            );
        }
    }

    // --- Tiled mode (AC-15 – AC-17, AC-19) ---

    #[test]
    fn wrap_x_stroke_across_right_edge_continues_on_left() {
        assert_eq!(
            stroke_on(8, 4, wrap(true, false), &[(5, 1), (10, 1)]),
            set(&[(5, 1), (6, 1), (7, 1), (0, 1), (1, 1), (2, 1)])
        );
        // One jump across the seam keeps the diagonal unbroken.
        assert_eq!(
            stroke_on(8, 4, wrap(true, false), &[(6, 0), (9, 3)]),
            set(&[(6, 0), (7, 1), (0, 2), (1, 3)])
        );
        // And across the left edge.
        assert_eq!(
            stroke_on(8, 4, wrap(true, true), &[(1, 2), (-2, 2)]),
            set(&[(1, 2), (0, 2), (7, 2), (6, 2)])
        );
    }

    #[test]
    fn wrap_y_stroke_across_bottom_edge_continues_on_top() {
        assert_eq!(
            stroke_on(3, 6, wrap(false, true), &[(1, 4), (1, 7)]),
            set(&[(1, 4), (1, 5), (1, 0), (1, 1)])
        );
    }

    #[test]
    fn wrap_x_press_on_copy_paints_canvas_pixel() {
        // A press on the right-hand copy: column 10 is canvas column 2.
        assert_eq!(
            stroke_on(8, 4, wrap(true, false), &[(10, 3)]),
            set(&[(2, 3)])
        );
        assert_eq!(
            stroke_on(8, 4, wrap(true, true), &[(-1, -1)]),
            set(&[(7, 3)])
        );
    }

    #[test]
    fn wrap_both_size_8_stamp_at_corner_reaches_all_corners() {
        let aid = Aid {
            size: 8,
            ..wrap(true, true)
        };
        let span: Vec<i32> = (12..16).chain(0..4).collect();
        let want = span
            .iter()
            .flat_map(|&y| span.iter().map(move |&x| (x, y)))
            .collect();
        assert_eq!(stroke_on(16, 16, aid, &[(0, 0)]), want);
    }

    #[test]
    fn wrap_off_skips_out_of_bounds() {
        // No wrap, and wrap along the other axis only: the part beyond
        // the right edge is skipped.
        for aid in [PLAIN, wrap(false, true)] {
            assert_eq!(
                stroke_on(8, 4, aid, &[(5, 1), (10, 1)]),
                set(&[(5, 1), (6, 1), (7, 1)])
            );
        }
        assert_eq!(
            stroke_on(8, 4, wrap(true, false), &[(2, 2), (2, 6)]),
            set(&[(2, 2), (2, 3)])
        );
    }

    #[test]
    fn wrap_stroke_is_one_undo_entry() {
        let (mut sprite, mut cels) = doc(8, 8, (0, 0, 8, 8), BLUE);
        let before = cel_state(&cels);
        let mut bus = Bus::new();
        let aid = Aid {
            size: 2,
            ..wrap(true, true)
        };
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            aid,
            &[(6, 6), (10, 9), (-3, 1)],
        );
        let after = cel_state(&cels);
        assert!(painted(&cels, RED).contains(&(0, 0)), "wrapped part");
        assert!(painted(&cels, RED).contains(&(6, 6)), "unwrapped part");
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(cel_state(&cels), before);
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!(cel_state(&cels), after);
    }

    #[test]
    fn mirror_image_beyond_canvas_wraps_in_tiled_mode() {
        // A = 12 on width 8: (1, 0) mirrors to 10, which wraps to 2.
        let aid = Aid {
            wrap: Wrap { x: true, y: false },
            ..mirror(Some(12), None)
        };
        assert_eq!(stroke_on(8, 2, aid, &[(1, 0)]), set(&[(1, 0), (2, 0)]));
    }

    // --- Cel growth (AC-21) ---

    #[test]
    fn stroke_outside_small_cel_grows_it_and_undo_restores_it() {
        let (mut sprite, mut cels) = doc(32, 32, (2, 2, 4, 4), BLUE);
        let before = cel_state(&cels);
        let mut bus = Bus::new();
        run(&mut bus, &mut sprite, &mut cels, PLAIN, &[(3, 3), (10, 3)]);
        let after = cel_state(&cels);
        let red = painted(&cels, RED);
        assert!(red.contains(&(10, 3)), "painted beyond the old cel");
        assert_eq!(red.len(), 8);
        let blue = painted(&cels, BLUE);
        assert_eq!(blue.len(), 16 - 3, "old pixels kept, three overpainted");
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(cel_state(&cels), before, "bounds and pixels restored");
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!(cel_state(&cels), after);
    }

    #[test]
    fn mirror_image_outside_small_cel_grows_it() {
        let (mut sprite, mut cels) = doc(16, 16, (0, 0, 4, 4), Rgba::TRANSPARENT);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            mirror(Some(16), None),
            &[(1, 1)],
        );
        assert_eq!(painted(&cels, RED), set(&[(1, 1), (14, 1)]));
    }

    #[test]
    fn wrapped_stroke_grows_cel_to_full_canvas_extent_on_wrapped_axis() {
        let (mut sprite, mut cels) = doc(32, 32, (0, 0, 4, 4), BLUE);
        let before = cel_state(&cels);
        let mut bus = Bus::new();
        run(
            &mut bus,
            &mut sprite,
            &mut cels,
            wrap(true, false),
            &[(2, 1), (-2, 1)],
        );
        let ((x, _), w, h, _) = cel_state(&cels);
        assert_eq!((x, w), (0, 32), "full canvas width");
        assert_eq!(h, 4, "rows unchanged");
        assert!(painted(&cels, RED).contains(&(30, 1)));
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(cel_state(&cels), before);
    }

    #[test]
    fn stroke_outside_canvas_and_cel_does_not_grow_the_cel() {
        let (mut sprite, mut cels) = doc(8, 8, (0, 0, 4, 4), Rgba::TRANSPARENT);
        let before = cel_state(&cels);
        run(&mut Bus::new(), &mut sprite, &mut cels, PLAIN, &[(-3, -3)]);
        assert_eq!(cel_state(&cels), before);
    }
}
