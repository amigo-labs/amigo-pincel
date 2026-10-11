//! Layer model.

use crate::color::BlendMode;
use crate::document::buffer::ImageBuffer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The kind of content a layer holds. Phase 1 supports raster only (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayerKind {
    /// A standard pixel (raster) layer.
    #[default]
    Raster,
}

/// A single image layer (spec §5.1).
///
/// `pixels` always matches the document's canvas dimensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// Stable unique identifier.
    pub id: Uuid,
    /// User-editable display name.
    pub name: String,
    /// Layer content kind.
    pub kind: LayerKind,
    /// Straight-alpha RGBA8 pixels, canvas-sized.
    pub pixels: ImageBuffer,
    /// Opacity in `[0.0, 1.0]`, applied before the blend mode.
    pub opacity: f32,
    /// Blend mode used when compositing onto the layers below.
    pub blend_mode: BlendMode,
    /// Whether the layer participates in the composite.
    pub visible: bool,
    /// Whether pixel edits are blocked.
    pub locked: bool,
}

impl Layer {
    /// Creates a fully transparent raster layer of the given size.
    pub fn new_transparent(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind: LayerKind::Raster,
            pixels: ImageBuffer::new_transparent(width, height),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            locked: false,
        }
    }

    /// Creates a raster layer backed by an existing pixel buffer.
    pub fn from_pixels(name: impl Into<String>, pixels: ImageBuffer) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind: LayerKind::Raster,
            pixels,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            locked: false,
        }
    }

    /// Builder: sets opacity, clamped to `[0.0, 1.0]`.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Builder: sets the blend mode.
    pub fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = mode;
        self
    }

    /// Builder: sets visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_transparent_has_default_properties() {
        let l = Layer::new_transparent("Layer 1", 8, 8);
        assert_eq!(l.opacity, 1.0);
        assert_eq!(l.blend_mode, BlendMode::Normal);
        assert!(l.visible);
        assert!(!l.locked);
        assert_eq!(l.kind, LayerKind::Raster);
        assert_eq!(l.pixels.width(), 8);
    }

    #[test]
    fn with_opacity_clamps_out_of_range() {
        assert_eq!(
            Layer::new_transparent("l", 1, 1).with_opacity(2.0).opacity,
            1.0
        );
        assert_eq!(
            Layer::new_transparent("l", 1, 1).with_opacity(-0.5).opacity,
            0.0
        );
    }

    #[test]
    fn distinct_layers_have_distinct_ids() {
        let a = Layer::new_transparent("a", 1, 1);
        let b = Layer::new_transparent("b", 1, 1);
        assert_ne!(a.id, b.id);
    }
}
