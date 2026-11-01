//! Errors of the trace API.

/// Why a session could not be created or a trace could not run.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TraceError {
    /// The width or height is 0.
    #[error("the image is empty ({width} × {height})")]
    EmptyImage {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// A side exceeds [`MAX_SIDE`](crate::MAX_SIDE).
    #[error("the image is {width} × {height}; each side may be at most {max}")]
    TooLarge {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// The largest side accepted.
        max: u32,
    },
    /// The buffer is not `width × height × 4` bytes long.
    #[error("the buffer holds {actual} bytes, a {width} × {height} RGBA image needs {expected}")]
    BufferLength {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Bytes required.
        expected: usize,
        /// Bytes given.
        actual: usize,
    },
    /// Memory for the session or a trace could not be reserved.
    #[error("not enough memory to trace this image")]
    OutOfMemory,
    /// An option is out of range.
    #[error("invalid option `{option}`: {reason}")]
    InvalidOption {
        /// The option's name, e.g. `colors` or `alpha_max`.
        option: &'static str,
        /// What is wrong with it.
        reason: String,
    },
    /// Pixel-exact mode met more distinct colours than it supports.
    #[error("pixel-exact mode supports at most {max} colours; the image has {count}")]
    TooManyColors {
        /// Distinct colours with alpha > 0 in the image.
        count: usize,
        /// The limit.
        max: usize,
    },
}

/// A zeroed buffer of `len` elements, or [`TraceError::OutOfMemory`].
pub(crate) fn try_vec<T: Clone>(len: usize, value: T) -> Result<Vec<T>, TraceError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len)
        .map_err(|_| TraceError::OutOfMemory)?;
    v.resize(len, value);
    Ok(v)
}
