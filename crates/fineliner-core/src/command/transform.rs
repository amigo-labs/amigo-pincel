//! Discrete transform commands: layer flip / 180° rotation and canvas flip /
//! 90°/180° rotation (spec §10.2, §10.3).
//!
//! These are lossless index permutations, so every one is invertible by
//! re-applying its inverse — no pixel snapshot is needed. Layer flips and 180°
//! rotation preserve dimensions and act on the active layer; canvas operations
//! transform every layer in lockstep and (for 90° turns) swap the canvas
//! dimensions. Canvas transforms clear any selection (its geometry no longer
//! matches); the prior selection is restored on undo.

use super::snapshot::{restore, snapshot, DocSnapshot};
use super::Command;
use crate::document::{CanvasSize, Document, ImageBuffer};
use crate::error::DocumentError;
use crate::selection::SelectionMask;
use crate::transform::{
    flip_horizontal, flip_vertical, rotate_180, rotate_90_ccw, rotate_90_cw, scale, Interpolation,
};
use std::any::Any;

/// A dimension-preserving transform of a single layer (spec §10.2/§10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerTransform {
    /// Mirror left-to-right.
    FlipHorizontal,
    /// Mirror top-to-bottom.
    FlipVertical,
    /// Rotate 180°.
    Rotate180,
}

impl LayerTransform {
    /// Applies the transform to a layer's pixels in place.
    fn apply_to(self, doc: &mut Document, index: usize) -> Result<(), DocumentError> {
        let len = doc.layers.len();
        let layer = doc
            .layers
            .get_mut(index)
            .ok_or(DocumentError::LayerIndexOutOfBounds { index, len })?;
        layer.pixels = match self {
            LayerTransform::FlipHorizontal => flip_horizontal(&layer.pixels),
            LayerTransform::FlipVertical => flip_vertical(&layer.pixels),
            LayerTransform::Rotate180 => rotate_180(&layer.pixels),
        };
        Ok(())
    }
}

/// Flips or 180°-rotates the active layer (spec §10.2/§10.3). Self-inverse.
pub struct TransformLayer {
    index: usize,
    op: LayerTransform,
}

impl TransformLayer {
    /// Transforms the layer at `index`.
    pub fn new(index: usize, op: LayerTransform) -> Self {
        Self { index, op }
    }
}

impl Command for TransformLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.op.apply_to(doc, self.index)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        // Flip-H, flip-V and 180° rotation are each their own inverse.
        self.op.apply_to(doc, self.index)
    }

    fn label(&self) -> &str {
        match self.op {
            LayerTransform::FlipHorizontal => "Flip Layer Horizontal",
            LayerTransform::FlipVertical => "Flip Layer Vertical",
            LayerTransform::Rotate180 => "Rotate Layer 180°",
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Flips the whole canvas (all layers) horizontally or vertically (spec §10.2).
/// Dimensions are preserved, so the command is self-inverse.
pub struct FlipCanvas {
    horizontal: bool,
    prev_selection: Option<Option<SelectionMask>>,
}

impl FlipCanvas {
    /// Flips the canvas horizontally (`true`) or vertically (`false`).
    pub fn new(horizontal: bool) -> Self {
        Self {
            horizontal,
            prev_selection: None,
        }
    }

    fn flip_all(&self, doc: &mut Document) {
        for layer in &mut doc.layers {
            layer.pixels = if self.horizontal {
                flip_horizontal(&layer.pixels)
            } else {
                flip_vertical(&layer.pixels)
            };
        }
    }
}

impl Command for FlipCanvas {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if self.prev_selection.is_none() {
            self.prev_selection = Some(doc.selection.take());
        } else {
            doc.selection = None;
        }
        self.flip_all(doc);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.flip_all(doc);
        if let Some(sel) = self.prev_selection.clone() {
            doc.selection = sel;
        }
        Ok(())
    }

    fn label(&self) -> &str {
        if self.horizontal {
            "Flip Canvas Horizontal"
        } else {
            "Flip Canvas Vertical"
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A 90°/180° rotation of the whole canvas (spec §10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasRotation {
    /// Quarter turn clockwise (swaps dimensions).
    Cw90,
    /// Quarter turn counter-clockwise (swaps dimensions).
    Ccw90,
    /// Half turn (dimensions preserved).
    Rotate180,
}

impl CanvasRotation {
    /// The rotation that undoes this one.
    fn inverse(self) -> CanvasRotation {
        match self {
            CanvasRotation::Cw90 => CanvasRotation::Ccw90,
            CanvasRotation::Ccw90 => CanvasRotation::Cw90,
            CanvasRotation::Rotate180 => CanvasRotation::Rotate180,
        }
    }
}

/// Rotates the whole canvas (all layers) by a quarter or half turn (spec §10.3).
pub struct RotateCanvas {
    rotation: CanvasRotation,
    prev_selection: Option<Option<SelectionMask>>,
}

impl RotateCanvas {
    /// Rotates the canvas by `rotation`.
    pub fn new(rotation: CanvasRotation) -> Self {
        Self {
            rotation,
            prev_selection: None,
        }
    }

    /// Rotates every layer and updates the canvas size accordingly.
    fn rotate_all(doc: &mut Document, rotation: CanvasRotation) -> Result<(), DocumentError> {
        for layer in &mut doc.layers {
            layer.pixels = match rotation {
                CanvasRotation::Cw90 => rotate_90_cw(&layer.pixels),
                CanvasRotation::Ccw90 => rotate_90_ccw(&layer.pixels),
                CanvasRotation::Rotate180 => rotate_180(&layer.pixels),
            };
        }
        if matches!(rotation, CanvasRotation::Cw90 | CanvasRotation::Ccw90) {
            // Quarter turns swap the canvas dimensions.
            let (w, h) = (doc.canvas.width(), doc.canvas.height());
            doc.canvas = CanvasSize::new(h, w)?;
        }
        Ok(())
    }
}

impl Command for RotateCanvas {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if self.prev_selection.is_none() {
            self.prev_selection = Some(doc.selection.take());
        } else {
            doc.selection = None;
        }
        Self::rotate_all(doc, self.rotation)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        Self::rotate_all(doc, self.rotation.inverse())?;
        if let Some(sel) = self.prev_selection.clone() {
            doc.selection = sel;
        }
        Ok(())
    }

    fn label(&self) -> &str {
        match self.rotation {
            CanvasRotation::Cw90 => "Rotate Canvas 90° CW",
            CanvasRotation::Ccw90 => "Rotate Canvas 90° CCW",
            CanvasRotation::Rotate180 => "Rotate Canvas 180°",
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Scales the whole image (all layers) to a new size, resizing the canvas to
/// match (spec §10.5). Resampling uses the chosen [`Interpolation`]. Lossy, so
/// the prior state is snapshotted for undo; the selection is cleared.
pub struct ScaleImage {
    width: u32,
    height: u32,
    interp: Interpolation,
    before: Option<DocSnapshot>,
}

impl ScaleImage {
    /// Scales the image to `width` × `height` using `interp`.
    pub fn new(width: u32, height: u32, interp: Interpolation) -> Self {
        Self {
            width,
            height,
            interp,
            before: None,
        }
    }
}

impl Command for ScaleImage {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let new_canvas = CanvasSize::new(self.width, self.height)?;
        if self.before.is_none() {
            self.before = Some(snapshot(doc));
        }
        for layer in &mut doc.layers {
            layer.pixels = scale(&layer.pixels, self.width, self.height, self.interp);
        }
        doc.canvas = new_canvas;
        doc.selection = None;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let snap = self.before.take().ok_or(DocumentError::RegionOutOfBounds)?;
        restore(doc, snap)
    }

    fn label(&self) -> &str {
        "Scale Image"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Crops the canvas to the bounding box of the current selection (spec §10.6).
///
/// Because every layer is canvas-sized in this model, pixels outside the new
/// canvas are clipped (ADR-010). A no-op when there is no selection.
pub struct CropToSelection {
    before: Option<DocSnapshot>,
}

impl CropToSelection {
    /// Creates the command.
    pub fn new() -> Self {
        Self { before: None }
    }
}

impl Default for CropToSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for CropToSelection {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let Some(bbox) = doc.selection.as_ref().and_then(|s| s.bounding_box()) else {
            return Ok(()); // nothing selected — nothing to crop to
        };
        let new_canvas = CanvasSize::new(bbox.w, bbox.h)?;
        if self.before.is_none() {
            self.before = Some(snapshot(doc));
        }
        for layer in &mut doc.layers {
            layer.pixels = layer.pixels.copy_region(bbox)?;
        }
        doc.canvas = new_canvas;
        doc.selection = None;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        match self.before.take() {
            Some(snap) => restore(doc, snap),
            None => Ok(()), // apply was a no-op
        }
    }

    fn label(&self) -> &str {
        "Crop to Selection"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Rotates the active layer's content 90° (spec §10.3).
///
/// The rotated content is re-centered into the (unchanged) canvas, so the layer
/// stays canvas-sized; on a non-square canvas the corners are clipped. The prior
/// pixels are snapshotted for an exact undo.
pub struct RotateLayer90 {
    index: usize,
    ccw: bool,
    before: Option<ImageBuffer>,
}

impl RotateLayer90 {
    /// Rotates the layer at `index` 90° clockwise (`ccw = false`) or CCW.
    pub fn new(index: usize, ccw: bool) -> Self {
        Self {
            index,
            ccw,
            before: None,
        }
    }
}

impl Command for RotateLayer90 {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let (cw, ch) = (doc.canvas.width(), doc.canvas.height());
        let len = doc.layers.len();
        let layer = doc
            .layers
            .get_mut(self.index)
            .ok_or(DocumentError::LayerIndexOutOfBounds {
                index: self.index,
                len,
            })?;
        if self.before.is_none() {
            self.before = Some(layer.pixels.clone());
        }
        let rotated = if self.ccw {
            rotate_90_ccw(&layer.pixels)
        } else {
            rotate_90_cw(&layer.pixels)
        };
        let dx = (cw as i32 - rotated.width() as i32) / 2;
        let dy = (ch as i32 - rotated.height() as i32) / 2;
        layer.pixels = rotated.offset_copy(cw, ch, dx, dy);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.before.take().ok_or(DocumentError::RegionOutOfBounds)?;
        let len = doc.layers.len();
        let layer = doc
            .layers
            .get_mut(self.index)
            .ok_or(DocumentError::LayerIndexOutOfBounds {
                index: self.index,
                len,
            })?;
        layer.pixels = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        if self.ccw {
            "Rotate Layer 90° CCW"
        } else {
            "Rotate Layer 90° CW"
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::geometry::Rect;

    fn doc_with_corner(w: u32, h: u32) -> Document {
        // A single bright pixel at the top-left corner marks orientation.
        let mut doc = Document::new(w, h).unwrap();
        doc.layers[0].pixels.set_pixel(0, 0, Color::WHITE);
        doc
    }

    #[test]
    fn transform_layer_flip_h_round_trips() {
        let mut doc = doc_with_corner(4, 3);
        let mut cmd = TransformLayer::new(0, LayerTransform::FlipHorizontal);
        cmd.apply(&mut doc).unwrap();
        // The corner pixel moved to the top-right.
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 0), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn transform_layer_180_round_trips() {
        let mut doc = doc_with_corner(4, 3);
        let mut cmd = TransformLayer::new(0, LayerTransform::Rotate180);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 2), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn flip_canvas_transforms_all_layers_and_round_trips() {
        let mut doc = doc_with_corner(4, 3);
        doc.add_layer("Top").unwrap();
        doc.layers[1].pixels.set_pixel(0, 0, Color::WHITE);
        let mut cmd = FlipCanvas::new(true);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 0), Some(Color::WHITE));
        assert_eq!(doc.layers[1].pixels.get_pixel(3, 0), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(doc.layers[1].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn rotate_canvas_90_swaps_dimensions_and_round_trips() {
        let mut doc = doc_with_corner(4, 3);
        let mut cmd = RotateCanvas::new(CanvasRotation::Cw90);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (3, 4));
        assert_eq!(doc.layers[0].pixels.width(), 3);
        cmd.revert(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 3));
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn rotate_canvas_clears_selection_and_restores_on_undo() {
        let mut doc = doc_with_corner(4, 3);
        doc.selection = Some(SelectionMask::new_full(4, 3));
        let mut cmd = RotateCanvas::new(CanvasRotation::Cw90);
        cmd.apply(&mut doc).unwrap();
        assert!(doc.selection.is_none());
        cmd.revert(&mut doc).unwrap();
        assert!(doc.selection.is_some());
    }

    #[test]
    fn scale_image_resizes_canvas_and_all_layers_and_round_trips() {
        let mut doc = doc_with_corner(4, 4);
        doc.add_layer("Top").unwrap();
        let mut cmd = ScaleImage::new(8, 8, Interpolation::Nearest);
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (8, 8));
        assert_eq!(doc.layers[0].pixels.width(), 8);
        assert_eq!(doc.layers[1].pixels.width(), 8);
        cmd.revert(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn crop_to_selection_shrinks_canvas_to_bbox_and_round_trips() {
        let mut doc = doc_with_corner(8, 8);
        // Mark a pixel inside the future crop region so we can locate it after.
        doc.layers[0].pixels.set_pixel(3, 3, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(8, 8, Rect::new(2, 2, 4, 4)));
        let mut cmd = CropToSelection::new();
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 4));
        // Old (3,3) maps to (1,1) within the cropped canvas.
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 1), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (8, 8));
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 3), Some(Color::WHITE));
    }

    #[test]
    fn crop_to_selection_without_selection_is_noop() {
        let mut doc = doc_with_corner(8, 8);
        let mut cmd = CropToSelection::new();
        cmd.apply(&mut doc).unwrap();
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (8, 8));
    }

    #[test]
    fn rotate_layer_90_keeps_canvas_size_and_round_trips() {
        // Square canvas: the rotation is lossless and re-centering is a no-op.
        let mut doc = doc_with_corner(4, 4);
        let mut cmd = RotateLayer90::new(0, false);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(
            (doc.layers[0].pixels.width(), doc.layers[0].pixels.height()),
            (4, 4)
        );
        // CW rotation sends the top-left corner to the top-right.
        assert_eq!(doc.layers[0].pixels.get_pixel(3, 0), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
    }
}
