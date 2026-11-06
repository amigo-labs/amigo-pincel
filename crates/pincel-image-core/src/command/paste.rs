//! The `PasteLayer` command: a clipboard image as a new layer (spec §5.4).

use super::HistoryKind;
use super::snapshot::{DocSnapshot, restore, snapshot};
use super::{Command, ResizeCanvas};
use crate::document::{Document, ImageBuffer, Layer, MAX_LAYERS};
use crate::error::DocumentError;
use std::any::Any;

/// Inserts an image as a new layer above the active one, with its top-left
/// corner at a canvas position.
///
/// With `expand`, a canvas that doesn't cover the image there first grows to
/// (anchored top-left); otherwise the image is cropped to the canvas. Both
/// steps are one undo step: a paste that grows the canvas snapshots the
/// prior document on apply; one that doesn't only remembers the active
/// layer, since undo takes the new layer back out
/// (docs/specs/performance-image-undo-memory.md).
pub struct PasteLayer {
    /// The pasted image, until the layer is built on first apply.
    pixels: Option<ImageBuffer>,
    x: i32,
    y: i32,
    expand: bool,
    /// The layer revert took out of the document; redo re-inserts it exactly.
    layer: Option<Layer>,
    /// Where the layer went on first apply; redo reuses it even if the
    /// active layer changed in between, so later commands' indices hold.
    index: Option<usize>,
    prev: Option<PasteUndo>,
}

/// What undoing a paste needs: a whole-document snapshot when the paste
/// grew the canvas, otherwise just the active index.
enum PasteUndo {
    Expanded(DocSnapshot),
    Inserted { active: usize },
}

impl PasteLayer {
    /// Pastes `pixels` at the top-left, growing the canvas to fit it when
    /// `expand` is set.
    pub fn new(pixels: ImageBuffer, expand: bool) -> Self {
        Self::at(pixels, 0, 0, expand)
    }

    /// Pastes `pixels` with their top-left at canvas `(x, y)`; with `expand`
    /// the canvas grows (anchored top-left) to cover them there.
    pub fn at(pixels: ImageBuffer, x: i32, y: i32, expand: bool) -> Self {
        Self {
            pixels: Some(pixels),
            x,
            y,
            expand,
            layer: None,
            index: None,
            prev: None,
        }
    }
}

/// Canvas extent along one axis that covers `len` pixels starting at `pos`.
/// The checks of `insert_layer` that do not depend on the layer.
fn check_insert(doc: &Document, index: usize) -> Result<(), DocumentError> {
    let len = doc.layers.len();
    if len >= MAX_LAYERS {
        return Err(DocumentError::LayerLimit { max: MAX_LAYERS });
    }
    if index > len {
        return Err(DocumentError::LayerIndexOutOfBounds { index, len });
    }
    Ok(())
}

fn cover(canvas: u32, pos: i32, len: u32) -> Result<u32, DocumentError> {
    let end = i64::from(pos) + i64::from(len);
    u32::try_from(end.max(i64::from(canvas))).map_err(|_| DocumentError::RegionOutOfBounds)
}

impl Command for PasteLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let (cw, ch) = (doc.canvas.width(), doc.canvas.height());
        let (w, h) = match (&self.layer, &self.pixels) {
            (Some(layer), _) => (layer.pixels.width(), layer.pixels.height()),
            (None, Some(px)) if self.expand => (
                cover(cw, self.x, px.width())?,
                cover(ch, self.y, px.height())?,
            ),
            (None, Some(_)) => (cw, ch),
            (None, None) => return Err(DocumentError::RegionOutOfBounds),
        };
        let prev = if (w, h) != (cw, ch) {
            let snap = snapshot(doc);
            ResizeCanvas::new(w, h).apply(doc)?;
            PasteUndo::Expanded(snap)
        } else {
            PasteUndo::Inserted {
                active: doc.active_layer_index(),
            }
        };
        // Directly above the active layer — above a group's whole subtree —
        // sharing its parent (docs/specs/image-layer-groups.md AC-19).
        let active = doc.active_layer_index();
        let layer = match (self.layer.take(), self.pixels.take()) {
            (Some(layer), _) => layer,
            (None, Some(px)) => Layer::from_pixels(
                format!("Layer {}", doc.layers.len() + 1),
                px.offset_copy(w, h, self.x, self.y),
            )
            .with_parent(doc.active_layer().parent),
            (None, None) => return Err(DocumentError::RegionOutOfBounds),
        };
        let index = *self.index.get_or_insert(doc.subtree_end(active));
        // The layer moves into the document and revert takes it back out,
        // so it is held only while undone. Checked first, so a refused
        // paste keeps its layer for a retry.
        if let Err(err) = check_insert(doc, index) {
            if let PasteUndo::Expanded(snap) = prev {
                restore(doc, snap)?;
            }
            self.layer = Some(layer);
            return Err(err);
        }
        doc.insert_layer(index, layer)?;
        self.prev = Some(prev);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        let index = self.index.ok_or(DocumentError::RegionOutOfBounds)?;
        if index >= doc.layers.len() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index,
                len: doc.layers.len(),
            });
        }
        let layer = doc.layers.remove(index);
        self.layer = Some(layer);
        match prev {
            PasteUndo::Expanded(snap) => restore(doc, snap),
            PasteUndo::Inserted { active } => doc.set_active_layer(active),
        }
    }

    fn undo_bytes(&self) -> usize {
        let layer = self.layer.as_ref().map_or(0, |l| l.pixels.data().len());
        let pixels = self.pixels.as_ref().map_or(0, |p| p.data().len());
        let snap = match &self.prev {
            Some(PasteUndo::Expanded(snap)) => snap.bytes(),
            _ => 0,
        };
        layer + pixels + snap
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::Paste
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::selection::SelectionMask;

    fn image(w: u32, h: u32) -> ImageBuffer {
        let mut buf = ImageBuffer::new_transparent(w, h);
        buf.set_pixel(0, 0, Color::WHITE);
        buf.set_pixel(w - 1, h - 1, Color::WHITE);
        buf
    }

    #[test]
    fn paste_with_a_group_active_inserts_above_the_group_at_its_level() {
        use crate::document::tree::tests::tree_doc;
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("top", None, false),
        ]);
        doc.set_active_layer(1).unwrap();
        let mut cmd = PasteLayer::new(image(6, 6), true);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.active_layer_index(), 3);
        assert_eq!(doc.parent_index(3), None);
        assert_eq!(doc.layers[4].name, "top");
        assert!(doc.is_well_formed());
        assert_eq!(doc.layers[1].pixels.width(), 0, "the group stays empty");
        assert_eq!(doc.layers[2].pixels.width(), 6, "the canvas grew");
        // With a child active the paste stays in the child's group.
        doc.set_active_layer(2).unwrap();
        PasteLayer::new(image(2, 2), false).apply(&mut doc).unwrap();
        assert_eq!(doc.parent_index(3), Some(1));
        assert!(doc.is_well_formed());
    }

    #[test]
    fn paste_larger_with_expand_grows_canvas_and_revert_restores() {
        let mut doc = Document::new(4, 4).unwrap();
        doc.layers[0].pixels.set_pixel(3, 3, Color::WHITE);
        doc.selection = Some(SelectionMask::new_full(4, 4));
        let mut cmd = PasteLayer::new(image(6, 5), true);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (6, 5));
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[1].pixels.get_pixel(5, 4), Some(Color::WHITE));
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 3), Some(Color::WHITE));
        assert_eq!(doc.layers[0].pixels.width(), 6);

        cmd.revert(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.active_layer_index(), 0);
        assert!(doc.selection.is_some());
    }

    #[test]
    fn paste_larger_without_expand_crops_to_canvas() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = PasteLayer::new(image(6, 5), false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        assert_eq!(doc.layers[1].pixels.width(), 4);
        assert_eq!(doc.layers[1].pixels.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(
            doc.layers[1].pixels.get_pixel(3, 3),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn paste_smaller_pads_transparent() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = PasteLayer::new(image(2, 2), true);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (8, 8));
        assert_eq!(doc.layers[1].pixels.get_pixel(1, 1), Some(Color::WHITE));
        assert_eq!(
            doc.layers[1].pixels.get_pixel(7, 7),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn paste_redo_after_undo_restores_layer() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = PasteLayer::new(image(6, 6), true);
        cmd.apply(&mut doc).unwrap();
        let id = doc.layers[1].id;
        cmd.revert(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (6, 6));
        assert_eq!(doc.layers[1].id, id);
        assert_eq!(doc.layers[1].pixels.get_pixel(5, 5), Some(Color::WHITE));
    }

    #[test]
    fn paste_redo_keeps_the_first_insertion_index() {
        // A/B stack, paste above A, undo, select B, redo: the paste must
        // land between A and B again, not above B.
        let mut doc = Document::new(4, 4).unwrap();
        doc.add_layer("B").unwrap();
        doc.set_active_layer(0).unwrap();
        let mut cmd = PasteLayer::new(image(2, 2), false);
        cmd.apply(&mut doc).unwrap();
        let pasted = doc.layers[1].id;
        cmd.revert(&mut doc).unwrap();
        doc.set_active_layer(1).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[1].id, pasted);
        assert_eq!(doc.layers[2].name, "B");
    }

    #[test]
    fn paste_inserts_above_the_active_layer() {
        let mut doc = Document::new(4, 4).unwrap();
        doc.add_layer("Top").unwrap();
        doc.set_active_layer(0).unwrap();
        let mut cmd = PasteLayer::new(image(2, 2), false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[2].name, "Top");
    }

    #[test]
    fn paste_at_places_the_image_at_its_position() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = PasteLayer::at(image(2, 2), 3, 4, false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[1].pixels.get_pixel(3, 4), Some(Color::WHITE));
        assert_eq!(doc.layers[1].pixels.get_pixel(4, 5), Some(Color::WHITE));
        assert_eq!(
            doc.layers[1].pixels.get_pixel(0, 0),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn paste_at_with_expand_covers_the_image_at_its_position() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = PasteLayer::at(image(6, 5), 1, 2, true);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (7, 7));
        assert_eq!(doc.layers[1].pixels.get_pixel(6, 6), Some(Color::WHITE));
        assert_eq!(doc.layers[1].pixels.get_pixel(1, 2), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn paste_at_without_expand_crops_at_the_position() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = PasteLayer::at(image(6, 5), 1, 2, false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        assert_eq!(doc.layers[1].pixels.get_pixel(1, 2), Some(Color::WHITE));
    }
}
