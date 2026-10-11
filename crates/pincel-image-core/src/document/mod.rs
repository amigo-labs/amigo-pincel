//! Document model: the in-memory representation of one open image (spec §3).

mod buffer;
mod canvas;
mod layer;

pub use buffer::ImageBuffer;
pub use canvas::{CanvasSize, MAX_CANVAS_DIM, MIN_CANVAS_DIM};
pub use layer::{Layer, LayerKind};

use crate::error::DocumentError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Maximum number of layers a document may hold (spec §3.2).
pub const MAX_LAYERS: usize = 999;

/// sRGB is the only color profile in Phase 1 (spec §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorProfile {
    /// Standard sRGB.
    #[default]
    Srgb,
}

/// Document-level metadata (spec §3.1).
///
/// Timestamps are Unix seconds. Core is platform-independent and does not read
/// the system clock (which is unavailable under `wasm32-unknown-unknown`); the
/// platform layer is responsible for setting `created_at`/`modified_at`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Dots per inch, default 96.0.
    pub dpi: f32,
    /// Color profile (sRGB only in Phase 1).
    pub color_profile: ColorProfile,
    /// Creation time, Unix seconds (0 until set by the platform layer).
    pub created_at: u64,
    /// Last modification time, Unix seconds.
    pub modified_at: u64,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            color_profile: ColorProfile::Srgb,
            created_at: 0,
            modified_at: 0,
        }
    }
}

/// One open image file with its layers, selection, and metadata (spec §3.1).
///
/// History (undo stack) is stored separately by the [`crate::command`] layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Stable unique identifier.
    pub id: Uuid,
    /// Display title (filename or "Untitled N").
    pub title: String,
    /// Canvas dimensions.
    pub canvas: CanvasSize,
    /// Layers ordered bottom (index 0) to top.
    ///
    /// Private so the ≥1-layer invariant, active-layer validity, and 999-layer
    /// limit can only be changed through the methods below. Read access is via
    /// [`Document::layers`].
    pub(crate) layers: Vec<Layer>,
    /// Index of the active layer.
    active_layer: usize,
    /// Optional global selection mask; `None` means everything is selected.
    pub selection: Option<crate::selection::SelectionMask>,
    /// Document metadata.
    pub metadata: DocumentMetadata,
}

impl Document {
    /// Creates a new document with a single transparent layer.
    ///
    /// Returns `InvalidCanvasSize` if either dimension is out of range.
    pub fn new(width: u32, height: u32) -> Result<Self, DocumentError> {
        let canvas = CanvasSize::new(width, height)?;
        let layer = Layer::new_transparent("Layer 1", width, height);
        Ok(Self {
            id: Uuid::new_v4(),
            title: "Untitled 1".to_string(),
            canvas,
            layers: vec![layer],
            active_layer: 0,
            selection: None,
            metadata: DocumentMetadata::default(),
        })
    }

    /// Creates a document holding a single raster layer from `pixels`.
    ///
    /// The canvas matches the buffer's dimensions. Returns `InvalidCanvasSize`
    /// if those dimensions are out of range.
    pub fn from_pixels(pixels: ImageBuffer) -> Result<Self, DocumentError> {
        let canvas = CanvasSize::new(pixels.width(), pixels.height())?;
        Ok(Self {
            id: Uuid::new_v4(),
            title: "Untitled 1".to_string(),
            canvas,
            layers: vec![Layer::from_pixels("Background", pixels)],
            active_layer: 0,
            selection: None,
            metadata: DocumentMetadata::default(),
        })
    }

    /// Read-only view of the layers, ordered bottom (index 0) to top.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Number of layers (always ≥ 1).
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Index of the active layer.
    pub fn active_layer_index(&self) -> usize {
        self.active_layer
    }

    /// Sets the active layer index.
    ///
    /// Returns `LayerIndexOutOfBounds` if `index >= layers.len()`.
    pub fn set_active_layer(&mut self, index: usize) -> Result<(), DocumentError> {
        if index >= self.layers.len() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index,
                len: self.layers.len(),
            });
        }
        self.active_layer = index;
        Ok(())
    }

    /// Reference to the active layer (always present; a document has ≥1 layer).
    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active_layer]
    }

    /// Mutable reference to the active layer.
    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active_layer]
    }

    /// Inserts a layer at `index`, shifting later layers up.
    ///
    /// The inserted layer becomes active. Returns `LayerLimit` at 999 layers,
    /// or `LayerIndexOutOfBounds` if `index > layers.len()`.
    pub fn insert_layer(&mut self, index: usize, layer: Layer) -> Result<(), DocumentError> {
        if self.layers.len() >= MAX_LAYERS {
            return Err(DocumentError::LayerLimit { max: MAX_LAYERS });
        }
        if index > self.layers.len() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index,
                len: self.layers.len(),
            });
        }
        self.layers.insert(index, layer);
        self.active_layer = index;
        Ok(())
    }

    /// Adds a new transparent layer directly above the active layer.
    pub fn add_layer(&mut self, name: impl Into<String>) -> Result<(), DocumentError> {
        let layer = Layer::new_transparent(name, self.canvas.width(), self.canvas.height());
        self.insert_layer(self.active_layer + 1, layer)
    }

    /// Removes the layer at `index`.
    ///
    /// Returns `LastLayer` if it is the only layer (a document always keeps
    /// ≥1 layer, spec §3.2), or `LayerIndexOutOfBounds` for a bad index.
    /// The returned `Layer` lets callers (e.g. undo) restore it.
    pub fn remove_layer(&mut self, index: usize) -> Result<Layer, DocumentError> {
        if index >= self.layers.len() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index,
                len: self.layers.len(),
            });
        }
        if self.layers.len() == 1 {
            return Err(DocumentError::LastLayer);
        }
        let removed = self.layers.remove(index);
        if self.active_layer >= self.layers.len() {
            self.active_layer = self.layers.len() - 1;
        }
        Ok(removed)
    }

    /// Moves the layer at `from` to `to`, shifting others to fill the gap.
    pub fn move_layer(&mut self, from: usize, to: usize) -> Result<(), DocumentError> {
        let len = self.layers.len();
        if from >= len || to >= len {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index: from.max(to),
                len,
            });
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        self.active_layer = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_one_layer_and_valid_canvas() {
        let doc = Document::new(640, 480).unwrap();
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.canvas.width(), 640);
        assert_eq!(doc.canvas.height(), 480);
        assert_eq!(doc.active_layer_index(), 0);
    }

    #[test]
    fn new_document_rejects_invalid_canvas() {
        assert!(Document::new(0, 100).is_err());
    }

    #[test]
    fn add_layer_inserts_above_active_and_activates_it() {
        let mut doc = Document::new(10, 10).unwrap();
        doc.add_layer("Layer 2").unwrap();
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[1].name, "Layer 2");
    }

    #[test]
    fn remove_last_layer_is_blocked() {
        let mut doc = Document::new(10, 10).unwrap();
        assert_eq!(doc.remove_layer(0), Err(DocumentError::LastLayer));
    }

    #[test]
    fn remove_layer_adjusts_active_index() {
        let mut doc = Document::new(10, 10).unwrap();
        doc.add_layer("Layer 2").unwrap(); // active = 1
        doc.remove_layer(1).unwrap();
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.active_layer_index(), 0);
    }

    #[test]
    fn move_layer_reorders() {
        let mut doc = Document::new(10, 10).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let bottom_id = doc.layers[0].id;
        doc.move_layer(0, 1).unwrap();
        assert_eq!(doc.layers[1].id, bottom_id);
    }

    #[test]
    fn set_active_layer_out_of_bounds_errors() {
        let mut doc = Document::new(10, 10).unwrap();
        assert!(doc.set_active_layer(5).is_err());
    }
}
