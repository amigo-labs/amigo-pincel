//! Pincel core library.
//!
//! Pure document model, tools, commands, and rendering for the Pincel
//! pixel-art editor. No platform dependencies, no file I/O. See
//! `docs/specs/pincel.md` for the design specification.

pub mod codec;
pub mod command;
pub mod document;
pub mod error;
pub mod geometry;
pub mod render;

pub use codec::{AsepriteReadOutput, CodecError, read_aseprite, write_aseprite};
pub use command::{
    AddFrame, AddLayer, AddTileset, AnyCommand, Bus, Command, CommandError, DrawEllipse, DrawLine,
    DrawRectangle, FillRegion, MoveSelectionContent, PlaceTile, SetPixel, SetTilePixel,
};

pub use document::{
    BlendMode, Cel, CelData, CelKey, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId,
    LayerKind, Metadata, Palette, PaletteEntry, PathRef, PixelBuffer, Rgba, Slice, SliceId,
    SliceKey, Sprite, SpriteBuilder, Tag, TagDirection, TileImage, TileRef, Tileset, TilesetId,
};
pub use error::DocumentError;
pub use geometry::{Point, Rect};
pub use render::{
    ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays, RenderError, compose,
};
