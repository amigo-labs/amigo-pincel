//! Shared non-separable convolution used by sharpen and distort effects, and
//! the tap accumulator both it and the separable blurs
//! ([`crate::blur`]) are built on.

/// Convolves a flat premultiplied RGBA f32 buffer with a 3×3 `kernel`, clamping
/// at the edges. `kernel` is row-major from top-left `(-1, -1)` to bottom-right
/// `(1, 1)` (index 4 is the centre).
///
/// Every output channel starts at 0.0 and adds `sample * weight` for the nine
/// taps in kernel order, as the edge-clamping definition does; only the first
/// and last column clamp per tap, the columns between add contiguous runs of
/// floats tap by tap ([`accumulate_taps`]), so the result is bit-identical.
pub(crate) fn convolve_3x3(src: &[f32], w: usize, h: usize, kernel: &[f32; 9]) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    if w == 0 || h == 0 {
        return out;
    }
    let stride = w * 4;
    // Columns 1..w-1 have both horizontal neighbours inside the row.
    let span = w.saturating_sub(2) * 4;
    let mut taps: Vec<(&[f32], f32)> = Vec::with_capacity(kernel.len());
    for (y, out_row) in out.chunks_exact_mut(stride).enumerate() {
        // Source rows y - 1, y and y + 1, clamped to the image.
        let rows: [&[f32]; 3] = std::array::from_fn(|ky| {
            let sy = (y + ky).saturating_sub(1).min(h - 1);
            &src[sy * stride..(sy + 1) * stride]
        });
        let (out_pixels, _) = out_row.as_chunks_mut::<4>();
        for x in [0, w - 1].into_iter().take(w.min(2)) {
            let mut acc = [0.0f32; 4];
            for (row, weights) in rows.iter().zip(kernel.as_chunks::<3>().0) {
                let (pixels, _) = row.as_chunks::<4>();
                for (kx, &weight) in weights.iter().enumerate() {
                    let sx = (x + kx).saturating_sub(1).min(w - 1);
                    for (a, &s) in acc.iter_mut().zip(&pixels[sx]) {
                        *a += s * weight;
                    }
                }
            }
            out_pixels[x] = acc;
        }
        if span > 0 {
            // Tap (ky, kx) of pixel 1 + i is pixel i + kx of row ky.
            taps.clear();
            taps.extend(
                kernel
                    .iter()
                    .enumerate()
                    .map(|(k, &weight)| (&rows[k / 3][(k % 3) * 4..(k % 3) * 4 + span], weight)),
            );
            accumulate_taps(&mut out_row[4..4 + span], &taps);
        }
    }
    out
}

/// Floats per accumulator tile of [`accumulate_taps`]: small enough to stay in
/// registers or L1 while every tap adds to it.
const TILE: usize = 32;

/// Sets `out[i]` to the sum of `source[i] * weight` over `taps` in order,
/// starting from 0.0. Every source is at least `out.len()` floats long.
pub(crate) fn accumulate_taps(out: &mut [f32], taps: &[(&[f32], f32)]) {
    let (full, rest) = out.as_chunks_mut::<TILE>();
    for (t, out_tile) in full.iter_mut().enumerate() {
        let start = t * TILE;
        let mut acc = [0.0f32; TILE];
        for &(source, weight) in taps {
            for (a, &s) in acc.iter_mut().zip(&source[start..start + TILE]) {
                *a += s * weight;
            }
        }
        *out_tile = acc;
    }
    let start = full.len() * TILE;
    let mut acc = [0.0f32; TILE];
    let acc = &mut acc[..rest.len()];
    let end = start + rest.len();
    for &(source, weight) in taps {
        for (a, &s) in acc.iter_mut().zip(&source[start..end]) {
            *a += s * weight;
        }
    }
    rest.copy_from_slice(acc);
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
