//! Layer-property commands: rename, opacity, blend mode, visibility, lock
//! (spec §5.2 / §7.3). Each remembers the prior value on first apply so revert
//! and redo restore it exactly.

use super::Command;
use crate::color::BlendMode;
use crate::document::{Document, Layer};
use crate::error::DocumentError;
use std::any::Any;

/// Returns a mutable reference to the layer at `index`, or a bounds error.
fn layer_mut(doc: &mut Document, index: usize) -> Result<&mut Layer, DocumentError> {
    let len = doc.layer_count();
    doc.layers
        .get_mut(index)
        .ok_or(DocumentError::LayerIndexOutOfBounds { index, len })
}

/// Renames the layer at `index`.
pub struct RenameLayer {
    index: usize,
    name: String,
    prev: Option<String>,
}

impl RenameLayer {
    /// Renames the layer at `index` to `name`.
    pub fn new(index: usize, name: impl Into<String>) -> Self {
        Self {
            index,
            name: name.into(),
            prev: None,
        }
    }
}

impl Command for RenameLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = layer_mut(doc, self.index)?;
        self.prev = Some(std::mem::replace(&mut layer.name, self.name.clone()));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        layer_mut(doc, self.index)?.name = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        "Rename Layer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Sets the opacity of the layer at `index`, clamped to `[0.0, 1.0]`.
///
/// Consecutive sets on the same layer merge into one undo step so a slider drag
/// produces a single history entry (spec §5.2: "live preview").
pub struct SetLayerOpacity {
    index: usize,
    opacity: f32,
    prev: Option<f32>,
}

impl SetLayerOpacity {
    /// Sets the opacity of the layer at `index`.
    pub fn new(index: usize, opacity: f32) -> Self {
        Self {
            index,
            opacity: opacity.clamp(0.0, 1.0),
            prev: None,
        }
    }
}

impl Command for SetLayerOpacity {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = layer_mut(doc, self.index)?;
        self.prev = Some(layer.opacity);
        layer.opacity = self.opacity;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        layer_mut(doc, self.index)?.opacity = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        "Layer Opacity"
    }

    fn merge_with(&mut self, newer: &dyn Command) -> bool {
        match newer.as_any().downcast_ref::<SetLayerOpacity>() {
            // Same layer: adopt the newer target value, keep our original `prev`.
            Some(other) if other.index == self.index => {
                self.opacity = other.opacity;
                true
            }
            _ => false,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Sets the blend mode of the layer at `index`.
pub struct SetLayerBlendMode {
    index: usize,
    mode: BlendMode,
    prev: Option<BlendMode>,
}

impl SetLayerBlendMode {
    /// Sets the blend mode of the layer at `index`.
    pub fn new(index: usize, mode: BlendMode) -> Self {
        Self {
            index,
            mode,
            prev: None,
        }
    }
}

impl Command for SetLayerBlendMode {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = layer_mut(doc, self.index)?;
        self.prev = Some(layer.blend_mode);
        layer.blend_mode = self.mode;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        layer_mut(doc, self.index)?.blend_mode = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        "Layer Blend Mode"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggles the visibility of the layer at `index`.
pub struct SetLayerVisible {
    index: usize,
    visible: bool,
    prev: Option<bool>,
}

impl SetLayerVisible {
    /// Sets the visibility of the layer at `index`.
    pub fn new(index: usize, visible: bool) -> Self {
        Self {
            index,
            visible,
            prev: None,
        }
    }
}

impl Command for SetLayerVisible {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = layer_mut(doc, self.index)?;
        self.prev = Some(layer.visible);
        layer.visible = self.visible;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        layer_mut(doc, self.index)?.visible = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        "Layer Visibility"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggles the lock state of the layer at `index`.
pub struct SetLayerLocked {
    index: usize,
    locked: bool,
    prev: Option<bool>,
}

impl SetLayerLocked {
    /// Sets the lock state of the layer at `index`.
    pub fn new(index: usize, locked: bool) -> Self {
        Self {
            index,
            locked,
            prev: None,
        }
    }
}

impl Command for SetLayerLocked {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = layer_mut(doc, self.index)?;
        self.prev = Some(layer.locked);
        layer.locked = self.locked;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let prev = self.prev.take().ok_or(DocumentError::RegionOutOfBounds)?;
        layer_mut(doc, self.index)?.locked = prev;
        Ok(())
    }

    fn label(&self) -> &str {
        "Layer Lock"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_layer_round_trip() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = RenameLayer::new(0, "Sky");
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].name, "Sky");
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].name, "Layer 1");
    }

    #[test]
    fn set_opacity_clamps_and_round_trips() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = SetLayerOpacity::new(0, 2.0);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].opacity, 1.0); // clamped
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].opacity, 1.0); // restored to prior 1.0
    }

    #[test]
    fn set_opacity_merges_consecutive_same_layer_edits() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut first = SetLayerOpacity::new(0, 0.5);
        first.apply(&mut doc).unwrap();
        let mut second = SetLayerOpacity::new(0, 0.2);
        second.apply(&mut doc).unwrap();
        // The slider drag's later value is absorbed into the first command.
        assert!(first.merge_with(&second));
        first.revert(&mut doc).unwrap();
        // Reverting the merged command restores the value before the drag began.
        assert_eq!(doc.layers()[0].opacity, 1.0);
    }

    #[test]
    fn set_opacity_does_not_merge_across_layers() {
        let mut doc = Document::new(4, 4).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let mut a = SetLayerOpacity::new(0, 0.5);
        a.apply(&mut doc).unwrap();
        let mut b = SetLayerOpacity::new(1, 0.5);
        b.apply(&mut doc).unwrap();
        assert!(!a.merge_with(&b));
    }

    #[test]
    fn set_blend_mode_round_trip() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = SetLayerBlendMode::new(0, BlendMode::Multiply);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].blend_mode, BlendMode::Multiply);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers()[0].blend_mode, BlendMode::Normal);
    }

    #[test]
    fn set_visible_round_trip() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = SetLayerVisible::new(0, false);
        cmd.apply(&mut doc).unwrap();
        assert!(!doc.layers()[0].visible);
        cmd.revert(&mut doc).unwrap();
        assert!(doc.layers()[0].visible);
    }

    #[test]
    fn set_locked_round_trip() {
        let mut doc = Document::new(4, 4).unwrap();
        let mut cmd = SetLayerLocked::new(0, true);
        cmd.apply(&mut doc).unwrap();
        assert!(doc.layers()[0].locked);
        cmd.revert(&mut doc).unwrap();
        assert!(!doc.layers()[0].locked);
    }

    #[test]
    fn property_command_rejects_bad_index() {
        let mut doc = Document::new(4, 4).unwrap();
        assert!(RenameLayer::new(5, "x").apply(&mut doc).is_err());
    }
}
