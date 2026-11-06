//! Geometric transforms of pixel buffers (spec §10).
//!
//! Pure buffer→buffer functions: the exact 90°/180° rotations and axis flips
//! (lossless index permutations) plus arbitrary [`scale`] with selectable
//! [`Interpolation`]. Layer- and canvas-level wiring (the `TransformLayer` /
//! `ScaleImage` commands, canvas resize, crop) builds on these in later tasks.
//!
//! Resampling is done per channel in f32 and clamped on output (CLAUDE.md §6.1).

use crate::color::Color;
use crate::document::ImageBuffer;
use serde::{Deserialize, Serialize};

/// Resampling quality for [`scale`] and rotation by arbitrary angles (spec §10.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Interpolation {
    /// Nearest-neighbor: crisp, preserves exact colors (pixel-art mode).
    Nearest,
    /// Bilinear: smooth 2×2 weighted average (the default).
    #[default]
    Bilinear,
    /// Bicubic: 4×4 Catmull-Rom, sharper than bilinear on photos.
    Bicubic,
    /// Supersampling: each target pixel is the alpha-weighted mean of the
    /// source area it covers; falls back to [`Interpolation::Bicubic`] when an
    /// edge grows (docs/specs/image-resize-autocrop.md AC-10, AC-11).
    Supersampling,
}

/// Mirrors the buffer left-to-right (same dimensions).
pub fn flip_horizontal(src: &ImageBuffer) -> ImageBuffer {
    let (w, h) = (src.width(), src.height());
    let mut out = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get_pixel(w - 1 - x, y) {
                out.set_pixel(x, y, c);
            }
        }
    }
    out
}

/// Mirrors the buffer top-to-bottom (same dimensions).
pub fn flip_vertical(src: &ImageBuffer) -> ImageBuffer {
    let (w, h) = (src.width(), src.height());
    let mut out = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get_pixel(x, h - 1 - y) {
                out.set_pixel(x, y, c);
            }
        }
    }
    out
}

/// Rotates the buffer 90° clockwise (dimensions swap).
pub fn rotate_90_cw(src: &ImageBuffer) -> ImageBuffer {
    let (w, h) = (src.width(), src.height());
    let mut out = ImageBuffer::new_transparent(h, w);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get_pixel(x, y) {
                out.set_pixel(h - 1 - y, x, c);
            }
        }
    }
    out
}

/// Rotates the buffer 90° counter-clockwise (dimensions swap).
pub fn rotate_90_ccw(src: &ImageBuffer) -> ImageBuffer {
    let (w, h) = (src.width(), src.height());
    let mut out = ImageBuffer::new_transparent(h, w);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get_pixel(x, y) {
                out.set_pixel(y, w - 1 - x, c);
            }
        }
    }
    out
}

/// Rotates the buffer 180° (same dimensions).
pub fn rotate_180(src: &ImageBuffer) -> ImageBuffer {
    let (w, h) = (src.width(), src.height());
    let mut out = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get_pixel(x, y) {
                out.set_pixel(w - 1 - x, h - 1 - y, c);
            }
        }
    }
    out
}

/// Samples `src` at integer `(x, y)`, clamping to the edge, as f32 channels.
fn sample_clamped(src: &ImageBuffer, x: i64, y: i64) -> [f32; 4] {
    let cx = x.clamp(0, src.width() as i64 - 1) as u32;
    let cy = y.clamp(0, src.height() as i64 - 1) as u32;
    let c = src.get_pixel(cx, cy).unwrap_or(Color::TRANSPARENT);
    [c.r as f32, c.g as f32, c.b as f32, c.a as f32]
}

/// Catmull-Rom cubic weights for a fractional offset `t` in `[0, 1)`.
fn cubic_weights(t: f32) -> [f32; 4] {
    // Keys' filter with a = -0.5 (Catmull-Rom), taps at offsets -1, 0, 1, 2.
    let t2 = t * t;
    let t3 = t2 * t;
    [
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    ]
}

/// Packs four clamped f32 channels back into a [`Color`].
fn pack(ch: [f32; 4]) -> Color {
    Color::rgba(
        ch[0].clamp(0.0, 255.0).round() as u8,
        ch[1].clamp(0.0, 255.0).round() as u8,
        ch[2].clamp(0.0, 255.0).round() as u8,
        ch[3].clamp(0.0, 255.0).round() as u8,
    )
}

/// Resamples `src` to `new_w × new_h` using `interp` (spec §10.5).
///
/// Returns a 1×1 transparent buffer for a zero target dimension. Sampling maps
/// destination pixel centers into source space and clamps at the edges.
pub fn scale(src: &ImageBuffer, new_w: u32, new_h: u32, interp: Interpolation) -> ImageBuffer {
    if new_w == 0 || new_h == 0 {
        return ImageBuffer::new_transparent(1, 1);
    }
    let (sw, sh) = (src.width(), src.height());
    let interp = match interp {
        Interpolation::Supersampling if new_w <= sw && new_h <= sh => {
            return area_average(src, new_w, new_h);
        }
        Interpolation::Supersampling => Interpolation::Bicubic,
        other => other,
    };
    let mut out = ImageBuffer::new_transparent(new_w, new_h);
    let scale_x = sw as f32 / new_w as f32;
    let scale_y = sh as f32 / new_h as f32;
    for dy in 0..new_h {
        // Map the destination pixel center to a source coordinate.
        let fy = (dy as f32 + 0.5) * scale_y - 0.5;
        for dx in 0..new_w {
            let fx = (dx as f32 + 0.5) * scale_x - 0.5;
            let color = match interp {
                Interpolation::Nearest => {
                    pack(sample_clamped(src, fx.round() as i64, fy.round() as i64))
                }
                Interpolation::Bilinear => sample_bilinear(src, fx, fy),
                Interpolation::Bicubic | Interpolation::Supersampling => {
                    sample_bicubic(src, fx, fy)
                }
            };
            out.set_pixel(dx, dy, color);
        }
    }
    out
}

/// For each target index along an axis of `src_len` source pixels shrunk to
/// `dst_len`, the source indices it covers and their overlap. In units where
/// a source pixel is `dst_len` long, a target pixel is `src_len` long, so the
/// overlaps are exact integers summing to `src_len`.
fn area_spans(src_len: u32, dst_len: u32) -> Vec<Vec<(u32, u64)>> {
    let (s, d) = (u64::from(src_len), u64::from(dst_len));
    (0..d)
        .map(|t| {
            let (start, end) = (t * s, (t + 1) * s);
            (start / d..end.div_ceil(d))
                .filter_map(|i| {
                    let overlap = end.min((i + 1) * d) - start.max(i * d);
                    (overlap > 0).then_some((i as u32, overlap))
                })
                .collect()
        })
        .collect()
}

/// Area-average downscale (Supersampling): alpha is the area-weighted mean of
/// the covered alphas, RGB the mean weighted by area and alpha. Requires
/// `new_w <= width` and `new_h <= height`.
fn area_average(src: &ImageBuffer, new_w: u32, new_h: u32) -> ImageBuffer {
    let xs = area_spans(src.width(), new_w);
    let ys = area_spans(src.height(), new_h);
    let total = f64::from(src.width()) * f64::from(src.height());
    let mut out = ImageBuffer::new_transparent(new_w, new_h);
    for (dy, ycov) in ys.iter().enumerate() {
        for (dx, xcov) in xs.iter().enumerate() {
            let mut acc = [0f64; 4];
            for &(sy, wy) in ycov {
                for &(sx, wx) in xcov {
                    let c = src.get_pixel(sx, sy).unwrap_or(Color::TRANSPARENT);
                    let wa = (wx * wy) as f64 * f64::from(c.a);
                    acc[0] += wa * f64::from(c.r);
                    acc[1] += wa * f64::from(c.g);
                    acc[2] += wa * f64::from(c.b);
                    acc[3] += wa;
                }
            }
            if acc[3] > 0.0 {
                let ch = |v: f64| v.round().clamp(0.0, 255.0) as u8;
                let color = Color::rgba(
                    ch(acc[0] / acc[3]),
                    ch(acc[1] / acc[3]),
                    ch(acc[2] / acc[3]),
                    ch(acc[3] / total),
                );
                out.set_pixel(dx as u32, dy as u32, color);
            }
        }
    }
    out
}

/// Bilinear sample at fractional source coordinate `(fx, fy)`.
fn sample_bilinear(src: &ImageBuffer, fx: f32, fy: f32) -> Color {
    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    // Sample the four surrounding texels once, then lerp per channel.
    let c00 = sample_clamped(src, x0, y0);
    let c10 = sample_clamped(src, x0 + 1, y0);
    let c01 = sample_clamped(src, x0, y0 + 1);
    let c11 = sample_clamped(src, x0 + 1, y0 + 1);
    let mut ch = [0.0f32; 4];
    for (i, out) in ch.iter_mut().enumerate() {
        let top = c00[i] * (1.0 - tx) + c10[i] * tx;
        let bot = c01[i] * (1.0 - tx) + c11[i] * tx;
        *out = top * (1.0 - ty) + bot * ty;
    }
    pack(ch)
}

/// Bicubic (Catmull-Rom) sample at fractional source coordinate `(fx, fy)`.
fn sample_bicubic(src: &ImageBuffer, fx: f32, fy: f32) -> Color {
    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let wx = cubic_weights(fx - x0 as f32);
    let wy = cubic_weights(fy - y0 as f32);
    let mut ch = [0.0f32; 4];
    for (j, wyj) in wy.iter().enumerate() {
        let sy = y0 - 1 + j as i64;
        let mut row = [0.0f32; 4];
        for (i, wxi) in wx.iter().enumerate() {
            let sx = x0 - 1 + i as i64;
            let s = sample_clamped(src, sx, sy);
            for c in 0..4 {
                row[c] += s[c] * wxi;
            }
        }
        for c in 0..4 {
            ch[c] += row[c] * wyj;
        }
    }
    pack(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer with a distinct color per pixel so permutations are detectable.
    fn ramp(w: u32, h: u32) -> ImageBuffer {
        let mut b = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                b.set_pixel(x, y, Color::rgba((x * 10) as u8, (y * 10) as u8, 0, 255));
            }
        }
        b
    }

    #[test]
    fn flip_horizontal_twice_is_identity() {
        let src = ramp(4, 3);
        assert_eq!(flip_horizontal(&flip_horizontal(&src)), src);
    }

    #[test]
    fn flip_vertical_twice_is_identity() {
        let src = ramp(4, 3);
        assert_eq!(flip_vertical(&flip_vertical(&src)), src);
    }

    #[test]
    fn flip_horizontal_mirrors_columns() {
        let src = ramp(4, 1);
        let f = flip_horizontal(&src);
        assert_eq!(f.get_pixel(0, 0), src.get_pixel(3, 0));
        assert_eq!(f.get_pixel(3, 0), src.get_pixel(0, 0));
    }

    #[test]
    fn rotate_90_cw_four_times_is_identity() {
        let src = ramp(4, 3);
        let r = rotate_90_cw(&rotate_90_cw(&rotate_90_cw(&rotate_90_cw(&src))));
        assert_eq!(r, src);
    }

    #[test]
    fn rotate_90_swaps_dimensions() {
        let r = rotate_90_cw(&ramp(4, 3));
        assert_eq!((r.width(), r.height()), (3, 4));
    }

    #[test]
    fn rotate_90_cw_then_ccw_is_identity() {
        let src = ramp(4, 3);
        assert_eq!(rotate_90_ccw(&rotate_90_cw(&src)), src);
    }

    #[test]
    fn rotate_180_is_two_90s() {
        let src = ramp(4, 3);
        assert_eq!(rotate_180(&src), rotate_90_cw(&rotate_90_cw(&src)));
    }

    #[test]
    fn rotate_90_cw_moves_top_left_to_top_right() {
        // CW rotation sends the source top-left pixel to the dst top-right.
        let src = ramp(4, 3);
        let r = rotate_90_cw(&src);
        let top_right_x = r.width() - 1;
        assert_eq!(r.get_pixel(top_right_x, 0), src.get_pixel(0, 0));
    }

    #[test]
    fn scale_nearest_doubling_is_exact_block_replication() {
        let src = ramp(2, 2);
        let up = scale(&src, 4, 4, Interpolation::Nearest);
        // Each source pixel becomes a 2×2 block.
        assert_eq!(up.get_pixel(0, 0), src.get_pixel(0, 0));
        assert_eq!(up.get_pixel(1, 1), src.get_pixel(0, 0));
        assert_eq!(up.get_pixel(2, 0), src.get_pixel(1, 0));
    }

    #[test]
    fn scale_to_same_size_is_identity_for_nearest() {
        let src = ramp(5, 4);
        assert_eq!(scale(&src, 5, 4, Interpolation::Nearest), src);
    }

    #[test]
    fn scale_up_then_down_bicubic_is_near_identity() {
        // A smooth ramp survives ×2 then ×0.5 within a small tolerance.
        let src = ramp(8, 8);
        let up = scale(&src, 16, 16, Interpolation::Bicubic);
        let back = scale(&up, 8, 8, Interpolation::Bicubic);
        let mut max_diff = 0i32;
        for y in 0..8 {
            for x in 0..8 {
                let a = src.get_pixel(x, y).unwrap();
                let b = back.get_pixel(x, y).unwrap();
                max_diff = max_diff
                    .max((a.r as i32 - b.r as i32).abs())
                    .max((a.g as i32 - b.g as i32).abs());
            }
        }
        assert!(max_diff <= 6, "max channel diff {max_diff}");
    }

    #[test]
    fn scale_zero_dimension_yields_one_by_one() {
        let out = scale(&ramp(4, 4), 0, 4, Interpolation::Bilinear);
        assert_eq!((out.width(), out.height()), (1, 1));
    }

    fn px(c: Color) -> [u8; 4] {
        [c.r, c.g, c.b, c.a]
    }

    #[test]
    fn scale_supersampling_integer_factor_is_block_mean() {
        // 4 × 2 → 2 × 1: each target pixel is the mean of a 2 × 2 block.
        let mut src = ImageBuffer::new_transparent(4, 2);
        let vals = [10u8, 20, 30, 40, 50, 60, 70, 80];
        for (i, v) in vals.iter().enumerate() {
            src.set_pixel(i as u32 % 4, i as u32 / 4, Color::rgba(*v, 0, 0, 255));
        }
        let out = scale(&src, 2, 1, Interpolation::Supersampling);
        assert_eq!((out.width(), out.height()), (2, 1));
        // (10 + 20 + 50 + 60) / 4 = 35, (30 + 40 + 70 + 80) / 4 = 55.
        assert_eq!(px(out.get_pixel(0, 0).unwrap()), [35, 0, 0, 255]);
        assert_eq!(px(out.get_pixel(1, 0).unwrap()), [55, 0, 0, 255]);
    }

    #[test]
    fn scale_supersampling_three_to_two_weighs_partial_pixels_by_area() {
        // Target 0 covers source 0 fully and half of source 1: (a + b/2) / 1.5.
        let mut src = ImageBuffer::new_transparent(3, 1);
        src.set_pixel(0, 0, Color::rgba(0, 0, 0, 255));
        src.set_pixel(1, 0, Color::rgba(90, 0, 0, 255));
        src.set_pixel(2, 0, Color::rgba(240, 0, 0, 255));
        let out = scale(&src, 2, 1, Interpolation::Supersampling);
        // (0 + 45) / 1.5 = 30; (45 + 240) / 1.5 = 190.
        assert_eq!(px(out.get_pixel(0, 0).unwrap()), [30, 0, 0, 255]);
        assert_eq!(px(out.get_pixel(1, 0).unwrap()), [190, 0, 0, 255]);
    }

    #[test]
    fn scale_supersampling_weights_color_by_alpha() {
        // Opaque red next to transparent black stays red at half alpha.
        let mut src = ImageBuffer::new_transparent(2, 1);
        src.set_pixel(0, 0, Color::rgba(255, 0, 0, 255));
        let out = scale(&src, 1, 1, Interpolation::Supersampling);
        let c = out.get_pixel(0, 0).unwrap();
        assert_eq!([c.r, c.g, c.b], [255, 0, 0]);
        assert!(c.a == 127 || c.a == 128, "alpha {}", c.a);
    }

    #[test]
    fn scale_supersampling_fully_transparent_area_is_transparent_black() {
        let mut src = ImageBuffer::new_transparent(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                src.set_pixel(x, y, Color::rgba(200, 100, 50, 0));
            }
        }
        let out = scale(&src, 2, 2, Interpolation::Supersampling);
        assert_eq!(px(out.get_pixel(1, 1).unwrap()), [0, 0, 0, 0]);
    }

    #[test]
    fn scale_supersampling_growing_an_edge_equals_bicubic() {
        let src = ramp(6, 6);
        assert_eq!(
            scale(&src, 4, 9, Interpolation::Supersampling),
            scale(&src, 4, 9, Interpolation::Bicubic)
        );
        assert_eq!(
            scale(&src, 12, 12, Interpolation::Supersampling),
            scale(&src, 12, 12, Interpolation::Bicubic)
        );
    }

    #[test]
    fn scale_supersampling_same_size_is_identity() {
        let src = ramp(5, 4);
        assert_eq!(scale(&src, 5, 4, Interpolation::Supersampling), src);
    }

    /// The exact area-weighted mean of target `(dx, dy)`, by brute force over
    /// a grid fine enough that every source and target edge lies on it.
    fn exact_area_mean(src: &ImageBuffer, nw: u32, nh: u32, dx: u32, dy: u32) -> [f64; 4] {
        let (sw, sh) = (src.width(), src.height());
        let (mut a, mut r, mut g, mut b, mut n) = (0f64, 0f64, 0f64, 0f64, 0f64);
        // Subpixel (u, v) on a sw·nw × sh·nh grid: source u / nw, target u / sw.
        for v in dy * sh..(dy + 1) * sh {
            for u in dx * sw..(dx + 1) * sw {
                let c = src.get_pixel(u / nw, v / nh).unwrap();
                let ca = f64::from(c.a);
                a += ca;
                r += ca * f64::from(c.r);
                g += ca * f64::from(c.g);
                b += ca * f64::from(c.b);
                n += 1.0;
            }
        }
        if a == 0.0 {
            return [0.0; 4];
        }
        [r / a, g / a, b / a, a / n]
    }

    proptest::proptest! {
        #[test]
        fn scale_supersampling_matches_exact_area_mean(
            sw in 1u32..9, sh in 1u32..9, nw_seed in 1u32..9, nh_seed in 1u32..9,
            data in proptest::collection::vec(proptest::num::u8::ANY, 8 * 8 * 4),
        ) {
            let (nw, nh) = (nw_seed.min(sw), nh_seed.min(sh));
            let mut src = ImageBuffer::new_transparent(sw, sh);
            for y in 0..sh {
                for x in 0..sw {
                    let i = ((y * 8 + x) * 4) as usize;
                    src.set_pixel(x, y, Color::rgba(data[i], data[i + 1], data[i + 2], data[i + 3]));
                }
            }
            let out = scale(&src, nw, nh, Interpolation::Supersampling);
            for dy in 0..nh {
                for dx in 0..nw {
                    let want = exact_area_mean(&src, nw, nh, dx, dy);
                    let got = px(out.get_pixel(dx, dy).unwrap());
                    for ch in 0..4 {
                        let diff = (f64::from(got[ch]) - want[ch]).abs();
                        proptest::prop_assert!(diff <= 1.0, "({dx},{dy}) ch{ch}: {got:?} vs {want:?}");
                    }
                }
            }
        }
    }
}
