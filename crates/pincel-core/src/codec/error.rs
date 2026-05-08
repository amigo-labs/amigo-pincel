//! Error type for codec adapters.

use thiserror::Error;

use crate::error::DocumentError;

/// Errors produced by the Aseprite read / write adapters.
#[derive(Debug, Error)]
pub enum CodecError {
    /// The underlying parser rejected the byte stream.
    #[error("aseprite parse failed: {0}")]
    Parse(String),

    /// The file uses a color depth Pincel does not yet ingest. The current
    /// adapter handles RGBA only; indexed and grayscale are deferred.
    #[error("unsupported aseprite color mode (only RGBA is currently supported)")]
    UnsupportedColorMode,

    /// The file contains a layer kind we do not yet round-trip (tilemap layers
    /// land in M8, unknown layer types are forwarded as-is on write).
    #[error("unsupported aseprite layer kind: {kind}")]
    UnsupportedLayerKind {
        /// Layer kind code as encoded in the source file (`0x2004` chunk word).
        kind: u16,
    },

    /// The file uses a blend mode Pincel does not recognize.
    #[error("unsupported aseprite blend mode: {mode}")]
    UnsupportedBlendMode {
        /// Blend mode code as encoded in the source file.
        mode: u16,
    },

    /// The file contains a cel kind we do not yet ingest (compressed tilemap
    /// is M8; unknown cel types are forwarded as-is on write).
    #[error("unsupported aseprite cel kind: {kind}")]
    UnsupportedCelKind {
        /// Cel type code as encoded in the source file (Cel chunk type word).
        kind: u16,
    },

    /// The file references a layer index outside the layer table.
    #[error("cel references unknown layer index: {index}")]
    LayerIndexOutOfRange {
        /// 0-based layer index taken from the cel chunk.
        index: usize,
    },

    /// Pixel decode failed inside `aseprite-loader`.
    #[error("aseprite image decode failed: {0}")]
    Image(String),

    /// Building the [`crate::Sprite`] model rejected the adapter output.
    #[error(transparent)]
    Document(#[from] DocumentError),
}
