//! Layer-structure commands: `AddLayer`, `RemoveLayer`, `MoveLayer` (spec §7.3).

use super::Command;
use crate::document::{Document, Layer};
use crate::error::DocumentError;
use std::any::Any;
use uuid::Uuid;

/// Inserts a new transparent layer at a given index.
pub struct AddLayer {
    index: usize,
    /// The inserted layer, captured on first apply so redo restores it exactly.
    layer: Option<Layer>,
    prev_active: usize,
}

impl AddLayer {
    /// Adds a layer directly above layer `active`.
    pub fn above(active: usize) -> Self {
        Self {
            index: active + 1,
            layer: None,
            prev_active: active,
        }
    }
}

impl Command for AddLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        let layer = match self.layer.take() {
            Some(l) => l,
            None => Layer::new_transparent(
                format!("Layer {}", doc.layers.len() + 1),
                doc.canvas.width(),
                doc.canvas.height(),
            ),
        };
        let restored = layer.clone();
        doc.insert_layer(self.index, layer)?;
        self.layer = Some(restored);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.remove_layer(self.index)?;
        doc.set_active_layer(self.prev_active.min(doc.layers.len() - 1))
    }

    fn label(&self) -> &str {
        "Add Layer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Removes the layer at `index`, remembering it for undo.
pub struct RemoveLayer {
    index: usize,
    removed: Option<Layer>,
    prev_active: usize,
}

impl RemoveLayer {
    /// Removes the layer at `index`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            removed: None,
            prev_active: 0,
        }
    }
}

impl Command for RemoveLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        self.removed = Some(doc.remove_layer(self.index)?);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer = self
            .removed
            .take()
            .ok_or(DocumentError::RegionOutOfBounds)?;
        doc.insert_layer(self.index, layer)?;
        doc.set_active_layer(self.prev_active)
    }

    fn label(&self) -> &str {
        "Delete Layer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Reorders a layer from one index to another.
pub struct MoveLayer {
    from: usize,
    to: usize,
    prev_active: usize,
}

impl MoveLayer {
    /// Moves the layer at `from` to `to`.
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            prev_active: 0,
        }
    }
}

impl Command for MoveLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        doc.move_layer(self.from, self.to)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.move_layer(self.to, self.from)?;
        // move_layer activates the moved layer; restore the prior selection.
        doc.set_active_layer(self.prev_active)
    }

    fn label(&self) -> &str {
        "Move Layer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Duplicates the layer at `index`, inserting the copy directly above it.
///
/// The copy is an independent layer (fresh id) with name `"<name> copy"`,
/// preserving the original's pixels, opacity, blend mode, visibility, and lock.
pub struct DuplicateLayer {
    index: usize,
    /// The inserted copy, captured on first apply so redo restores it exactly.
    copy: Option<Layer>,
    prev_active: usize,
}

impl DuplicateLayer {
    /// Duplicates the layer at `index`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            copy: None,
            prev_active: 0,
        }
    }
}

impl Command for DuplicateLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        let copy = match self.copy.take() {
            Some(l) => l,
            None => {
                let original =
                    doc.layers()
                        .get(self.index)
                        .ok_or(DocumentError::LayerIndexOutOfBounds {
                            index: self.index,
                            len: doc.layer_count(),
                        })?;
                let mut copy = original.clone();
                copy.id = Uuid::new_v4();
                copy.name = format!("{} copy", original.name);
                copy
            }
        };
        let restored = copy.clone();
        doc.insert_layer(self.index + 1, copy)?;
        self.copy = Some(restored);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.remove_layer(self.index + 1)?;
        doc.set_active_layer(self.prev_active)
    }

    fn label(&self) -> &str {
        "Duplicate Layer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_layer_round_trip() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = AddLayer::above(0);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn remove_layer_round_trip_preserves_layer_identity() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let target_id = doc.layers[1].id;
        let mut cmd = RemoveLayer::at(1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 1);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].id, target_id);
    }

    #[test]
    fn move_layer_round_trip() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let bottom_id = doc.layers[0].id;
        let mut cmd = MoveLayer::new(0, 1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[1].id, bottom_id);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].id, bottom_id);
    }

    #[test]
    fn duplicate_layer_inserts_independent_copy_above() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.layers[0].name = "Base".to_string();
        let original_id = doc.layers[0].id;
        let mut cmd = DuplicateLayer::at(0);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        // Copy sits directly above the original and is the active layer.
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[1].name, "Base copy");
        assert_ne!(doc.layers[1].id, original_id);
        assert_eq!(doc.layers[0].id, original_id);
    }

    #[test]
    fn duplicate_layer_round_trip_restores_active() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        doc.set_active_layer(0).unwrap();
        let mut cmd = DuplicateLayer::at(1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 3);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active_layer_index(), 0);
    }

    #[test]
    fn move_layer_revert_restores_previous_active_layer() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap(); // 3 layers below, indices 0..=2
        doc.add_layer("Layer 3").unwrap();
        doc.set_active_layer(0).unwrap();

        let mut cmd = MoveLayer::new(2, 0);
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        // The active layer selected before the move is restored.
        assert_eq!(doc.active_layer_index(), 0);
    }
}
