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

    /// [`crate::EffectSpec::from_params`] got a name it does not know.
    #[error("unknown effect `{0}`")]
    UnknownEffect(String),

    /// [`crate::EffectSpec::from_params`] got the wrong number of
    /// parameters for the named effect.
    #[error("effect `{effect}` expects {expected} parameter(s), got {got}")]
    ParamCount {
        effect: String,
        expected: usize,
        got: usize,
    },

    /// [`crate::EffectSpec::from_params`] got a parameter value that has
    /// no meaning for the effect (e.g. a colour slot that is not an
    /// integer `0xRRGGBBAA`), so nothing is clamped and nothing runs.
    #[error("effect `{effect}` parameter {index} is invalid: {reason}")]
    InvalidParam {
        effect: String,
        index: usize,
        reason: &'static str,
    },
}
