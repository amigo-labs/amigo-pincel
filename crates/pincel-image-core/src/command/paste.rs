//! The `PasteLayer` command: a clipboard image as a new layer (spec §5.4).

use super::snapshot::{DocSnapshot, restore, snapshot};
use super::{Command, ResizeCanvas};
use crate::document::{Document, ImageBuffer, Layer};
use crate::error::DocumentError;
use std::any::Any;

/// Inserts an image as a new layer above the active one, at the top-left.
///
/// With `expand`, a canvas smaller than the image first grows to fit it
/// (anchored top-left); otherwise the image is cropped to the canvas. Both
/// steps are one undo step: the prior document is snapshotted on apply.
pub struct PasteLayer {
    /// The pasted image, until the layer is built on first apply.
    pixels: Option<ImageBuffer>,
    expand: bool,
    /// The inserted layer, captured on first apply so redo restores it exactly.
    layer: Option<Layer>,
    prev: Option<DocSnapshot>,
}

impl PasteLayer {
    /// Pastes `pixels`, growing the canvas to fit it when `expand` is set.
    pub fn new(pixels: ImageBuffer, expand: bool) -> Self {
        Self {
            pixels: Some(pixels),
            expand,
            layer: None,
            prev: None,
        }
    }
}

impl Command for PasteLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let (cw, ch) = (doc.canvas.width(), doc.canvas.height());
        let (w, h) = match (&self.layer, &self.pixels) {
            (Some(layer), _) => (layer.pixels.width(), layer.pixels.height()),
            (None, Some(px)) if self.expand => (cw.max(px.width()), ch.max(px.height())),
            (None, Some(_)) => (cw, ch),
            (None, None) => return Err(DocumentError::RegionOutOfBounds),
        };
        let snap = snapshot(doc);
        if (w, h) != (cw, ch) {
            ResizeCanvas::new(w, h).apply(doc)?;
        }
        let layer = match (&self.layer, self.pixels.take()) {
            (Some(layer), _) => layer.clone(),
            (None, Some(px)) => Layer::from_pixels(
                format!("Layer {}", doc.layers.len() + 1),
                px.offset_copy(w, h, 0, 0),
            ),
            (None, None) => return Err(DocumentError::RegionOutOfBounds),
        };
        if let Err(err) = doc.insert_layer(doc.active_layer_index() + 1, layer.clone()) {
            restore(doc, snap)?;
            self.layer = Some(layer);
            return Err(err);
        }
        self.layer = Some(layer);
        self.prev = Some(snap);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let snap = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        restore(doc, snap)
    }

    fn label(&self) -> &str {
        "Paste"
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
    fn paste_inserts_above_the_active_layer() {
        let mut doc = Document::new(4, 4).unwrap();
        doc.add_layer("Top").unwrap();
        doc.set_active_layer(0).unwrap();
        let mut cmd = PasteLayer::new(image(2, 2), false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[2].name, "Top");
    }
}
