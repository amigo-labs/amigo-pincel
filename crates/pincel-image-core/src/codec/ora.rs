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
//! the `svg:*` `composite-op` names. Groups are nested `<stack>`s with the
//! same attributes plus `isolation="isolate"`; on read every nested stack
//! becomes a group (docs/specs/image-layer-groups-ora.md). Layer PNGs are
//! placed at their `x` / `y` offset on a canvas-sized buffer, and unknown
//! composite ops fall back to Normal.

use std::io::{Cursor, Read, Write};

use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{CodecError, decode, to_png_bytes};
use crate::color::BlendMode;
use crate::document::tree::subtree_end;
use crate::document::{Document, ImageBuffer, Layer, MAX_LAYERS};
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
    write_run(
        &mut zip,
        &mut xml,
        layers,
        0..layers.len(),
        doc.active_layer_index(),
        compression,
        1,
    )?;
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

/// Writes the whole subtrees in `run` of `layers` top-most first: a raster
/// layer as `<layer>` with its PNG, a group as a nested, isolated `<stack>`
/// around its children (docs/specs/image-layer-groups-ora.md AC-03).
fn write_run<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    xml: &mut String,
    layers: &[Layer],
    run: std::ops::Range<usize>,
    active: usize,
    compression: u8,
    depth: usize,
) -> Result<(), OraError> {
    let mut roots = Vec::new();
    let mut i = run.start;
    while i < run.end {
        let end = subtree_end(layers, i);
        roots.push((i, end));
        i = end;
    }
    let indent = "  ".repeat(depth);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    // stack.xml lists layers top-most first; storage is bottom-up.
    for (index, end) in roots.into_iter().rev() {
        let layer = &layers[index];
        let attrs = format!(
            "name=\"{}\"{} opacity=\"{:.3}\" visibility=\"{}\" composite-op=\"{}\"{}{}",
            escape(&layer.name),
            if layer.is_group() {
                String::new()
            } else {
                format!(" src=\"data/layer{index}.png\" x=\"0\" y=\"0\"")
            },
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
        );
        if layer.is_group() {
            xml.push_str(&format!("{indent}<stack {attrs} isolation=\"isolate\">\n"));
            write_run(
                zip,
                xml,
                layers,
                index + 1..end,
                active,
                compression,
                depth + 1,
            )?;
            xml.push_str(&format!("{indent}</stack>\n"));
        } else {
            zip.start_file(format!("data/layer{index}.png"), stored)?;
            zip.write_all(&to_png_bytes(&layer.pixels, compression)?)?;
            xml.push_str(&format!("{indent}<layer {attrs} />\n"));
        }
    }
    Ok(())
}

/// One element of `stack.xml` that the reader cares about, in document order.
enum StackEvent {
    /// A `<stack …>` start tag (`true` when self-closing) with its attributes.
    Open(String, bool),
    /// A `</stack>` end tag.
    Close,
    /// A `<layer …>` tag with its attributes.
    Layer(String),
}

/// The `<stack>`, `</stack>` and `<layer>` tags of `xml` in document order.
fn stack_events(xml: &str) -> Vec<StackEvent> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let delimited = |name: &str| {
            after.strip_prefix(name).is_some_and(|tail| {
                tail.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
            })
        };
        let end = find_tag_end(after);
        let tag = &after[..end];
        if delimited("/stack") {
            out.push(StackEvent::Close);
        } else if delimited("stack") {
            let attrs = &tag["stack".len()..];
            let closed = attrs.trim_end().ends_with('/');
            out.push(StackEvent::Open(attrs.to_string(), closed));
        } else if delimited("layer") {
            out.push(StackEvent::Layer(tag["layer".len()..].to_string()));
        }
        rest = &after[end.min(after.len())..];
    }
    out
}

/// Opacity, visibility, blend mode and lock of a `<layer>` or `<stack>` tag.
fn read_properties(layer: &mut Layer, tag: &str) {
    layer.opacity = attr(tag, "opacity")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    layer.visible = attr(tag, "visibility").as_deref() != Some("hidden");
    layer.blend_mode = blend_from_op(attr(tag, "composite-op").as_deref().unwrap_or(""));
    layer.locked = attr(tag, "edit-locked").as_deref() == Some("true");
}

/// Parse an OpenRaster archive into a document.
///
/// Nested `<stack>`s become groups with their name, opacity, visibility,
/// blend mode and lock; every group composites isolated, whatever its
/// `isolation` (docs/specs/image-layer-groups-ora.md). Unbalanced stacks
/// are recovered: a stray `</stack>` is ignored and open stacks close at
/// the end of `stack.xml`.
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

    // Document order is top-most first, each group before its children.
    // `items[i].1` is the item index of the group holding item `i`.
    let mut items: Vec<(Layer, Option<usize>)> = Vec::new();
    // The open `<stack>`s: `None` for the root stack, else its group item.
    let mut open: Vec<Option<usize>> = Vec::new();
    let mut selected = None;
    let mut layers_read = 0;
    for event in stack_events(&xml) {
        let parent = open.last().copied().flatten();
        match event {
            StackEvent::Close => {
                open.pop();
            }
            StackEvent::Open(_, closed) if open.is_empty() => {
                if !closed {
                    open.push(None);
                }
            }
            StackEvent::Open(tag, closed) => {
                let n = items.iter().filter(|(l, _)| l.is_group()).count() + 1;
                let mut group =
                    Layer::new_group(attr(&tag, "name").unwrap_or_else(|| format!("Group {n}")));
                read_properties(&mut group, &tag);
                if attr(&tag, "selected").as_deref() == Some("true") {
                    selected = Some(items.len());
                }
                items.push((group, parent));
                if !closed {
                    open.push(Some(items.len() - 1));
                }
            }
            StackEvent::Layer(tag) => {
                layers_read += 1;
                let src = attr(&tag, "src")
                    .ok_or_else(|| OraError::Stack("<layer> without `src`".into()))?;
                let png = decode(&read_entry(&mut zip, &src)?)?;
                let x = attr(&tag, "x").and_then(|v| v.parse().ok()).unwrap_or(0);
                let y = attr(&tag, "y").and_then(|v| v.parse().ok()).unwrap_or(0);
                let pixels: ImageBuffer = if (x, y, png.width(), png.height()) == (0, 0, w, h) {
                    png
                } else {
                    png.offset_copy(w, h, x, y)
                };
                let mut layer = Layer::from_pixels(
                    attr(&tag, "name").unwrap_or_else(|| format!("Layer {layers_read}")),
                    pixels,
                );
                read_properties(&mut layer, &tag);
                if attr(&tag, "selected").as_deref() == Some("true") {
                    selected = Some(items.len());
                }
                items.push((layer, parent));
            }
        }
        if items.len() > MAX_LAYERS {
            return Err(DocumentError::LayerLimit { max: MAX_LAYERS }.into());
        }
    }

    let mut doc = Document::new(w, h)?;
    if items.iter().all(|(l, _)| l.is_group()) {
        return Ok(doc); // nothing to paint on: a blank document
    }
    // Storage is bottom-first with each group before its children: walk the
    // tree, listing every level's children in reverse document order.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); items.len() + 1];
    for (i, (_, parent)) in items.iter().enumerate() {
        children[parent.map_or(0, |p| p + 1)].push(i);
    }
    let mut order = Vec::with_capacity(items.len());
    let mut todo: Vec<usize> = children[0].clone();
    while let Some(i) = todo.pop() {
        order.push(i);
        todo.extend(children[i + 1].iter().copied());
    }
    let ids: Vec<_> = items.iter().map(|(l, _)| l.id).collect();
    let mut slots: Vec<Option<(Layer, Option<usize>)>> = items.into_iter().map(Some).collect();
    let mut layers = Vec::with_capacity(order.len());
    let mut active = None;
    for i in order {
        let (mut layer, parent) = slots[i].take().expect("each item is listed once");
        layer.parent = parent.map(|p| ids[p]);
        if Some(i) == selected {
            active = Some(layers.len());
        }
        layers.push(layer);
    }
    let top = layers.len() - 1;
    doc.layers = layers;
    doc.set_active_layer(active.unwrap_or(top))?;
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

    /// Decodes the archive entry `name` of an `.ora`.
    fn entry_image(bytes: &[u8], name: &str) -> ImageBuffer {
        let mut zip = ZipArchive::new(Cursor::new(bytes)).expect("zip");
        decode(&read_entry(&mut zip, name).expect("entry")).expect("png")
    }

    #[test]
    fn to_ora_bytes_merged_image_is_the_visible_composite() {
        let doc = sample_doc();
        let bytes = to_ora_bytes(&doc, 6).expect("write");
        let merged = entry_image(&bytes, "mergedimage.png");
        assert_eq!(merged, compose(doc.layers()));
        // The hidden top layer is left out, the multiplied one blends in.
        let mut shown = sample_doc();
        shown.set_active_layer(2).expect("active");
        shown.active_layer_mut().visible = true;
        shown
            .active_layer_mut()
            .pixels
            .set_pixel(0, 0, Color::rgba(0, 255, 0, 255));
        assert_ne!(merged, compose(shown.layers()));
        assert_ne!(merged, doc.layers()[0].pixels);
    }

    #[test]
    fn to_ora_bytes_thumbnail_fits_256_px_with_the_canvas_aspect() {
        for ((w, h), thumb) in [
            ((600, 300), (256, 128)),
            ((100, 1000), (25, 256)),
            ((40, 30), (40, 30)),
        ] {
            let doc = Document::new(w, h).expect("doc");
            let bytes = to_ora_bytes(&doc, 1).expect("write");
            let t = entry_image(&bytes, "Thumbnails/thumbnail.png");
            assert_eq!((t.width(), t.height()), thumb, "{w}×{h}");
        }
    }

    /// An `.ora` with `xml` as its `stack.xml` and a 1×1 grey `a.png`.
    fn ora_with_stack(xml: &str) -> Vec<u8> {
        let layer = ImageBuffer::from_raw(1, 1, vec![9, 9, 9, 255]).expect("buf");
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let o = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", o).expect("start");
        zip.write_all(ORA_MIMETYPE.as_bytes()).expect("write");
        zip.start_file("stack.xml", o).expect("start");
        zip.write_all(xml.as_bytes()).expect("write");
        zip.start_file("a.png", o).expect("start");
        zip.write_all(&to_png_bytes(&layer, 6).expect("png"))
            .expect("write");
        zip.finish().expect("finish").into_inner()
    }

    /// `(name, parent name, is_group)` bottom-first.
    fn tree(doc: &Document) -> Vec<(String, Option<String>, bool)> {
        (0..doc.layer_count())
            .map(|i| {
                let l = &doc.layers()[i];
                let parent = doc.parent_index(i).map(|p| doc.layers()[p].name.clone());
                (l.name.clone(), parent, l.is_group())
            })
            .collect()
    }

    #[test]
    fn from_ora_bytes_keeps_nested_stack_attributes() {
        let bytes = ora_with_stack(
            "<?xml version='1.0'?><image w='3' h='2'><stack>\
             <stack name='group' opacity='0.5' visibility='hidden' \
             composite-op='svg:multiply' edit-locked='true'>\
             <layer name='dot' src='a.png' x='2' y='1' composite-op='krita:unknown'/>\
             <stack name='empty'/></stack>\
             <layer src='a.png'/></stack></image>",
        );
        let doc = from_ora_bytes(&bytes).expect("read");
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (3, 2));
        assert!(doc.is_well_formed());
        let s = |v: &str| v.to_string();
        assert_eq!(
            tree(&doc),
            [
                (s("Layer 2"), None, false),
                (s("group"), None, true),
                (s("empty"), Some(s("group")), true),
                (s("dot"), Some(s("group")), false),
            ]
        );
        let group = &doc.layers()[1];
        assert!((group.opacity - 0.5).abs() < 0.001);
        assert!(!group.visible && group.locked);
        assert_eq!(group.blend_mode, BlendMode::Multiply);
        let dot = &doc.layers()[3];
        assert_eq!(dot.blend_mode, BlendMode::Normal);
        assert_eq!(dot.pixels.get_pixel(2, 1), Some(Color::rgba(9, 9, 9, 255)));
        assert_eq!(dot.pixels.get_pixel(0, 0), Some(Color::rgba(0, 0, 0, 0)));
        // AC-02: the hidden group hides `dot`, which is visible itself.
        assert!(dot.visible);
        assert_eq!(
            compose(doc.layers()).get_pixel(2, 1),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn from_ora_bytes_composites_a_half_opaque_stack_as_a_whole() {
        let bytes = ora_with_stack(
            "<image w='1' h='1'><stack><stack opacity='0.5'>\
             <layer src='a.png'/><layer src='a.png'/></stack></stack></image>",
        );
        let doc = from_ora_bytes(&bytes).expect("read");
        assert_eq!(
            compose(doc.layers()).get_pixel(0, 0).map(|c| c.a),
            Some(128)
        );
    }

    #[test]
    fn from_ora_bytes_recovers_unbalanced_stacks() {
        // A stray close tag is ignored; the open `<stack>`s close at the end.
        let bytes = ora_with_stack(
            "<image w='1' h='1'><stack><layer name='a' src='a.png'/></stack></stack>\
             <stack><stack name='g'><stack name='h'><layer name='b' src='a.png'/>",
        );
        let doc = from_ora_bytes(&bytes).expect("read");
        let s = |v: &str| v.to_string();
        assert_eq!(
            tree(&doc),
            [
                (s("g"), None, true),
                (s("h"), Some(s("g")), true),
                (s("b"), Some(s("h")), false),
                (s("a"), None, false),
            ]
        );
    }

    #[test]
    fn from_ora_bytes_over_the_layer_limit_fails() {
        let mut xml = String::from("<image w='1' h='1'><stack>");
        for _ in 0..500 {
            xml.push_str("<stack><layer src='a.png'/></stack>");
        }
        xml.push_str("</stack></image>");
        let err = from_ora_bytes(&ora_with_stack(&xml)).unwrap_err();
        assert!(matches!(
            err,
            OraError::Document(DocumentError::LayerLimit { .. })
        ));
    }

    #[test]
    fn from_ora_bytes_reads_a_flat_stack_at_the_root() {
        let doc = from_ora_bytes(&to_ora_bytes(&sample_doc(), 1).expect("write")).expect("read");
        assert!(
            doc.layers()
                .iter()
                .all(|l| l.parent.is_none() && !l.is_group())
        );
    }

    /// bg, G (60 %, Screen, locked) { a, H (hidden) { b } }, top.
    fn grouped_doc() -> Document {
        let mut doc = Document::new(3, 2).expect("doc");
        let g = Layer::new_group("G")
            .with_opacity(0.6)
            .with_blend_mode(BlendMode::Screen);
        let gid = g.id;
        let mut h = Layer::new_group("H").with_parent(Some(gid));
        h.visible = false;
        let hid = h.id;
        let mut a = Layer::new_transparent("a", 3, 2).with_parent(Some(gid));
        a.pixels.set_pixel(1, 0, Color::rgba(200, 10, 10, 255));
        let mut b = Layer::new_transparent("b", 3, 2).with_parent(Some(hid));
        b.pixels.set_pixel(2, 1, Color::rgba(10, 200, 10, 200));
        b.blend_mode = BlendMode::Difference;
        let mut top = Layer::new_transparent("top", 3, 2);
        top.pixels.set_pixel(0, 1, Color::rgba(10, 10, 200, 100));
        for (i, l) in [g, a, h, b, top].into_iter().enumerate() {
            doc.insert_layer(i + 1, l).expect("insert");
        }
        doc.layers[1].locked = true;
        doc.active_layer_mut().pixels.set_pixel(0, 0, Color::WHITE);
        doc.set_active_layer(3).expect("active");
        doc
    }

    #[test]
    fn to_ora_bytes_writes_groups_as_nested_isolated_stacks() {
        let bytes = to_ora_bytes(&grouped_doc(), 1).expect("write");
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_slice())).expect("zip");
        let xml = String::from_utf8(read_entry(&mut zip, "stack.xml").expect("xml")).expect("utf8");
        let at = |needle: &str| {
            xml.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {xml}"))
        };
        let g = at("<stack name=\"G\"");
        let g_tag = &xml[g..g + xml[g..].find('>').expect("tag end")];
        for attr in [
            "opacity=\"0.600\"",
            "visibility=\"visible\"",
            "composite-op=\"svg:screen\"",
            "edit-locked=\"true\"",
            "isolation=\"isolate\"",
        ] {
            assert!(g_tag.contains(attr), "{attr} in {g_tag}");
        }
        assert!(
            xml[at("<stack name=\"H\"")..]
                .starts_with("<stack name=\"H\" opacity=\"1.000\" visibility=\"hidden\"")
        );
        // Top-most first, each child inside its group's element.
        let order = [
            "name=\"top\"",
            "name=\"G\"",
            "name=\"H\"",
            "name=\"b\"",
            "</stack>",
            "name=\"a\"",
            "</stack>",
            "name=\"bg\"",
        ];
        let mut from = 0;
        for needle in order {
            let needle = if needle == "name=\"bg\"" {
                "name=\"Layer 1\""
            } else {
                needle
            };
            let pos = xml[from..]
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} after {from} in {xml}"));
            from += pos + needle.len();
        }
    }

    #[test]
    fn to_ora_bytes_then_from_ora_bytes_keeps_the_layer_tree() {
        let doc = grouped_doc();
        let back = from_ora_bytes(&to_ora_bytes(&doc, 1).expect("write")).expect("read");
        assert_eq!(tree(&back), tree(&doc));
        assert_eq!(back.active_layer_index(), doc.active_layer_index());
        for (a, b) in doc.layers().iter().zip(back.layers()) {
            assert_eq!(a.kind, b.kind, "{}", a.name);
            assert_eq!(a.pixels, b.pixels, "{}", a.name);
            assert_eq!(a.blend_mode, b.blend_mode, "{}", a.name);
            assert_eq!((a.visible, a.locked), (b.visible, b.locked), "{}", a.name);
            assert!((a.opacity - b.opacity).abs() < 0.001, "{}", a.name);
        }
    }

    #[test]
    fn to_ora_bytes_merged_image_applies_groups_in_isolation() {
        let doc = grouped_doc();
        let merged = entry_image(&to_ora_bytes(&doc, 1).expect("write"), "mergedimage.png");
        assert_eq!(merged, compose(doc.layers()));
        // Not the flat fold that ignores the group's opacity and blend mode.
        let flat: Vec<Layer> = doc
            .layers()
            .iter()
            .filter(|l| !l.is_group())
            .map(|l| Layer {
                parent: None,
                ..l.clone()
            })
            .collect();
        assert_ne!(merged, compose(&flat));
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
