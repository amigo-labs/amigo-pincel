//! Command and undo/redo system (spec §7).
//!
//! Every mutation to a [`Document`] is expressed as a [`Command`] so it can be
//! undone and redone. The [`CommandBus`] applies a command and records it on the
//! [`UndoStack`].

mod auto_crop;
mod batch;
mod damage;
mod layers;
mod live;
pub mod live_spec;
mod merge;
mod paste;
mod pixels;
mod properties;
mod resize;
mod selection;
mod snapshot;
pub mod spec;
mod tiles;
mod transform;

pub use auto_crop::{AutoCrop, auto_crop_rect};
pub(crate) use batch::Batch;
pub use damage::Damage;
pub use layers::{AddGroup, AddLayer, DuplicateLayer, MoveLayer, MoveLayerTo, RemoveLayer};
pub use live::{
    BoxShape, LIVE_REACH, LiveEdit, LiveGradient, LiveObject, LiveParams, LiveShape, LiveShapeKind,
};
pub use merge::{FlattenImage, MergeDown, MergeGroup, MergeVisible};
pub use paste::PasteLayer;
pub use pixels::SetPixels;
pub use properties::{
    RenameLayer, SetLayerBlendMode, SetLayerLocked, SetLayerOpacity, SetLayerVisible,
};
pub use resize::{Anchor, ResizeCanvas};
pub use selection::{MoveSelection, SetSelection};
pub use tiles::TILE as UNDO_TILE;
pub use transform::{
    CanvasRotation, CropToSelection, FlipCanvas, LayerTransform, RotateCanvas, RotateLayer90,
    ScaleImage, TransformLayer,
};

use crate::document::Document;
use crate::error::DocumentError;
use std::any::Any;

/// Default maximum undo depth (spec §7.2; configurable 10–500).
pub const DEFAULT_UNDO_CAPACITY: usize = 100;

/// Default cap on the undo data a history holds: 512 MB
/// (docs/specs/performance-image-undo-memory.md AC-05).
pub const DEFAULT_UNDO_BYTE_CAP: usize = 512 * 1024 * 1024;

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

    /// The part of the composite that [`apply`](Command::apply) or
    /// [`revert`](Command::revert) may change, for render caches
    /// (docs/specs/performance-image-cache.md). The default, [`Damage::Full`],
    /// is always correct; commands override it to report less.
    fn damage(&self) -> Damage {
        Damage::Full
    }

    /// Bytes of pixel and selection data this command holds right now to
    /// undo or redo itself (docs/specs/performance-image-undo-memory.md).
    /// The history sums it for its byte cap; a command holding none keeps
    /// the default 0.
    fn undo_bytes(&self) -> usize {
        0
    }

    /// Downcast support, required for type-aware merging.
    fn as_any(&self) -> &dyn Any;
}

/// A linear undo/redo history (spec §7.2). No tree-view in Phase 1.
pub struct UndoStack {
    undo: Vec<Box<dyn Command>>,
    redo: Vec<Box<dyn Command>>,
    capacity: usize,
    /// [`Command::undo_bytes`] summed over both stacks, kept up to date.
    bytes: usize,
    byte_cap: usize,
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
            bytes: 0,
            byte_cap: DEFAULT_UNDO_BYTE_CAP,
        }
    }

    /// Sets the cap on held undo data in bytes (default
    /// [`DEFAULT_UNDO_BYTE_CAP`]); applies from the next push.
    pub fn set_byte_cap(&mut self, bytes: usize) {
        self.byte_cap = bytes;
    }

    /// Bytes of undo and redo data held, as tracked on every push, undo,
    /// redo and drop.
    pub fn stored_bytes(&self) -> usize {
        self.bytes
    }

    /// The same sum as [`stored_bytes`](Self::stored_bytes), recomputed
    /// from the commands themselves — a check of the bookkeeping.
    pub fn recount_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|c| c.undo_bytes())
            .sum()
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
        self.bytes = 0;
    }

    /// Records an already-applied command.
    ///
    /// First tries to merge into the top command; otherwise pushes it. Any new
    /// command discards the redo branch (spec §7.2). Enforces the capacity by
    /// dropping the oldest command — unless it begins the live object, whose
    /// steps are never dropped: then its oldest edit folds into the begin
    /// step (docs/specs/image-live-objects.md AC-14).
    pub fn push(&mut self, cmd: Box<dyn Command>) {
        for dropped in self.redo.drain(..) {
            self.bytes -= dropped.undo_bytes();
        }
        if let Some(top) = self.undo.last_mut() {
            let before = top.undo_bytes();
            if top.merge_with(cmd.as_ref()) {
                self.bytes = self.bytes - before + top.undo_bytes();
                self.enforce_byte_cap();
                return;
            }
        }
        self.bytes += cmd.undo_bytes();
        self.undo.push(cmd);
        if self.undo.len() > self.capacity {
            self.drop_oldest();
        }
        self.enforce_byte_cap();
    }

    /// Drops the oldest undo step, keeping the byte count — unless it begins
    /// the live object: then its oldest edit folds into the begin step
    /// (docs/specs/image-live-objects.md AC-14).
    fn drop_oldest(&mut self) {
        let folded = match self.undo.as_slice() {
            [first, second, ..] => live::LiveStep::fold(first.as_ref(), second.as_ref()),
            _ => None,
        };
        match folded {
            Some(step) => {
                for gone in self.undo.drain(..2) {
                    self.bytes -= gone.undo_bytes();
                }
                self.bytes += step.undo_bytes();
                self.undo.insert(0, step);
            }
            None => {
                let dropped = self.undo.remove(0);
                self.bytes -= dropped.undo_bytes();
            }
        }
    }

    /// Drops the oldest undo steps until the held data fits the byte cap;
    /// the newest step always stays (docs/specs/performance-image-undo-memory.md
    /// AC-05). Redo steps never count here: a push has discarded them.
    fn enforce_byte_cap(&mut self) {
        while self.bytes > self.byte_cap && self.undo.len() > 1 {
            self.drop_oldest();
        }
    }

    /// Takes the newest undo step off the stack, keeping the byte count.
    pub(crate) fn pop_undo(&mut self) -> Option<Box<dyn Command>> {
        let step = self.undo.pop()?;
        self.bytes -= step.undo_bytes();
        Some(step)
    }

    /// Drops every undo step above the oldest `keep` and the redo branch,
    /// keeping the byte count.
    pub(crate) fn truncate_undo(&mut self, keep: usize) {
        for gone in self.undo.drain(keep.min(self.undo.len())..) {
            self.bytes -= gone.undo_bytes();
        }
        for gone in self.redo.drain(..) {
            self.bytes -= gone.undo_bytes();
        }
    }

    /// Reverts the most recent command, moving it to the redo stack.
    ///
    /// Returns `Ok(false)` if there was nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<bool, DocumentError> {
        match self.undo.pop() {
            Some(mut cmd) => {
                let before = cmd.undo_bytes();
                let reverted = cmd.revert(doc);
                // A failed step is dropped with whatever it held.
                self.bytes -= before;
                reverted?;
                self.bytes += cmd.undo_bytes();
                self.redo.push(cmd);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// The damage the next [`undo`](UndoStack::undo) reports, if any.
    fn next_undo_damage(&self) -> Option<Damage> {
        self.undo.last().map(|cmd| cmd.damage())
    }

    /// The damage the next [`redo`](UndoStack::redo) reports, if any.
    fn next_redo_damage(&self) -> Option<Damage> {
        self.redo.last().map(|cmd| cmd.damage())
    }

    /// Re-applies the most recently undone command.
    ///
    /// Returns `Ok(false)` if there was nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<bool, DocumentError> {
        match self.redo.pop() {
            Some(mut cmd) => {
                let before = cmd.undo_bytes();
                let applied = cmd.apply(doc);
                // A failed step is dropped with whatever it held.
                self.bytes -= before;
                applied?;
                self.bytes += cmd.undo_bytes();
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
    /// What the commands applied, undone or redone since the last
    /// [`take_damage`](CommandBus::take_damage) changed in the composite.
    damage: Damage,
    /// What the next live-object edit may merge with
    /// (docs/specs/image-live-objects.md AC-03).
    live_merge: Option<live::MergeKey>,
}

impl CommandBus {
    /// Creates a bus for `document` with a default-capacity history.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            history: UndoStack::new(),
            damage: Damage::Full,
            live_merge: None,
        }
    }

    /// Applies `cmd` to the document and records it in history. A live
    /// object is committed first (docs/specs/image-live-objects.md AC-08).
    pub fn apply(&mut self, mut cmd: Box<dyn Command>) -> Result<(), DocumentError> {
        self.commit_if_live()?;
        let applied = cmd.apply(&mut self.document);
        // A failed command may have changed part of the document.
        self.damage.add(match applied {
            Ok(()) => cmd.damage(),
            Err(_) => Damage::Full,
        });
        applied?;
        self.history.push(cmd);
        Ok(())
    }

    /// Undoes the last command. Returns `false` if nothing to undo.
    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        self.live_merge = None;
        let damage = self.history.next_undo_damage();
        let undone = self.history.undo(&mut self.document);
        self.record(damage, undone.is_ok());
        undone
    }

    /// Redoes the last undone command. Returns `false` if nothing to redo.
    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        self.live_merge = None;
        let damage = self.history.next_redo_damage();
        let redone = self.history.redo(&mut self.document);
        self.record(damage, redone.is_ok());
        redone
    }

    /// Returns what changed in the composite since the last call (all of it
    /// for a new bus) and starts collecting afresh. A render cache calls this
    /// before presenting (docs/specs/performance-image-cache.md).
    pub fn take_damage(&mut self) -> Damage {
        std::mem::take(&mut self.damage)
    }

    /// Adds the damage of an undo or redo; a failed one may have changed
    /// anything.
    fn record(&mut self, damage: Option<Damage>, ok: bool) {
        match (damage, ok) {
            (Some(d), true) => self.damage.add(d),
            (Some(_), false) => self.damage.add(Damage::Full),
            (None, _) => {}
        }
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
    fn take_damage_on_a_new_bus_is_full_then_none() {
        let mut bus = CommandBus::new(Document::new(4, 4).unwrap());
        assert_eq!(bus.take_damage(), Damage::Full);
        assert_eq!(bus.take_damage(), Damage::None);
    }

    #[test]
    fn take_damage_after_pixels_undo_and_redo_is_their_rect() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        bus.take_damage();
        let patch = crate::ImageBuffer::new_transparent(2, 3);
        bus.apply(Box::new(SetPixels::new(0, Rect::new(1, 2, 2, 3), patch)))
            .unwrap();
        let rect = Damage::rect(Rect::new(1, 2, 2, 3));
        assert_eq!(bus.take_damage(), rect);
        bus.undo().unwrap();
        assert_eq!(bus.take_damage(), rect);
        bus.redo().unwrap();
        assert_eq!(bus.take_damage(), rect);
    }

    #[test]
    fn take_damage_after_selection_rename_and_new_layer_is_none() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        bus.take_damage();
        let mask = crate::selection::SelectionMask::rectangle(8, 8, Rect::new(0, 0, 2, 2));
        bus.apply(Box::new(SetSelection::replace(mask))).unwrap();
        bus.apply(Box::new(RenameLayer::new(0, "Sky"))).unwrap();
        bus.apply(Box::new(SetLayerLocked::new(0, true))).unwrap();
        bus.apply(Box::new(AddLayer::above(0))).unwrap();
        bus.undo().unwrap();
        assert_eq!(bus.take_damage(), Damage::None);
    }

    #[test]
    fn take_damage_after_a_canvas_resize_is_full() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        bus.take_damage();
        bus.apply(Box::new(ResizeCanvas::new(4, 4))).unwrap();
        assert_eq!(bus.take_damage(), Damage::Full);
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

    /// A whole-layer edit of a `w`×`h` document filled with `v`.
    fn fill_layer(bus: &mut CommandBus, v: u8) {
        let (w, h) = (bus.document.canvas.width(), bus.document.canvas.height());
        let data = vec![v; (w * h * 4) as usize];
        let after = crate::ImageBuffer::from_raw(w, h, data).unwrap();
        bus.apply(Box::new(SetPixels::new(0, Rect::new(0, 0, w, h), after)))
            .unwrap();
    }

    #[test]
    fn byte_cap_drops_oldest_steps_and_keeps_the_rest_exact() {
        // 256×256: each whole-layer edit holds four 64 KiB tiles.
        let mut bus = CommandBus::new(Document::new(256, 256).unwrap());
        let step = 256 * 256 * 4;
        bus.history.set_byte_cap(3 * step);
        let mut states = vec![bus.document.clone()];
        for v in 1..=5u8 {
            fill_layer(&mut bus, v * 40);
            states.push(bus.document.clone());
            assert!(bus.history.stored_bytes() <= 3 * step);
            assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
        }
        assert_eq!(bus.history.undo_depth(), 3, "two oldest steps dropped");
        for want in states[2..5].iter().rev() {
            assert!(bus.undo().unwrap());
            assert!(&bus.document == want, "undo restores its state exactly");
        }
        assert!(!bus.undo().unwrap(), "nothing older is left");
    }

    #[test]
    fn byte_cap_keeps_a_step_larger_than_the_cap_alone() {
        let mut bus = CommandBus::new(Document::new(256, 256).unwrap());
        let before = bus.document.clone();
        fill_layer(&mut bus, 10);
        fill_layer(&mut bus, 20);
        bus.history.set_byte_cap(1000);
        let middle = bus.document.clone();
        fill_layer(&mut bus, 30);
        assert_eq!(bus.history.undo_depth(), 1, "only the step just performed");
        assert!(bus.history.stored_bytes() > 1000);
        assert!(bus.undo().unwrap());
        assert!(bus.document == middle);
        assert!(bus.document != before);
        assert!(!bus.undo().unwrap());
    }

    #[test]
    fn stored_bytes_follow_undo_redo_and_new_pushes() {
        let mut bus = CommandBus::new(Document::new(200, 130).unwrap());
        assert_eq!(bus.history.stored_bytes(), 0);
        fill_layer(&mut bus, 9);
        bus.apply(Box::new(AddLayer::above(0))).unwrap();
        bus.apply(Box::new(crate::MergeDown::at(1))).unwrap();
        let held = bus.history.stored_bytes();
        assert!(held > 0);
        for _ in 0..3 {
            assert!(bus.undo().unwrap());
            assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
        }
        // Undone, the added layer is held for redo.
        assert!(bus.history.stored_bytes() >= 200 * 130 * 4);
        assert!(bus.redo().unwrap());
        assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
        fill_layer(&mut bus, 77);
        assert_eq!(bus.history.redo_depth(), 0);
        assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
    }
}
