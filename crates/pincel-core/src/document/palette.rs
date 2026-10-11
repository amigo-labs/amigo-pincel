//! Palette and palette entries. See `docs/specs/pincel.md` §3.7.

use super::color::Rgba;

/// A single palette entry: a color plus an optional name (Aseprite supports
/// named palette entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub rgba: Rgba,
    pub name: Option<String>,
}

impl PaletteEntry {
    pub const fn new(rgba: Rgba) -> Self {
        Self { rgba, name: None }
    }

    pub fn with_name(rgba: Rgba, name: impl Into<String>) -> Self {
        Self {
            rgba,
            name: Some(name.into()),
        }
    }
}

/// A color palette. Up to 256 entries are addressable in indexed color mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette {
    pub colors: Vec<PaletteEntry>,
}

impl Palette {
    pub const fn new() -> Self {
        Self { colors: Vec::new() }
    }

    pub fn from_entries(colors: Vec<PaletteEntry>) -> Self {
        Self { colors }
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// The DawnBringer 32 palette (DB32, by Richard "DawnBringer" Fhager),
    /// Aseprite's default and the palette a new Pixel-mode document starts
    /// with (spec §3.7, Decision Log 2026-10-11).
    pub fn db32() -> Self {
        Self::from_entries(
            DB32.iter()
                .map(|&c| PaletteEntry::new(Rgba::from_u32(c << 8 | 0xff)))
                .collect(),
        )
    }
}

/// DB32 as `0xRRGGBB`, in the palette's canonical order.
const DB32: [u32; 32] = [
    0x000000, 0x222034, 0x45283c, 0x663931, 0x8f563b, 0xdf7126, 0xd9a066, 0xeec39a, 0xfbf236,
    0x99e550, 0x6abe30, 0x37946e, 0x4b692f, 0x524b24, 0x323c39, 0x3f3f74, 0x306082, 0x5b6ee1,
    0x639bff, 0x5fcde4, 0xcbdbfc, 0xffffff, 0x9badb7, 0x847e87, 0x696a6a, 0x595652, 0x76428a,
    0xac3232, 0xd95763, 0xd77bba, 0x8f974a, 0x8a6f30,
];

/// Maximum palette size: indexed color addresses entries with one byte.
pub const MAX_PALETTE_ENTRIES: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_default_is_empty() {
        let p = Palette::default();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn db32_has_32_opaque_entries_from_black_to_olive() {
        let p = Palette::db32();
        assert_eq!(p.len(), 32);
        assert_eq!(p.colors[0].rgba, Rgba::BLACK);
        assert_eq!(p.colors[21].rgba, Rgba::WHITE);
        assert_eq!(p.colors[31].rgba, Rgba::new(0x8a, 0x6f, 0x30, 0xff));
        assert!(p.colors.iter().all(|e| e.rgba.a == 255));
    }

    #[test]
    fn palette_entry_with_name_records_it() {
        let e = PaletteEntry::with_name(Rgba::BLACK, "shadow");
        assert_eq!(e.rgba, Rgba::BLACK);
        assert_eq!(e.name.as_deref(), Some("shadow"));
    }
}
