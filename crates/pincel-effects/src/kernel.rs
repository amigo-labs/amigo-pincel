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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{hash, unit};

    /// The original per-tap clamping `convolve_3x3`, frozen verbatim as the
    /// bit-exact reference for the optimised one.
    fn convolve_3x3_reference(src: &[f32], w: usize, h: usize, kernel: &[f32; 9]) -> Vec<f32> {
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

    #[test]
    fn convolve_3x3_matches_reference_bit_for_bit() {
        for case in 0..400u32 {
            let seed = case.wrapping_mul(2_654_435_761);
            let (w, h) = if case == 0 {
                (1, 1)
            } else {
                (
                    1 + (hash(seed, 0, 10) % 40) as usize,
                    1 + (hash(seed, 0, 11) % 40) as usize,
                )
            };
            let src: Vec<f32> = (0..w * h * 4)
                .map(|i| match hash(seed, i, 0) % 8 {
                    0 => 0.0,
                    1 => 1.0,
                    _ => unit(hash(seed, i, 1)),
                })
                .collect();
            let kernel: [f32; 9] = std::array::from_fn(|k| unit(hash(seed, k, 2)) * 4.0 - 2.0);
            let got = convolve_3x3(&src, w, h, &kernel);
            let want = convolve_3x3_reference(&src, w, h, &kernel);
            assert_eq!(got.len(), want.len());
            for (i, (g, r)) in got.iter().zip(&want).enumerate() {
                assert_eq!(
                    g.to_bits(),
                    r.to_bits(),
                    "case {case}: {w}x{h}, element {i}"
                );
            }
        }
    }
}
