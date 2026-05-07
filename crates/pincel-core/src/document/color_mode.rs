//! Document-wide color mode (RGBA, indexed, grayscale).

/// The pixel encoding used by all image cels and tile images in a sprite.
///
/// Mirrors the Aseprite color mode field. `Grayscale` is Phase 2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ColorMode {
    /// 32 bpp, non-premultiplied RGBA.
    #[default]
    Rgba,
    /// 8 bpp palette indices. The given index is treated as fully transparent.
    Indexed { transparent_index: u8 },
    /// 16 bpp value + alpha. Phase 2.
    Grayscale,
}

impl ColorMode {
    /// Bytes occupied by one pixel in this color mode.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba => 4,
            Self::Indexed { .. } => 1,
            Self::Grayscale => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_per_pixel_matches_format() {
        assert_eq!(ColorMode::Rgba.bytes_per_pixel(), 4);
        assert_eq!(
            ColorMode::Indexed {
                transparent_index: 0
            }
            .bytes_per_pixel(),
            1
        );
        assert_eq!(ColorMode::Grayscale.bytes_per_pixel(), 2);
    }
}
