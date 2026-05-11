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
use crate::geometry::Rect;

/// Free-form metadata associated with a sprite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Pixel aspect ratio as `(width, height)`. `(1, 1)` is square pixels;
    /// `(0, 0)` is treated as `(1, 1)` by readers, matching older `.aseprite`
    /// files where the field was unset.
    pub pixel_ratio: (u8, u8),
}

/// The Pincel document. Every persistent field mirrors the Aseprite v1.3 file
/// format one-to-one. A small amount of transient editor state (e.g.
/// [`Sprite::selection`]) also lives here for convenience; those fields are
/// explicitly documented and the codec drops them on save.
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
    /// Active marquee selection in sprite coordinates, or `None` when nothing
    /// is selected. **Transient editor state, not part of the file format**
    /// and not tracked by the undo stack in the M7.8a slice; the rect may
    /// extend past the canvas and consumers should clip as needed. See spec
    /// §5.2 (Selection (Rect)).
    ///
    /// The intended invariant is that a stored rect is non-empty —
    /// [`Sprite::set_selection`] enforces this on the write path. Direct
    /// field assignment can still introduce `Some(empty_rect)`; readers
    /// should prefer [`Sprite::has_selection`], which treats an empty stored
    /// rect as "no selection".
    pub selection: Option<Rect>,
}

impl Sprite {
    /// Start a builder for a sprite with the given canvas dimensions.
    pub fn builder(width: u32, height: u32) -> SpriteBuilder {
        SpriteBuilder::new(width, height)
    }

    /// Replace the active marquee selection. An empty `rect` (zero width or
    /// height) clears the selection instead of storing a degenerate marquee.
    pub fn set_selection(&mut self, rect: Rect) {
        self.selection = if rect.is_empty() { None } else { Some(rect) };
    }

    /// Drop the active marquee selection, if any.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// `true` when a non-empty marquee selection is active. An empty stored
    /// rect (which [`Sprite::set_selection`] refuses to store, but which
    /// direct field assignment could introduce) is reported as no selection.
    pub fn has_selection(&self) -> bool {
        self.selection.is_some_and(|r| !r.is_empty())
    }

    /// Look up a layer by id. Returns `None` if no layer has the given id.
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Mutable layer lookup by id.
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Look up a tileset by id. Returns `None` if no tileset has the given id.
    pub fn tileset(&self, id: TilesetId) -> Option<&Tileset> {
        self.tilesets.iter().find(|t| t.id == id)
    }

    /// Mutable tileset lookup by id.
    pub fn tileset_mut(&mut self, id: TilesetId) -> Option<&mut Tileset> {
        self.tilesets.iter_mut().find(|t| t.id == id)
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
            selection: None,
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
    fn builder_starts_without_a_selection() {
        let s = Sprite::builder(16, 16)
            .build()
            .expect("minimal sprite should build");
        assert_eq!(s.selection, None);
        assert!(!s.has_selection());
    }

    #[test]
    fn set_selection_stores_the_rect() {
        let mut s = Sprite::builder(32, 32)
            .build()
            .expect("sprite should build");
        s.set_selection(Rect::new(4, 5, 10, 6));
        assert_eq!(s.selection, Some(Rect::new(4, 5, 10, 6)));
        assert!(s.has_selection());
    }

    #[test]
    fn set_selection_replaces_an_existing_rect() {
        let mut s = Sprite::builder(32, 32)
            .build()
            .expect("sprite should build");
        s.set_selection(Rect::new(0, 0, 4, 4));
        s.set_selection(Rect::new(8, 8, 2, 3));
        assert_eq!(s.selection, Some(Rect::new(8, 8, 2, 3)));
    }

    #[test]
    fn set_selection_with_empty_rect_clears() {
        let mut s = Sprite::builder(32, 32)
            .build()
            .expect("sprite should build");
        s.set_selection(Rect::new(2, 2, 5, 5));
        s.set_selection(Rect::new(2, 2, 0, 5));
        assert_eq!(s.selection, None);
        s.set_selection(Rect::new(2, 2, 5, 5));
        s.set_selection(Rect::new(2, 2, 5, 0));
        assert_eq!(s.selection, None);
        assert!(!s.has_selection());
    }

    #[test]
    fn clear_selection_drops_the_rect() {
        let mut s = Sprite::builder(32, 32)
            .build()
            .expect("sprite should build");
        s.set_selection(Rect::new(1, 1, 2, 2));
        s.clear_selection();
        assert_eq!(s.selection, None);
    }

    #[test]
    fn has_selection_treats_direct_empty_rect_assignment_as_no_selection() {
        // `selection` is `pub`, so a caller could bypass `set_selection`'s
        // empty-clears rule. `has_selection` is documented as defensive
        // against that, since downstream rendering and selection-aware
        // commands key off it.
        let mut s = Sprite::builder(16, 16)
            .build()
            .expect("sprite should build");
        s.selection = Some(Rect::new(2, 2, 0, 5));
        assert!(!s.has_selection());
        s.selection = Some(Rect::new(2, 2, 5, 0));
        assert!(!s.has_selection());
    }

    #[test]
    fn selection_rect_may_extend_past_the_canvas() {
        // The model intentionally does not clip selections to the canvas;
        // higher layers (writer, render) clip on use. Spec §5.2 leaves this
        // to the consumer.
        let mut s = Sprite::builder(8, 8).build().expect("sprite should build");
        s.set_selection(Rect::new(-4, -3, 100, 100));
        assert_eq!(s.selection, Some(Rect::new(-4, -3, 100, 100)));
    }

    #[test]
    fn layer_lookup_returns_matching_layer() {
        let s = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::image(LayerId::new(7), "fg"))
            .build()
            .expect("sprite should build");
        assert_eq!(
            s.layer(LayerId::new(0)).map(|l| l.name.as_str()),
            Some("bg")
        );
        assert_eq!(
            s.layer(LayerId::new(7)).map(|l| l.name.as_str()),
            Some("fg")
        );
        assert!(s.layer(LayerId::new(99)).is_none());
    }

    #[test]
    fn layer_mut_lets_callers_rename() {
        let mut s = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .build()
            .expect("sprite should build");
        s.layer_mut(LayerId::new(0)).expect("present").name = "main".into();
        assert_eq!(s.layers[0].name, "main");
    }

    #[test]
    fn tileset_lookup_returns_matching_tileset() {
        let s = Sprite::builder(16, 16)
            .add_tileset(Tileset::new(TilesetId::new(0), "ground", (8, 8)))
            .add_tileset(Tileset::new(TilesetId::new(5), "decor", (16, 16)))
            .build()
            .expect("sprite should build");
        assert_eq!(
            s.tileset(TilesetId::new(0)).map(|t| t.name.as_str()),
            Some("ground")
        );
        assert_eq!(
            s.tileset(TilesetId::new(5)).map(|t| t.tile_size),
            Some((16, 16))
        );
        assert!(s.tileset(TilesetId::new(99)).is_none());
    }

    #[test]
    fn tileset_mut_lets_callers_push_tiles() {
        use crate::document::color_mode::ColorMode;
        let mut s = Sprite::builder(8, 8)
            .add_tileset(Tileset::new(TilesetId::new(0), "t", (8, 8)))
            .build()
            .expect("sprite should build");
        let t = s.tileset_mut(TilesetId::new(0)).expect("present");
        t.tiles.push(TileImage {
            pixels: PixelBuffer::empty(8, 8, ColorMode::Rgba),
        });
        assert_eq!(s.tilesets[0].tile_count(), 1);
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
