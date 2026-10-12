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
        Self::preset(PalettePreset::Db32)
    }

    /// One of the built-in palettes (spec §3.7).
    pub fn preset(preset: PalettePreset) -> Self {
        Self::from_entries(
            preset
                .colors()
                .iter()
                .map(|&c| PaletteEntry::new(Rgba::from_u32(c << 8 | 0xff)))
                .collect(),
        )
    }
}

/// The built-in palettes a Pixel-mode document can switch to (spec §3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PalettePreset {
    /// DawnBringer 32, Aseprite's default.
    Db32,
    /// DawnBringer 16.
    Db16,
    /// The PICO-8 fantasy console's 16 colours.
    Pico8,
    /// The original Game Boy's four greens.
    GameBoy,
    /// The NES (2C02) palette without duplicates and unusable blacks.
    Nes,
}

impl PalettePreset {
    /// Every preset, in menu order.
    pub const ALL: [PalettePreset; 5] = [
        PalettePreset::Db32,
        PalettePreset::Db16,
        PalettePreset::Pico8,
        PalettePreset::GameBoy,
        PalettePreset::Nes,
    ];

    /// Stable id used across the wasm boundary.
    pub fn name(self) -> &'static str {
        match self {
            PalettePreset::Db32 => "db32",
            PalettePreset::Db16 => "db16",
            PalettePreset::Pico8 => "pico8",
            PalettePreset::GameBoy => "gameboy",
            PalettePreset::Nes => "nes",
        }
    }

    /// The preset with id `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }

    fn colors(self) -> &'static [u32] {
        match self {
            PalettePreset::Db32 => &DB32,
            PalettePreset::Db16 => &DB16,
            PalettePreset::Pico8 => &PICO8,
            PalettePreset::GameBoy => &GAME_BOY,
            PalettePreset::Nes => &NES,
        }
    }
}

/// DB16 as `0xRRGGBB`.
const DB16: [u32; 16] = [
    0x140c1c, 0x442434, 0x30346d, 0x4e4a4e, 0x854c30, 0x346524, 0xd04648, 0x757161, 0x597dce,
    0xd27d2c, 0x8595a1, 0x6daa2c, 0xd2aa99, 0x6dc2ca, 0xdad45e, 0xdeeed6,
];

/// PICO-8 as `0xRRGGBB`, in index order.
const PICO8: [u32; 16] = [
    0x000000, 0x1d2b53, 0x7e2553, 0x008751, 0xab5236, 0x5f574f, 0xc2c3c7, 0xfff1e8, 0xff004d,
    0xffa300, 0xffec27, 0x00e436, 0x29adff, 0x83769c, 0xff77a8, 0xffccaa,
];

/// Game Boy (DMG) greens, darkest first.
const GAME_BOY: [u32; 4] = [0x0f380f, 0x306230, 0x8bac0f, 0x9bbc0f];

/// NES (2C02) as `0xRRGGBB`, the common 54-colour set.
const NES: [u32; 54] = [
    0x000000, 0xfcfcfc, 0xf8f8f8, 0xbcbcbc, 0x7c7c7c, 0xa4e4fc, 0x3cbcfc, 0x0078f8, 0x0000fc,
    0xb8b8f8, 0x6888fc, 0x0058f8, 0x0000bc, 0xd8b8f8, 0x9878f8, 0x6844fc, 0x4428bc, 0xf8b8f8,
    0xf878f8, 0xd800cc, 0x940084, 0xf8a4c0, 0xf85898, 0xe40058, 0xa80020, 0xf0d0b0, 0xf87858,
    0xf83800, 0xa81000, 0xfce0a8, 0xfca044, 0xe45c10, 0x881400, 0xf8d878, 0xf8b800, 0xac7c00,
    0x503000, 0xd8f878, 0xb8f818, 0x00b800, 0x007800, 0xb8f8b8, 0x58d854, 0x00a800, 0x006800,
    0xb8f8d8, 0x58f898, 0x00a844, 0x005800, 0x00fcfc, 0x00e8d8, 0x008888, 0x004058, 0x787878,
];

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
    fn preset_sizes_and_names_round_trip() {
        let sizes = [32, 16, 16, 4, 54];
        for (p, n) in PalettePreset::ALL.into_iter().zip(sizes) {
            assert_eq!(Palette::preset(p).len(), n, "{}", p.name());
            assert_eq!(PalettePreset::from_name(p.name()), Some(p));
        }
        assert_eq!(PalettePreset::from_name("vga"), None);
        assert_eq!(Palette::db32(), Palette::preset(PalettePreset::Db32));
    }

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
