//! Canvas dimensions.

use crate::error::DocumentError;
use serde::{Deserialize, Serialize};

/// Minimum canvas dimension in pixels (spec §3.2).
pub const MIN_CANVAS_DIM: u32 = 1;
/// Maximum canvas dimension in pixels (spec §3.2).
pub const MAX_CANVAS_DIM: u32 = 32767;

/// The pixel dimensions of a document's canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasSize {
    width: u32,
    height: u32,
}

impl CanvasSize {
    /// Creates a validated canvas size.
    ///
    /// Both dimensions must lie in `1..=32767` (spec §3.2), otherwise
    /// `InvalidCanvasSize` is returned.
    pub fn new(width: u32, height: u32) -> Result<Self, DocumentError> {
        if !(MIN_CANVAS_DIM..=MAX_CANVAS_DIM).contains(&width)
            || !(MIN_CANVAS_DIM..=MAX_CANVAS_DIM).contains(&height)
        {
            return Err(DocumentError::InvalidCanvasSize { width, height });
        }
        Ok(Self { width, height })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_boundary_values() {
        assert!(CanvasSize::new(1, 1).is_ok());
        assert!(CanvasSize::new(32767, 32767).is_ok());
    }

    #[test]
    fn new_rejects_zero_dimension() {
        assert_eq!(
            CanvasSize::new(0, 10),
            Err(DocumentError::InvalidCanvasSize {
                width: 0,
                height: 10
            })
        );
    }

    #[test]
    fn new_rejects_oversized_dimension() {
        assert!(CanvasSize::new(32768, 10).is_err());
    }
}
