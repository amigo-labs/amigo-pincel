//! Codec adapters between Pincel's `Sprite` model and external file formats.
//!
//! Phase 1 wires the Aseprite read path (M4, [`aseprite_read`]) and the
//! Aseprite write path (M5, [`aseprite_write`]) on top of the standalone
//! `aseprite-loader` and `aseprite-writer` crates. See `docs/specs/pincel.md` §7.
//!
//! [`png`] adds the export-only PNG path (single frame or grid atlas) on top
//! of `compose()`. It is write-only by design: `.aseprite` stays the project
//! format, PNG is what leaves the app for a game engine.
//!
//! [`animation`] adds the animated export (APNG, GIF) on the same path,
//! sharing the nearest-neighbour upscale in `scale` (spec §7.5).

mod animation;
mod aseprite_read;
mod aseprite_write;
mod error;
mod gpl;
mod png;
mod scale;
mod sheet_data;

pub use animation::{
    AnimationFormat, AnimationInfo, AnimationOptions, animation_frames, animation_info,
    export_animation,
};
pub use aseprite_read::{AsepriteReadOutput, read_aseprite};
pub use aseprite_write::write_aseprite;
pub use error::CodecError;
pub use gpl::{GplError, read_gpl, write_gpl};
pub use png::{
    AtlasFrame, AtlasManifest, AtlasOptions, AtlasOutput, ExportError, ImportError, ImportedImage,
    export_atlas_png, export_frame_png, import_png,
};
pub use sheet_data::aseprite_sheet_json;
