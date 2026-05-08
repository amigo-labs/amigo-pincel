//! Scalar and enum types that mirror `aseprite-loader`.
//!
//! Names and discriminants match the reader so that data can flow
//! `aseprite-loader` → `aseprite-writer` without translation. Field
//! widths use the same Rust types the loader exposes (`u8`/`u16`/`i16`/
//! `u32`), so the encoded bytes are identical.

/// 32-bit RGBA color, one byte per channel.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// Color depth (bits per pixel).
///
/// See `aseprite-loader::binary::color_depth::ColorDepth`. Discriminants
/// match the on-disk encoding (32 = RGBA, 16 = grayscale, 8 = indexed).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ColorDepth {
    Rgba,
    Grayscale,
    Indexed,
}

impl ColorDepth {
    /// Bits per pixel as written to the file header.
    pub const fn bpp(self) -> u16 {
        match self {
            Self::Rgba => 32,
            Self::Grayscale => 16,
            Self::Indexed => 8,
        }
    }
}

/// Layer compositing mode.
///
/// Discriminants match the `BlendMode` enum in `aseprite-loader` and the
/// numeric values in the Aseprite file spec (§Layer Chunk).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum BlendMode {
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Darken = 4,
    Lighten = 5,
    ColorDodge = 6,
    ColorBurn = 7,
    HardLight = 8,
    SoftLight = 9,
    Difference = 10,
    Exclusion = 11,
    Hue = 12,
    Saturation = 13,
    Color = 14,
    Luminosity = 15,
    Addition = 16,
    Subtract = 17,
    Divide = 18,
}

impl BlendMode {
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Layer kind.
///
/// `Tilemap` requires a tileset index; see `LayerChunk::tileset_index`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum LayerType {
    Normal = 0,
    Group = 1,
    Tilemap = 2,
}

impl LayerType {
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Bit flags for `LayerChunk::flags`.
///
/// Matches `aseprite-loader::binary::chunks::layer::LayerFlags`.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct LayerFlags(u16);

impl LayerFlags {
    pub const VISIBLE: Self = Self(0x1);
    pub const EDITABLE: Self = Self(0x2);
    pub const LOCK_MOVEMENT: Self = Self(0x4);
    pub const BACKGROUND: Self = Self(0x8);
    pub const PREFER_LINKED_CELS: Self = Self(0x10);
    pub const COLLAPSED: Self = Self(0x20);
    pub const REFERENCE: Self = Self(0x40);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_bits_truncate(bits: u16) -> Self {
        Self(bits & 0x7F)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl core::ops::BitOr for LayerFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for LayerFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Tag playback direction.
///
/// Matches `aseprite-loader::binary::chunks::tags::AnimationDirection`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationDirection {
    Forward = 0,
    Reverse = 1,
    PingPong = 2,
    PingPongReverse = 3,
}

impl AnimationDirection {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Bit flags for `PaletteEntry::flags`.
///
/// Matches `aseprite-loader::binary::chunks::palette::PaletteEntryFlags`.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct PaletteEntryFlags(u16);

impl PaletteEntryFlags {
    pub const HAS_NAME: Self = Self(0x1);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_mode_discriminants_match_spec() {
        assert_eq!(BlendMode::Normal.as_u16(), 0);
        assert_eq!(BlendMode::Multiply.as_u16(), 1);
        assert_eq!(BlendMode::Divide.as_u16(), 18);
    }

    #[test]
    fn color_depth_bpp_matches_spec() {
        assert_eq!(ColorDepth::Rgba.bpp(), 32);
        assert_eq!(ColorDepth::Grayscale.bpp(), 16);
        assert_eq!(ColorDepth::Indexed.bpp(), 8);
    }

    #[test]
    fn layer_flags_bitor_combines_bits() {
        let f = LayerFlags::VISIBLE | LayerFlags::EDITABLE;
        assert_eq!(f.bits(), 0x3);
        assert!(f.contains(LayerFlags::VISIBLE));
        assert!(f.contains(LayerFlags::EDITABLE));
        assert!(!f.contains(LayerFlags::BACKGROUND));
    }

    #[test]
    fn layer_flags_truncate_drops_unknown_bits() {
        let f = LayerFlags::from_bits_truncate(0xFFFF);
        assert_eq!(f.bits(), 0x7F);
    }

    #[test]
    fn animation_direction_discriminants_match_spec() {
        assert_eq!(AnimationDirection::Forward.as_u8(), 0);
        assert_eq!(AnimationDirection::PingPongReverse.as_u8(), 3);
    }
}
