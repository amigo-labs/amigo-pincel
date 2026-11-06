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

/// Snapshot bytes of `doc` (see the module docs).
pub fn to_snapshot_bytes(doc: &Document) -> Vec<u8> {
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
    let pixels: usize = doc.layers.iter().map(|l| l.pixels.data().len()).sum();
    let mask = doc.selection.as_ref().map_or(0, |m| m.data().len());
    let mut out = Vec::with_capacity(12 + json.len() + pixels + mask);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    for layer in &doc.layers {
        out.extend_from_slice(layer.pixels.data());
    }
    if let Some(m) = &doc.selection {
        out.extend_from_slice(m.data());
    }
    out
}

/// The document `bytes` (from [`to_snapshot_bytes`]) describe.
///
/// Errors with `InvalidSnapshot` on bytes that are not a whole snapshot.
pub fn from_snapshot_bytes(bytes: &[u8]) -> Result<Document, DocumentError> {
    let bad = || DocumentError::InvalidSnapshot;
    let rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(bad)?;
    let (len, rest) = rest.split_first_chunk::<4>().ok_or_else(bad)?;
    let len = u32::from_le_bytes(*len) as usize;
    if rest.len() < len {
        return Err(bad());
    }
    let (json, mut data) = rest.split_at(len);
    let header: Header = serde_json::from_slice(json).map_err(|_| bad())?;
    let mut doc = header.doc;
    if header.sizes.len() != doc.layers.len() {
        return Err(bad());
    }
    let mut take = |n: usize| -> Result<Vec<u8>, DocumentError> {
        if data.len() < n {
            return Err(bad());
        }
        let (head, tail) = data.split_at(n);
        data = tail;
        Ok(head.to_vec())
    };
    for (layer, &(w, h)) in doc.layers.iter_mut().zip(&header.sizes) {
        let raw = take(w as usize * h as usize * 4)?;
        layer.pixels = ImageBuffer::from_raw(w, h, raw)?;
    }
    if let Some((w, h)) = header.selection {
        let raw = take(w as usize * h as usize)?;
        doc.selection = Some(SelectionMask::from_raw(w, h, raw).ok_or_else(bad)?);
    }
    if !data.is_empty() || doc.active_layer >= doc.layers.len() {
        return Err(bad());
    }
    Ok(doc)
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
    fn snapshot_rejects_truncated_or_foreign_bytes() {
        let bytes = to_snapshot_bytes(&sample());
        assert!(from_snapshot_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(from_snapshot_bytes(b"not a snapshot").is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(from_snapshot_bytes(&longer).is_err());
    }
}
