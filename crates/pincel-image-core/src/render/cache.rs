//! A cached composite for presenting a document (docs/specs/performance-image-cache.md).
//!
//! [`RenderCache`] keeps the full-resolution composite of a document plus a
//! pyramid of box-filtered half-size levels, each split into tiles that are
//! composed (or filtered) only when a presented region needs them. Edits
//! arrive as [`Damage`] from the [`CommandBus`](crate::command::CommandBus):
//! a damaged rect is recomposed right away where the full-resolution level
//! already holds valid tiles, and the pyramid tiles above it are marked stale.

use super::compose_region;
use crate::command::Damage;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;

/// Edge length of a cache tile in pixels of its own level.
const TILE: u32 = 128;

/// The cache may take at most this many hundredths of `width × height × 4`
/// bytes (AC-05); pyramid levels that would exceed it are not kept.
const BUDGET_PERCENT: u64 = 134;

/// The composite of a document, cached for presenting (spec §4.4,
/// docs/specs/performance-image-cache.md).
///
/// Level 0 is byte-identical to [`compose`](super::compose) of the document;
/// level `l` (`1..`) is the 2×2 box filter (premultiplied alpha) of level
/// `l - 1`, `ceil(w / 2^l) × ceil(h / 2^l)` pixels, down to 1×1 or until the
/// memory budget (`1.34 × width × height × 4` bytes) is reached.
///
/// The document must only change through a
/// [`CommandBus`](crate::command::CommandBus) whose damage is passed to
/// [`update`](RenderCache::update) before each presentation; a change of the
/// canvas size is always detected.
#[derive(Debug, Clone)]
pub struct RenderCache {
    width: u32,
    height: u32,
    tile: u32,
    levels: Vec<Level>,
}

/// One pyramid level: its pixels and which tiles hold current data.
#[derive(Debug, Clone)]
struct Level {
    pixels: ImageBuffer,
    tiles_x: u32,
    tiles_y: u32,
    valid: Vec<bool>,
}

impl Level {
    fn new(width: u32, height: u32, tile: u32) -> Self {
        let tiles_x = width.div_ceil(tile);
        let tiles_y = height.div_ceil(tile);
        Self {
            pixels: ImageBuffer::new_transparent(width, height),
            tiles_x,
            tiles_y,
            valid: vec![false; (tiles_x * tiles_y) as usize],
        }
    }

    fn width(&self) -> u32 {
        self.pixels.width()
    }

    fn height(&self) -> u32 {
        self.pixels.height()
    }

    fn bytes(&self) -> usize {
        self.pixels.data().len() + self.valid.len()
    }

    /// Tile index ranges `(tx0..tx1, ty0..ty1)` covering the level rect
    /// `[x0, x1) × [y0, y1)`.
    fn tile_span(&self, tile: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> [u32; 4] {
        [
            x0 / tile,
            y0 / tile,
            x1.div_ceil(tile).min(self.tiles_x),
            y1.div_ceil(tile).min(self.tiles_y),
        ]
    }

    /// The level rect of tile `(tx, ty)`, clipped to the level.
    fn tile_rect(&self, tile: u32, tx: u32, ty: u32) -> [u32; 4] {
        let x0 = tx * tile;
        let y0 = ty * tile;
        [
            x0,
            y0,
            (x0 + tile).min(self.width()),
            (y0 + tile).min(self.height()),
        ]
    }
}

impl RenderCache {
    /// Creates an empty cache sized to `doc`'s canvas; nothing is composed
    /// until a region is presented.
    pub fn new(doc: &Document) -> Self {
        Self::with_tile(doc, TILE)
    }

    /// [`new`](RenderCache::new) with a custom tile edge, so tests cross
    /// tile borders on small canvases.
    pub(crate) fn with_tile(doc: &Document, tile: u32) -> Self {
        let (width, height) = (doc.canvas.width(), doc.canvas.height());
        Self {
            width,
            height,
            tile,
            levels: build_levels(width, height, tile),
        }
    }

    /// Canvas size the cache is built for.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Number of pyramid levels kept, the full-resolution level included.
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Bytes the cache holds: every level's pixels and tile flags.
    pub fn byte_size(&self) -> usize {
        self.levels.iter().map(Level::bytes).sum()
    }

    /// Brings the cache up to date with `doc` after `damage` (from
    /// [`CommandBus::take_damage`](crate::command::CommandBus::take_damage)).
    ///
    /// Damaged rects are recomposed at full resolution where that level
    /// already holds valid tiles, and the pyramid above them goes stale;
    /// [`Damage::Full`] or a changed canvas size drops everything.
    pub fn update(&mut self, doc: &Document, damage: Damage) {
        if self.size() != (doc.canvas.width(), doc.canvas.height()) {
            *self = Self::with_tile(doc, self.tile);
            return;
        }
        match damage {
            Damage::None => {}
            Damage::Full => {
                for level in &mut self.levels {
                    level.valid.fill(false);
                }
            }
            Damage::Rects(rects) => {
                for rect in rects {
                    self.recompose(doc, rect);
                }
            }
        }
    }

    /// Recomposes canvas `rect` in the valid tiles of level 0 and marks the
    /// pyramid tiles above it stale.
    fn recompose(&mut self, doc: &Document, rect: Rect) {
        let Some(r) = rect.intersect(Rect::new(0, 0, self.width, self.height)) else {
            return;
        };
        let (x0, y0) = (r.x as u32, r.y as u32);
        let (x1, y1) = (x0 + r.w, y0 + r.h);
        let tile = self.tile;
        let base = &mut self.levels[0];
        let [tx0, ty0, tx1, ty1] = base.tile_span(tile, x0, y0, x1, y1);
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                if !base.valid[(ty * base.tiles_x + tx) as usize] {
                    continue;
                }
                let [ax, ay, bx, by] = base.tile_rect(tile, tx, ty);
                let part = Rect::new(
                    ax.max(x0) as i32,
                    ay.max(y0) as i32,
                    bx.min(x1) - ax.max(x0),
                    by.min(y1) - ay.max(y0),
                );
                paste(&mut base.pixels, part, &compose_region(doc.layers(), part));
            }
        }
        for (l, level) in self.levels.iter_mut().enumerate().skip(1) {
            let [lx0, ly0, lx1, ly1] = [x0 >> l, y0 >> l, ceil_shift(x1, l), ceil_shift(y1, l)];
            let [tx0, ty0, tx1, ty1] = level.tile_span(tile, lx0, ly0, lx1, ly1);
            for ty in ty0..ty1 {
                for tx in tx0..tx1 {
                    level.valid[(ty * level.tiles_x + tx) as usize] = false;
                }
            }
        }
    }

    /// The full-resolution composite of the whole canvas, byte-identical to
    /// [`compose`](super::compose) of `doc` once [`update`](RenderCache::update)
    /// has seen every change.
    pub fn composite(&mut self, doc: &Document) -> &ImageBuffer {
        self.sync_size(doc);
        self.ensure(doc, 0, [0, 0, self.width, self.height]);
        &self.levels[0].pixels
    }

    /// Presents canvas `region` (clipped to the canvas) at `out_w × out_h`.
    ///
    /// An output the size of the clipped region (zoom ≥ 1) is the matching
    /// crop of the full-resolution composite, byte-identical to
    /// [`compose_region`]. A smaller output (zoom < 1) is a bilinear sample
    /// of the pyramid level whose scale is the largest power of two not
    /// above the reduction. `out_w` / `out_h` are clamped to `1..=` the
    /// clipped size; a region outside the canvas yields a 0×0 buffer.
    pub fn region(&mut self, doc: &Document, region: Rect, out_w: u32, out_h: u32) -> ImageBuffer {
        self.sync_size(doc);
        let Some(r) = region.intersect(Rect::new(0, 0, self.width, self.height)) else {
            return ImageBuffer::new_transparent(0, 0);
        };
        let (ow, oh) = (out_w.clamp(1, r.w), out_h.clamp(1, r.h));
        let (x0, y0) = (r.x as u32, r.y as u32);
        if (ow, oh) == (r.w, r.h) {
            self.ensure(doc, 0, [x0, y0, x0 + r.w, y0 + r.h]);
            return crop(&self.levels[0].pixels, r);
        }
        let reduction = (f64::from(r.w) / f64::from(ow)).min(f64::from(r.h) / f64::from(oh));
        let mut l = 0;
        while l + 1 < self.levels.len() && f64::from(1u32 << (l + 1)) <= reduction {
            l += 1;
        }
        let level = &self.levels[l];
        let cols = taps(x0, r.w, ow, l, level.width());
        let rows = taps(y0, r.h, oh, l, level.height());
        let span = |t: &[Tap]| {
            let lo = t.iter().map(|t| t.i0).min().unwrap_or(0);
            let hi = t.iter().map(|t| t.i1).max().unwrap_or(0);
            (lo, hi + 1)
        };
        let ((sx0, sx1), (sy0, sy1)) = (span(&cols), span(&rows));
        self.ensure(doc, l, [sx0, sy0, sx1, sy1]);
        sample(&self.levels[l].pixels, &cols, &rows)
    }

    /// Rebuilds the cache when `doc`'s canvas size no longer matches.
    fn sync_size(&mut self, doc: &Document) {
        if self.size() != (doc.canvas.width(), doc.canvas.height()) {
            *self = Self::with_tile(doc, self.tile);
        }
    }

    /// Makes the level-`l` rect `[x0, x1) × [y0, y1)` current: stale tiles
    /// are composed (level 0) or box-filtered from the level below.
    fn ensure(&mut self, doc: &Document, l: usize, [x0, y0, x1, y1]: [u32; 4]) {
        let tile = self.tile;
        let [tx0, ty0, tx1, ty1] = self.levels[l].tile_span(tile, x0, y0, x1, y1);
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                let index = (ty * self.levels[l].tiles_x + tx) as usize;
                if self.levels[l].valid[index] {
                    continue;
                }
                let [ax, ay, bx, by] = self.levels[l].tile_rect(tile, tx, ty);
                if l == 0 {
                    let part = Rect::new(ax as i32, ay as i32, bx - ax, by - ay);
                    let pixels = compose_region(doc.layers(), part);
                    paste(&mut self.levels[0].pixels, part, &pixels);
                } else {
                    let below = &self.levels[l - 1];
                    let need = [
                        ax * 2,
                        ay * 2,
                        (bx * 2).min(below.width()),
                        (by * 2).min(below.height()),
                    ];
                    self.ensure(doc, l - 1, need);
                    let (lower, upper) = self.levels.split_at_mut(l);
                    halve(&lower[l - 1].pixels, &mut upper[0].pixels, [ax, ay, bx, by]);
                }
                self.levels[l].valid[index] = true;
            }
        }
    }
}

/// Pyramid levels for a `width × height` canvas within the memory budget.
fn build_levels(width: u32, height: u32, tile: u32) -> Vec<Level> {
    level_sizes(width, height, tile)
        .into_iter()
        .map(|(w, h)| Level::new(w, h, tile))
        .collect()
}

/// Bytes a `w × h` level takes: RGBA8 pixels plus one flag per tile.
fn level_bytes(w: u32, h: u32, tile: u32) -> u64 {
    u64::from(w) * u64::from(h) * 4 + u64::from(w.div_ceil(tile)) * u64::from(h.div_ceil(tile))
}

/// Sizes of the levels kept for a `width × height` canvas: full resolution,
/// then halves (rounded up) down to 1×1, as long as the total stays within
/// `1.34 × width × height × 4` bytes (AC-05).
fn level_sizes(width: u32, height: u32, tile: u32) -> Vec<(u32, u32)> {
    let budget = BUDGET_PERCENT * u64::from(width) * u64::from(height) * 4 / 100;
    let mut sizes = vec![(width, height)];
    let mut used = level_bytes(width, height, tile);
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        (w, h) = (w.div_ceil(2), h.div_ceil(2));
        used += level_bytes(w, h, tile);
        if used > budget {
            break;
        }
        sizes.push((w, h));
    }
    sizes
}

/// `ceil(v / 2^l)`.
fn ceil_shift(v: u32, l: usize) -> u32 {
    (v + (1 << l) - 1) >> l
}

/// Copies `src` into `dst` at `rect` (which lies inside `dst`).
fn paste(dst: &mut ImageBuffer, rect: Rect, src: &ImageBuffer) {
    let stride = dst.width() as usize * 4;
    let row = rect.w as usize * 4;
    let data = dst.data_mut();
    for (y, src_row) in src.data().chunks_exact(row).enumerate() {
        let start = (rect.y as usize + y) * stride + rect.x as usize * 4;
        data[start..start + row].copy_from_slice(src_row);
    }
}

/// The `rect` of `src` (which lies inside `src`) as a new buffer.
fn crop(src: &ImageBuffer, rect: Rect) -> ImageBuffer {
    let stride = src.width() as usize * 4;
    let row = rect.w as usize * 4;
    let mut data = Vec::with_capacity(row * rect.h as usize);
    for y in 0..rect.h as usize {
        let start = (rect.y as usize + y) * stride + rect.x as usize * 4;
        data.extend_from_slice(&src.data()[start..start + row]);
    }
    ImageBuffer::from_raw(rect.w, rect.h, data)
        .unwrap_or_else(|_| ImageBuffer::new_transparent(0, 0))
}

/// Writes the level-`l` rect `[x0, x1) × [y0, y1)` of `dst` as the 2×2 box
/// filter of `src` (level `l - 1`), averaging in premultiplied alpha with
/// integer rounding; source pixels past the edge are left out.
fn halve(src: &ImageBuffer, dst: &mut ImageBuffer, [x0, y0, x1, y1]: [u32; 4]) {
    let (sw, sh) = (src.width(), src.height());
    let s = src.data();
    let stride = sw as usize * 4;
    let dw = dst.width() as usize;
    let d = dst.data_mut();
    for y in y0..y1 {
        let top = 2 * y as usize * stride;
        let full_rows = 2 * y + 1 < sh;
        for x in x0..x1 {
            let o = (y as usize * dw + x as usize) * 4;
            let px = &mut d[o..o + 4];
            if full_rows && 2 * x + 1 < sw {
                let i = top + 8 * x as usize;
                let quad = [i, i + 4, i + stride, i + stride + 4];
                let a = quad.map(|i| u32::from(s[i + 3]));
                if a[0] == a[1] && a[0] == a[2] && a[0] == a[3] {
                    // Equal alphas cancel out of the premultiplied average:
                    // (a·Σc + 2a) / 4a = (Σc + 2) / 4, the same bytes as the
                    // general case without a division.
                    for c in 0..3 {
                        let sum: u32 = quad.iter().map(|&i| u32::from(s[i + c])).sum();
                        px[c] = if a[0] == 0 { 0 } else { ((sum + 2) >> 2) as u8 };
                    }
                    px[3] = a[0] as u8;
                    continue;
                }
            }
            let (mut n, mut sa) = (0u32, 0u32);
            let mut sc = [0u32; 3];
            for sy in [2 * y, 2 * y + 1].into_iter().filter(|&sy| sy < sh) {
                for sx in [2 * x, 2 * x + 1].into_iter().filter(|&sx| sx < sw) {
                    let i = sy as usize * stride + sx as usize * 4;
                    let a = u32::from(s[i + 3]);
                    n += 1;
                    sa += a;
                    for c in 0..3 {
                        sc[c] += u32::from(s[i + c]) * a;
                    }
                }
            }
            for c in 0..3 {
                px[c] = (sc[c] + sa / 2).checked_div(sa).unwrap_or(0) as u8;
            }
            px[3] = ((sa + n / 2) / n.max(1)) as u8;
        }
    }
}

/// One output column (or row) of a bilinear sample: the two level pixels
/// it reads and the weight of the second, in 1/256.
#[derive(Debug, Clone, Copy)]
struct Tap {
    i0: u32,
    i1: u32,
    f: u32,
}

/// Bilinear taps for `out` output pixels spanning canvas `[start, start +
/// len)`, read from a level of scale `2^l` that is `size` pixels long.
fn taps(start: u32, len: u32, out: u32, l: usize, size: u32) -> Vec<Tap> {
    let scale = f64::from(1u32 << l);
    let last = size.saturating_sub(1);
    (0..out)
        .map(|o| {
            let canvas = f64::from(start) + (f64::from(o) + 0.5) * f64::from(len) / f64::from(out);
            let u = (canvas / scale - 0.5).max(0.0);
            let i0 = (u.floor() as u32).min(last);
            let i1 = (i0 + 1).min(last);
            let f = if i1 == i0 {
                0
            } else {
                ((u - u.floor()) * 256.0).round() as u32
            };
            Tap { i0, i1, f }
        })
        .collect()
}

/// Samples `src` at the column and row taps, interpolating in
/// premultiplied alpha with integer rounding.
fn sample(src: &ImageBuffer, cols: &[Tap], rows: &[Tap]) -> ImageBuffer {
    let s = src.data();
    let stride = src.width() as usize * 4;
    let mut out = vec![0u8; cols.len() * rows.len() * 4];
    let line = cols.len() * 4;
    for (row, dst) in rows.iter().zip(out.chunks_exact_mut(line.max(1))) {
        let (r0, r1) = (row.i0 as usize * stride, row.i1 as usize * stride);
        let (wy1, wy0) = (row.f, 256 - row.f);
        for (col, px) in cols.iter().zip(dst.as_chunks_mut::<4>().0) {
            let (c0, c1) = (col.i0 as usize * 4, col.i1 as usize * 4);
            let (wx1, wx0) = (col.f, 256 - col.f);
            let taps = [
                (r0 + c0, wy0 * wx0),
                (r0 + c1, wy0 * wx1),
                (r1 + c0, wy1 * wx0),
                (r1 + c1, wy1 * wx1),
            ];
            *px = blend4(s, taps);
        }
    }
    ImageBuffer::from_raw(cols.len() as u32, rows.len() as u32, out)
        .unwrap_or_else(|_| ImageBuffer::new_transparent(0, 0))
}

/// The weighted average of four RGBA8 pixels at byte offsets into `s`,
/// weights summing to 65536, in premultiplied alpha. Weights times
/// premultiplied channels stay below 2^32.
fn blend4(s: &[u8], taps: [(usize, u32); 4]) -> [u8; 4] {
    let a = taps.map(|(i, _)| u32::from(s[i + 3]));
    if a[0] == a[1] && a[0] == a[2] && a[0] == a[3] {
        // Equal alphas cancel out of the premultiplied average, so this is
        // the same result as the general case below, without divisions.
        if a[0] == 0 {
            return [0; 4];
        }
        let mut px = [0u8; 4];
        for (c, out) in px.iter_mut().take(3).enumerate() {
            let sum: u32 = taps.iter().map(|&(i, w)| u32::from(s[i + c]) * w).sum();
            *out = ((sum + (1 << 15)) >> 16) as u8;
        }
        px[3] = a[0] as u8;
        return px;
    }
    let mut sa = 0u32;
    let mut sc = [0u32; 3];
    for (&(i, w), &alpha) in taps.iter().zip(&a) {
        let aw = alpha * w;
        sa += aw;
        for (c, acc) in sc.iter_mut().enumerate() {
            *acc += u32::from(s[i + c]) * aw;
        }
    }
    let mut px = [0u8; 4];
    for (out, c) in px.iter_mut().zip(sc) {
        // `sa` is zero when only zero-weight taps are opaque.
        *out = (u64::from(c) + u64::from(sa / 2))
            .checked_div(u64::from(sa))
            .unwrap_or(0) as u8;
    }
    px[3] = ((sa + (1 << 15)) >> 16) as u8;
    px
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{BlendMode, Color};
    use crate::command::{
        AddLayer, CanvasRotation, Command, CommandBus, CropToSelection, MoveLayer, RemoveLayer,
        ResizeCanvas, RotateCanvas, ScaleImage, SetLayerBlendMode, SetLayerOpacity,
        SetLayerVisible, SetPixels, SetSelection,
    };
    use crate::document::Layer;
    use crate::render::compose;
    use crate::selection::SelectionMask;
    use crate::transform::Interpolation;
    use proptest::prelude::*;

    /// A `w × h` patch whose pixels vary with position and `seed`, with
    /// alphas from transparent to opaque.
    fn patch(w: u32, h: u32, seed: u32) -> ImageBuffer {
        let mut buf = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = seed.wrapping_mul(2_654_435_761) ^ (x * 31 + y * 17);
                let a = [0, 40, 128, 200, 255][(v % 5) as usize];
                buf.set_pixel(
                    x,
                    y,
                    Color::rgba(v as u8, (v >> 8) as u8, (v >> 16) as u8, a),
                );
            }
        }
        buf
    }

    /// A bus over a `w × h` document with `n` patterned layers.
    fn bus(w: u32, h: u32, n: u32) -> CommandBus {
        let mut doc = Document::from_pixels(patch(w, h, 1)).expect("valid canvas");
        for i in 1..n {
            doc.insert_layer(i as usize, Layer::from_pixels("l", patch(w, h, i + 7)))
                .expect("below the layer limit");
        }
        CommandBus::new(doc)
    }

    fn size(doc: &Document) -> (u32, u32) {
        (doc.canvas.width(), doc.canvas.height())
    }

    const MODES: [BlendMode; 4] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Difference,
    ];

    /// A canvas rect as `(x, y, w, h)`.
    type Area = (i32, i32, u32, u32);

    /// One user action on the document.
    #[derive(Debug, Clone)]
    enum Op {
        Paint { layer: usize, rect: Area, seed: u32 },
        AddLayer,
        RemoveLayer(usize),
        MoveLayer(usize, usize),
        Visible(usize, bool),
        Opacity(usize, u8),
        Blend(usize, usize),
        Select(Area),
        Undo,
        Redo,
    }

    fn op() -> impl Strategy<Value = Op> {
        let rect = (-4i32..40, -4i32..40, 1u32..20, 1u32..20);
        prop_oneof![
            4 => (0usize..8, rect.clone(), any::<u32>())
                .prop_map(|(layer, rect, seed)| Op::Paint { layer, rect, seed }),
            1 => Just(Op::AddLayer),
            1 => (0usize..8).prop_map(Op::RemoveLayer),
            1 => (0usize..8, 0usize..8).prop_map(|(a, b)| Op::MoveLayer(a, b)),
            1 => (0usize..8, any::<bool>()).prop_map(|(i, v)| Op::Visible(i, v)),
            1 => (0usize..8, any::<u8>()).prop_map(|(i, o)| Op::Opacity(i, o)),
            1 => (0usize..8, 0usize..MODES.len()).prop_map(|(i, m)| Op::Blend(i, m)),
            1 => rect.prop_map(Op::Select),
            2 => Just(Op::Undo),
            1 => Just(Op::Redo),
        ]
    }

    /// Runs `op` through the bus; refused commands (bad indices, the last
    /// layer) are part of the sequence too.
    fn run(bus: &mut CommandBus, op: &Op) {
        let n = bus.document.layer_count();
        let (w, h) = size(&bus.document);
        let cmd: Box<dyn Command> = match *op {
            Op::Undo => {
                bus.undo().expect("undo");
                return;
            }
            Op::Redo => {
                bus.redo().expect("redo");
                return;
            }
            Op::Paint {
                layer,
                rect: (x, y, rw, rh),
                seed,
            } => {
                let Some(r) = Rect::new(x, y, rw, rh).intersect(Rect::new(0, 0, w, h)) else {
                    return;
                };
                Box::new(SetPixels::new(layer % n, r, patch(r.w, r.h, seed)))
            }
            Op::AddLayer => Box::new(AddLayer::above(bus.document.active_layer_index())),
            Op::RemoveLayer(i) => Box::new(RemoveLayer::at(i % n)),
            Op::MoveLayer(a, b) => Box::new(MoveLayer::new(a % n, b % n)),
            Op::Visible(i, v) => Box::new(SetLayerVisible::new(i % n, v)),
            Op::Opacity(i, o) => Box::new(SetLayerOpacity::new(i % n, f32::from(o) / 255.0)),
            Op::Blend(i, m) => Box::new(SetLayerBlendMode::new(i % n, MODES[m])),
            Op::Select((x, y, rw, rh)) => Box::new(SetSelection::replace(
                SelectionMask::rectangle(w, h, Rect::new(x, y, rw, rh)),
            )),
        };
        let _ = bus.apply(cmd);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// AC-01: after any edit sequence the zoom-1 presentation equals a
        /// fresh compose, whichever tiles happened to be presented between
        /// the edits.
        #[test]
        fn region_at_zoom_one_matches_a_fresh_compose_after_any_edits(
            (w, h, n) in (1u32..40, 1u32..40, 1u32..4),
            ops in prop::collection::vec((op(), (0i32..40, 0i32..40, 1u32..40, 1u32..40)), 1..24),
        ) {
            let mut bus = bus(w, h, n);
            let mut cache = RenderCache::with_tile(&bus.document, 8);
            for (op, (x, y, rw, rh)) in &ops {
                run(&mut bus, op);
                let damage = bus.take_damage();
                cache.update(&bus.document, damage);
                let view = Rect::new(*x, *y, *rw, *rh);
                let shown = cache.region(&bus.document, view, *rw, *rh);
                let fresh = compose_region(bus.document.layers(), view);
                if fresh.data().is_empty() {
                    // Off the canvas: compose_region keeps a zero-height
                    // width here, the cache answers 0×0.
                    prop_assert!(shown.data().is_empty());
                } else {
                    prop_assert_eq!(&shown, &fresh);
                }
            }
            let whole = Rect::new(0, 0, w, h);
            prop_assert_eq!(cache.region(&bus.document, whole, w, h), compose(bus.document.layers()));
        }

        /// AC-07: after any edit sequence a zoomed-out presentation equals
        /// sampling the pyramid of a cache built fresh from the document.
        #[test]
        fn region_below_zoom_one_matches_a_fresh_pyramid_after_any_edits(
            (w, h, n) in (1u32..40, 1u32..40, 1u32..4),
            ops in prop::collection::vec(
                (op(), (0i32..40, 0i32..40, 1u32..40, 1u32..40), (1u32..40, 1u32..40)),
                1..24,
            ),
        ) {
            let mut bus = bus(w, h, n);
            let mut cache = RenderCache::with_tile(&bus.document, 4);
            for (op, (x, y, rw, rh), (ow, oh)) in &ops {
                run(&mut bus, op);
                let damage = bus.take_damage();
                cache.update(&bus.document, damage);
                let view = Rect::new(*x, *y, *rw, *rh);
                let shown = cache.region(&bus.document, view, *ow, *oh);
                let fresh = RenderCache::with_tile(&bus.document, 4)
                    .region(&bus.document, view, *ow, *oh);
                prop_assert_eq!(shown, fresh);
            }
        }
    }

    proptest! {
        /// `halve`'s division-free path for equal alphas gives the same
        /// bytes as the premultiplied box average it shortcuts.
        #[test]
        fn halve_matches_the_premultiplied_box_average(
            (w, h) in (1u32..9, 1u32..9),
            seed in any::<u32>(),
            alphas in prop::collection::vec(prop::sample::select(vec![0u8, 1, 77, 255]), 1..4),
        ) {
            let mut src = patch(w, h, seed);
            for (i, px) in src.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
                // Few distinct alphas, so equal-alpha quads are common.
                px[3] = alphas[i % alphas.len()];
            }
            let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
            let mut got = ImageBuffer::new_transparent(hw, hh);
            halve(&src, &mut got, [0, 0, hw, hh]);
            for y in 0..hh {
                for x in 0..hw {
                    let (mut n, mut sa, mut sc) = (0u32, 0u32, [0u32; 3]);
                    for sy in (2 * y..2 * y + 2).filter(|&sy| sy < h) {
                        for sx in (2 * x..2 * x + 2).filter(|&sx| sx < w) {
                            let p = src.get_pixel(sx, sy).expect("inside");
                            let a = u32::from(p.a);
                            n += 1;
                            sa += a;
                            for (acc, c) in sc.iter_mut().zip([p.r, p.g, p.b]) {
                                *acc += u32::from(c) * a;
                            }
                        }
                    }
                    let c = sc.map(|c| (c + sa / 2).checked_div(sa).unwrap_or(0) as u8);
                    let want = Color::rgba(c[0], c[1], c[2], ((sa + n / 2) / n) as u8);
                    prop_assert_eq!(got.get_pixel(x, y), Some(want), "({}, {})", x, y);
                }
            }
        }

        /// The division-free path for equal alphas gives the same bytes as
        /// the premultiplied average it shortcuts.
        #[test]
        fn blend4_matches_the_premultiplied_average(
            px in prop::array::uniform4(prop::array::uniform4(any::<u8>())),
            equal_alpha in prop::option::of(any::<u8>()),
            (fx, fy) in (0u32..=256, 0u32..=256),
        ) {
            let mut s = Vec::new();
            for mut p in px {
                if let Some(a) = equal_alpha {
                    p[3] = a;
                }
                s.extend_from_slice(&p);
            }
            let w = [(256 - fy) * (256 - fx), (256 - fy) * fx, fy * (256 - fx), fy * fx];
            let taps = [(0, w[0]), (4, w[1]), (8, w[2]), (12, w[3])];
            let sa: u64 = taps.iter().map(|&(i, w)| u64::from(s[i + 3]) * u64::from(w)).sum();
            let mut want = [0u8; 4];
            for (c, out) in want.iter_mut().take(3).enumerate() {
                let sc: u64 = taps
                    .iter()
                    .map(|&(i, w)| u64::from(s[i + c]) * u64::from(s[i + 3]) * u64::from(w))
                    .sum();
                *out = (sc + sa / 2).checked_div(sa).unwrap_or(0) as u8;
            }
            want[3] = ((sa + (1 << 15)) >> 16) as u8;
            prop_assert_eq!(blend4(&s, taps), want);
        }
    }

    #[test]
    fn region_after_canvas_size_changes_matches_compose() {
        // AC-06: resize, scale, crop and rotate each present the new canvas.
        let mut bus = bus(23, 17, 3);
        let mut cache = RenderCache::with_tile(&bus.document, 8);
        cache.region(&bus.document, Rect::new(0, 0, 23, 17), 23, 17);
        let mask = SelectionMask::rectangle(30, 12, Rect::new(3, 2, 11, 9));
        let steps: Vec<Box<dyn Command>> = vec![
            Box::new(ResizeCanvas::new(30, 12)),
            Box::new(ScaleImage::new(19, 25, Interpolation::Bilinear)),
            Box::new(SetSelection::replace(mask)),
            Box::new(CropToSelection::new()),
            Box::new(RotateCanvas::new(CanvasRotation::Cw90)),
        ];
        for cmd in steps {
            bus.apply(cmd).expect("command applies");
            let damage = bus.take_damage();
            cache.update(&bus.document, damage);
            let (w, h) = size(&bus.document);
            let shown = cache.region(&bus.document, Rect::new(0, 0, w, h), w, h);
            assert_eq!(shown, compose(bus.document.layers()));
            assert_eq!(cache.size(), (w, h));
        }
    }

    #[test]
    fn region_detects_a_size_change_it_was_not_told_about() {
        let mut bus = bus(9, 9, 2);
        let mut cache = RenderCache::with_tile(&bus.document, 4);
        cache.region(&bus.document, Rect::new(0, 0, 9, 9), 9, 9);
        bus.apply(Box::new(ResizeCanvas::new(5, 7)))
            .expect("resize");
        let shown = cache.region(&bus.document, Rect::new(0, 0, 5, 7), 5, 7);
        assert_eq!(shown, compose(bus.document.layers()));
    }

    #[test]
    fn region_at_zoom_one_is_a_crop_without_a_pyramid_level() {
        // Edge case "zoom exactly 1": no level above 0 is touched.
        let bus = bus(20, 20, 2);
        let mut cache = RenderCache::with_tile(&bus.document, 4);
        cache.region(&bus.document, Rect::new(0, 0, 20, 20), 20, 20);
        assert!(cache.levels[1..].iter().all(|l| l.valid.iter().all(|v| !v)));
        assert!(cache.levels[0].valid.iter().all(|&v| v));
    }

    #[test]
    fn region_outside_the_canvas_is_empty() {
        let bus = bus(8, 8, 1);
        let mut cache = RenderCache::new(&bus.document);
        let out = cache.region(&bus.document, Rect::new(10, 10, 4, 4), 2, 2);
        assert_eq!((out.width(), out.height()), (0, 0));
    }

    #[test]
    fn region_at_half_size_of_an_opaque_doc_is_its_box_filter() {
        // A 2×2 checker of black and white averages to mid gray at 1:2.
        let mut doc = Document::new(2, 2).expect("canvas");
        for (x, y, c) in [(0, 0, 0), (1, 0, 255), (0, 1, 255), (1, 1, 0)] {
            doc.active_layer_mut()
                .pixels
                .set_pixel(x, y, Color::rgba(c, c, c, 255));
        }
        let mut cache = RenderCache::new(&doc);
        let out = cache.region(&doc, Rect::new(0, 0, 2, 2), 1, 1);
        assert_eq!(out.get_pixel(0, 0), Some(Color::rgba(128, 128, 128, 255)));
    }

    #[test]
    fn byte_size_stays_within_the_budget_and_keeps_every_level() {
        // AC-05: at most 1.34 × w × h × 4 bytes, and levels down to 1×1 for
        // real documents (the MAX_CANVAS_AREA case computed without
        // allocating it).
        for (w, h) in [(1, 1), (2, 1), (3, 3), (7, 5), (640, 480), (3840, 2160)] {
            let doc = Document::new(w, h).expect("canvas");
            let cache = RenderCache::new(&doc);
            let budget = 134 * u64::from(w) * u64::from(h) * 4 / 100;
            assert!(cache.byte_size() as u64 <= budget, "{w}×{h}");
        }
        for (w, h) in [(640, 480), (3840, 2160), (8192, 8192)] {
            let sizes = level_sizes(w, h, TILE);
            let bytes: u64 = sizes
                .iter()
                .map(|&(lw, lh)| level_bytes(lw, lh, TILE))
                .sum();
            let budget = 134 * u64::from(w) * u64::from(h) * 4 / 100;
            assert!(bytes <= budget, "{w}×{h}");
            assert_eq!(sizes.last(), Some(&(1, 1)), "{w}×{h} keeps every level");
        }
    }
}
