//! Layers: image, tilemap, and group. See `docs/specs/pincel.md` §3.2.

use super::tileset::TilesetId;

/// Stable identifier for a layer within a sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub u32);

impl LayerId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Aseprite blend modes. Numeric values match the `.aseprite` file format
/// (see <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u16)]
pub enum BlendMode {
    #[default]
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Darken = 4,
    Lighten = 5,
    ColorDodge = 6,
    ColorBurn = 7,
    HardLight = 8,
    SoftLight = 9,
    Difference = 10,
    Exclusion = 11,
    Hue = 12,
    Saturation = 13,
    Color = 14,
    Luminosity = 15,
    Addition = 16,
    Subtract = 17,
    Divide = 18,
}

/// What kind of content a layer holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerKind {
    Image,
    Tilemap { tileset_id: TilesetId },
    Group,
}

/// A layer in the sprite. Z-order is by index in `Sprite::layers`
/// (index `0` is the bottom-most layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub editable: bool,
    pub blend_mode: BlendMode,
    pub opacity: u8,
    /// `Some(parent)` if this layer is nested inside a group.
    pub parent: Option<LayerId>,
}

impl Layer {
    /// Create an opaque, visible image layer.
    pub fn image(id: LayerId, name: impl Into<String>) -> Self {
        Self::with_kind(id, name, LayerKind::Image)
    }

    /// Create a tilemap layer bound to the given tileset.
    pub fn tilemap(id: LayerId, name: impl Into<String>, tileset_id: TilesetId) -> Self {
        Self::with_kind(id, name, LayerKind::Tilemap { tileset_id })
    }

    /// Create a group layer (a folder for nesting other layers).
    pub fn group(id: LayerId, name: impl Into<String>) -> Self {
        Self::with_kind(id, name, LayerKind::Group)
    }

    fn with_kind(id: LayerId, name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            visible: true,
            editable: true,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            parent: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_layer_has_default_appearance() {
        let l = Layer::image(LayerId::new(0), "bg");
        assert_eq!(l.id, LayerId::new(0));
        assert_eq!(l.name, "bg");
        assert_eq!(l.kind, LayerKind::Image);
        assert!(l.visible);
        assert!(l.editable);
        assert_eq!(l.blend_mode, BlendMode::Normal);
        assert_eq!(l.opacity, 255);
        assert!(l.parent.is_none());
    }

    #[test]
    fn tilemap_layer_carries_tileset_id() {
        let l = Layer::tilemap(LayerId::new(2), "tiles", TilesetId::new(7));
        assert_eq!(
            l.kind,
            LayerKind::Tilemap {
                tileset_id: TilesetId::new(7)
            }
        );
    }

    #[test]
    fn blend_mode_numeric_values_match_aseprite() {
        assert_eq!(BlendMode::Normal as u16, 0);
        assert_eq!(BlendMode::Multiply as u16, 1);
        assert_eq!(BlendMode::Divide as u16, 18);
    }
}
