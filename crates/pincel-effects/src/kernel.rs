//! Shared non-separable convolution used by sharpen and distort effects.

/// Convolves a flat premultiplied RGBA f32 buffer with a 3×3 `kernel`, clamping
/// at the edges. `kernel` is row-major from top-left `(-1, -1)` to bottom-right
/// `(1, 1)` (index 4 is the centre).
pub(crate) fn convolve_3x3(src: &[f32], w: usize, h: usize, kernel: &[f32; 9]) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            let mut k = 0;
            for ky in -1i32..=1 {
                let sy = (y as i32 + ky).clamp(0, h as i32 - 1) as usize;
                for kx in -1i32..=1 {
                    let sx = (x as i32 + kx).clamp(0, w as i32 - 1) as usize;
                    let weight = kernel[k];
                    let o = (sy * w + sx) * 4;
                    acc[0] += src[o] * weight;
                    acc[1] += src[o + 1] * weight;
                    acc[2] += src[o + 2] * weight;
                    acc[3] += src[o + 3] * weight;
                    k += 1;
                }
            }
            let o = (y * w + x) * 4;
            out[o..o + 4].copy_from_slice(&acc);
        }
    }
    out
}
