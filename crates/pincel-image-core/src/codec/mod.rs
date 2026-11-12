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
use crate::render::over_background_rgb;
use image::codecs::bmp::BmpEncoder;
use image::codecs::ico::IcoEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::tga::TgaEncoder;
use image::codecs::tiff::TiffEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, ImageFormat};
use std::io::{Cursor, Seek, Write};
pub mod ora;
pub mod psd;
pub mod snapshot;

pub use ora::{OraError, from_ora_bytes, to_ora_bytes};

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

    /// A DDS file whose compression is not BC1 – BC3
    /// (docs/specs/image-formats-more.md AC-11).
    #[error("this DDS variant is not supported (only BC1, BC2 and BC3)")]
    DdsUnsupported,

    /// A DDS file whose width or height is not a multiple of 4 (AC-11).
    #[error("DDS width and height must be multiples of 4 (this file is {width} × {height})")]
    DdsSize {
        /// Declared width.
        width: u32,
        /// Declared height.
        height: u32,
    },

    /// ICO holds at most 256 × 256 px per image (AC-08).
    #[error("ICO holds at most 256 × 256 px")]
    IcoTooLarge,
}

/// Largest ICO image edge, px.
pub const ICO_MAX_SIDE: u32 = 256;

/// A little-endian `u32` at `at`, if the bytes reach that far.
fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Refuses a DDS file Pincel cannot read with its own reason (AC-11): a
/// compression other than BC1 – BC3 (FourCC `DXT1` / `DXT3` / `DXT5`, or a
/// DX10 header with DXGI format 70 – 78), or a size that is not a multiple
/// of 4. A header too short to tell is left to the decoder (AC-12).
fn check_dds(bytes: &[u8]) -> Result<(), CodecError> {
    let (Some(height), Some(width), Some(four_cc)) =
        (le_u32(bytes, 12), le_u32(bytes, 16), bytes.get(84..88))
    else {
        return Ok(());
    };
    let supported = match four_cc {
        b"DXT1" | b"DXT3" | b"DXT5" => true,
        b"DX10" => match le_u32(bytes, 128) {
            Some(dxgi) => (70..=78).contains(&dxgi),
            None => return Ok(()),
        },
        _ => false,
    };
    if !supported {
        return Err(CodecError::DdsUnsupported);
    }
    if width % 4 != 0 || height % 4 != 0 {
        return Err(CodecError::DdsSize { width, height });
    }
    Ok(())
}

/// Whether `hint` (a MIME type or a file name) names a TGA file.
fn hints_tga(hint: &str) -> bool {
    let h = hint.to_ascii_lowercase();
    h == "image/x-targa" || h == "image/x-tga" || h.ends_with(".tga")
}

/// Decodes image bytes like [`decode`], with a format `hint` (a MIME type
/// or a file name) for formats without a signature: TGA is recognised only
/// through its hint, and only when the bytes carry no known signature
/// (docs/specs/image-formats-more.md AC-04).
pub fn decode_hinted(bytes: &[u8], hint: Option<&str>) -> Result<ImageBuffer, CodecError> {
    let format = match image::guess_format(bytes) {
        Ok(f) => Some(f),
        Err(_) if hint.is_some_and(hints_tga) => Some(ImageFormat::Tga),
        Err(_) => None,
    };
    if format == Some(ImageFormat::Dds) {
        check_dds(bytes)?;
    }
    let img = match format {
        Some(f) => image::load_from_memory_with_format(bytes, f),
        None => image::load_from_memory(bytes),
    }
    .map_err(CodecError::Decode)?;
    // Moves the pixels when the decoder already gave RGBA8, converts otherwise.
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(ImageBuffer::from_raw(w, h, rgba.into_raw())?)
}

/// Decodes image bytes (PNG/JPEG/WebP/BMP/GIF/TIFF/DDS/ICO) into an RGBA8 buffer.
///
/// The format is auto-detected from the byte signature. Animated GIFs yield the
/// first frame; higher bit depths are down-converted to 8-bit. Metadata (EXIF
/// etc.) is dropped.
pub fn decode(bytes: &[u8]) -> Result<ImageBuffer, CodecError> {
    decode_hinted(bytes, None)
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
    let rgb = over_background_rgb(buf, Color::WHITE);
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

/// Encodes the buffer as lossless RGBA8 TIFF to `writer`.
///
/// TIFF needs a seekable sink (the IFD offsets are patched after the strips).
pub fn encode_tiff<W: Write + Seek>(buf: &ImageBuffer, writer: W) -> Result<(), CodecError> {
    TiffEncoder::new(writer)
        .write_image(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Encodes the buffer as RLE-compressed 32-bit TGA to `writer`.
pub fn encode_tga<W: Write>(buf: &ImageBuffer, writer: W) -> Result<(), CodecError> {
    TgaEncoder::new(writer)
        .write_image(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Encodes the buffer as an ICO holding one image (PNG-compressed) to
/// `writer`; [`CodecError::IcoTooLarge`] above 256 × 256 px.
pub fn encode_ico<W: Write>(buf: &ImageBuffer, writer: W) -> Result<(), CodecError> {
    if buf.width() > ICO_MAX_SIDE || buf.height() > ICO_MAX_SIDE {
        return Err(CodecError::IcoTooLarge);
    }
    IcoEncoder::new(writer)
        .write_image(
            buf.data(),
            buf.width(),
            buf.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(CodecError::Encode)
}

/// Convenience: encode to TGA bytes.
pub fn to_tga_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    encode_tga(buf, &mut out)?;
    Ok(out)
}

/// Convenience: encode to ICO bytes.
pub fn to_ico_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    encode_ico(buf, &mut out)?;
    Ok(out)
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

/// Convenience: encode to lossless TIFF bytes.
pub fn to_tiff_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let mut out = Cursor::new(Vec::new());
    encode_tiff(buf, &mut out)?;
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
    fn to_tiff_bytes_round_trip_is_pixel_exact() {
        let mut src = gradient(9, 7);
        src.set_pixel(3, 2, Color::rgba(10, 20, 30, 0));
        src.set_pixel(4, 2, Color::rgba(200, 100, 50, 77));
        let bytes = to_tiff_bytes(&src).unwrap();
        assert!(bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"));
        assert_eq!(decode(&bytes).unwrap(), src);
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

    /// Decoding before `into_rgba8`: the decoded image copied by
    /// `to_rgba8`, kept as the reference the moving decode must reproduce.
    fn decode_hinted_reference(
        bytes: &[u8],
        hint: Option<&str>,
    ) -> Result<ImageBuffer, CodecError> {
        let format = match image::guess_format(bytes) {
            Ok(f) => Some(f),
            Err(_) if hint.is_some_and(hints_tga) => Some(ImageFormat::Tga),
            Err(_) => None,
        };
        if format == Some(ImageFormat::Dds) {
            check_dds(bytes)?;
        }
        let img = match format {
            Some(f) => image::load_from_memory_with_format(bytes, f),
            None => image::load_from_memory(bytes),
        }
        .map_err(CodecError::Decode)?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Ok(ImageBuffer::from_raw(w, h, rgba.into_raw())?)
    }

    /// A 7 × 5 PNG of `color` type: varied 8-bit samples (16-bit types get
    /// each sample twice), so every channel and the alpha differ per pixel.
    fn png_of(color: ExtendedColorType) -> Vec<u8> {
        let (w, h) = (7u32, 5u32);
        let samples = w as usize * h as usize * usize::from(color.bits_per_pixel()) / 8;
        let data: Vec<u8> = (0..samples).map(|i| (i * 37 % 251) as u8).collect();
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(&data, w, h, color)
            .expect("PNG encodes");
        out
    }

    #[test]
    fn decode_matches_the_copying_decode_for_every_png_color_type() {
        let mut inputs: Vec<(String, Vec<u8>)> = [
            ExtendedColorType::Rgba8,
            ExtendedColorType::Rgb8,
            ExtendedColorType::La8,
            ExtendedColorType::L8,
            ExtendedColorType::Rgba16,
            ExtendedColorType::Rgb16,
            ExtendedColorType::La16,
            ExtendedColorType::L16,
        ]
        .into_iter()
        .map(|c| (format!("PNG {c:?}"), png_of(c)))
        .collect();
        inputs.push((
            "JPEG".to_owned(),
            to_jpeg_bytes(&gradient(16, 12), 90).expect("JPEG encodes"),
        ));
        for (name, bytes) in inputs {
            let fast = decode(&bytes).expect("decodes");
            let reference = decode_hinted_reference(&bytes, None).expect("decodes");
            assert_eq!(
                (fast.width(), fast.height()),
                (reference.width(), reference.height()),
                "{name}"
            );
            assert!(fast.data() == reference.data(), "{name}");
        }
    }
}
