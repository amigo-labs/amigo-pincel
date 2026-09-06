//! Crate-level error type.

use thiserror::Error;

/// Errors produced when constructing an [`EffectImage`](crate::EffectImage).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EffectError {
    /// The supplied byte buffer did not match `width * height * 4`.
    #[error("buffer length {got} does not match {width}x{height} RGBA8 ({expected} bytes)")]
    BufferSizeMismatch {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
        /// Expected byte count (`width * height * 4`).
        expected: usize,
        /// Actual byte count supplied.
        got: usize,
    },
}
