//! A fast, uncompressed copy of a whole [`Document`] for handing it to
//! another wasm instance — the job worker that encodes autosaves and
//! exports (docs/specs/performance-background-jobs.md).
//!
//! Layout: the magic `PNCLSNAP`, a little-endian `u32` header length, the
//! JSON header (the document with every pixel buffer and the selection
//! emptied, plus their sizes), then each layer's RGBA8 bytes in order and
//! the selection's coverage bytes. No compression: copying is the point.

use serde::{Deserialize, Serialize};

use crate::document::{Document, ImageBuffer, Layer};
use crate::error::DocumentError;
use crate::selection::SelectionMask;

const MAGIC: &[u8; 8] = b"PNCLSNAP";

#[derive(Serialize, Deserialize)]
struct Header {
    /// The document with empty pixel buffers and no selection.
    doc: Document,
    /// Width and height of each layer's pixels, in layer order.
    sizes: Vec<(u32, u32)>,
    /// Width and height of the selection mask, if there is one.
    selection: Option<(u32, u32)>,
}

/// The first part of [`to_snapshot_bytes`]: magic, header length and the
/// JSON header. The pixel parts follow it — [`layer_bytes`] of each layer
/// in order, then the selection's coverage bytes if there is a selection —
/// so a caller can hand them over one at a time
/// (docs/specs/performance-background-jobs.md AC-04).
pub fn snapshot_header(doc: &Document) -> Vec<u8> {
    let layers = doc
        .layers
        .iter()
        .map(|l| Layer {
            id: l.id,
            name: l.name.clone(),
            kind: l.kind,
            parent: l.parent,
            pixels: ImageBuffer::new_transparent(0, 0),
            opacity: l.opacity,
            blend_mode: l.blend_mode,
            visible: l.visible,
            locked: l.locked,
            generation: l.generation.clone(),
        })
        .collect();
    let header = Header {
        doc: Document {
            id: doc.id,
            title: doc.title.clone(),
            canvas: doc.canvas,
            layers,
            active_layer: doc.active_layer_index(),
            selection: None,
            metadata: doc.metadata,
        },
        sizes: doc
            .layers
            .iter()
            .map(|l| (l.pixels.width(), l.pixels.height()))
            .collect(),
        selection: doc.selection.as_ref().map(|m| (m.width(), m.height())),
    };
    // Serializing plain data into a Vec cannot fail.
    let json = serde_json::to_vec(&header).expect("snapshot header serializes");
    let mut out = Vec::with_capacity(12 + json.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    out
}

/// The pixel part of layer `index` in a snapshot of `doc`; `None` past the
/// last layer.
pub fn layer_bytes(doc: &Document, index: usize) -> Option<&[u8]> {
    doc.layers.get(index).map(|l| l.pixels.data())
}

/// Snapshot bytes of `doc` (see the module docs).
pub fn to_snapshot_bytes(doc: &Document) -> Vec<u8> {
    let header = snapshot_header(doc);
    let pixels: usize = doc.layers.iter().map(|l| l.pixels.data().len()).sum();
    let mask = doc.selection.as_ref().map_or(0, |m| m.data().len());
    let mut out = Vec::with_capacity(header.len() + pixels + mask);
    out.extend_from_slice(&header);
    for layer in &doc.layers {
        out.extend_from_slice(layer.pixels.data());
    }
    if let Some(m) = &doc.selection {
        out.extend_from_slice(m.data());
    }
    out
}

/// Parses the header part; returns the document without pixels, the layer
/// sizes, the selection size and the bytes after the header.
fn parse_header(bytes: &[u8]) -> Result<(Header, &[u8]), DocumentError> {
    let bad = || DocumentError::InvalidSnapshot;
    let rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(bad)?;
    let (len, rest) = rest.split_first_chunk::<4>().ok_or_else(bad)?;
    let len = u32::from_le_bytes(*len) as usize;
    if rest.len() < len {
        return Err(bad());
    }
    let (json, data) = rest.split_at(len);
    let header: Header = serde_json::from_slice(json).map_err(|_| bad())?;
    if header.sizes.len() != header.doc.layers.len() {
        return Err(bad());
    }
    Ok((header, data))
}

/// Fills the header's document with `layers` (one pixel part per layer)
/// and `selection`.
fn assemble(
    header: Header,
    layers: impl IntoIterator<Item = Vec<u8>>,
    selection: Option<Vec<u8>>,
) -> Result<Document, DocumentError> {
    let bad = || DocumentError::InvalidSnapshot;
    let mut doc = header.doc;
    let mut layers = layers.into_iter();
    for (layer, &(w, h)) in doc.layers.iter_mut().zip(&header.sizes) {
        let raw = layers.next().ok_or_else(bad)?;
        if raw.len() != w as usize * h as usize * 4 {
            return Err(bad());
        }
        layer.pixels = ImageBuffer::from_raw(w, h, raw)?;
    }
    if layers.next().is_some() {
        return Err(bad());
    }
    match (header.selection, selection) {
        (Some((w, h)), Some(raw)) if raw.len() == w as usize * h as usize => {
            doc.selection = Some(SelectionMask::from_raw(w, h, raw).ok_or_else(bad)?);
        }
        (None, None) => {}
        _ => return Err(bad()),
    }
    if doc.active_layer >= doc.layers.len() {
        return Err(bad());
    }
    Ok(doc)
}

/// The document `bytes` (from [`to_snapshot_bytes`]) describe.
///
/// Errors with `InvalidSnapshot` on bytes that are not a whole snapshot.
pub fn from_snapshot_bytes(bytes: &[u8]) -> Result<Document, DocumentError> {
    let bad = || DocumentError::InvalidSnapshot;
    let (header, mut data) = parse_header(bytes)?;
    let mut take = |n: usize| -> Result<Vec<u8>, DocumentError> {
        if data.len() < n {
            return Err(bad());
        }
        let (head, tail) = data.split_at(n);
        data = tail;
        Ok(head.to_vec())
    };
    let layers = header
        .sizes
        .iter()
        .map(|&(w, h)| take(w as usize * h as usize * 4))
        .collect::<Result<Vec<_>, _>>()?;
    let selection = header
        .selection
        .map(|(w, h)| take(w as usize * h as usize))
        .transpose()?;
    if !data.is_empty() {
        return Err(bad());
    }
    assemble(header, layers, selection)
}

/// The document a snapshot handed over in parts describes: `header` from
/// [`snapshot_header`], one pixel part per layer and the selection part.
///
/// Errors with `InvalidSnapshot` when a part is missing, extra or of the
/// wrong size.
pub fn from_snapshot_parts(
    header: &[u8],
    layers: impl IntoIterator<Item = Vec<u8>>,
    selection: Option<Vec<u8>>,
) -> Result<Document, DocumentError> {
    let (parsed, rest) = parse_header(header)?;
    if !rest.is_empty() {
        return Err(DocumentError::InvalidSnapshot);
    }
    assemble(parsed, layers, selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::to_ora_bytes;
    use crate::color::Color;
    use crate::geometry::Rect;

    fn sample() -> Document {
        let mut doc = Document::new(70, 40).expect("doc");
        doc.add_layer("Ink").expect("layer");
        doc.layers[1]
            .pixels
            .set_pixel(5, 6, Color::rgba(1, 2, 3, 4));
        doc.layers[1].opacity = 0.5;
        doc.layers[1].generation = Some(crate::document::generation::sample());
        doc.layers[0].pixels.set_pixel(69, 39, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(70, 40, Rect::new(3, 4, 20, 10)));
        doc.title = "photo.png".into();
        doc
    }

    #[test]
    fn snapshot_round_trip_restores_the_document() {
        let doc = sample();
        let back = from_snapshot_bytes(&to_snapshot_bytes(&doc)).expect("snapshot");
        assert_eq!(back, doc);
    }

    #[test]
    fn snapshot_round_trip_encodes_byte_identical_ora() {
        let doc = sample();
        let back = from_snapshot_bytes(&to_snapshot_bytes(&doc)).expect("snapshot");
        assert_eq!(
            to_ora_bytes(&back, 1).expect("ora"),
            to_ora_bytes(&doc, 1).expect("ora")
        );
    }

    #[test]
    fn snapshot_parts_concatenate_to_the_snapshot_bytes() {
        let doc = sample();
        let mut joined = snapshot_header(&doc);
        for i in 0..doc.layers.len() {
            joined.extend_from_slice(layer_bytes(&doc, i).expect("layer"));
        }
        joined.extend_from_slice(doc.selection.as_ref().expect("selection").data());
        assert_eq!(joined, to_snapshot_bytes(&doc));
        assert!(layer_bytes(&doc, doc.layers.len()).is_none());
    }

    #[test]
    fn snapshot_parts_restore_the_document() {
        let doc = sample();
        let layers = (0..doc.layers.len()).map(|i| layer_bytes(&doc, i).expect("layer").to_vec());
        let selection = doc.selection.as_ref().map(|m| m.data().to_vec());
        let back = from_snapshot_parts(&snapshot_header(&doc), layers, selection).expect("parts");
        assert_eq!(back, doc);
    }

    #[test]
    fn snapshot_parts_reject_missing_extra_or_resized_parts() {
        let doc = sample();
        let header = snapshot_header(&doc);
        let parts = || -> Vec<Vec<u8>> {
            (0..doc.layers.len())
                .map(|i| layer_bytes(&doc, i).expect("layer").to_vec())
                .collect()
        };
        let sel = || doc.selection.as_ref().map(|m| m.data().to_vec());
        let mut fewer = parts();
        fewer.pop();
        assert!(from_snapshot_parts(&header, fewer, sel()).is_err());
        let mut more = parts();
        more.push(vec![0; 4]);
        assert!(from_snapshot_parts(&header, more, sel()).is_err());
        let mut short = parts();
        short[0].pop();
        assert!(from_snapshot_parts(&header, short, sel()).is_err());
        assert!(from_snapshot_parts(&header, parts(), None).is_err());
        let mut padded = header.clone();
        padded.push(0);
        assert!(from_snapshot_parts(&padded, parts(), sel()).is_err());
    }

    #[test]
    fn snapshot_rejects_truncated_or_foreign_bytes() {
        let bytes = to_snapshot_bytes(&sample());
        assert!(from_snapshot_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(from_snapshot_bytes(b"not a snapshot").is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(from_snapshot_bytes(&longer).is_err());
    }
}
