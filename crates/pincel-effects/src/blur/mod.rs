//! Blur effects (Fineliner spec §11.1): Gaussian, Box, Motion, Radial.
//!
//! All four run in premultiplied-alpha gamma space (see [`crate`]). Gaussian and
//! Box are separable convolutions built on [`convolve_axis`]; Motion and Radial
//! resample along a path with [`sample_bilinear`].

mod box_blur;
mod gaussian;
mod motion;
mod radial;

pub use box_blur::BoxBlur;
pub use gaussian::GaussianBlur;
pub use motion::MotionBlur;
pub use radial::{RadialBlur, RadialKind};

/// Convolves a flat premultiplied RGBA f32 buffer with a 1-D `kernel` along one
/// axis, clamping at the edges. `kernel` is centred on `kernel.len() / 2` and is
/// expected to sum to 1.0 for a blur.
pub(crate) fn convolve_axis(
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
