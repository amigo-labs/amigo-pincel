//! Codec adapters between Pincel's `Sprite` model and external file formats.
//!
//! Phase 1 wires the Aseprite read path (M4, [`aseprite_read`]) and the
//! Aseprite write path (M5, [`aseprite_write`]) on top of the standalone
//! `aseprite-loader` and `aseprite-writer` crates. See `docs/specs/pincel.md` §7.

mod aseprite_read;
mod aseprite_write;
mod error;

pub use aseprite_read::{AsepriteReadOutput, read_aseprite};
pub use aseprite_write::write_aseprite;
pub use error::CodecError;
