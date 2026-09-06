//! Pixel helpers shared by the transform commands (flip / rotate /
//! reframe / scale). Everything here works on canvas-sized straight-alpha
//! RGBA8 buffers; the commands own the cel bookkeeping and undo.

use crate::document::{Cel, CelData, ColorMode, FrameIndex, LayerId, PixelBuffer};
use crate::geometry::Rect;

/// Orientation change applied by the flip / rotate commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    FlipHorizontal,
    FlipVertical,
    Rotate90Cw,
    Rotate90Ccw,
    Rotate180,
}

impl Orientation {
    /// `true` for the two quarter turns, which swap width and height.
    pub const fn swaps_axes(self) -> bool {
        matches!(self, Self::Rotate90Cw | Self::Rotate90Ccw)
    }

    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::FlipHorizontal => "flip_horizontal",
            Self::FlipVertical => "flip_vertical",
            Self::Rotate90Cw => "rotate_90_cw",
            Self::Rotate90Ccw => "rotate_90_ccw",
            Self::Rotate180 => "rotate_180",
        }
    }

    /// Inverse of [`Orientation::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::FlipHorizontal,
            Self::FlipVertical,
            Self::Rotate90Cw,
            Self::Rotate90Ccw,
            Self::Rotate180,
        ]
        .into_iter()
        .find(|o| o.name() == name)
    }
}

/// Resampling filter for [`resample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interpolation {
    #[default]
    Nearest,
    Bilinear,
    Bicubic,
}

impl Interpolation {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Bilinear => "bilinear",
            Self::Bicubic => "bicubic",
        }
    }

    /// Inverse of [`Interpolation::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Nearest, Self::Bilinear, Self::Bicubic]
            .into_iter()
            .find(|i| i.name() == name)
    }
}

/// A plain `w × h` RGBA8 raster used as the transform working format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Raster {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Raster {
    pub fn transparent(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * 4],
        }
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize {
        (y as usize * self.width as usize + x as usize) * 4
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.idx(x, y);
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    pub fn put(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let i = self.idx(x, y);
        self.data[i..i + 4].copy_from_slice(&px);
    }

    /// Copy `src` raw at `(ox, oy)`, clipped to `self`.
    pub fn blit(&mut self, src: &Raster, ox: i64, oy: i64) {
        for sy in 0..src.height {
            let dy = oy + i64::from(sy);
            if dy < 0 || dy >= i64::from(self.height) {
                continue;
            }
            for sx in 0..src.width {
                let dx = ox + i64::from(sx);
                if dx < 0 || dx >= i64::from(self.width) {
                    continue;
                }
                let px = src.get(sx, sy);
                self.put(dx as u32, dy as u32, px);
            }
        }
    }

    /// Sub-rectangle copy (`rect` must lie inside `self`).
    pub fn crop(&self, rect: Rect) -> Raster {
        let mut out = Raster::transparent(rect.width, rect.height);
        for y in 0..rect.height {
            for x in 0..rect.width {
                out.put(x, y, self.get(rect.x as u32 + x, rect.y as u32 + y));
            }
        }
        out
    }

    /// Fill `rect` (clipped) with transparent black.
    pub fn clear(&mut self, rect: Rect) {
        let r = rect.intersect(Rect::new(0, 0, self.width, self.height));
        for y in 0..r.height {
            for x in 0..r.width {
                self.put(r.x as u32 + x, r.y as u32 + y, [0, 0, 0, 0]);
            }
        }
    }

    /// Reoriented copy. Quarter turns swap the dimensions.
    pub fn oriented(&self, op: Orientation) -> Raster {
        let (w, h) = (self.width, self.height);
        let (ow, oh) = if op.swaps_axes() { (h, w) } else { (w, h) };
        let mut out = Raster::transparent(ow, oh);
        for y in 0..h {
            for x in 0..w {
                let (nx, ny) = match op {
                    Orientation::FlipHorizontal => (w - 1 - x, y),
                    Orientation::FlipVertical => (x, h - 1 - y),
                    Orientation::Rotate90Cw => (h - 1 - y, x),
                    Orientation::Rotate90Ccw => (y, w - 1 - x),
                    Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
                };
                out.put(nx, ny, self.get(x, y));
            }
        }
        out
    }

    /// Reorient the contents of `rect` about the rect's centre, in place.
    /// Pixels a quarter turn pushes outside a non-square `rect` are
    /// dropped; pixels outside `rect` are untouched.
    pub fn orient_region(&mut self, rect: Rect, op: Orientation) {
        let rect = rect.intersect(Rect::new(0, 0, self.width, self.height));
        if rect.is_empty() {
            return;
        }
        let region = self.crop(rect).oriented(op);
        self.clear(rect);
        // Centre the (possibly axis-swapped) result on the rect centre.
        let ox = i64::from(rect.x) + (i64::from(rect.width) - i64::from(region.width)) / 2;
        let oy = i64::from(rect.y) + (i64::from(rect.height) - i64::from(region.height)) / 2;
        let clipped = Raster::clip_to(&region, ox, oy, rect);
        self.blit(&clipped.0, clipped.1, clipped.2);
    }

    /// Clip `src` placed at `(ox, oy)` to `bounds`, returning the clipped
    /// raster and its new placement.
    fn clip_to(src: &Raster, ox: i64, oy: i64, bounds: Rect) -> (Raster, i64, i64) {
        let bx0 = i64::from(bounds.x);
        let by0 = i64::from(bounds.y);
        let bx1 = bx0 + i64::from(bounds.width);
        let by1 = by0 + i64::from(bounds.height);
        let x0 = ox.max(bx0);
        let y0 = oy.max(by0);
        let x1 = (ox + i64::from(src.width)).min(bx1);
        let y1 = (oy + i64::from(src.height)).min(by1);
        if x0 >= x1 || y0 >= y1 {
            return (Raster::transparent(0, 0), x0, y0);
        }
        let sub = src.crop(Rect::new(
            (x0 - ox) as i32,
            (y0 - oy) as i32,
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        ));
        (sub, x0, y0)
    }

    /// Resample to `w × h` with the given filter. Works in premultiplied
    /// alpha so transparent neighbours never bleed colour.
    pub fn resample(&self, w: u32, h: u32, filter: Interpolation) -> Raster {
        let mut out = Raster::transparent(w, h);
        if w == 0 || h == 0 || self.width == 0 || self.height == 0 {
            return out;
        }
        let sx = self.width as f32 / w as f32;
        let sy = self.height as f32 / h as f32;
        let sample = |x: i64, y: i64| -> [f32; 4] {
            let cx = x.clamp(0, i64::from(self.width) - 1) as u32;
            let cy = y.clamp(0, i64::from(self.height) - 1) as u32;
            let p = self.get(cx, cy);
            let a = f32::from(p[3]) / 255.0;
            [
                f32::from(p[0]) * a,
                f32::from(p[1]) * a,
                f32::from(p[2]) * a,
                f32::from(p[3]),
            ]
        };
        for y in 0..h {
            for x in 0..w {
                let fx = (x as f32 + 0.5) * sx - 0.5;
                let fy = (y as f32 + 0.5) * sy - 0.5;
                let pm = match filter {
                    Interpolation::Nearest => {
                        let nx = ((x as f32 + 0.5) * sx).floor() as i64;
                        let ny = ((y as f32 + 0.5) * sy).floor() as i64;
                        sample(nx, ny)
                    }
                    Interpolation::Bilinear => {
                        let x0 = fx.floor();
                        let y0 = fy.floor();
                        let tx = fx - x0;
                        let ty = fy - y0;
                        let (x0, y0) = (x0 as i64, y0 as i64);
                        let p00 = sample(x0, y0);
                        let p10 = sample(x0 + 1, y0);
                        let p01 = sample(x0, y0 + 1);
                        let p11 = sample(x0 + 1, y0 + 1);
                        let mut r = [0f32; 4];
                        for c in 0..4 {
                            let top = p00[c] * (1.0 - tx) + p10[c] * tx;
                            let bot = p01[c] * (1.0 - tx) + p11[c] * tx;
                            r[c] = top * (1.0 - ty) + bot * ty;
                        }
                        r
                    }
                    Interpolation::Bicubic => {
                        let x0 = fx.floor();
                        let y0 = fy.floor();
                        let tx = fx - x0;
                        let ty = fy - y0;
                        let (x0, y0) = (x0 as i64, y0 as i64);
                        let wx = cubic_weights(tx);
                        let wy = cubic_weights(ty);
                        let mut r = [0f32; 4];
                        for (j, wyj) in wy.iter().enumerate() {
                            for (i, wxi) in wx.iter().enumerate() {
                                let p = sample(x0 - 1 + i as i64, y0 - 1 + j as i64);
                                let wgt = wxi * wyj;
                                for c in 0..4 {
                                    r[c] += p[c] * wgt;
                                }
                            }
                        }
                        r
                    }
                };
                let a = pm[3].clamp(0.0, 255.0);
                let px = if a <= 0.0 {
                    [0, 0, 0, 0]
                } else {
                    let inv = 255.0 / a;
                    [
                        (pm[0] * inv).round().clamp(0.0, 255.0) as u8,
                        (pm[1] * inv).round().clamp(0.0, 255.0) as u8,
                        (pm[2] * inv).round().clamp(0.0, 255.0) as u8,
                        a.round() as u8,
                    ]
                };
                out.put(x, y, px);
            }
        }
        out
    }
}

/// Catmull-Rom weights for the four taps around a sample at fraction `t`.
fn cubic_weights(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

/// Render an image cel into a canvas-sized raster (raw copy, clipped).
pub(crate) fn cel_to_canvas(
    cel: &Cel,
    buffer: &PixelBuffer,
    canvas_w: u32,
    canvas_h: u32,
) -> Raster {
    let src = Raster {
        width: buffer.width,
        height: buffer.height,
        data: buffer.data.clone(),
    };
    let mut out = Raster::transparent(canvas_w, canvas_h);
    out.blit(&src, i64::from(cel.position.0), i64::from(cel.position.1));
    out
}

/// Wrap a raster as a full-canvas image cel at `(0, 0)`.
pub(crate) fn raster_to_cel(layer: LayerId, frame: FrameIndex, raster: Raster) -> Cel {
    let mut buf = PixelBuffer::empty(raster.width, raster.height, ColorMode::Rgba);
    buf.data = raster.data;
    Cel::image(layer, frame, buf)
}

/// Borrow the RGBA image buffer of a cel, or `None` for other cel kinds.
pub(crate) fn rgba_buffer(cel: &Cel) -> Option<&PixelBuffer> {
    match &cel.data {
        CelData::Image(b) if b.color_mode == ColorMode::Rgba => Some(b),
        _ => None,
    }
}

/// Map a sprite-space rect through a whole-canvas orientation change.
pub(crate) fn orient_rect(r: Rect, op: Orientation, canvas_w: u32, canvas_h: u32) -> Rect {
    let (w, h) = (i64::from(canvas_w), i64::from(canvas_h));
    let (x0, y0) = (i64::from(r.x), i64::from(r.y));
    let (x1, y1) = (x0 + i64::from(r.width), y0 + i64::from(r.height));
    let (nx0, ny0, nx1, ny1) = match op {
        Orientation::FlipHorizontal => (w - x1, y0, w - x0, y1),
        Orientation::FlipVertical => (x0, h - y1, x1, h - y0),
        Orientation::Rotate180 => (w - x1, h - y1, w - x0, h - y0),
        // (x, y) → (h - y, x) for a point; for a rect use its corners.
        Orientation::Rotate90Cw => (h - y1, x0, h - y0, x1),
        // (x, y) → (y, w - x).
        Orientation::Rotate90Ccw => (y0, w - x1, y1, w - x0),
    };
    Rect::new(
        nx0 as i32,
        ny0 as i32,
        (nx1 - nx0) as u32,
        (ny1 - ny0) as u32,
    )
}

/// Map a sprite-space point through a whole-canvas orientation change
/// (pixel-centre semantics: the pixel at `(x, y)` lands on the pixel the
/// rect mapping of its 1×1 cell produces).
pub(crate) fn orient_point(
    p: (i32, i32),
    op: Orientation,
    canvas_w: u32,
    canvas_h: u32,
) -> (i32, i32) {
    let r = orient_rect(Rect::new(p.0, p.1, 1, 1), op, canvas_w, canvas_h);
    (r.x, r.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(w: u32, h: u32, px: &[[u8; 4]]) -> Raster {
        let mut r = Raster::transparent(w, h);
        for (i, p) in px.iter().enumerate() {
            r.put(i as u32 % w, i as u32 / w, *p);
        }
        r
    }

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const B: [u8; 4] = [0, 0, 255, 255];
    const T: [u8; 4] = [0, 0, 0, 0];

    #[test]
    fn oriented_flip_and_rotate_move_pixels_as_expected() {
        // 2×1: [R G]
        let src = raster(2, 1, &[R, G]);
        assert_eq!(
            src.oriented(Orientation::FlipHorizontal).data,
            raster(2, 1, &[G, R]).data
        );
        let cw = src.oriented(Orientation::Rotate90Cw);
        assert_eq!((cw.width, cw.height), (1, 2));
        assert_eq!(cw.get(0, 0), R);
        assert_eq!(cw.get(0, 1), G);
        let ccw = src.oriented(Orientation::Rotate90Ccw);
        assert_eq!(ccw.get(0, 0), G);
        assert_eq!(ccw.get(0, 1), R);
        assert_eq!(
            src.oriented(Orientation::Rotate180).data,
            raster(2, 1, &[G, R]).data
        );
    }

    #[test]
    fn four_quarter_turns_are_the_identity() {
        let src = raster(3, 2, &[R, G, B, T, R, G]);
        let mut cur = src.clone();
        for _ in 0..4 {
            cur = cur.oriented(Orientation::Rotate90Cw);
        }
        assert_eq!(cur, src);
    }

    #[test]
    fn orient_region_only_touches_the_rect() {
        // 3×1: [R G B], flip the first two horizontally.
        let mut r = raster(3, 1, &[R, G, B]);
        r.orient_region(Rect::new(0, 0, 2, 1), Orientation::FlipHorizontal);
        assert_eq!(r.data, raster(3, 1, &[G, R, B]).data);
    }

    #[test]
    fn orient_region_quarter_turn_on_non_square_rect_clips_to_the_rect() {
        // 3×1 rect rotated 90° becomes 1×3 centred on the rect: only the
        // middle pixel survives, at the centre column.
        let mut r = raster(3, 1, &[R, G, B]);
        r.orient_region(Rect::new(0, 0, 3, 1), Orientation::Rotate90Cw);
        assert_eq!(r.data, raster(3, 1, &[T, G, T]).data);
    }

    #[test]
    fn resample_nearest_upscale_replicates_pixels() {
        let src = raster(2, 1, &[R, G]);
        let out = src.resample(4, 2, Interpolation::Nearest);
        assert_eq!(out.data, raster(4, 2, &[R, R, G, G, R, R, G, G]).data);
    }

    #[test]
    fn resample_identity_size_is_lossless_for_all_filters() {
        let src = raster(3, 2, &[R, G, B, T, R, G]);
        for f in [
            Interpolation::Nearest,
            Interpolation::Bilinear,
            Interpolation::Bicubic,
        ] {
            assert_eq!(src.resample(3, 2, f), src, "{f:?}");
        }
    }

    #[test]
    fn resample_bilinear_midpoint_blends_and_keeps_transparent_edges_clean() {
        let src = raster(2, 1, &[R, T]);
        let out = src.resample(2, 1, Interpolation::Bilinear);
        // Same size → identity; the transparent pixel stays fully clear.
        assert_eq!(out.get(1, 0), T);
        let down = src.resample(1, 1, Interpolation::Bilinear);
        // Half-covered: premultiplied average keeps the red hue, alpha 128.
        let p = down.get(0, 0);
        assert_eq!(p[0], 255);
        assert_eq!(p[3], 128);
    }

    #[test]
    fn orient_rect_matches_pixel_mapping() {
        // Pixel (0,0) on a 3×2 canvas rotated CW lands at (1,0).
        assert_eq!(orient_point((0, 0), Orientation::Rotate90Cw, 3, 2), (1, 0));
        assert_eq!(orient_point((2, 1), Orientation::Rotate90Cw, 3, 2), (0, 2));
        assert_eq!(orient_point((0, 0), Orientation::Rotate90Ccw, 3, 2), (0, 2));
        assert_eq!(
            orient_point((0, 0), Orientation::FlipHorizontal, 3, 2),
            (2, 0)
        );
        assert_eq!(
            orient_rect(Rect::new(0, 0, 2, 1), Orientation::Rotate180, 3, 2),
            Rect::new(1, 1, 2, 1)
        );
    }
}
