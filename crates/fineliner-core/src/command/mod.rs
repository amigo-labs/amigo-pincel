//! Command and undo/redo system (spec §7).
//!
//! Every mutation to a [`Document`] is expressed as a [`Command`] so it can be
//! undone and redone. The [`CommandBus`] applies a command and records it on the
//! [`UndoStack`].

mod layers;
mod merge;
mod pixels;
mod properties;
mod resize;
mod selection;
mod snapshot;
mod transform;

pub use layers::{AddLayer, DuplicateLayer, MoveLayer, RemoveLayer};
pub use merge::{FlattenImage, MergeDown, MergeVisible};
pub use pixels::{PixelEdit, SetPixels};
pub use properties::{
    RenameLayer, SetLayerBlendMode, SetLayerLocked, SetLayerOpacity, SetLayerVisible,
};
pub use resize::{Anchor, ResizeCanvas};
pub use selection::SetSelection;
pub use transform::{
    CanvasRotation, CropToSelection, FlipCanvas, LayerTransform, RotateCanvas, RotateLayer90,
    ScaleImage, TransformLayer,
};

use crate::document::Document;
use crate::error::DocumentError;
use std::any::Any;

/// Default maximum undo depth (spec §7.2; configurable 10–500).
pub const DEFAULT_UNDO_CAPACITY: usize = 100;

/// An undoable mutation of a [`Document`] (spec §7.1).
pub trait Command: Send + Sync {
    /// Applies the change. Called once when first executed and again on redo.
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError>;

    /// Reverts the change, restoring the document to its prior state.
    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError>;

    /// Human-readable label shown in the history panel (e.g. "Pencil Stroke").
    fn label(&self) -> &str;

    /// Attempts to absorb `newer` into `self` for a single undo step.
    ///
    /// Returns `true` if `newer` was merged (and should therefore not be pushed
    /// separately). Used to coalesce the many pixel edits of one pointer drag.
    /// `newer` is assumed already applied to the document.
    fn merge_with(&mut self, _newer: &dyn Command) -> bool {
        false
    }

    /// Downcast support, required for type-aware merging.
    fn as_any(&self) -> &dyn Any;
}

/// A linear undo/redo history (spec §7.2). No tree-view in Phase 1.
pub struct UndoStack {
    undo: Vec<Box<dyn Command>>,
    redo: Vec<Box<dyn Command>>,
    capacity: usize,
}

impl UndoStack {
    /// Creates a stack with [`DEFAULT_UNDO_CAPACITY`].
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_UNDO_CAPACITY)
    }

    /// Creates a stack with a custom capacity, clamped to `10..=500` (spec §7.2).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            capacity: capacity.clamp(10, 500),
        }
    }

    /// Number of commands available to undo.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of commands available to redo.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Clears all history.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Records an already-applied command.
    ///
    /// First tries to merge into the top command; otherwise pushes it. Any new
    /// command discards the redo branch (spec §7.2). Enforces the capacity by
    /// dropping the oldest command.
    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.redo.clear();
        if let Some(top) = self.undo.last_mut() {
            if top.merge_with(cmd.as_ref()) {
                return;
            }
        }
        self.undo.push(cmd);
        if self.undo.len() > self.capacity {
            self.undo.remove(0);
        }
    }

    /// Reverts the most recent command, moving it to the redo stack.
    ///
    /// Returns `Ok(false)` if there was nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<bool, DocumentError> {
        match self.undo.pop() {
            Some(mut cmd) => {
                cmd.revert(doc)?;
                self.redo.push(cmd);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Re-applies the most recently undone command.
    ///
    /// Returns `Ok(false)` if there was nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<bool, DocumentError> {
        match self.redo.pop() {
            Some(mut cmd) => {
                cmd.apply(doc)?;
                self.undo.push(cmd);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Couples a [`Document`] with its [`UndoStack`] and applies commands to both.
pub struct CommandBus {
    /// The document being edited.
    pub document: Document,
    /// The undo/redo history for `document`.
    pub history: UndoStack,
}

impl CommandBus {
    /// Creates a bus for `document` with a default-capacity history.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            history: UndoStack::new(),
        }
    }

    /// Applies `cmd` to the document and records it in history.
    pub fn apply(&mut self, mut cmd: Box<dyn Command>) -> Result<(), DocumentError> {
        cmd.apply(&mut self.document)?;
        self.history.push(cmd);
        Ok(())
    }

    /// Undoes the last command. Returns `false` if nothing to undo.
    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        self.history.undo(&mut self.document)
    }

    /// Redoes the last undone command. Returns `false` if nothing to redo.
    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        self.history.redo(&mut self.document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::geometry::Rect;

    fn pixel(doc: &Document, layer: usize, x: u32, y: u32) -> Color {
        doc.layers[layer].pixels.get_pixel(x, y).unwrap()
    }

    #[test]
    fn apply_then_undo_restores_state() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        let mut patch = crate::ImageBuffer::new_transparent(2, 2);
        for i in 0..4 {
            patch.set_pixel(i % 2, i / 2, Color::WHITE);
        }
        bus.apply(Box::new(SetPixels::new(0, Rect::new(1, 1, 2, 2), patch)))
            .unwrap();
        assert_eq!(pixel(&bus.document, 0, 1, 1), Color::WHITE);

        assert!(bus.undo().unwrap());
        assert_eq!(pixel(&bus.document, 0, 1, 1), Color::TRANSPARENT);
    }

    #[test]
    fn new_command_after_undo_discards_redo() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        bus.apply(Box::new(AddLayer::above(0))).unwrap();
        assert!(bus.undo().unwrap());
        assert_eq!(bus.history.redo_depth(), 1);

        bus.apply(Box::new(AddLayer::above(0))).unwrap();
        assert_eq!(bus.history.redo_depth(), 0);
        assert!(!bus.redo().unwrap());
    }

    #[test]
    fn undo_on_empty_history_returns_false() {
        let mut bus = CommandBus::new(Document::new(4, 4).unwrap());
        assert!(!bus.undo().unwrap());
    }

    #[test]
    fn capacity_drops_oldest_command() {
        let mut stack = UndoStack::with_capacity(10);
        let mut doc = Document::new(4, 4).unwrap();
        for _ in 0..15 {
            let mut cmd: Box<dyn Command> = Box::new(AddLayer::above(0));
            cmd.apply(&mut doc).unwrap();
            stack.push(cmd);
        }
        assert_eq!(stack.undo_depth(), 10);
    }

    #[test]
    fn clear_empties_both_stacks() {
        let mut bus = CommandBus::new(Document::new(4, 4).unwrap());
        bus.apply(Box::new(AddLayer::above(0))).unwrap();
        bus.undo().unwrap();
        bus.history.clear();
        assert!(!bus.history.can_undo());
        assert!(!bus.history.can_redo());
    }
}
