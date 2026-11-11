//! Blur effects (Fineliner spec §11.1): Gaussian, Box, Motion, Radial; and
//! paint.net's Bokeh, Fragment, Median, Sketch and Surface Blur
//! (docs/specs/effects-blurs.md).
//!
//! The averaging blurs run in premultiplied-alpha gamma space (see [`crate`]).
//! Gaussian and Box are separable convolutions built on [`convolve_axis`];
//! Motion, Radial, Fragment and Sketch resample along a path with
//! [`sample_bilinear`]. Median works on straight colours of opaque pixels.
//! Every blur reads neighbours outside the layer as the nearest edge pixel.

use crate::kernel::accumulate_taps;

mod bokeh;
mod box_blur;
mod fragment;
mod gaussian;
mod median;
mod motion;
mod radial;
mod sketch;
mod surface;

pub use bokeh::Bokeh;
pub use box_blur::BoxBlur;
pub use fragment::Fragment;
pub use gaussian::GaussianBlur;
pub(crate) use gaussian::gaussian_kernel;
pub use median::Median;
pub use motion::MotionBlur;
pub use radial::{RadialBlur, RadialKind};
pub use sketch::SketchBlur;
pub use surface::SurfaceBlur;

/// Reads a parameter as finite (non-finite reads as 0) and clamps it to
/// `lo..=hi`, the convention of every effect (`effects-adjustments`).
pub(crate) fn param(v: f32, lo: f32, hi: f32) -> f32 {
    let v = if v.is_finite() { v } else { 0.0 };
    v.clamp(lo, hi)
}

/// Like [`param`], rounded to a whole number.
pub(crate) fn whole(v: f32, lo: u32, hi: u32) -> u32 {
    param(v, lo as f32, hi as f32).round() as u32
}

/// Reads an angle in degrees as finite (non-finite reads as 0) and wraps it
/// into `0..360`, as the angle dial does.
pub(crate) fn angle(v: f32) -> f32 {
    if v.is_finite() {
        let w = v.rem_euclid(360.0);
        // rem_euclid can round up to exactly 360.0 for tiny negatives.
        if w >= 360.0 { 0.0 } else { w }
    } else {
        0.0
    }
}

/// Convolves a flat premultiplied RGBA f32 buffer with a 1-D `kernel` along one
/// axis, clamping at the edges. `kernel` is centred on `kernel.len() / 2` and is
/// expected to sum to 1.0 for a blur.
///
/// Every output channel starts at 0.0 and adds `sample * weight` for the taps
/// in kernel order, so the result is bit-identical however the loops are
/// arranged: [`axis_row`] clamps a source coordinate only for the pixels whose
/// taps leave the image and adds contiguous runs of floats tap by tap
/// ([`accumulate_taps`]) everywhere else, which the compiler vectorises.
pub(crate) fn convolve_axis(
    src: &[f32],
    w: usize,
    h: usize,
    kernel: &[f32],
    horizontal: bool,
) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    if w == 0 {
        return out;
    }
    crate::par::for_each_row(&mut out, w * 4, |y, row| {
        axis_row(src, w, h, kernel, horizontal, y, row);
    });
    out
}

/// Computes output row `y` of [`convolve_axis`] into `row`
/// (`out[y * w * 4..(y + 1) * w * 4]`, all zeros on entry).
fn axis_row(
    src: &[f32],
    w: usize,
    h: usize,
    kernel: &[f32],
    horizontal: bool,
    y: usize,
    row: &mut [f32],
) {
    if kernel.is_empty() {
        return;
    }
    let stride = w * 4;
    let center = kernel.len() / 2;
    if !horizontal {
        let taps: Vec<(&[f32], f32)> = kernel
            .iter()
            .enumerate()
            .map(|(k, &weight)| {
                // Row `y + k - center`, clamped to the image.
                let sy = (y + k).saturating_sub(center).min(h - 1);
                (&src[sy * stride..(sy + 1) * stride], weight)
            })
            .collect();
        accumulate_taps(row, &taps);
        return;
    }
    let src_row = &src[y * stride..(y + 1) * stride];
    let right = kernel.len() - 1 - center;
    // Interior pixels `lo..hi` have every tap inside the row:
    // x - center >= 0 and x + right <= w - 1. A non-empty interior starts at
    // `lo == center`, so tap `k` of pixel `lo + i` is pixel `i + k`.
    let lo = center.min(w);
    let hi = w.saturating_sub(right).max(lo);
    let (pixels, _) = src_row.as_chunks::<4>();
    let (out_pixels, _) = row.as_chunks_mut::<4>();
    for x in (0..lo).chain(hi..w) {
        let mut acc = [0.0f32; 4];
        for (k, &weight) in kernel.iter().enumerate() {
            let sx = (x + k).saturating_sub(center).min(w - 1);
            for (a, &s) in acc.iter_mut().zip(&pixels[sx]) {
                *a += s * weight;
            }
        }
        out_pixels[x] = acc;
    }
    let span = (hi - lo) * 4;
    if span > 0 {
        let taps: Vec<(&[f32], f32)> = kernel
            .iter()
            .enumerate()
            .map(|(k, &weight)| (&src_row[k * 4..k * 4 + span], weight))
            .collect();
        accumulate_taps(&mut row[lo * 4..hi * 4], &taps);
    }
}

/// Bilinearly samples a flat premultiplied RGBA f32 buffer at fractional
/// `(x, y)`, clamping coordinates to the image bounds.
pub(crate) fn sample_bilinear(src: &[f32], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let fx = x.clamp(0.0, (w - 1) as f32);
    let fy = y.clamp(0.0, (h - 1) as f32);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let wx = fx - x0 as f32;
    let wy = fy - y0 as f32;
    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let p00 = src[(y0 * w + x0) * 4 + c];
        let p10 = src[(y0 * w + x1) * 4 + c];
        let p01 = src[(y1 * w + x0) * 4 + c];
        let p11 = src[(y1 * w + x1) * 4 + c];
        let top = p00 + (p10 - p00) * wx;
        let bot = p01 + (p11 - p01) * wx;
        out[c] = top + (bot - top) * wy;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{hash, unit};

    /// The original per-tap clamping `convolve_axis`, frozen verbatim as the
    /// bit-exact reference for the optimised one.
    fn convolve_axis_reference(
        src: &[f32],
        w: usize,
        h: usize,
        kernel: &[f32],
        horizontal: bool,
    ) -> Vec<f32> {
        let mut out = vec![0.0f32; src.len()];
        let center = (kernel.len() / 2) as isize;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for (k, &weight) in kernel.iter().enumerate() {
                    let offset = k as isize - center;
                    let (sx, sy) = if horizontal {
                        ((x as isize + offset).clamp(0, w as isize - 1) as usize, y)
                    } else {
                        (x, (y as isize + offset).clamp(0, h as isize - 1) as usize)
                    };
                    let o = (sy * w + sx) * 4;
                    acc[0] += src[o] * weight;
                    acc[1] += src[o + 1] * weight;
                    acc[2] += src[o + 2] * weight;
                    acc[3] += src[o + 3] * weight;
                }
                let o = (y * w + x) * 4;
                out[o..o + 4].copy_from_slice(&acc);
            }
        }
        out
    }

    /// `n` seeded values in `[0, 1]`, with exact 0.0 and 1.0 mixed in.
    fn values(seed: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| match hash(seed, i, 0) % 8 {
                0 => 0.0,
                1 => 1.0,
                _ => unit(hash(seed, i, 1)),
            })
            .collect()
    }

    /// `len` seeded weights in `[-1, 1]`.
    fn weights(seed: u32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|k| unit(hash(seed, k, 2)) * 2.0 - 1.0)
            .collect()
    }

    fn assert_bits_eq(got: &[f32], want: &[f32], context: &str) {
        assert_eq!(got.len(), want.len(), "{context}: length");
        for (i, (g, r)) in got.iter().zip(want).enumerate() {
            assert_eq!(g.to_bits(), r.to_bits(), "{context}: element {i}");
        }
    }

    #[test]
    fn convolve_axis_matches_reference_bit_for_bit() {
        for case in 0..400u32 {
            let seed = case.wrapping_mul(2_654_435_761);
            let w = 1 + (hash(seed, 0, 10) % 40) as usize;
            let h = 1 + (hash(seed, 0, 11) % 40) as usize;
            let len = 1 + 2 * (hash(seed, 0, 12) % 21) as usize;
            let src = values(seed, w * h * 4);
            let kernel = weights(seed, len);
            for horizontal in [true, false] {
                let context = format!("case {case}: {w}x{h}, len {len}, horizontal {horizontal}");
                assert_bits_eq(
                    &convolve_axis(&src, w, h, &kernel, horizontal),
                    &convolve_axis_reference(&src, w, h, &kernel, horizontal),
                    &context,
                );
            }
        }
    }

    #[test]
    fn convolve_axis_single_pixel_and_wide_kernel_matches_reference() {
        let src = values(7, 4);
        for len in [1, 3, 41] {
            let kernel = weights(len as u32, len);
            for horizontal in [true, false] {
                assert_bits_eq(
                    &convolve_axis(&src, 1, 1, &kernel, horizontal),
                    &convolve_axis_reference(&src, 1, 1, &kernel, horizontal),
                    &format!("1x1, len {len}, horizontal {horizontal}"),
                );
            }
        }
    }
}
