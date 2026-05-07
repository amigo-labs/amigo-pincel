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
}

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
    fn palette_entry_with_name_records_it() {
        let e = PaletteEntry::with_name(Rgba::BLACK, "shadow");
        assert_eq!(e.rgba, Rgba::BLACK);
        assert_eq!(e.name.as_deref(), Some("shadow"));
    }
}
