//! GIMP palette (`.gpl`) read / write (spec §3.7). Plain text:
//!
//! ```text
//! GIMP Palette
//! Name: My palette
//! Columns: 8
//! # comment
//!   0   0   0 Black
//! 255 255 255 White
//! ```
//!
//! Entries are opaque RGB; alpha is not part of the format. `Name:` and
//! `Columns:` headers and `#` comments are optional and skipped on read.

use super::palette_formats::{from_palette, to_palette};
use crate::document::Palette;

pub use pincel_palette::GplError;

/// Parse a GIMP palette. Unnamed entries (or GIMP's `Untitled`) get no name.
///
/// The codec lives in `pincel-palette`, shared with Image mode
/// (image-palette-gradient); this converts its entries into a [`Palette`].
pub fn read_gpl(text: &str) -> Result<Palette, GplError> {
    pincel_palette::read_gpl(text).map(to_palette)
}

/// Write `palette` as a GIMP palette called `name`. Alpha is dropped; an
/// unnamed entry is labelled with its hex code, as GIMP does.
pub fn write_gpl(palette: &Palette, name: &str) -> String {
    pincel_palette::write_gpl(&from_palette(palette), name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{PalettePreset, Rgba};

    #[test]
    fn write_then_read_round_trips_colours_and_names() {
        let mut p = Palette::preset(PalettePreset::Pico8);
        p.colors[0].name = Some("Ink".into());
        let back = read_gpl(&write_gpl(&p, "PICO-8")).expect("parses");
        assert_eq!(back.len(), 16);
        assert_eq!(back.colors[0].name.as_deref(), Some("Ink"));
        assert_eq!(back.colors[8].rgba, p.colors[8].rgba);
    }

    #[test]
    fn read_skips_headers_comments_and_blank_lines() {
        let text = "\u{feff}GIMP Palette\nName: x\nColumns: 4\n# hi\n\n 10  20 30\tRed-ish\n1 2 3 Untitled\n";
        let p = read_gpl(text).expect("parses");
        assert_eq!(p.len(), 2);
        assert_eq!(p.colors[0].rgba, Rgba::new(10, 20, 30, 255));
        assert_eq!(p.colors[0].name.as_deref(), Some("Red-ish"));
        assert_eq!(p.colors[1].name, None);
    }

    #[test]
    fn read_rejects_missing_header_and_bad_lines() {
        assert_eq!(read_gpl("JASC-PAL\n"), Err(GplError::MissingHeader));
        assert_eq!(read_gpl("GIMP Palette\n1 2\n"), Err(GplError::BadLine(2)));
        assert_eq!(
            read_gpl("GIMP Palette\n1 2 300\n"),
            Err(GplError::BadLine(2))
        );
    }

    #[test]
    fn read_rejects_more_than_256_entries() {
        let text = format!("GIMP Palette\n{}", "0 0 0\n".repeat(257));
        assert_eq!(read_gpl(&text), Err(GplError::TooManyEntries));
    }
}
