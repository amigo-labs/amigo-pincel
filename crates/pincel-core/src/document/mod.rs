//! Document model: the in-memory representation of an Aseprite-equivalent
//! sprite. See `docs/specs/pincel.md` §3.

mod cel;
mod cel_map;
mod color;
mod color_mode;
mod frame;
mod layer;
mod palette;
mod slice;
mod tag;
mod tileset;

pub use cel::{Cel, CelData, PixelBuffer, TileRef};
pub use cel_map::{CelKey, CelMap};
pub use color::Rgba;
pub use color_mode::ColorMode;
pub use frame::{Frame, FrameIndex};
pub use layer::{BlendMode, Layer, LayerId, LayerKind};
pub use palette::{Palette, PaletteEntry};
pub use slice::{Slice, SliceId, SliceKey};
pub use tag::{Tag, TagDirection};
pub use tileset::{PathRef, TileImage, Tileset, TilesetId};

use std::collections::BTreeSet;

use crate::error::DocumentError;

/// Free-form metadata associated with a sprite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Pixel aspect ratio as `(width, height)`. `(1, 1)` is square pixels;
    /// `(0, 0)` is treated as `(1, 1)` by readers, matching older `.aseprite`
    /// files where the field was unset.
    pub pixel_ratio: (u8, u8),
}

/// The Pincel document — isomorphic to the Aseprite v1.3 file format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    /// Z-order: index `0` is the bottom-most layer.
    pub layers: Vec<Layer>,
    /// Sequential, in playback order.
    pub frames: Vec<Frame>,
    pub palette: Palette,
    pub tilesets: Vec<Tileset>,
    pub tags: Vec<Tag>,
    pub slices: Vec<Slice>,
    pub metadata: Metadata,
}

impl Sprite {
    /// Start a builder for a sprite with the given canvas dimensions.
    pub fn builder(width: u32, height: u32) -> SpriteBuilder {
        SpriteBuilder::new(width, height)
    }
}

/// Fluent builder for [`Sprite`]. Validates non-zero canvas dimensions and
/// uniqueness of layer / tileset / slice ids on `build`.
#[derive(Debug, Clone)]
pub struct SpriteBuilder {
    width: u32,
    height: u32,
    color_mode: ColorMode,
    layers: Vec<Layer>,
    frames: Vec<Frame>,
    palette: Palette,
    tilesets: Vec<Tileset>,
    tags: Vec<Tag>,
    slices: Vec<Slice>,
    metadata: Metadata,
}

impl SpriteBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            color_mode: ColorMode::default(),
            layers: Vec::new(),
            frames: Vec::new(),
            palette: Palette::default(),
            tilesets: Vec::new(),
            tags: Vec::new(),
            slices: Vec::new(),
            metadata: Metadata::default(),
        }
    }

    pub fn color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    pub fn add_layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn add_frame(mut self, frame: Frame) -> Self {
        self.frames.push(frame);
        self
    }

    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    pub fn add_tileset(mut self, tileset: Tileset) -> Self {
        self.tilesets.push(tileset);
        self
    }

    pub fn add_tag(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
    }

    pub fn add_slice(mut self, slice: Slice) -> Self {
        self.slices.push(slice);
        self
    }

    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Consume the builder and return a validated [`Sprite`].
    pub fn build(self) -> Result<Sprite, DocumentError> {
        if self.width == 0 || self.height == 0 {
            return Err(DocumentError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        ensure_unique(self.layers.iter().map(|l| l.id.0), "layer")?;
        ensure_unique(self.tilesets.iter().map(|t| t.id.0), "tileset")?;
        ensure_unique(self.slices.iter().map(|s| s.id.0), "slice")?;

        Ok(Sprite {
            width: self.width,
            height: self.height,
            color_mode: self.color_mode,
            layers: self.layers,
            frames: self.frames,
            palette: self.palette,
            tilesets: self.tilesets,
            tags: self.tags,
            slices: self.slices,
            metadata: self.metadata,
        })
    }
}

fn ensure_unique<I: Iterator<Item = u32>>(ids: I, kind: &'static str) -> Result<(), DocumentError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(DocumentError::DuplicateId { kind, id });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_constructs_minimal_sprite() {
        let s = Sprite::builder(64, 32)
            .build()
            .expect("minimal sprite should build");
        assert_eq!(s.width, 64);
        assert_eq!(s.height, 32);
        assert_eq!(s.color_mode, ColorMode::Rgba);
        assert!(s.layers.is_empty());
        assert!(s.frames.is_empty());
        assert!(s.palette.is_empty());
    }

    #[test]
    fn builder_rejects_zero_dimensions() {
        assert_eq!(
            Sprite::builder(0, 16).build(),
            Err(DocumentError::InvalidDimensions {
                width: 0,
                height: 16
            })
        );
        assert_eq!(
            Sprite::builder(16, 0).build(),
            Err(DocumentError::InvalidDimensions {
                width: 16,
                height: 0
            })
        );
    }

    #[test]
    fn builder_detects_duplicate_layer_ids() {
        let err = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(1), "bg"))
            .add_layer(Layer::image(LayerId::new(1), "fg"))
            .build()
            .expect_err("duplicate layer id should fail to build");
        assert_eq!(
            err,
            DocumentError::DuplicateId {
                kind: "layer",
                id: 1
            }
        );
    }

    #[test]
    fn builder_detects_duplicate_tileset_ids() {
        let make_tileset = |id| Tileset {
            id: TilesetId::new(id),
            name: "t".into(),
            tile_size: (16, 16),
            tiles: Vec::new(),
            base_index: 1,
            external_file: None,
        };
        let err = Sprite::builder(8, 8)
            .add_tileset(make_tileset(3))
            .add_tileset(make_tileset(3))
            .build()
            .expect_err("duplicate tileset id should fail to build");
        assert_eq!(
            err,
            DocumentError::DuplicateId {
                kind: "tileset",
                id: 3
            }
        );
    }

    #[test]
    fn builder_assembles_full_document() {
        let tileset = Tileset {
            id: TilesetId::new(1),
            name: "tiles".into(),
            tile_size: (16, 16),
            tiles: Vec::new(),
            base_index: 1,
            external_file: None,
        };
        let s = Sprite::builder(32, 32)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::tilemap(LayerId::new(1), "tiles", TilesetId::new(1)))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(120))
            .palette(Palette::from_entries(vec![
                PaletteEntry::new(Rgba::TRANSPARENT),
                PaletteEntry::with_name(Rgba::WHITE, "ink"),
            ]))
            .add_tileset(tileset)
            .add_tag(Tag::new("idle", FrameIndex::new(0), FrameIndex::new(1)))
            .build()
            .expect("full sprite should build");
        assert_eq!(s.layers.len(), 2);
        assert_eq!(s.frames.len(), 2);
        assert_eq!(s.palette.len(), 2);
        assert_eq!(s.tilesets.len(), 1);
        assert_eq!(s.tags.len(), 1);
        assert_eq!(
            s.color_mode,
            ColorMode::Indexed {
                transparent_index: 0
            }
        );
    }
}
