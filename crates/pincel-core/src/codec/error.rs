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

    /// A field on the document does not fit into its on-disk slot
    /// (canvas dimensions / cel position / cel dimensions / linked frame
    /// index / tag frame range / layer count / layer depth).
    #[error("{what} value {value} does not fit on disk")]
    OutOfRange {
        /// Short human-readable description, e.g. `"canvas width"`.
        what: &'static str,
        /// Numeric value that exceeded the on-disk slot. Signed so that
        /// negative cel positions surface verbatim.
        value: i64,
    },

    /// A linked cel points at a frame index past `Sprite::frames`.
    #[error("linked cel target frame {index} is past sprite.frames")]
    LinkedFrameNotFound {
        /// Frame index referenced by the linked cel.
        index: u32,
    },

    /// A layer's `parent` references a [`crate::LayerId`] that isn't
    /// present in `Sprite::layers`.
    #[error("layer parent {id} not found in sprite.layers")]
    LayerParentNotFound {
        /// Numeric value of the missing parent's [`crate::LayerId`].
        id: u32,
    },

    /// The layer parent graph contains a cycle reachable from the named
    /// layer id.
    #[error("layer parent graph contains a cycle at layer id {id}")]
    LayerCycle {
        /// Numeric value of a [`crate::LayerId`] inside the cycle.
        id: u32,
    },

    /// A cel references a [`crate::LayerId`] that isn't present in
    /// `Sprite::layers`.
    #[error("cel references unknown layer id {id}")]
    CelLayerNotFound {
        /// Numeric value of the cel's [`crate::LayerId`].
        id: u32,
    },

    /// A cel's frame index is past `Sprite::frames`.
    #[error("cel frame index {index} is past sprite.frames")]
    CelFrameNotFound {
        /// Frame index that did not match any frame in `Sprite::frames`.
        index: u32,
    },

    /// `aseprite-writer` rejected the staged file. Pre-validation in the
    /// adapter catches the structural mistakes; this variant carries
    /// errors that the writer itself raises (e.g. zlib I/O failures).
    #[error(transparent)]
    Write(#[from] aseprite_writer::WriteError),
}
