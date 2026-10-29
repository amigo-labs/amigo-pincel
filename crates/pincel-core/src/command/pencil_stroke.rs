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
//! are skipped silently, like [`super::DrawLine`].

use std::collections::HashMap;

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, PixelBuffer, Rgba, Sprite};
use crate::geometry::Point;

use super::Command;
use super::dirty::DirtyRegion;
use super::draw_line::bresenham;
use super::error::CommandError;

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

/// One Pixel-mode Pencil / Eraser stroke (one undo entry).
#[derive(Debug, Clone, PartialEq)]
pub struct PencilStroke {
    layer: LayerId,
    frame: FrameIndex,
    /// Paint color; the Eraser passes [`Rgba::TRANSPARENT`].
    color: Rgba,
    brush: PixelBrush,
    pixel_perfect: bool,
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
    position: (i32, i32),
}

impl Target<'_> {
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
    let position = cel.position;
    let CelData::Image(buffer) = &mut cel.data else {
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
            samples: vec![start],
            pressure: vec![clamp_pressure(pressure)],
            path: Vec::new(),
            coverage: HashMap::new(),
            prior: HashMap::new(),
            bounds: Bounds::default(),
        }
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
        let count = self.coverage.entry((b.x, b.y)).or_insert(1);
        *count -= 1;
        if *count > 0 {
            return;
        }
        self.coverage.remove(&(b.x, b.y));
        if let (Some(off), Some(&before)) = (t.offset(b.x, b.y), self.prior.get(&(b.x, b.y))) {
            t.write(off, before);
            seg.add(b.x, b.y);
        }
    }

    /// Stamp the brush shape at size `size` centred on `p`. The one
    /// stamping routine every stroke variant goes through.
    fn stamp(&mut self, t: &mut Target, p: Point, size: u8, seg: &mut Bounds) {
        for (dx, dy) in PixelBrush::new(size, self.brush.shape()).offsets() {
            let (x, y) = (p.x + dx, p.y + dy);
            if self.pixel_perfect {
                *self.coverage.entry((x, y)).or_insert(0) += 1;
            }
            self.paint(t, x, y, seg);
        }
    }

    /// Write the stroke color to one pixel, recording its first-touch value.
    fn paint(&mut self, t: &mut Target, x: i32, y: i32, seg: &mut Bounds) {
        let Some(off) = t.offset(x, y) else {
            return;
        };
        let before = t.read(off);
        self.prior.entry((x, y)).or_insert(before);
        t.write(off, self.color);
        self.bounds.add(x, y);
        seg.add(x, y);
    }
}

impl Command for PencilStroke {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        self.path.clear();
        self.coverage.clear();
        self.prior.clear();
        self.bounds = Bounds::default();
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
}
