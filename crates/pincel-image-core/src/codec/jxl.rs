//! JPEG XL (docs/specs/image-formats-more-modern.md): decoding with
//! `jxl-oxide` — bare codestream or ISOBMFF container, lossy or lossless,
//! the first frame of an animation, more than 8 bits rounded down to 8 —
//! and lossless encoding with `zune-jpegxl`.

use std::io::Cursor;

use jxl_oxide::{JxlImage, PixelFormat};
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_jpegxl::JxlSimpleEncoder;

use super::CodecError;
use crate::document::{CanvasSize, ImageBuffer};

/// The bare codestream's signature.
const CODESTREAM: [u8; 2] = [0xFF, 0x0A];
/// The ISOBMFF container's signature box.
const CONTAINER: [u8; 12] = [
    0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A,
];

/// Whether `bytes` start like a JPEG XL file (AC-04).
pub fn is_jxl(bytes: &[u8]) -> bool {
    bytes.starts_with(&CODESTREAM) || bytes.starts_with(&CONTAINER)
}

fn jxl_error(e: impl std::fmt::Display) -> CodecError {
    CodecError::Jxl(e.to_string())
}

/// Decodes a JPEG XL file into an RGBA8 buffer. The declared size is
/// checked before any pixel is decoded, so a file that only claims a huge
/// canvas fails with the size error (AC-12).
pub fn decode_jxl(bytes: &[u8]) -> Result<ImageBuffer, CodecError> {
    let image = JxlImage::builder()
        .read(Cursor::new(bytes))
        .map_err(jxl_error)?;
    CanvasSize::new(image.width(), image.height())?;
    let format = image.pixel_format();
    if !image.is_loading_done() && image.num_loaded_keyframes() == 0 {
        return Err(CodecError::Jxl(
            "the file ends before its first frame".into(),
        ));
    }
    let render = image.render_frame(0).map_err(jxl_error)?;
    let mut stream = render.stream();
    let (w, h, channels) = (stream.width(), stream.height(), stream.channels() as usize);
    let mut samples = vec![0u8; w as usize * h as usize * channels];
    let written = stream.write_to_buffer(&mut samples);
    if written != samples.len() {
        return Err(CodecError::Jxl(
            "the frame has fewer pixels than declared".into(),
        ));
    }
    let rgba = match (format, channels) {
        (PixelFormat::Rgba, 4) => samples,
        (PixelFormat::Rgb, 3) => expand(&samples, 3, |p| [p[0], p[1], p[2], 255]),
        (PixelFormat::Graya, 2) => expand(&samples, 2, |p| [p[0], p[0], p[0], p[1]]),
        (PixelFormat::Gray, 1) => expand(&samples, 1, |p| [p[0], p[0], p[0], 255]),
        _ => return Err(CodecError::Jxl("CMYK JPEG XL is not supported".into())),
    };
    Ok(ImageBuffer::from_raw(w, h, rgba)?)
}

fn expand(samples: &[u8], channels: usize, px: impl Fn(&[u8]) -> [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() / channels * 4);
    for p in samples.chunks_exact(channels) {
        out.extend_from_slice(&px(p));
    }
    out
}

/// Encodes `buf` as lossless JPEG XL (AC-05, AC-06). The encoder needs at
/// least 2 px on each side.
pub fn to_jxl_bytes(buf: &ImageBuffer) -> Result<Vec<u8>, CodecError> {
    let options = EncoderOptions::new(
        buf.width() as usize,
        buf.height() as usize,
        ColorSpace::RGBA,
        BitDepth::Eight,
    )
    .set_num_threads(1);
    let mut out = Vec::new();
    JxlSimpleEncoder::new(buf.data(), options)
        .encode(&mut out)
        .map_err(|e| CodecError::Jxl(format!("{e:?}")))?;
    Ok(out)
}
