//! Codec adapters between Pincel's `Sprite` model and external file formats.
//!
//! Phase 1 only contains the Aseprite read path (M4); the write path lives in
//! the standalone `aseprite-writer` crate (M5) and will be glued in here later.
//! See `docs/specs/pincel.md` §7.

mod aseprite_read;
mod error;

pub use aseprite_read::{AsepriteReadOutput, read_aseprite};
pub use error::CodecError;
