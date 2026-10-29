//! Integer nearest-neighbour upscale for exports. See
//! `docs/specs/pincel.md` §7.5 and `docs/specs/scaled-export.md`.
//!
//! Exports compose at zoom 1 and scale afterwards, so every source pixel
//! becomes an `N×N` block of exactly its RGBA value. Shared by the
//! animated export and the scaled PNG / sheet export.

use super::png::ExportError;

/// Largest accepted export scale factor.
pub(crate) const MAX_EXPORT_SCALE: u32 = 10;

/// Largest exported width or height in pixels.
pub(crate) const MAX_EXPORT_EDGE: u32 = 16384;

/// Validate `scale` against `1..=10` and return the scaled size of a
/// `width × height` image, refusing anything above [`MAX_EXPORT_EDGE`] on
/// either edge.
pub(crate) fn scaled_size(width: u32, height: u32, scale: u32) -> Result<(u32, u32), ExportError> {
    if !(1..=MAX_EXPORT_SCALE).contains(&scale) {
        return Err(ExportError::InvalidScale { scale });
    }
    let w = u64::from(width) * u64::from(scale);
    let h = u64::from(height) * u64::from(scale);
    let limit = u64::from(MAX_EXPORT_EDGE);
    if w > limit || h > limit {
        return Err(ExportError::ScaledTooLarge {
            width: w,
            height: h,
        });
    }
    // Both edges are at most 16384 here, so the casts are lossless.
    Ok((w as u32, h as u32))
}

/// Upscale a tight RGBA8 `width × height` buffer by `scale` into `out`
/// (cleared first), replicating each pixel into a `scale × scale` block.
///
/// `scale` must already be validated by [`scaled_size`]; `src` must hold
/// exactly `width * height * 4` bytes.
pub(crate) fn upscale_nearest(src: &[u8], width: u32, height: u32, scale: u32, out: &mut Vec<u8>) {
    out.clear();
    let n = scale as usize;
    let src_stride = width as usize * 4;
    out.reserve(src_stride * n * height as usize * n);
    for row in src.chunks_exact(src_stride.max(1)).take(height as usize) {
        let start = out.len();
        for px in row.as_chunks::<4>().0 {
            for _ in 0..n {
                out.extend_from_slice(px);
            }
        }
        let end = out.len();
        // Repeat the widened row `n - 1` more times.
        for _ in 1..n {
            out.extend_from_within(start..end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_size_multiplies_both_edges() {
        assert_eq!(scaled_size(3, 2, 4).expect("valid"), (12, 8));
    }

    #[test]
    fn scaled_size_scale_out_of_range_returns_err() {
        for scale in [0, 11, 255] {
            let err = scaled_size(1, 1, scale).expect_err("refused");
            assert!(matches!(err, ExportError::InvalidScale { scale: s } if s == scale));
            assert!(err.to_string().contains("1–10"));
        }
    }

    #[test]
    fn scaled_size_exactly_at_limit_is_accepted() {
        assert_eq!(scaled_size(1638, 1, 10).expect("fits"), (16380, 10));
        assert_eq!(scaled_size(16384, 4, 1).expect("at limit"), (16384, 4));
    }

    #[test]
    fn scaled_size_above_limit_returns_err_naming_limit() {
        let err = scaled_size(1639, 1, 10).expect_err("16390 px refused");
        assert!(matches!(
            err,
            ExportError::ScaledTooLarge {
                width: 16390,
                height: 10
            }
        ));
        assert!(err.to_string().contains("16384"));
        assert!(scaled_size(1, 16385, 1).is_err());
    }

    #[test]
    fn upscale_nearest_makes_exact_blocks() {
        // 2×1: red, half-alpha blue.
        let src = [255, 0, 0, 255, 0, 0, 255, 128];
        let mut out = Vec::new();
        upscale_nearest(&src, 2, 1, 3, &mut out);
        assert_eq!(out.len(), 6 * 3 * 4);
        for y in 0..3 {
            for x in 0..6 {
                let i = (y * 6 + x) * 4;
                let expected = if x < 3 { &src[0..4] } else { &src[4..8] };
                assert_eq!(&out[i..i + 4], expected, "pixel ({x},{y})");
            }
        }
    }

    #[test]
    fn upscale_nearest_scale_one_is_identity() {
        let src: Vec<u8> = (0..2 * 2 * 4).map(|v| v as u8).collect();
        let mut out = Vec::new();
        upscale_nearest(&src, 2, 2, 1, &mut out);
        assert_eq!(out, src);
    }
}
