//! # fineliner-core
//!
//! Pure logic core for the Fineliner image editor: document model, color and
//! blend modes, geometry, commands/undo, compositing, codecs, and tools.
//!
//! This crate has **no platform dependencies** — no async runtime, no
//! `wasm-bindgen`, no Tauri. It is `std`-only and safe to compile to
//! `wasm32-unknown-unknown`. See `CLAUDE.md` §5.1.

#![forbid(unsafe_code)]

pub mod codec;
pub mod color;
pub mod command;
pub mod document;
pub mod error;
pub mod geometry;
pub mod render;
pub mod selection;
pub mod tools;
pub mod transform;

pub use color::{BlendMode, Color};
pub use command::{
    AddLayer, Anchor, CanvasRotation, Command, CommandBus, CropToSelection, DuplicateLayer,
    FlattenImage, FlipCanvas, LayerTransform, MergeDown, MergeVisible, MoveLayer, RemoveLayer,
    RenameLayer, ResizeCanvas, RotateCanvas, RotateLayer90, ScaleImage, SetLayerBlendMode,
    SetLayerLocked, SetLayerOpacity, SetLayerVisible, SetPixels, SetSelection, TransformLayer,
    UndoStack,
};
pub use document::{
    CanvasSize, ColorProfile, Document, DocumentMetadata, ImageBuffer, Layer, LayerKind,
    MAX_CANVAS_DIM, MAX_LAYERS, MIN_CANVAS_DIM,
};
pub use error::DocumentError;
pub use geometry::{Point, Rect, Size};
pub use render::{compose, compose_over};
pub use selection::{apply_mode, magic_wand, SelectionMask, SelectionMode};
pub use tools::{
    delete_selection, Brush, BrushShape, DashPattern, Eraser, EraserMode, Eyedropper, Fill,
    FillOptions, Move, Pencil, SampleSize, SampleSource, Shape, ShapeMode, ShapeStyle, Shapes,
    Text, TextAlign, TextStyle,
};
pub use transform::{
    flip_horizontal, flip_vertical, rotate_180, rotate_90_ccw, rotate_90_cw, scale, Interpolation,
};
