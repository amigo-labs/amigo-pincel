//! Aseprite v1.3 file format writer.
//!
//! Standalone, no Pincel dependency. The data model mirrors
//! `aseprite-loader` so reader output can be re-emitted without
//! translation. See `README.md` for the format reference and trademark
//! disclaimer.
//!
//! ## Phase 1 status
//!
//! Crate is being implemented incrementally. See `STATUS.md` at the
//! workspace root for the current milestone.

mod bytes;
mod error;
mod file;
mod types;
mod write;

pub use error::WriteError;
pub use file::{
    AseFile, CelChunk, CelContent, Frame, Header, LayerChunk, NinePatch, PaletteChunk,
    PaletteEntry, Pivot, SliceChunk, SliceKey, Tag, TilesetChunk,
};
pub use types::{
    AnimationDirection, BlendMode, Color, ColorDepth, LayerFlags, LayerType, PaletteEntryFlags,
};
pub use write::write;
