//! 32-bit RGBA color used for palette entries, slice overlays, tag colors, …

/// 32-bit RGBA color, non-premultiplied. Each channel is `0..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);
    pub const BLACK: Self = Self::new(0, 0, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pack into a single `u32` in `0xRRGGBBAA` order.
    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }

    /// Unpack from a `u32` in `0xRRGGBBAA` order.
    pub const fn from_u32(v: u32) -> Self {
        Self::new((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_roundtrips_through_u32() {
        let c = Rgba::new(0x12, 0x34, 0x56, 0x78);
        assert_eq!(c.to_u32(), 0x1234_5678);
        assert_eq!(Rgba::from_u32(0x1234_5678), c);
    }

    #[test]
    fn rgba_default_is_transparent_black() {
        assert_eq!(Rgba::default(), Rgba::TRANSPARENT);
    }
}
