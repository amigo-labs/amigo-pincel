//! Heavy one-shot jobs on a document — encoding for export and autosave,
//! effect regions and full-resolution effect previews — shared by every
//! backend that runs them: the wasm job worker and the main-thread
//! fallback (`pincel-image-wasm`) and the desktop app's native job backend
//! (`pincel-tauri`). One function per job keeps their results
//! byte-identical (docs/specs/performance-background-jobs.md,
//! docs/specs/performance-desktop-jobs.md AC-01).

use crate::codec::{
    CodecError, OraError, to_bmp_bytes, to_ico_bytes, to_jpeg_bytes, to_ora_bytes, to_png_bytes,
    to_tga_bytes, to_tiff_bytes, to_webp_bytes,
};
use crate::document::{Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::Rect;
use crate::render::compose;
use crate::selection::masked_region;
use pincel_effects::{EffectError, EffectImage, EffectSpec};
use thiserror::Error;

/// Why a job failed.
#[derive(Debug, Error)]
pub enum JobError {
    /// The job named a layer the document does not have.
    #[error("layer index out of bounds")]
    LayerOutOfBounds,
    /// The job named a format no encoder exists for.
    #[error("unknown encode format `{0}`")]
    UnknownFormat(String),
    /// An effect region's wire form could not be read.
    #[error("effect region header is truncated")]
    TruncatedRegion,
    /// The effect could not run.
    #[error(transparent)]
    Effect(#[from] EffectError),
    /// A pixel buffer did not fit the document.
    #[error(transparent)]
    Document(#[from] DocumentError),
    /// A flat image encoder failed.
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// The OpenRaster writer failed.
    #[error(transparent)]
    Ora(#[from] OraError),
}

/// What a document can be written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeFormat {
    /// OpenRaster project with every layer (save, autosave).
    Ora,
    /// Flattened PNG.
    Png,
    /// Flattened JPEG.
    Jpeg,
    /// Flattened lossless WebP.
    Webp,
    /// Flattened 32-bit BMP.
    Bmp,
    /// Flattened lossless TIFF.
    Tiff,
    /// Flattened RLE-compressed 32-bit TGA.
    Tga,
    /// Flattened one-image ICO (at most 256 × 256 px).
    Ico,
}

impl EncodeFormat {
    /// The format named `name` (`"ora"`, `"png"`, `"jpeg"`, `"webp"`,
    /// `"bmp"`, `"tiff"`, `"tga"`, `"ico"` — the UI's `EncodeFormat`).
    pub fn parse(name: &str) -> Result<Self, JobError> {
        Ok(match name {
            "ora" => Self::Ora,
            "png" => Self::Png,
            "jpeg" => Self::Jpeg,
            "webp" => Self::Webp,
            "bmp" => Self::Bmp,
            "tiff" => Self::Tiff,
            "tga" => Self::Tga,
            "ico" => Self::Ico,
            other => return Err(JobError::UnknownFormat(other.to_string())),
        })
    }
}

/// Encodes `doc` as `format`: `level` is the 0–9 PNG / OpenRaster
/// compression level, `quality` the 1–100 JPEG quality; formats ignore
/// what they do not use.
pub fn encode(
    doc: &Document,
    format: EncodeFormat,
    level: u8,
    quality: u8,
) -> Result<Vec<u8>, JobError> {
    Ok(match format {
        EncodeFormat::Ora => to_ora_bytes(doc, level)?,
        EncodeFormat::Png => to_png_bytes(&compose(doc.layers()), level)?,
        EncodeFormat::Jpeg => to_jpeg_bytes(&compose(doc.layers()), quality)?,
        EncodeFormat::Webp => to_webp_bytes(&compose(doc.layers()))?,
        EncodeFormat::Bmp => to_bmp_bytes(&compose(doc.layers()))?,
        EncodeFormat::Tiff => to_tiff_bytes(&compose(doc.layers()))?,
        EncodeFormat::Tga => to_tga_bytes(&compose(doc.layers()))?,
        EncodeFormat::Ico => to_ico_bytes(&compose(doc.layers()))?,
    })
}

/// Runs `spec` on the layer at `layer` and keeps the result inside the
/// selection (spec §8.1): the changed region and its new pixels, or `None`
/// when the selection is empty.
pub fn effect_region(
    doc: &Document,
    layer: usize,
    spec: &EffectSpec,
) -> Result<Option<(Rect, ImageBuffer)>, JobError> {
    let before = &doc
        .layers()
        .get(layer)
        .ok_or(JobError::LayerOutOfBounds)?
        .pixels;
    let (w, h) = (before.width(), before.height());
    let src = EffectImage::from_rgba8(w, h, before.data().to_vec())?;
    let selection = doc.selection.as_ref();
    let out = spec.run_in(&src, |x, y| selection.is_none_or(|m| m.coverage(x, y) > 0));
    let after = ImageBuffer::from_raw(w, h, out.into_raw())?;
    Ok(masked_region(before, &after, selection))
}

/// The composite of `doc` with `spec` run on the layer at `layer`, without
/// changing the document (the effect dialog's full-resolution preview).
pub fn preview_effect(
    doc: &Document,
    layer: usize,
    spec: &EffectSpec,
) -> Result<ImageBuffer, JobError> {
    let region = effect_region(doc, layer, spec)?;
    let mut layers = doc.layers().to_vec();
    if let Some((rect, after)) = region {
        layers[layer].pixels.paste_region(rect, &after)?;
    }
    Ok(compose(&layers))
}

/// The wire form of an effect region: empty for `None`, otherwise a
/// 16-byte header (x, y as little-endian `i32`, width, height as
/// little-endian `u32`) followed by the region's RGBA8 pixels.
pub fn encode_region(region: Option<(Rect, ImageBuffer)>) -> Vec<u8> {
    let Some((rect, pixels)) = region else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(16 + pixels.data().len());
    out.extend_from_slice(&rect.x.to_le_bytes());
    out.extend_from_slice(&rect.y.to_le_bytes());
    out.extend_from_slice(&rect.w.to_le_bytes());
    out.extend_from_slice(&rect.h.to_le_bytes());
    out.extend_from_slice(pixels.data());
    out
}

/// Reads [`encode_region`]'s wire form back.
pub fn decode_region(bytes: &[u8]) -> Result<Option<(Rect, ImageBuffer)>, JobError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let field = |i: usize| -> Result<[u8; 4], JobError> {
        bytes
            .get(i * 4..i * 4 + 4)
            .and_then(|b| b.try_into().ok())
            .ok_or(JobError::TruncatedRegion)
    };
    let rect = Rect {
        x: i32::from_le_bytes(field(0)?),
        y: i32::from_le_bytes(field(1)?),
        w: u32::from_le_bytes(field(2)?),
        h: u32::from_le_bytes(field(3)?),
    };
    let pixels = ImageBuffer::from_raw(rect.w, rect.h, bytes[16..].to_vec())?;
    Ok(Some((rect, pixels)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_format_parse_unknown_name_returns_err() {
        assert_eq!(EncodeFormat::parse("png").ok(), Some(EncodeFormat::Png));
        assert_eq!(EncodeFormat::parse("tga").ok(), Some(EncodeFormat::Tga));
        assert_eq!(EncodeFormat::parse("ico").ok(), Some(EncodeFormat::Ico));
        assert!(matches!(
            EncodeFormat::parse("psd"),
            Err(JobError::UnknownFormat(_))
        ));
    }

    #[test]
    fn decode_region_reads_what_encode_region_wrote() {
        let pixels = ImageBuffer::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).expect("2x1");
        let rect = Rect::new(-3, 4, 2, 1);
        let bytes = encode_region(Some((rect, pixels.clone())));
        let (back_rect, back) = decode_region(&bytes).expect("decodes").expect("some");
        assert_eq!(back_rect, rect);
        assert_eq!(back.data(), pixels.data());
        assert!(decode_region(&[]).expect("empty").is_none());
        assert!(matches!(
            decode_region(&bytes[..10]),
            Err(JobError::TruncatedRegion)
        ));
    }

    #[test]
    fn preview_effect_with_bad_layer_returns_err() {
        let doc = Document::new(4, 4).expect("doc");
        let spec = EffectSpec::from_params("invert", &[]).expect("invert");
        assert!(matches!(
            preview_effect(&doc, 9, &spec),
            Err(JobError::LayerOutOfBounds)
        ));
    }
}
