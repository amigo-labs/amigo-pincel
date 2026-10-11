//! Image import/export (spec §13).
//!
//! Decoding auto-detects the format and yields straight-alpha RGBA8. Encoders
//! write to any [`std::io::Write`] sink (CLAUDE.md §5.1) and convenience
//! wrappers return `Vec<u8>` for the WASM layer.
//!
//! WebP export is lossless in Phase 1 (see ADR-007): the pure-Rust `image`
//! crate cannot encode lossy WebP and CLAUDE.md forbids system dependencies.

use crate::color::Color;
use crate::document::ImageBuffer;
use crate::error::DocumentError;
use crate::render::over_background;
use image::codecs::bmp::BmpEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::io::{Cursor, Write};
use thiserror::Error;

/// Errors from decoding or encoding image data.
#[derive(Debug, Error)]
pub enum CodecError {
    /// The bytes could not be decoded into a known image format.
    #[error("failed to decode image: {0}")]
    Decode(image::ImageError),

    /// Encoding to the target format failed.
    #[error("failed to encode image: {0}")]
    Encode(image::ImageError),

    /// The decoded pixels did not form a valid buffer.
    #[error(transparent)]
    Buffer(#[from] DocumentError),
}

/// Decodes image bytes (PNG/JPEG/WebP/BMP/GIF/TIFF) into an RGBA8 buffer.
///
/// The format is auto-detected from the byte signature. Animated GIFs yield the
/// first frame; higher bit depths are down-converted to 8-bit. Metadata (EXIF
/// etc.) is dropped.
pub fn decode(bytes: &[u8]) -> Result<ImageBuffer, CodecError> {
    let img = image::load_from_memory(bytes).map_err(CodecError::Decode)?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(ImageBuffer::from_raw(w, h, rgba.into_raw())?)
}

/// Maps a 0–9 compression request (spec §13.2) onto the `image` crate's levels.
fn png_compression(level: u8) -> CompressionType {
    match level {
        0..=2 => CompressionType::Fast,
        3..=6 => CompressionType::Default,
        _ => CompressionType::Best,
    }
}

/// Encodes the buffer as PNG to `writer`. `compression` is 0 (fast)–9 (best).
pub fn encode_png<W: Write>(
    buf: &ImageBuffer,
    writer: W,
    compression: u8,
) -> Result<(), CodecError> {
    let encoder =
        PngEncoder::new_with_quality(writer, png_compression(compression), FilterType::Adaptive);
    encoder
        .write_image(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Encodes the buffer as JPEG to `writer`. `quality` is 1–100.
///
/// JPEG has no alpha channel; the buffer is composited over opaque white in
/// linear light, matching Flatten Image (spec §5.2).
pub fn encode_jpeg<W: Write>(buf: &ImageBuffer, writer: W, quality: u8) -> Result<(), CodecError> {
    let flat = over_background(buf.clone(), Color::WHITE);
    let mut rgb = Vec::with_capacity(flat.data().len() / 4 * 3);
    for px in flat.data().as_chunks::<4>().0 {
        rgb.extend_from_slice(&px[..3]);
    }
    let mut encoder = JpegEncoder::new_with_quality(writer, quality.clamp(1, 100));
    encoder
        .encode(&rgb, buf.width(), buf.height(), ExtendedColorType::Rgb8)
        .map_err(CodecError::Encode)
}

/// Encodes the buffer as BMP (32-bit RGBA) to `writer`.
pub fn encode_bmp<W: Write>(buf: &ImageBuffer, mut writer: W) -> Result<(), CodecError> {
    let mut encoder = BmpEncoder::new(&mut writer);
    encoder
        .encode(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Encodes the buffer as lossless WebP to `writer` (see ADR-007).
pub fn encode_webp<W: Write>(buf: &ImageBuffer, writer: W) -> Result<(), CodecError> {
    let encoder = WebPEncoder::new_lossless(writer);
    encoder
        .write_image(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Convenience: encode to PNG bytes.
pub fn to_png_bytes(buf: &ImageBuffer, compression: u8) -> Result<Vec<u8>, CodecError> {
    let mut out = Cursor::new(Vec::new());
    encode_png(buf, &mut out, compression)?;
    Ok(out.into_inner())
}

/// Convenience: encode to JPEG bytes.
pub fn to_jpeg_bytes(buf: &ImageBuffer, quality: u8) -> Result<Vec<u8>, CodecError> {
    let mut out = Cursor::new(Vec::new());
    encode_jpeg(buf, &mut out, quality)?;
    Ok(out.into_inner())
}

/// Convenience: encode to BMP bytes.
pub fn to_bmp_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let mut out = Cursor::new(Vec::new());
    encode_bmp(buf, &mut out)?;
    Ok(out.into_inner())
}

/// Convenience: encode to lossless WebP bytes.
pub fn to_webp_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let mut out = Cursor::new(Vec::new());
    encode_webp(buf, &mut out)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn gradient(w: u32, h: u32) -> ImageBuffer {
        let mut buf = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.set_pixel(x, y, Color::rgba((x * 8) as u8, (y * 8) as u8, 128, 255));
            }
        }
        buf
    }

    #[test]
    fn png_round_trip_is_pixel_exact() {
        let src = gradient(16, 12);
        let bytes = to_png_bytes(&src, 6).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(src, back);
    }

    #[test]
    fn bmp_round_trip_is_pixel_exact() {
        let src = gradient(10, 10);
        let bytes = to_bmp_bytes(&src).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(src, back);
    }

    #[test]
    fn webp_lossless_round_trip_is_pixel_exact() {
        let src = gradient(8, 8);
        let bytes = to_webp_bytes(&src).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(src.width(), back.width());
        assert_eq!(src.data(), back.data());
    }

    #[test]
    fn png_one_by_one_round_trips() {
        let mut src = ImageBuffer::new_transparent(1, 1);
        src.set_pixel(0, 0, Color::rgba(1, 2, 3, 4));
        let bytes = to_png_bytes(&src, 9).unwrap();
        assert_eq!(decode(&bytes).unwrap(), src);
    }

    #[test]
    fn png_preserves_full_transparency() {
        let src = ImageBuffer::new_transparent(4, 4);
        let bytes = to_png_bytes(&src, 0).unwrap();
        let back = decode(&bytes).unwrap();
        assert!(back.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn jpeg_round_trip_is_perceptually_close() {
        let src = gradient(16, 16);
        let bytes = to_jpeg_bytes(&src, 90).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(back.width(), 16);
        // Lossy: assert mean absolute error per channel is small.
        let mut total = 0u64;
        for (a, b) in src
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(back.data().as_chunks::<4>().0)
        {
            for c in 0..3 {
                total += (a[c] as i32 - b[c] as i32).unsigned_abs() as u64;
            }
        }
        let mae = total as f64 / (16.0 * 16.0 * 3.0);
        assert!(mae < 8.0, "JPEG mean abs error too high: {mae}");
    }

    #[test]
    fn jpeg_flattens_transparency_over_white() {
        // JPEG has no alpha; transparent regions must flatten to (near-)white,
        // matching FlattenImage/compose_over — not to black.
        let src = ImageBuffer::new_transparent(8, 8);
        let bytes = to_jpeg_bytes(&src, 90).unwrap();
        let back = decode(&bytes).unwrap();
        let p = back.get_pixel(0, 0).unwrap();
        assert!(
            p.r > 240 && p.g > 240 && p.b > 240,
            "transparent pixel flattened to {p:?}, expected near-white"
        );
    }

    #[test]
    fn decode_garbage_errors() {
        assert!(decode(&[0, 1, 2, 3, 4]).is_err());
    }
}
