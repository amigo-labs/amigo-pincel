//! Error types for the document model.

use thiserror::Error;

/// Errors raised when constructing or mutating the [`crate::Sprite`] model.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocumentError {
    /// The supplied canvas dimensions are invalid (zero on either axis).
    #[error("invalid sprite dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// A duplicate identifier was supplied where uniqueness is required.
    #[error("duplicate {kind} id: {id}")]
    DuplicateId { kind: &'static str, id: u32 },

    /// A reference to a non-existent identifier.
    #[error("unknown {kind} id: {id}")]
    UnknownId { kind: &'static str, id: u32 },
}
