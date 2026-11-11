//! The PNG decode / encode the palette image reader and writer need: RGBA8
//! in, RGBA8 out. Error texts match `pincel-core`'s `import_png` /
//! `encode_png`, so a palette error reads the same as before the codecs
//! moved here (image-palette-gradient AC-21).

use thiserror::Error;

/// Why PNG bytes could not be decoded.
#[derive(Debug, Error)]
pub(crate) enum DecodeError {
    /// The bytes are not a PNG the decoder can read.
    #[error("png decode failed: {0}")]
    Decode(String),
    /// The decoder produced a colour layout this reader does not handle.
    #[error("unsupported png colour layout {0:?}")]
    UnsupportedLayout(String),
}

/// Why pixels could not be encoded as a PNG.
#[derive(Debug, Error)]
#[error("png encode failed: {0}")]
pub(crate) struct EncodeError(String);

/// Decode a PNG into straight-alpha RGBA8 pixels, row-major; every colour
/// type and bit depth is normalised to 8-bit RGBA, an animated PNG yields
/// its first frame.
pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut decoder = ::png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(::png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| DecodeError::Decode("image too large to buffer".to_owned()))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    buf.truncate(info.buffer_size());
    if info.bit_depth != ::png::BitDepth::Eight {
        return Err(DecodeError::UnsupportedLayout(format!(
            "{:?} {:?}",
            info.color_type, info.bit_depth
        )));
    }
    let pixel_count = info.width as usize * info.height as usize;
    let rgba = match info.color_type {
        ::png::ColorType::Rgba => buf,
        ::png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        ::png::ColorType::Grayscale => buf.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        ::png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[v, a]| [v, v, v, a])
            .collect(),
        other => return Err(DecodeError::UnsupportedLayout(format!("{other:?}"))),
    };
    if rgba.len() != pixel_count * 4 {
        return Err(DecodeError::Decode(format!(
            "decoded {} bytes for {}x{} pixels",
            rgba.len(),
            info.width,
            info.height
        )));
    }
    Ok(rgba)
}

/// Encode row-major RGBA8 `pixels` (`width × height × 4` bytes) as a PNG.
pub(crate) fn encode(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| EncodeError(e.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| EncodeError(e.to_string()))?;
    writer.finish().map_err(|e| EncodeError(e.to_string()))?;
    Ok(out)
}
