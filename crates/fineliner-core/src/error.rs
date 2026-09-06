//! Crate-level error types.

use thiserror::Error;

/// Errors produced by document and command operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocumentError {
    /// A canvas dimension was outside the allowed `1..=32767` range (spec §3.2).
    #[error("invalid canvas size {width}x{height}: dimensions must be in 1..=32767")]
    InvalidCanvasSize {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },

    /// Attempted to remove the last remaining layer (spec §3.2: always ≥1 layer).
    #[error("cannot remove the last remaining layer")]
    LastLayer,

    /// Attempted to exceed the 999-layer limit (spec §3.2).
    #[error("layer limit of {max} reached")]
    LayerLimit {
        /// The maximum number of layers.
        max: usize,
    },

    /// A layer index was out of bounds.
    #[error("layer index {index} out of bounds (len {len})")]
    LayerIndexOutOfBounds {
        /// The offending index.
        index: usize,
        /// Current number of layers.
        len: usize,
    },

    /// A pixel region did not fit the target buffer.
    #[error("region does not fit the target buffer")]
    RegionOutOfBounds,
}
