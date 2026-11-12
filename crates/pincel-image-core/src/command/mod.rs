//! Command and undo/redo system (spec §7).
//!
//! Every mutation to a [`Document`] is expressed as a [`Command`] so it can be
//! undone and redone. The [`CommandBus`] applies a command and records it on the
//! [`UndoStack`].

mod auto_crop;
mod batch;
mod clone_spec;
mod damage;
mod history;
mod layers;
mod live;
pub mod live_spec;
mod live_text;
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
pub use history::HistoryKind;
pub use layers::{AddGroup, AddLayer, DuplicateLayer, MoveLayer, MoveLayerTo, RemoveLayer};
pub use live::{
    BoxShape, LIVE_REACH, LiveEdit, LiveGradient, LiveObject, LiveParams, LiveShape, LiveShapeKind,
};
pub use live_text::{LiveFont, LiveText, text_box};
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

    /// The action that made this step, which the History panel names
    /// (docs/specs/history-panel.md AC-15).
    fn kind(&self) -> HistoryKind;

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

    /// Whether [`apply`](Command::apply) or [`revert`](Command::revert) may
    /// change the document's selection, for
    /// [`CommandBus::selection_generation`]. The default, `true`, is always
    /// correct; commands that only edit pixels override it.
    fn changes_selection(&self) -> bool {
        true
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
    /// Set once the capacity or the byte cap has dropped a step from the
    /// bottom of the history (docs/specs/history-panel.md AC-03).
    dropped: bool,
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
            dropped: false,
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

    /// The kinds of the undo steps, oldest first.
    pub fn undo_kinds(&self) -> impl Iterator<Item = HistoryKind> + '_ {
        self.undo.iter().map(|c| c.kind())
    }

    /// The kinds of the redo steps, in the order [`UndoStack::redo`]
    /// restores them (the next redo first).
    pub fn redo_kinds(&self) -> impl Iterator<Item = HistoryKind> + '_ {
        self.redo.iter().rev().map(|c| c.kind())
    }

    /// `true` once the capacity or the byte cap has dropped a step: the
    /// oldest state left is no longer the document's start.
    pub fn dropped_steps(&self) -> bool {
        self.dropped
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
                self.dropped = true;
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

    /// Whether the next [`undo`](UndoStack::undo) may change the selection,
    /// if there is one.
    fn next_undo_changes_selection(&self) -> Option<bool> {
        self.undo.last().map(|cmd| cmd.changes_selection())
    }

    /// Whether the next [`redo`](UndoStack::redo) may change the selection,
    /// if there is one.
    fn next_redo_changes_selection(&self) -> Option<bool> {
        self.redo.last().map(|cmd| cmd.changes_selection())
    }

    /// One step of a jump: undo (`back`) or redo, but a step that fails
    /// goes back to the stack it came from instead of being dropped
    /// (docs/specs/history-panel.md AC-12).
    fn step_keeping(&mut self, doc: &mut Document, back: bool) -> Result<bool, DocumentError> {
        let from = if back { &mut self.undo } else { &mut self.redo };
        let Some(mut cmd) = from.pop() else {
            return Ok(false);
        };
        let before = cmd.undo_bytes();
        let done = if back {
            cmd.revert(doc)
        } else {
            cmd.apply(doc)
        };
        self.bytes = self.bytes - before + cmd.undo_bytes();
        let to = match (&done, back) {
            (Err(_), true) | (Ok(()), false) => &mut self.undo,
            (Err(_), false) | (Ok(()), true) => &mut self.redo,
        };
        to.push(cmd);
        done.map(|()| true)
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
    /// Bumped by every apply, undo or redo that may have changed the
    /// selection; see [`selection_generation`](CommandBus::selection_generation).
    selection_generation: u32,
}

impl CommandBus {
    /// Creates a bus for `document` with a default-capacity history.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            history: UndoStack::new(),
            damage: Damage::Full,
            live_merge: None,
            selection_generation: 0,
        }
    }

    /// A counter that changes whenever an applied, undone or redone command
    /// may have changed `document.selection` (a failed one counts too), so
    /// a view can keep what it derived from the selection until it moves.
    /// It wraps around; compare it for equality only. Code that writes
    /// `document.selection` directly, outside a command, is not tracked.
    pub fn selection_generation(&self) -> u32 {
        self.selection_generation
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
        self.record_selection(Some(cmd.changes_selection()), applied.is_ok());
        applied?;
        self.history.push(cmd);
        Ok(())
    }

    /// Undoes the last command. Returns `false` if nothing to undo.
    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        self.live_merge = None;
        let damage = self.history.next_undo_damage();
        let selection = self.history.next_undo_changes_selection();
        let undone = self.history.undo(&mut self.document);
        self.record(damage, undone.is_ok());
        self.record_selection(selection, undone.is_ok());
        undone
    }

    /// Redoes the last undone command. Returns `false` if nothing to redo.
    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        self.live_merge = None;
        let damage = self.history.next_redo_damage();
        let selection = self.history.next_redo_changes_selection();
        let redone = self.history.redo(&mut self.document);
        self.record(damage, redone.is_ok());
        self.record_selection(selection, redone.is_ok());
        redone
    }

    /// Moves through the history by `delta` steps (`-n` undoes `n`, `+n`
    /// redoes `n`) as that many [`undo`](Self::undo) / [`redo`](Self::redo)
    /// calls would, clamped to the steps there are, and returns how many it
    /// moved. When a step fails the jump stops at the last state it
    /// reached, the failed step stays where it was, and the error comes
    /// back with the steps moved before it (docs/specs/history-panel.md
    /// AC-05, AC-12).
    pub fn jump(&mut self, delta: i64) -> Result<usize, (usize, DocumentError)> {
        self.live_merge = None;
        let back = delta < 0;
        let steps = delta.unsigned_abs();
        let mut moved = 0usize;
        while (moved as u64) < steps {
            let (damage, selection) = if back {
                (
                    self.history.next_undo_damage(),
                    self.history.next_undo_changes_selection(),
                )
            } else {
                (
                    self.history.next_redo_damage(),
                    self.history.next_redo_changes_selection(),
                )
            };
            let step = self.history.step_keeping(&mut self.document, back);
            self.record(damage, step.is_ok());
            self.record_selection(selection, step.is_ok());
            match step {
                Ok(true) => moved += 1,
                Ok(false) => break,
                Err(e) => return Err((moved, e)),
            }
        }
        Ok(moved)
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

    /// Moves the selection generation after a command ran: when it may
    /// change the selection, or failed (and may have changed anything).
    /// `None` means there was no command to run.
    fn record_selection(&mut self, changes_selection: Option<bool>, ok: bool) {
        if let Some(changes) = changes_selection
            && (changes || !ok)
        {
            self.selection_generation = self.selection_generation.wrapping_add(1);
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
    fn selection_generation_changes_on_select_deselect_undo_and_redo() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        let mut seen = vec![bus.selection_generation()];
        let mut step = |bus: &mut CommandBus, what: &str| {
            let g = bus.selection_generation();
            assert!(!seen.contains(&g), "{what} moves the generation");
            seen.push(g);
        };
        let mask = crate::selection::SelectionMask::rectangle(8, 8, Rect::new(1, 1, 3, 3));
        bus.apply(Box::new(SetSelection::replace(mask))).unwrap();
        step(&mut bus, "select");
        bus.apply(Box::new(SetSelection::clear())).unwrap();
        step(&mut bus, "deselect");
        assert!(bus.undo().unwrap());
        step(&mut bus, "undo of deselect");
        assert!(bus.redo().unwrap());
        step(&mut bus, "redo of deselect");
        assert_eq!(bus.jump(-2), Ok(2));
        step(&mut bus, "jump back over both");
    }

    #[test]
    fn selection_generation_stays_on_pixel_edits_and_their_undo() {
        let mut bus = CommandBus::new(Document::new(8, 8).unwrap());
        let mask = crate::selection::SelectionMask::rectangle(8, 8, Rect::new(1, 1, 3, 3));
        bus.apply(Box::new(SetSelection::replace(mask))).unwrap();
        let g = bus.selection_generation();
        // A brush stroke: several pixel edits merged into one undo step.
        for x in 1..4 {
            let mut dab = crate::ImageBuffer::new_transparent(1, 1);
            dab.set_pixel(0, 0, Color::WHITE);
            let cmd = SetPixels::new(0, Rect::new(x, 1, 1, 1), dab).with_stroke(7);
            bus.apply(Box::new(cmd)).unwrap();
        }
        assert_eq!(bus.history.undo_depth(), 2, "one step for the stroke");
        assert_eq!(bus.selection_generation(), g, "stroke keeps it");
        assert!(bus.undo().unwrap());
        assert!(bus.redo().unwrap());
        assert_eq!(
            bus.selection_generation(),
            g,
            "undo / redo of a stroke keep it"
        );
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
