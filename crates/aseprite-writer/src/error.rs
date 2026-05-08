//! Errors produced by the writer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    /// Underlying `io::Write` returned an error. Wraps `io::Error`.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

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

    /// Cel image pixel buffer length does not match `width * height * bpp/8`.
    #[error(
        "cel image data length {actual} does not match {width}x{height}x{bytes_per_pixel} bytes ({expected} expected)"
    )]
    CelImageSizeMismatch {
        width: u16,
        height: u16,
        bytes_per_pixel: u8,
        expected: usize,
        actual: usize,
    },

    /// Cel `layer_index` references a layer that does not exist in `AseFile::layers`.
    #[error("cel layer_index {layer_index} >= layer count {layers}")]
    CelLayerIndexOutOfRange { layer_index: u16, layers: usize },

    /// Linked cel `frame_position` references a frame outside the file.
    #[error("linked cel frame_position {frame_position} >= frame count {frames}")]
    CelLinkedFrameOutOfRange { frame_position: u16, frames: usize },
}
