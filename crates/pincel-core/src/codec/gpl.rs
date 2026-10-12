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

use thiserror::Error;

use crate::document::{MAX_PALETTE_ENTRIES, Palette, PaletteEntry, Rgba};

/// Why a `.gpl` file could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GplError {
    /// The first non-empty line is not `GIMP Palette`.
    #[error("not a GIMP palette (missing `GIMP Palette` header)")]
    MissingHeader,
    /// A colour line is not three numbers `0..=255` (1-based line number).
    #[error("line {0}: expected `R G B [name]` with values 0–255")]
    BadLine(usize),
    /// More entries than a palette can hold.
    #[error("palette has more than {MAX_PALETTE_ENTRIES} colours")]
    TooManyEntries,
}

/// Parse a GIMP palette. Unnamed entries (or GIMP's `Untitled`) get no name.
pub fn read_gpl(text: &str) -> Result<Palette, GplError> {
    let mut lines = text.lines().enumerate();
    let header = lines
        .by_ref()
        .find(|(_, l)| !l.trim().is_empty())
        .map(|(_, l)| l.trim().trim_start_matches('\u{feff}'));
    if header != Some("GIMP Palette") {
        return Err(GplError::MissingHeader);
    }
    let mut colors = Vec::new();
    for (i, raw) in lines {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("Name:")
            || line.starts_with("Columns:")
        {
            continue;
        }
        let mut parts = line.split_whitespace();
        let mut channel = || -> Result<u8, GplError> {
            parts
                .next()
                .and_then(|p| p.parse::<u8>().ok())
                .ok_or(GplError::BadLine(i + 1))
        };
        let (r, g, b) = (channel()?, channel()?, channel()?);
        let name = parts.collect::<Vec<_>>().join(" ");
        if colors.len() >= MAX_PALETTE_ENTRIES {
            return Err(GplError::TooManyEntries);
        }
        let rgba = Rgba::new(r, g, b, 255);
        colors.push(if name.is_empty() || name == "Untitled" {
            PaletteEntry::new(rgba)
        } else {
            PaletteEntry::with_name(rgba, name)
        });
    }
    Ok(Palette::from_entries(colors))
}

/// Write `palette` as a GIMP palette called `name`. Alpha is dropped; an
/// unnamed entry is labelled with its hex code, as GIMP does.
pub fn write_gpl(palette: &Palette, name: &str) -> String {
    let mut out = format!("GIMP Palette\nName: {name}\nColumns: 8\n#\n");
    for e in &palette.colors {
        let Rgba { r, g, b, .. } = e.rgba;
        let label = e
            .name
            .clone()
            .unwrap_or_else(|| format!("#{r:02x}{g:02x}{b:02x}"));
        out.push_str(&format!("{r:3} {g:3} {b:3}\t{label}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::PalettePreset;

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
