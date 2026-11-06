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

/// Like [`snapshot`], but moves the layers out of `doc` and puts `layers`
/// in their place, so the prior stack is not copied.
pub(crate) fn take_snapshot(doc: &mut Document, layers: Vec<Layer>) -> DocSnapshot {
    DocSnapshot {
        layers: std::mem::replace(&mut doc.layers, layers),
        canvas: doc.canvas,
        selection: doc.selection.clone(),
        active: doc.active_layer_index(),
    }
}

impl DocSnapshot {
    /// Bytes of pixel and selection data held.
    pub(crate) fn bytes(&self) -> usize {
        layers_bytes(&self.layers) + mask_bytes(self.selection.as_ref())
    }
}

/// Bytes of pixel data in `layers`.
pub(crate) fn layers_bytes(layers: &[Layer]) -> usize {
    layers.iter().map(|l| l.pixels.data().len()).sum()
}

/// Bytes of coverage data in `mask`.
pub(crate) fn mask_bytes(mask: Option<&SelectionMask>) -> usize {
    mask.map_or(0, |m| m.data().len())
}

/// Restores a previously captured document state.
pub(crate) fn restore(doc: &mut Document, snap: DocSnapshot) -> Result<(), DocumentError> {
    doc.layers = snap.layers;
    doc.canvas = snap.canvas;
    doc.selection = snap.selection;
    doc.set_active_layer(snap.active)
}
