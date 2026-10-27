//! The `SetSelection` command (spec §7.3).
//!
//! Replaces the document's selection mask with a new one (or clears it),
//! capturing the prior selection on first apply so undo restores it exactly.
//! Callers compute the resulting mask — combining a freshly drawn shape with the
//! existing selection via [`crate::selection::apply_mode`], or applying a
//! modifier such as Invert / Expand — and hand the final mask to this command.

use super::{Command, SetPixels};
use crate::document::Document;
use crate::error::DocumentError;
use crate::selection::SelectionMask;
use std::any::Any;

/// Sets (or clears) the document's selection (spec §7.3 `SetSelection`).
pub struct SetSelection {
    /// The selection to install; `None` clears it (Deselect).
    after: Option<SelectionMask>,
    /// The prior selection, captured on first apply for undo.
    before: Option<SelectionMask>,
    /// Whether `before` has been captured yet.
    captured: bool,
    label: &'static str,
}

impl SetSelection {
    /// Replaces the selection with `mask`.
    pub fn replace(mask: SelectionMask) -> Self {
        Self {
            after: Some(mask),
            before: None,
            captured: false,
            label: "Selection",
        }
    }

    /// Clears the selection (Deselect, spec §8.4).
    pub fn clear() -> Self {
        Self {
            after: None,
            before: None,
            captured: false,
            label: "Deselect",
        }
    }

    /// Sets the history label (e.g. "Invert Selection").
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = label;
        self
    }
}

impl Command for SetSelection {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if !self.captured {
            self.before = doc.selection.clone();
            self.captured = true;
        }
        doc.selection = self.after.clone();
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.selection = self.before.clone();
        Ok(())
    }

    fn label(&self) -> &str {
        self.label
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Moves the selected pixels and the outline over them together (spec §5.4).
///
/// The selection follows its content, as paint.net's Move Selected Pixels
/// does: without that, a second drag would lift the transparency the first
/// one left behind instead of the patch it moved. Pixels and mask are one
/// undo step, so undo puts both back.
pub struct MoveSelection {
    /// The pixel edit from [`crate::Move::translate_selection`].
    pixels: SetPixels,
    dx: i32,
    dy: i32,
    /// The selection before the move, captured on first apply for undo and
    /// for recomputing the shift on redo.
    before: Option<SelectionMask>,
    captured: bool,
}

impl MoveSelection {
    /// Pairs a translation of the selected pixels with the same shift of the
    /// mask.
    pub fn new(pixels: SetPixels, dx: i32, dy: i32) -> Self {
        Self {
            pixels,
            dx,
            dy,
            before: None,
            captured: false,
        }
    }
}

impl Command for MoveSelection {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if !self.captured {
            self.before = doc.selection.clone();
            self.captured = true;
        }
        self.pixels.apply(doc)?;
        // Always shift the *captured* mask, so a redo lands where the first
        // apply did instead of shifting twice.
        doc.selection = self
            .before
            .as_ref()
            .map(|mask| mask.translated(self.dx, self.dy));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.pixels.revert(doc)?;
        doc.selection = self.before.clone();
        Ok(())
    }

    fn label(&self) -> &str {
        "Move Selection"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    #[test]
    fn set_selection_round_trip_restores_none() {
        let mut doc = Document::new(8, 8).unwrap();
        assert!(doc.selection.is_none());
        let mask = SelectionMask::rectangle(8, 8, Rect::new(1, 1, 3, 3));
        let mut cmd = SetSelection::replace(mask);
        cmd.apply(&mut doc).unwrap();
        assert!(doc.selection.is_some());
        cmd.revert(&mut doc).unwrap();
        assert!(doc.selection.is_none());
    }

    #[test]
    fn set_selection_replaces_existing_and_undo_restores_it() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.selection = Some(SelectionMask::rectangle(8, 8, Rect::new(0, 0, 2, 2)));
        let first = doc.selection.clone();
        let mut cmd = SetSelection::replace(SelectionMask::rectangle(8, 8, Rect::new(4, 4, 2, 2)));
        cmd.apply(&mut doc).unwrap();
        assert_ne!(doc.selection, first);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.selection, first);
    }

    #[test]
    fn clear_deselects_and_undo_restores() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.selection = Some(SelectionMask::rectangle(8, 8, Rect::new(0, 0, 4, 4)));
        let before = doc.selection.clone();
        let mut cmd = SetSelection::clear();
        cmd.apply(&mut doc).unwrap();
        assert!(doc.selection.is_none());
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.selection, before);
    }
}

#[cfg(test)]
mod move_selection_tests {
    use super::*;
    use crate::color::Color;
    use crate::geometry::Rect;
    use crate::tools::Move;

    /// A 4×1 strip with one white pixel at x = 0, selected.
    fn doc_with_selected_pixel() -> Document {
        let mut doc = Document::new(4, 1).unwrap();
        doc.layers[0].pixels.set_pixel(0, 0, Color::WHITE);
        doc.selection = Some(SelectionMask::rectangle(4, 1, Rect::new(0, 0, 1, 1)));
        doc
    }

    fn move_by(doc: &Document, dx: i32) -> MoveSelection {
        let pixels = Move.translate_selection(0, dx, 0, doc).unwrap();
        MoveSelection::new(pixels, dx, 0)
    }

    #[test]
    fn the_outline_follows_the_pixels() {
        let mut doc = doc_with_selected_pixel();
        let mut cmd = move_by(&doc, 2);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
        let mask = doc.selection.as_ref().unwrap();
        assert_eq!(mask.coverage(0, 0), 0);
        assert_eq!(mask.coverage(2, 0), 255);
    }

    #[test]
    fn a_second_drag_carries_the_patch_further() {
        // The bug this exists for: with the outline left behind, the second
        // drag would lift the transparency at x = 0 instead.
        let mut doc = doc_with_selected_pixel();
        move_by(&doc, 1).apply(&mut doc).unwrap();
        move_by(&doc, 1).apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
        assert_eq!(doc.selection.as_ref().unwrap().coverage(2, 0), 255);
    }

    #[test]
    fn undo_restores_both_the_pixels_and_the_outline() {
        let mut doc = doc_with_selected_pixel();
        let mut cmd = move_by(&doc, 2);
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(doc.selection.as_ref().unwrap().coverage(0, 0), 255);
    }

    #[test]
    fn redo_lands_where_the_first_apply_did() {
        let mut doc = doc_with_selected_pixel();
        let mut cmd = move_by(&doc, 2);
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 0), Some(Color::WHITE));
        assert_eq!(doc.selection.as_ref().unwrap().coverage(2, 0), 255);
    }
}
