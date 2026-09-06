//! The `SetSelection` command (spec §7.3).
//!
//! Replaces the document's selection mask with a new one (or clears it),
//! capturing the prior selection on first apply so undo restores it exactly.
//! Callers compute the resulting mask — combining a freshly drawn shape with the
//! existing selection via [`crate::selection::apply_mode`], or applying a
//! modifier such as Invert / Expand — and hand the final mask to this command.

use super::Command;
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
