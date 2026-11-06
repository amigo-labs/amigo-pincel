//! Document model: the in-memory representation of one open image (spec §3).

mod buffer;
mod canvas;
mod layer;
pub(crate) mod tree;

pub use buffer::ImageBuffer;
pub use canvas::{CanvasSize, MAX_CANVAS_DIM, MIN_CANVAS_DIM};
pub use layer::{Layer, LayerKind};
pub use tree::LayerPlacement;

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
    pub(crate) active_layer: usize,
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
    /// `LayerIndexOutOfBounds` if `index > layers.len()`, or `LayerTreeBroken`
    /// if the layer's `parent` does not fit the place it is inserted at.
    pub fn insert_layer(&mut self, index: usize, layer: Layer) -> Result<(), DocumentError> {
        self.insert_subtree(index, vec![layer])
    }

    /// Adds a new transparent layer directly above the active layer: above
    /// its whole subtree for a group, inside the active layer's group, as
    /// the `AddLayer` command does.
    pub fn add_layer(&mut self, name: impl Into<String>) -> Result<(), DocumentError> {
        let parent = self.layers[self.active_layer].parent;
        let layer = Layer::new_transparent(name, self.canvas.width(), self.canvas.height())
            .with_parent(parent);
        self.insert_layer(self.subtree_end(self.active_layer), layer)
    }

    /// Removes the layer at `index`.
    ///
    /// Returns `LastLayer` if no raster layer would remain (a document always
    /// keeps ≥1 raster layer, spec §3.2 and docs/specs/image-layer-groups.md
    /// AC-20), `LayerTreeBroken` for a group that still has children, or
    /// `LayerIndexOutOfBounds` for a bad index. The returned `Layer` lets
    /// callers (e.g. undo) restore it.
    pub fn remove_layer(&mut self, index: usize) -> Result<Layer, DocumentError> {
        if index < self.layers.len() && self.subtree_end(index) != index + 1 {
            return Err(DocumentError::LayerTreeBroken);
        }
        let mut removed = self.remove_subtree(index)?;
        Ok(removed.remove(0))
    }

    /// Moves the layer at `from` to `to`, shifting others to fill the gap.
    ///
    /// Returns `LayerTreeBroken` (and changes nothing) when the move would
    /// split a group's run of descendants or leave a child outside its group.
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
        if !self.is_well_formed() {
            let layer = self.layers.remove(to);
            self.layers.insert(from, layer);
            return Err(DocumentError::LayerTreeBroken);
        }
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

    /// A document of `[Layer 1, G, c0, c1]`, `c0` / `c1` inside group `G`.
    fn with_group() -> (Document, Uuid) {
        let mut doc = Document::new(1, 1).unwrap();
        let g = Layer::new_group("G");
        let gid = g.id;
        doc.insert_layer(1, g).unwrap();
        for n in 0..2 {
            let c = Layer::new_transparent(format!("c{n}"), 1, 1).with_parent(Some(gid));
            doc.insert_layer(doc.layer_count(), c).unwrap();
        }
        (doc, gid)
    }

    #[test]
    fn add_layer_with_a_group_active_goes_above_its_subtree() {
        let (mut doc, _) = with_group();
        doc.set_active_layer(1).unwrap();
        doc.add_layer("New").unwrap();
        assert_eq!(doc.active_layer_index(), 4);
        assert_eq!(doc.layers[4].name, "New");
        assert_eq!(doc.layers[4].parent, None);
    }

    #[test]
    fn add_layer_with_a_child_active_stays_in_its_group() {
        let (mut doc, gid) = with_group();
        doc.set_active_layer(2).unwrap();
        doc.add_layer("New").unwrap();
        assert_eq!(doc.active_layer_index(), 3);
        assert_eq!(doc.layers[3].name, "New");
        assert_eq!(doc.layers[3].parent, Some(gid));
        assert_eq!(doc.layers[4].name, "c1");
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
