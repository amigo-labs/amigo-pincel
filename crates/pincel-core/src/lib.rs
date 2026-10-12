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
pub mod selection;

pub use codec::{
    AsepriteReadOutput, AtlasFrame, AtlasManifest, AtlasOptions, AtlasOutput, CodecError,
    ExportError, GplError, ImportError, ImportedImage, aseprite_sheet_json, export_atlas_png,
    export_frame_png, import_png, read_aseprite, read_gpl, write_aseprite, write_gpl,
};
pub use command::{
    AddFrame, AddLayer, AddPaletteEntry, AddSlice, AddTag, AddTile, AddTilemapLayer, AddTileset,
    Anchor, AnyCommand, Bus, ClearRegion, Command, CommandError, DirtyRegion, DrawEllipse,
    DrawLine, DrawRectangle, DrawShape, DrawText, DuplicateLayer, FillRegion, FlattenImage,
    Interpolation, MergeDown, MoveDirection, MoveLayer, MovePaletteEntry, MoveSelectionContent,
    Orientation, PlaceTile, ReframeCanvas, RemoveFrame, RemoveLayer, RemovePaletteEntry,
    RemoveSlice, RemoveTag, ReorderLayer, ReplaceCelPixels, ReplacePalette, ScaleImage,
    SetFrameDuration, SetLayerBlendMode, SetLayerEditable, SetLayerName, SetLayerOpacity,
    SetLayerVisible, SetPaletteColor, SetPixel, SetSliceKey, SetTagDirection, SetTagName,
    SetTagRange, SetTilePixel, ShapeKind, ShapeMode, ShapeStyle, TextAlign, TextStyle,
    TransformCanvas, TransformCel,
};

pub use document::{
    BlendMode, Cel, CelData, CelKey, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId,
    LayerKind, MAX_PALETTE_ENTRIES, Metadata, Palette, PaletteEntry, PalettePreset, PathRef,
    PixelBuffer, Rgba, Slice, SliceId, SliceKey, Sprite, SpriteBuilder, Tag, TagDirection,
    TileImage, TileRef, Tileset, TilesetId, UserData,
};
pub use error::DocumentError;
pub use geometry::{Point, Rect};
pub use render::{
    ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays, RenderError, compose,
};
pub use selection::{SelectionMask, SelectionMode};
