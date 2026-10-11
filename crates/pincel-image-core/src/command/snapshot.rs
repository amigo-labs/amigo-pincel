//! Full-document snapshot for reversing structural/lossy commands
//! (merge, flatten, scale, crop, layer rotation).
//!
//! These commands change the layer stack or canvas irreversibly, so undo
//! restores a verbatim copy of the prior state. Structural edits are
//! infrequent; the clone cost is acceptable (spec §7.1).

use crate::document::{CanvasSize, Document, Layer};
use crate::error::DocumentError;
use crate::selection::SelectionMask;

/// The prior document state, captured on first apply for exact reversal.
pub(crate) struct DocSnapshot {
    layers: Vec<Layer>,
    canvas: CanvasSize,
    selection: Option<SelectionMask>,
    active: usize,
}

/// Captures the current layers, canvas size, selection, and active index.
pub(crate) fn snapshot(doc: &Document) -> DocSnapshot {
    DocSnapshot {
        layers: doc.layers().to_vec(),
        canvas: doc.canvas,
        selection: doc.selection.clone(),
        active: doc.active_layer_index(),
    }
}

/// Restores a previously captured document state.
pub(crate) fn restore(doc: &mut Document, snap: DocSnapshot) -> Result<(), DocumentError> {
    doc.layers = snap.layers;
    doc.canvas = snap.canvas;
    doc.selection = snap.selection;
    doc.set_active_layer(snap.active)
}
