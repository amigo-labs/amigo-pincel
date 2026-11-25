//! Heavy one-shot jobs on a document — encoding for export and autosave,
//! effect regions and full-resolution effect previews — shared by every
//! backend that runs them: the wasm job worker and the main-thread
//! fallback (`pincel-image-wasm`). One function per job keeps their results
//! byte-identical (docs/specs/performance-background-jobs.md).

use crate::codec::{
    CodecError, OraError, to_bmp_bytes, to_ico_bytes, to_jpeg_bytes, to_jxl_bytes, to_ora_bytes,
    to_png_bytes, to_tga_bytes, to_tiff_bytes, to_webp_bytes,
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
    /// Flattened lossless JPEG XL (at least 2 px on each side).
    Jxl,
}

impl EncodeFormat {
    /// The format named `name` (`"ora"`, `"png"`, `"jpeg"`, `"webp"`,
    /// `"bmp"`, `"tiff"`, `"tga"`, `"ico"`, `"jxl"` — the UI's `EncodeFormat`).
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
            "jxl" => Self::Jxl,
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
        EncodeFormat::Jxl => to_jxl_bytes(&compose(doc.layers()))?,
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

/// `src` (a `src_w` × `src_h` grid of `bpp`-byte cells) point-sampled to
/// `w` × `h`: each output cell takes the source cell under its centre.
fn point_sample(src: &[u8], src_w: u32, src_h: u32, bpp: usize, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0; w as usize * h as usize * bpp];
    let xs: Vec<usize> = (0..w)
        .map(|x| ((u64::from(x) * 2 + 1) * u64::from(src_w) / (u64::from(w) * 2)) as usize)
        .collect();
    for y in 0..h {
        let sy = ((u64::from(y) * 2 + 1) * u64::from(src_h) / (u64::from(h) * 2)) as usize;
        let row = &src[sy * src_w as usize * bpp..(sy + 1) * src_w as usize * bpp];
        let dst = &mut out[y as usize * w as usize * bpp..(y as usize + 1) * w as usize * bpp];
        for (cell, &sx) in dst.chunks_exact_mut(bpp).zip(&xs) {
            cell.copy_from_slice(&row[sx * bpp..(sx + 1) * bpp]);
        }
    }
    out
}

/// `doc` point-sampled to a `w` × `h` canvas: every canvas-sized layer and
/// the selection; other layers (groups) keep their pixels.
pub fn sampled_document(doc: &Document, w: u32, h: u32) -> Result<Document, JobError> {
    let (cw, ch) = (doc.canvas.width(), doc.canvas.height());
    let mut small = doc.clone();
    small.canvas = crate::document::CanvasSize::new(w, h)?;
    for layer in &mut small.layers {
        let px = &layer.pixels;
        if px.width() == cw && px.height() == ch {
            let raw = point_sample(px.data(), cw, ch, 4, w, h);
            layer.pixels = ImageBuffer::from_raw(w, h, raw)?;
        }
    }
    if let Some(mask) = &doc.selection {
        let raw = point_sample(mask.data(), mask.width(), mask.height(), 1, w, h);
        small.selection = crate::selection::SelectionMask::from_raw(w, h, raw);
    }
    Ok(small)
}

/// The effect dialog's instant preview: [`preview_effect`] on `doc`
/// point-sampled to `w` × `h` (docs/specs/performance-background-jobs.md
/// AC-05). The caller scales the spec's lengths to the smaller canvas.
pub fn preview_effect_proxy(
    doc: &Document,
    layer: usize,
    spec: &EffectSpec,
    w: u32,
    h: u32,
) -> Result<ImageBuffer, JobError> {
    if layer >= doc.layers().len() {
        return Err(JobError::LayerOutOfBounds);
    }
    preview_effect(&sampled_document(doc, w, h)?, layer, spec)
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
        assert_eq!(EncodeFormat::parse("jxl").ok(), Some(EncodeFormat::Jxl));
        assert!(matches!(
            EncodeFormat::parse("psd"),
            Err(JobError::UnknownFormat(_))
        ));
    }

    #[test]
    fn encode_jxl_decodes_back_to_the_composite() {
        let doc = Document::new(5, 3).expect("doc");
        let bytes = encode(&doc, EncodeFormat::Jxl, 0, 0).expect("jxl");
        assert!(crate::codec::is_jxl(&bytes));
        let back = crate::codec::decode(&bytes).expect("decodes");
        assert_eq!(back, compose(doc.layers()));
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

    fn varied_doc() -> Document {
        let mut doc = Document::new(40, 30).expect("doc");
        doc.add_layer("Top").expect("layer");
        for (i, layer) in doc.layers.iter_mut().enumerate() {
            for (j, v) in layer.pixels.data_mut().iter_mut().enumerate() {
                *v = ((j * 7 + i * 31) % 251) as u8;
            }
        }
        doc.selection = Some(crate::selection::SelectionMask::rectangle(
            40,
            30,
            Rect::new(5, 4, 20, 15),
        ));
        doc
    }

    #[test]
    fn preview_effect_proxy_of_a_per_pixel_effect_samples_the_full_preview() {
        let doc = varied_doc();
        let spec = EffectSpec::from_params("invert", &[]).expect("invert");
        let full = preview_effect(&doc, 1, &spec).expect("full");
        let proxy = preview_effect_proxy(&doc, 1, &spec, 16, 12).expect("proxy");
        assert_eq!((proxy.width(), proxy.height()), (16, 12));
        assert_eq!(proxy.data(), point_sample(full.data(), 40, 30, 4, 16, 12));
    }

    #[test]
    fn preview_effect_proxy_leaves_the_document_alone() {
        let doc = varied_doc();
        let before = doc.clone();
        let spec = EffectSpec::from_params("gaussian_blur", &[2.0]).expect("blur");
        let proxy = preview_effect_proxy(&doc, 0, &spec, 20, 15).expect("proxy");
        assert_eq!((proxy.width(), proxy.height()), (20, 15));
        assert_eq!(doc, before);
        assert!(matches!(
            preview_effect_proxy(&doc, 5, &spec, 20, 15),
            Err(JobError::LayerOutOfBounds)
        ));
    }

    #[test]
    fn point_sample_takes_the_cell_under_each_centre() {
        let src: Vec<u8> = (0..16).collect();
        assert_eq!(point_sample(&src, 4, 4, 1, 2, 2), vec![5, 7, 13, 15]);
        assert_eq!(point_sample(&src, 4, 4, 1, 4, 4), src);
    }
}
