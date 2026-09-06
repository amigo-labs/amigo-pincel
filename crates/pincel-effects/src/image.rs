//! Owned RGBA8 pixel buffer and the f32 conversions effects work in.
//!
//! The crate is independent of `pincel-core`: callers hand over raw
//! RGBA8 bytes, effects run internally in f32, and the result comes back as
//! RGBA8. See [`crate`] docs for the colour-space and alpha conventions.

use crate::error::EffectError;

/// A rectangular block of straight-alpha RGBA8 pixels, row-major, 4 bytes per
/// pixel.
///
/// This mirrors `pincel-core`'s RGBA `PixelBuffer` byte layout so a layer's pixels
/// cross into the effects crate without reinterpretation, but the two types are
/// deliberately unrelated (the effects crate has no core dependency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl EffectImage {
    /// Wraps raw RGBA8 bytes.
    ///
    /// Returns [`EffectError::BufferSizeMismatch`] unless
    /// `data.len() == width * height * 4`.
    pub fn from_rgba8(width: u32, height: u32, data: Vec<u8>) -> Result<Self, EffectError> {
        let expected = width as usize * height as usize * 4;
        if data.len() != expected {
            return Err(EffectError::BufferSizeMismatch {
                width,
                height,
                expected,
                got: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Creates a fully transparent buffer of the given size.
    pub fn new_transparent(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * 4],
        }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Read-only view of the raw RGBA8 bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the buffer, returning the raw RGBA8 bytes.
    pub fn into_raw(self) -> Vec<u8> {
        self.data
    }

    /// Converts to a flat premultiplied-alpha f32 buffer in `[0.0, 1.0]`,
    /// length `width * height * 4`.
    ///
    /// RGB is multiplied by alpha so convolutions never bleed the (arbitrary)
    /// colour of fully transparent pixels into opaque neighbours. Values stay in
    /// gamma (sRGB) space — effects operate on sRGB samples (see [`crate`]).
    pub fn to_premultiplied_f32(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.data.len()];
        for (dst, px) in out.chunks_exact_mut(4).zip(self.data.chunks_exact(4)) {
            let a = px[3] as f32 / 255.0;
            dst[0] = (px[0] as f32 / 255.0) * a;
            dst[1] = (px[1] as f32 / 255.0) * a;
            dst[2] = (px[2] as f32 / 255.0) * a;
            dst[3] = a;
        }
        out
    }

    /// Rebuilds an RGBA8 buffer from a flat premultiplied-alpha f32 buffer.
    ///
    /// RGB is un-premultiplied (divided by alpha), then every channel is clamped
    /// to `[0.0, 1.0]` and rounded. `data.len()` must be `width * height * 4`.
    pub fn from_premultiplied_f32(width: u32, height: u32, data: &[f32]) -> Self {
        debug_assert_eq!(data.len(), width as usize * height as usize * 4);
        let mut out = vec![0u8; width as usize * height as usize * 4];
        for (dst, px) in out.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
            let a = px[3].clamp(0.0, 1.0);
            let inv = if a > 0.0 { 1.0 / a } else { 0.0 };
            dst[0] = to_u8((px[0] * inv).clamp(0.0, 1.0));
            dst[1] = to_u8((px[1] * inv).clamp(0.0, 1.0));
            dst[2] = to_u8((px[2] * inv).clamp(0.0, 1.0));
            dst[3] = to_u8(a);
        }
        Self {
            width,
            height,
            data: out,
        }
    }

    /// Bilinearly resamples to `(w, h)` in straight-alpha space.
    ///
    /// Used by [`Effect::preview`](crate::Effect::preview) to run an effect on a
    /// cheap downscaled copy. `w` and `h` are clamped to at least 1.
    pub fn resized(&self, w: u32, h: u32) -> Self {
        let w = w.max(1);
        let h = h.max(1);
        if w == self.width && h == self.height {
            return self.clone();
        }
        let mut out = vec![0u8; w as usize * h as usize * 4];
        // Map destination pixel centres back into source space.
        let sx = self.width as f32 / w as f32;
        let sy = self.height as f32 / h as f32;
        for dy in 0..h {
            let fy = ((dy as f32 + 0.5) * sy - 0.5).max(0.0);
            let y0 = fy.floor() as u32;
            let y1 = (y0 + 1).min(self.height - 1);
            let wy = fy - y0 as f32;
            for dx in 0..w {
                let fx = ((dx as f32 + 0.5) * sx - 0.5).max(0.0);
                let x0 = fx.floor() as u32;
                let x1 = (x0 + 1).min(self.width - 1);
                let wx = fx - x0 as f32;
                let o = (dy as usize * w as usize + dx as usize) * 4;
                for c in 0..4 {
                    let p00 = self.channel(x0, y0, c);
                    let p10 = self.channel(x1, y0, c);
                    let p01 = self.channel(x0, y1, c);
                    let p11 = self.channel(x1, y1, c);
                    let top = p00 + (p10 - p00) * wx;
                    let bot = p01 + (p11 - p01) * wx;
                    out[o + c] = to_u8((top + (bot - top) * wy) / 255.0);
                }
            }
        }
        Self {
            width: w,
            height: h,
            data: out,
        }
    }

    /// One channel of pixel `(x, y)` as f32 in `[0.0, 255.0]` (no bounds check).
    fn channel(&self, x: u32, y: u32, c: usize) -> f32 {
        self.data[(y as usize * self.width as usize + x as usize) * 4 + c] as f32
    }
}

/// Rounds an f32 in `[0.0, 1.0]` to the nearest RGBA8 byte.
fn to_u8(v: f32) -> u8 {
    (v * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rgba8_rejects_wrong_length() {
        let err = EffectImage::from_rgba8(2, 2, vec![0; 8]).unwrap_err();
        assert_eq!(
            err,
            EffectError::BufferSizeMismatch {
                width: 2,
                height: 2,
                expected: 16,
                got: 8,
            }
        );
    }

    #[test]
    fn premultiplied_round_trip_preserves_opaque_pixels() {
        let src = EffectImage::from_rgba8(1, 2, vec![10, 20, 30, 255, 200, 100, 50, 255]).unwrap();
        let f = src.to_premultiplied_f32();
        let back = EffectImage::from_premultiplied_f32(1, 2, &f);
        assert_eq!(back, src);
    }

    #[test]
    fn premultiplied_transparent_pixel_has_zero_color_weight() {
        // A transparent pixel with nonzero RGB contributes nothing premultiplied.
        let src = EffectImage::from_rgba8(1, 1, vec![255, 128, 64, 0]).unwrap();
        let f = src.to_premultiplied_f32();
        assert_eq!(f, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn resized_same_dims_is_identity() {
        let src = EffectImage::from_rgba8(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap();
        assert_eq!(src.resized(2, 1), src);
    }

    #[test]
    fn resized_solid_color_stays_solid() {
        // A flat colour must survive up- and down-scaling unchanged.
        let data: Vec<u8> = std::iter::repeat_n([90, 40, 200, 255], 16)
            .flatten()
            .collect();
        let src = EffectImage::from_rgba8(4, 4, data).unwrap();
        let down = src.resized(2, 2);
        for px in down.data().chunks_exact(4) {
            assert_eq!(px, &[90, 40, 200, 255]);
        }
    }
}
