//! Pincel core library.
//!
//! Pure document model, tools, commands, and rendering for the Pincel
//! pixel-art editor. No platform dependencies, no file I/O. See
//! `docs/specs/pincel.md` for the design specification.

pub mod clipboard;
pub mod codec;
pub mod command;
pub mod document;
pub mod error;
pub mod geometry;
pub mod render;
pub mod selection;

pub use clipboard::{CopiedPixels, CopyError, CopySource, copy_pixels};
pub use codec::{
    AnimationFormat, AnimationInfo, AnimationOptions, AsepriteReadOutput, AtlasFrame,
    AtlasManifest, AtlasOptions, AtlasOutput, CodecError, ExportError, GplError, ImportError,
    ImportedImage, PaletteFormat, PaletteFormatError, animation_frames, animation_info,
    aseprite_sheet_json, detect_palette_format, encode_png, export_animation, export_atlas_png,
    export_frame_png, export_frame_png_scaled, import_png, palette_from_png, palette_to_png,
    read_aseprite, read_gpl, read_hex, read_jasc_pal, read_palette, write_aseprite, write_gpl,
    write_hex, write_jasc_pal,
};
pub use command::{
    AddEmptyCel, AddFrame, AddLayer, AddPaletteEntry, AddSlice, AddTag, AddTile, AddTilemapLayer,
    AddTileset, Anchor, AnyCommand, BrushShape, Bus, ClearRegion, Command, CommandBatch,
    CommandError, DirtyRegion, DitherMatrix, DrawDitherGradient, DrawEllipse, DrawLine,
    DrawRectangle, DrawShape, DrawText, DuplicateLayer, FillRegion, FlattenImage, GradientKind,
    Ink, Interpolation, LayerPlacement, MAX_BRUSH_SIZE, MIN_BRUSH_SIZE, MergeDown, Mirror,
    MoveDirection, MoveLayer, MoveLayerTo, MovePaletteEntry, MoveSelectionContent, Orientation,
    PasteLayer, PastedImage, PatchCel, PencilStroke, PixelBrush, PlaceTile, PutCel,
    ROTSPRITE_MAX_SIDE, ReframeCanvas, RemoveFrame, RemoveLayer, RemovePaletteEntry, RemoveSlice,
    RemoveTag, ReorderLayer, ReplaceCelPixels, ReplacePalette, RotateCel, RotateMethod, ScaleImage,
    SetFrameDuration, SetGrid, SetLayerBlendMode, SetLayerEditable, SetLayerName, SetLayerOpacity,
    SetLayerVisible, SetPaletteColor, SetPixel, SetSliceKey, SetTagDirection, SetTagName,
    SetTagRange, SetTilePixel, ShapeKind, ShapeMode, ShapeStyle, TextAlign, TextStyle,
    TransformCanvas, TransformCel, Wrap,
};

pub use document::{
    BlendMode, Cel, CelData, CelKey, CelMap, ColorMode, ExternalFile, Frame, FrameIndex, Layer,
    LayerId, LayerKind, MAX_PALETTE_ENTRIES, Metadata, Palette, PaletteEntry, PalettePreset,
    PathRef, PixelBuffer, Rgba, Slice, SliceId, SliceKey, Sprite, SpriteBuilder, Tag, TagDirection,
    TileImage, TileRef, Tileset, TilesetId, UserData,
};
pub use error::DocumentError;
pub use geometry::{Point, Rect};
pub use render::{
    ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays, RenderError, compose,
};
pub use selection::{SelectionMask, SelectionMode, composite_canvas, layer_canvas};
