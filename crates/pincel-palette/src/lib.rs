//! Palette file codecs shared by Pincel's two modes (spec §3.7, Decision Log
//! 2026-11-05): GIMP `.gpl`, Lospec `.hex`, JASC `.pal` and PNG palette
//! images. Pixel mode (`pincel-core`) converts the entries into its document
//! palette; Image mode (`pincel-image-wasm`) keeps them as app state.
//!
//! Platform-free and without file I/O: readers take the file's bytes or
//! text, writers return them. Every reader caps a palette at
//! [`MAX_ENTRIES`] entries.

mod formats;
mod gpl;
mod png;

pub use formats::{
    PaletteFormat, PaletteFormatError, detect_palette_format, palette_from_png, palette_to_png,
    read_hex, read_jasc_pal, read_palette, write_hex, write_jasc_pal,
};
pub use gpl::{GplError, read_gpl, write_gpl};

/// Maximum number of entries a palette file may hold.
pub const MAX_ENTRIES: usize = 256;

/// 32-bit RGBA color, non-premultiplied, each channel `0..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (255 = opaque).
    pub a: u8,
}

impl Rgba {
    /// A color from its four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pack into a `u32` in `0xRRGGBBAA` order.
    pub const fn to_u32(self) -> u32 {
        u32::from_be_bytes([self.r, self.g, self.b, self.a])
    }

    /// Unpack from a `u32` in `0xRRGGBBAA` order.
    pub const fn from_u32(v: u32) -> Self {
        let [r, g, b, a] = v.to_be_bytes();
        Self::new(r, g, b, a)
    }
}

/// One palette entry: a color and an optional name (`.gpl` keeps names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// The entry's color.
    pub rgba: Rgba,
    /// The entry's name, if the file gave one.
    pub name: Option<String>,
}

impl PaletteEntry {
    /// An unnamed entry.
    pub const fn new(rgba: Rgba) -> Self {
        Self { rgba, name: None }
    }

    /// A named entry.
    pub fn with_name(rgba: Rgba, name: impl Into<String>) -> Self {
        Self {
            rgba,
            name: Some(name.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_u32_and_back_is_rrggbbaa() {
        let c = Rgba::new(0x11, 0x22, 0x33, 0x80);
        assert_eq!(c.to_u32(), 0x1122_3380);
        assert_eq!(Rgba::from_u32(0x1122_3380), c);
    }
}
