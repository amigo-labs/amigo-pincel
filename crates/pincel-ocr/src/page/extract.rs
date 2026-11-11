//! Line images for the recognizer: each line is cut out of the page with
//! a margin, other lines' ink and non-text ink (rules, frames) painted
//! over with the line's background, straightened when the page is skewed,
//! and scaled down when it is very tall. The recognizer's word boxes are
//! mapped back to the page as given.

use super::lines::{Layout, UNASSIGNED};
use super::{PageBox, PageWord, Source, luma};
use crate::Model;
use crate::page::ink::Ink;

/// Lines taller than this are scaled down before recognition.
const MAX_LINE_HEIGHT: usize = 160;
/// The height a scaled-down line gets.
const SCALED_HEIGHT: usize = 100;
/// Lines lower than this are scaled up twofold: the recognizer reads small
/// UI text better from a larger image (fewer stray marks before capitals).
const MIN_LINE_HEIGHT: usize = 40;

/// An RGB region of the page, `x0..x0 + width`, `y0..y0 + height`.
struct Region {
    x0: usize,
    y0: usize,
    width: usize,
    height: usize,
    data: Vec<[u8; 3]>,
}

impl Region {
    fn cut(src: &Source, x0: usize, y0: usize, x1: usize, y1: usize) -> Self {
        let (width, height) = (x1 - x0, y1 - y0);
        let mut data = Vec::with_capacity(width * height);
        for y in y0..y1 {
            for x in x0..x1 {
                data.push(src.rgb(x, y));
            }
        }
        Self {
            x0,
            y0,
            width,
            height,
            data,
        }
    }

    /// The colour of the median-luma pixel on the region's border.
    fn background(&self) -> [u8; 3] {
        let (w, h) = (self.width, self.height);
        let mut border: Vec<[u8; 3]> = Vec::with_capacity(2 * (w + h));
        for x in 0..w {
            border.push(self.data[x]);
            border.push(self.data[(h - 1) * w + x]);
        }
        for y in 0..h {
            border.push(self.data[y * w]);
            border.push(self.data[y * w + w - 1]);
        }
        border.sort_by_key(|&c| luma(c));
        border[border.len() / 2]
    }

    /// Bilinear sample at a point in page coordinates (pixel centres at
    /// `+0.5`); outside the region it is `fill`.
    fn sample(&self, x: f64, y: f64, fill: [u8; 3]) -> [u8; 3] {
        let fx = x - 0.5 - self.x0 as f64;
        let fy = y - 0.5 - self.y0 as f64;
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let at = |dx: f64, dy: f64| -> [f64; 3] {
            let (px, py) = (ix + dx, iy + dy);
            let c = if px < 0.0 || py < 0.0 || px >= self.width as f64 || py >= self.height as f64 {
                fill
            } else {
                self.data[py as usize * self.width + px as usize]
            };
            [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]
        };
        let (a, b, c, d) = (at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(1.0, 1.0));
        let mut out = [0u8; 3];
        for k in 0..3 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            out[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

/// An RGB image handed to the recognizer.
struct Crop {
    width: usize,
    height: usize,
    data: Vec<[u8; 3]>,
}

impl Crop {
    fn rgba(&self) -> Vec<u8> {
        self.data
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect()
    }

    /// Bilinear upscale by an integer factor.
    fn grow(&self, factor: usize) -> Crop {
        let (w, h) = (self.width * factor, self.height * factor);
        let mut data = Vec::with_capacity(w * h);
        let f = factor as f64;
        for y in 0..h {
            for x in 0..w {
                let sx = ((x as f64 + 0.5) / f - 0.5).clamp(0.0, (self.width - 1) as f64);
                let sy = ((y as f64 + 0.5) / f - 0.5).clamp(0.0, (self.height - 1) as f64);
                let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
                let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
                let (tx, ty) = (sx - x0 as f64, sy - y0 as f64);
                let px =
                    |xx: usize, yy: usize, k: usize| f64::from(self.data[yy * self.width + xx][k]);
                let mut c = [0u8; 3];
                for (k, v) in c.iter_mut().enumerate() {
                    let top = px(x0, y0, k) + (px(x1, y0, k) - px(x0, y0, k)) * tx;
                    let bot = px(x0, y1, k) + (px(x1, y1, k) - px(x0, y1, k)) * tx;
                    *v = (top + (bot - top) * ty).round() as u8;
                }
                data.push(c);
            }
        }
        Crop {
            width: w,
            height: h,
            data,
        }
    }

    /// Area-averaged downscale by an integer factor.
    fn shrink(&self, factor: usize) -> Crop {
        let (w, h) = (self.width.div_ceil(factor), self.height.div_ceil(factor));
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; 3];
                let mut n = 0u32;
                for sy in y * factor..((y + 1) * factor).min(self.height) {
                    for sx in x * factor..((x + 1) * factor).min(self.width) {
                        let c = self.data[sy * self.width + sx];
                        for k in 0..3 {
                            sum[k] += u32::from(c[k]);
                        }
                        n += 1;
                    }
                }
                data.push([
                    ((sum[0] + n / 2) / n) as u8,
                    ((sum[1] + n / 2) / n) as u8,
                    ((sum[2] + n / 2) / n) as u8,
                ]);
            }
        }
        Crop {
            width: w,
            height: h,
            data,
        }
    }
}

/// Recognizes line `index` of the layout; returns its words in page
/// coordinates, left to right.
pub(crate) fn recognize(
    model: &Model,
    src: &Source,
    ink: &Ink,
    layout: &Layout,
    index: usize,
) -> Vec<PageWord> {
    let line = &layout.lines[index];
    let b = line.bbox;
    let pad_u = line.xh.max(4.0);
    let pad_v = (0.3 * b.height()).max(3.0);
    let (u0, u1, v0, v1) = (b.u0 - pad_u, b.u1 + pad_u, b.v0 - pad_v, b.v1 + pad_v);
    // The page region that holds the padded line.
    let corners = [(u0, v0), (u1, v0), (u0, v1), (u1, v1)].map(|(u, v)| layout.to_xy(u, v));
    let (w, h) = (src.width as f64, src.height as f64);
    let x0 = corners
        .iter()
        .map(|c| c.0)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .clamp(0.0, w) as usize;
    let x1 = corners
        .iter()
        .map(|c| c.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .clamp(0.0, w) as usize;
    let y0 = corners
        .iter()
        .map(|c| c.1)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .clamp(0.0, h) as usize;
    let y1 = corners
        .iter()
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .clamp(0.0, h) as usize;
    if x1 <= x0 || y1 <= y0 {
        return Vec::new();
    }
    let mut region = Region::cut(src, x0, y0, x1, y1);
    let fill = region.background();
    mask(&mut region, ink, layout, index, fill);
    let level = layout.sin.abs() * (u1 - u0) < 1.0;
    let crop = if level {
        Crop {
            width: region.width,
            height: region.height,
            data: region.data.clone(),
        }
    } else {
        let cw = (u1 - u0).ceil() as usize;
        let ch = (v1 - v0).ceil() as usize;
        let mut data = Vec::with_capacity(cw * ch);
        for j in 0..ch {
            for i in 0..cw {
                let (x, y) = layout.to_xy(u0 + i as f64 + 0.5, v0 + j as f64 + 0.5);
                data.push(region.sample(x, y, fill));
            }
        }
        Crop {
            width: cw,
            height: ch,
            data,
        }
    };
    // Pixels of the line image per recognizer pixel.
    let (crop, scale) = if crop.height > MAX_LINE_HEIGHT {
        let factor = crop.height.div_ceil(SCALED_HEIGHT);
        (crop.shrink(factor), factor as f64)
    } else if crop.height < MIN_LINE_HEIGHT {
        (crop.grow(2), 0.5)
    } else {
        (crop, 1.0)
    };
    let Ok(found) = model.recognize_line(crop.width as u32, crop.height as u32, &crop.rgba())
    else {
        return Vec::new();
    };
    let (pw, ph) = (src.width as u32, src.height as u32);
    found
        .words
        .into_iter()
        .filter(|word| !word.text.is_empty())
        .map(|word| {
            let f = scale;
            let (l, t) = (f64::from(word.bbox.left) * f, f64::from(word.bbox.top) * f);
            let (r, bt) = (
                f64::from(word.bbox.left + word.bbox.width.max(1)) * f,
                f64::from(word.bbox.top + word.bbox.height.max(1)) * f,
            );
            let pts: [(f64, f64); 4] = if level {
                [(l, t), (r, t), (l, bt), (r, bt)].map(|(x, y)| (x + x0 as f64, y + y0 as f64))
            } else {
                [(l, t), (r, t), (l, bt), (r, bt)].map(|(i, j)| layout.to_xy(u0 + i, v0 + j))
            };
            let left = pts
                .iter()
                .map(|p| p.0)
                .fold(f64::INFINITY, f64::min)
                .floor();
            let top = pts
                .iter()
                .map(|p| p.1)
                .fold(f64::INFINITY, f64::min)
                .floor();
            let right = pts
                .iter()
                .map(|p| p.0)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                - 1.0;
            let bottom = pts
                .iter()
                .map(|p| p.1)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                - 1.0;
            let clamp = |v: f64, max: u32| v.clamp(0.0, f64::from(max - 1)) as u32;
            let (left, top) = (clamp(left, pw), clamp(top, ph));
            PageWord {
                text: word.text,
                bbox: PageBox {
                    left,
                    top,
                    right: clamp(right, pw).max(left),
                    bottom: clamp(bottom, ph).max(top),
                },
                confidence: word.confidence,
            }
        })
        .collect()
}

/// Paints the ink of other lines and non-text ink in the region with
/// `fill`.
fn mask(region: &mut Region, ink: &Ink, layout: &Layout, index: usize, fill: [u8; 3]) {
    let (rx0, ry0) = (region.x0 as i32, region.y0 as i32);
    let (rx1, ry1) = (
        rx0 + region.width as i32 - 1,
        ry0 + region.height as i32 - 1,
    );
    for c in layout.grid.near(rx0, ry0, rx1, ry1) {
        let owner = layout.owner[c as usize];
        if owner == index as u32 || owner == UNASSIGNED {
            continue;
        }
        let comp = &ink.comps[c as usize];
        if comp.right < rx0 || comp.left > rx1 || comp.bottom < ry0 || comp.top > ry1 {
            continue;
        }
        for run in ink.runs_of(comp) {
            let y = run.y as i32;
            if y < ry0 || y > ry1 {
                continue;
            }
            let a = (run.x0 as i32).max(rx0);
            let b = (run.x1 as i32).min(rx1);
            for x in a..=b {
                region.data[((y - ry0) * region.width as i32 + (x - rx0)) as usize] = fill;
            }
        }
    }
}
