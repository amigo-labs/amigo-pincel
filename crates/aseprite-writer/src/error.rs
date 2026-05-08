//! Errors produced by the writer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    /// Underlying `io::Write` returned an error. Wraps `io::Error`.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Header `frames` field disagrees with `frames.len()`.
    #[error("header.frames ({header}) does not match frames.len() ({actual})")]
    FrameCountMismatch { header: u16, actual: usize },

    /// A counted field would overflow its on-disk u16/u32 width.
    #[error("{what} count {count} exceeds maximum {max}")]
    TooMany {
        what: &'static str,
        count: u64,
        max: u64,
    },

    /// String length exceeds the on-disk u16 prefix.
    #[error("string '{preview}' is too long ({len} bytes > 65535)")]
    StringTooLong { preview: String, len: usize },

    /// `LayerType::Tilemap` requires a `tileset_index` to be set.
    #[error("tilemap layer '{name}' is missing tileset_index")]
    MissingTilesetIndex { name: String },

    /// A palette range first..=last would be empty (no entries).
    #[error("palette chunk has zero entries")]
    EmptyPalette,

    /// Palette range exceeds the u32 last-color-index slot.
    #[error("palette range first={first} + len={len} exceeds u32::MAX")]
    PaletteRangeOverflow { first: u32, len: usize },

    /// Tag `to_frame` is before `from_frame`.
    #[error("tag '{name}' has from_frame={from} > to_frame={to}")]
    InvalidTagRange { name: String, from: u16, to: u16 },
}
