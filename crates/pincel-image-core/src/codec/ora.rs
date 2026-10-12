//! OpenRaster (`.ora`) — Image mode's layered project format.
//!
//! OpenRaster is the open layered-image interchange format GIMP, Krita and
//! MyPaint read and write: a ZIP whose first entry is the uncompressed
//! `mimetype` (`image/openraster`), a `stack.xml` describing the layer
//! stack, one PNG per layer under `data/`, a full-size `mergedimage.png`
//! and a `Thumbnails/thumbnail.png` (≤ 256 px). See
//! <https://www.openraster.org/> and `docs/specs/pincel.md` §15
//! (2026-10-11).
//!
//! Mapping: opacity ↔ `opacity` (0–1), visible ↔ `visibility`, locked ↔
//! Krita's `edit-locked`, the active layer ↔ `selected`, blend modes ↔
//! the `svg:*` `composite-op` names. On read, nested `<stack>`s are
//! flattened in document order, layer PNGs are placed at their `x` / `y`
//! offset on a canvas-sized buffer, and unknown composite ops fall back
//! to Normal.

use std::io::{Cursor, Read, Write};

use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{CodecError, decode, to_png_bytes};
use crate::color::BlendMode;
use crate::document::{Document, ImageBuffer, Layer};
use crate::error::DocumentError;
use crate::render::compose;
use crate::transform::{Interpolation, scale};

/// The OpenRaster MIME type, stored as the archive's first entry.
pub const ORA_MIMETYPE: &str = "image/openraster";

/// Errors from reading or writing OpenRaster files.
#[derive(Debug, Error)]
pub enum OraError {
    /// The ZIP container could not be read or written.
    #[error("zip error: {0}")]
    Zip(String),
    /// A required archive entry is missing.
    #[error("missing archive entry `{0}`")]
    MissingEntry(String),
    /// The `mimetype` entry is not `image/openraster`.
    #[error("not an OpenRaster file (mimetype `{0}`)")]
    NotOpenRaster(String),
    /// `stack.xml` is malformed or lacks required attributes.
    #[error("invalid stack.xml: {0}")]
    Stack(String),
    /// A layer PNG could not be decoded or encoded.
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// The resulting document violates a document invariant.
    #[error(transparent)]
    Document(#[from] DocumentError),
}

impl From<zip::result::ZipError> for OraError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e.to_string())
    }
}

impl From<std::io::Error> for OraError {
    fn from(e: std::io::Error) -> Self {
        Self::Zip(e.to_string())
    }
}

fn composite_op(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "svg:src-over",
        BlendMode::Multiply => "svg:multiply",
        BlendMode::Screen => "svg:screen",
        BlendMode::Overlay => "svg:overlay",
        BlendMode::Darken => "svg:darken",
        BlendMode::Lighten => "svg:lighten",
        BlendMode::ColorDodge => "svg:color-dodge",
        BlendMode::ColorBurn => "svg:color-burn",
        BlendMode::HardLight => "svg:hard-light",
        BlendMode::SoftLight => "svg:soft-light",
        BlendMode::Difference => "svg:difference",
        BlendMode::Exclusion => "svg:exclusion",
    }
}

fn blend_from_op(op: &str) -> BlendMode {
    match op {
        "svg:multiply" => BlendMode::Multiply,
        "svg:screen" => BlendMode::Screen,
        "svg:overlay" => BlendMode::Overlay,
        "svg:darken" => BlendMode::Darken,
        "svg:lighten" => BlendMode::Lighten,
        "svg:color-dodge" => BlendMode::ColorDodge,
        "svg:color-burn" => BlendMode::ColorBurn,
        "svg:hard-light" => BlendMode::HardLight,
        "svg:soft-light" => BlendMode::SoftLight,
        "svg:difference" => BlendMode::Difference,
        "svg:exclusion" => BlendMode::Exclusion,
        _ => BlendMode::Normal,
    }
}

/// Serialize `doc` as an OpenRaster archive. `compression` is the 0–9 PNG
/// level for the layer images (autosave passes a fast level).
pub fn to_ora_bytes(doc: &Document, compression: u8) -> Result<Vec<u8>, OraError> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    // PNGs are already deflated; the XML is tiny. Store everything.
    zip.start_file("mimetype", stored)?;
    zip.write_all(ORA_MIMETYPE.as_bytes())?;

    let (w, h) = (doc.canvas.width(), doc.canvas.height());
    let mut xml = format!(
        "<?xml version='1.0' encoding='UTF-8'?>\n\
         <image version=\"0.0.6\" w=\"{w}\" h=\"{h}\" xres=\"72\" yres=\"72\">\n<stack>\n"
    );
    let layers = doc.layers();
    let active = doc.active_layer_index();
    // stack.xml lists layers top-most first; storage is bottom-up.
    for (index, layer) in layers.iter().enumerate().rev() {
        let src = format!("data/layer{index}.png");
        zip.start_file(&src, stored)?;
        zip.write_all(&to_png_bytes(&layer.pixels, compression)?)?;
        xml.push_str(&format!(
            "  <layer name=\"{}\" src=\"{src}\" x=\"0\" y=\"0\" opacity=\"{:.3}\" \
             visibility=\"{}\" composite-op=\"{}\"{}{} />\n",
            escape(&layer.name),
            layer.opacity.clamp(0.0, 1.0),
            if layer.visible { "visible" } else { "hidden" },
            composite_op(layer.blend_mode),
            if layer.locked {
                " edit-locked=\"true\""
            } else {
                ""
            },
            if index == active {
                " selected=\"true\""
            } else {
                ""
            },
        ));
    }
    xml.push_str("</stack>\n</image>\n");
    zip.start_file("stack.xml", stored)?;
    zip.write_all(xml.as_bytes())?;

    let merged = compose(layers);
    zip.start_file("mergedimage.png", stored)?;
    zip.write_all(&to_png_bytes(&merged, compression)?)?;

    let longest = w.max(h);
    let thumb = if longest > 256 {
        let tw = (u64::from(w) * 256 / u64::from(longest)).max(1) as u32;
        let th = (u64::from(h) * 256 / u64::from(longest)).max(1) as u32;
        scale(&merged, tw, th, Interpolation::Bilinear)
    } else {
        merged
    };
    zip.start_file("Thumbnails/thumbnail.png", stored)?;
    zip.write_all(&to_png_bytes(&thumb, compression)?)?;

    Ok(zip.finish()?.into_inner())
}

/// Parse an OpenRaster archive into a document.
pub fn from_ora_bytes(bytes: &[u8]) -> Result<Document, OraError> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let mimetype = read_entry(&mut zip, "mimetype")?;
    let mimetype = String::from_utf8_lossy(&mimetype).trim().to_string();
    if mimetype != ORA_MIMETYPE {
        return Err(OraError::NotOpenRaster(mimetype));
    }
    let xml = String::from_utf8(read_entry(&mut zip, "stack.xml")?)
        .map_err(|_| OraError::Stack("not UTF-8".into()))?;

    let image = tags(&xml, "image")
        .into_iter()
        .next()
        .ok_or_else(|| OraError::Stack("no <image> element".into()))?;
    let dim = |name: &str| -> Result<u32, OraError> {
        attr(&image, name)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| OraError::Stack(format!("<image> needs a numeric `{name}`")))
    };
    let (w, h) = (dim("w")?, dim("h")?);

    // `tags` returns document order, i.e. top-most layer first.
    let mut stack = Vec::new();
    let mut selected = None;
    for (i, tag) in tags(&xml, "layer").iter().enumerate() {
        let src =
            attr(tag, "src").ok_or_else(|| OraError::Stack("<layer> without `src`".into()))?;
        let png = decode(&read_entry(&mut zip, &src)?)?;
        let x = attr(tag, "x").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y = attr(tag, "y").and_then(|v| v.parse().ok()).unwrap_or(0);
        let pixels: ImageBuffer = if (x, y, png.width(), png.height()) == (0, 0, w, h) {
            png
        } else {
            png.offset_copy(w, h, x, y)
        };
        let mut layer = Layer::from_pixels(
            attr(tag, "name").unwrap_or_else(|| format!("Layer {}", i + 1)),
            pixels,
        );
        layer.opacity = attr(tag, "opacity")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        layer.visible = attr(tag, "visibility").as_deref() != Some("hidden");
        layer.blend_mode = blend_from_op(attr(tag, "composite-op").as_deref().unwrap_or(""));
        layer.locked = attr(tag, "edit-locked").as_deref() == Some("true");
        if attr(tag, "selected").as_deref() == Some("true") {
            selected = Some(i);
        }
        stack.push(layer);
    }

    let mut doc = Document::new(w, h)?;
    if stack.is_empty() {
        return Ok(doc);
    }
    let count = stack.len();
    // Insert bottom-up above the placeholder layer, then drop it.
    for (n, layer) in stack.into_iter().rev().enumerate() {
        doc.insert_layer(n + 1, layer)?;
    }
    doc.remove_layer(0)?;
    // `selected` indexes the top-first list; storage is bottom-up.
    doc.set_active_layer(selected.map_or(count - 1, |i| count - 1 - i))?;
    Ok(doc)
}

fn read_entry<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, OraError> {
    let mut file = zip
        .by_name(name)
        .map_err(|_| OraError::MissingEntry(name.to_string()))?;
    let mut out = Vec::new();
    file.read_to_end(&mut out)?;
    Ok(out)
}

/// Raw attribute text of every `<name …>` start tag in `xml`, in document
/// order. A deliberately small reader for `stack.xml`: it does not build a
/// tree, which is all OpenRaster's flat attribute-carrying elements need.
fn tags(xml: &str, name: &str) -> Vec<String> {
    let open = format!("<{name}");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        // Require a delimiter so `<layer` does not match `<layers`.
        if !after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            rest = after;
            continue;
        }
        let end = find_tag_end(after);
        out.push(after[..end].to_string());
        rest = &after[end..];
    }
    out
}

/// Index of the `>` closing a start tag, skipping quoted attribute values.
fn find_tag_end(s: &str) -> usize {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return i,
            _ => {}
        }
    }
    s.len()
}

/// Value of attribute `name` in a start tag's attribute text, unescaped.
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let pos = rest.find(name)?;
        let before_ok = pos == 0
            || rest[..pos]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let after = rest[pos + name.len()..].trim_start();
        if before_ok && let Some(after_eq) = after.strip_prefix('=') {
            let after_eq = after_eq.trim_start();
            let quote = after_eq.chars().next()?;
            if quote == '"' || quote == '\'' {
                let body = &after_eq[1..];
                let end = body.find(quote)?;
                return Some(unescape(&body[..end]));
            }
        }
        rest = &rest[pos + name.len()..];
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn sample_doc() -> Document {
        let mut doc = Document::new(4, 3).expect("doc");
        doc.active_layer_mut()
            .pixels
            .set_pixel(0, 0, Color::rgba(255, 0, 0, 255));
        doc.add_layer("Ink & \"notes\"").expect("layer");
        {
            let top = doc.active_layer_mut();
            top.pixels.set_pixel(3, 2, Color::rgba(0, 0, 255, 128));
            top.opacity = 0.5;
            top.blend_mode = BlendMode::Multiply;
            top.locked = true;
        }
        doc.add_layer("Hidden").expect("layer");
        doc.active_layer_mut().visible = false;
        doc.set_active_layer(1).expect("active");
        doc
    }

    #[test]
    fn to_ora_bytes_then_from_ora_bytes_round_trips_layers() {
        let doc = sample_doc();
        let bytes = to_ora_bytes(&doc, 6).expect("write");
        let back = from_ora_bytes(&bytes).expect("read");
        assert_eq!(back.canvas, doc.canvas);
        assert_eq!(back.layer_count(), 3);
        assert_eq!(back.active_layer_index(), 1);
        for (a, b) in doc.layers().iter().zip(back.layers()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.pixels, b.pixels);
            assert_eq!(a.blend_mode, b.blend_mode);
            assert_eq!(a.visible, b.visible);
            assert_eq!(a.locked, b.locked);
            assert!((a.opacity - b.opacity).abs() < 0.001);
        }
    }

    #[test]
    fn to_ora_bytes_stores_the_mimetype_first_and_uncompressed() {
        let bytes = to_ora_bytes(&sample_doc(), 6).expect("write");
        // Local file header: signature, method at offset 8 (0 = stored),
        // extra-field length at 28 (must be 0), name at 30, data after.
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0);
        assert_eq!(u16::from_le_bytes([bytes[28], bytes[29]]), 0);
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(&bytes[38..54], ORA_MIMETYPE.as_bytes());
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_slice())).expect("zip");
        for name in [
            "stack.xml",
            "mergedimage.png",
            "Thumbnails/thumbnail.png",
            "data/layer0.png",
        ] {
            assert!(zip.by_name(name).is_ok(), "{name} present");
        }
    }

    #[test]
    fn from_ora_bytes_places_offset_layers_and_flattens_nested_stacks() {
        let layer = ImageBuffer::from_raw(1, 1, vec![9, 9, 9, 255]).expect("buf");
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let o = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", o).expect("start");
        zip.write_all(ORA_MIMETYPE.as_bytes()).expect("write");
        zip.start_file("stack.xml", o).expect("start");
        zip.write_all(
            b"<?xml version='1.0'?><image w='3' h='2'><stack>\
              <stack name='group'><layer name='dot' src='a.png' x='2' y='1' \
              composite-op='krita:unknown'/></stack>\
              <layer src='a.png'/></stack></image>",
        )
        .expect("write");
        zip.start_file("a.png", o).expect("start");
        zip.write_all(&to_png_bytes(&layer, 6).expect("png"))
            .expect("write");
        let bytes = zip.finish().expect("finish").into_inner();

        let doc = from_ora_bytes(&bytes).expect("read");
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (3, 2));
        assert_eq!(doc.layer_count(), 2);
        // Top-first in the XML → `dot` is the top (last) layer.
        let dot = &doc.layers()[1];
        assert_eq!(dot.name, "dot");
        assert_eq!(dot.blend_mode, BlendMode::Normal);
        assert_eq!(dot.pixels.get_pixel(2, 1), Some(Color::rgba(9, 9, 9, 255)));
        assert_eq!(dot.pixels.get_pixel(0, 0), Some(Color::rgba(0, 0, 0, 0)));
        assert_eq!(doc.layers()[0].name, "Layer 2");
    }

    #[test]
    fn from_ora_bytes_rejects_other_mimetypes() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("mimetype", SimpleFileOptions::default())
            .expect("start");
        zip.write_all(b"application/zip").expect("write");
        let bytes = zip.finish().expect("finish").into_inner();
        assert!(matches!(
            from_ora_bytes(&bytes),
            Err(OraError::NotOpenRaster(_))
        ));
    }

    #[test]
    fn attr_reads_quoted_values_and_ignores_suffix_matches() {
        let tag = r#" name="a &amp; b" xname='no' x="3""#;
        assert_eq!(attr(tag, "name").as_deref(), Some("a & b"));
        assert_eq!(attr(tag, "x").as_deref(), Some("3"));
    }
}
