//! Palette exchange formats next to GIMP `.gpl` (spec §3.7, Decision Log
//! 2026-10-29): Lospec `.hex`, JASC `.pal` and PNG palette images.
//!
//! ```text
//! .hex            .pal              .png
//! ff0044          JASC-PAL          one pixel (or one cell) per colour,
//! #00ff00         0100              read top to bottom, left to right
//! ; comment       2
//!                 255 0 68
//!                 0 255 0 128
//! ```
//!
//! [`read_palette`] detects the format by content ([`detect_palette_format`])
//! so a mislabeled file still imports. Every reader caps the palette at
//! [`MAX_PALETTE_ENTRIES`] and refuses a file without colours.

use std::collections::HashSet;

use thiserror::Error;

use super::gpl::{GplError, read_gpl};
use super::png::{encode_png, import_png};
use crate::document::{MAX_PALETTE_ENTRIES, Palette, PaletteEntry, Rgba};

/// The 8-byte PNG file signature.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// A palette file format Pincel reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFormat {
    /// GIMP palette (`GIMP Palette` header), see [`super::read_gpl`].
    Gpl,
    /// Lospec's plain list, one `RRGGBB` per line.
    Hex,
    /// JASC-PAL (Paint Shop Pro, Aseprite, Pyxel).
    JascPal,
    /// A palette image: every distinct colour in reading order.
    Png,
}

/// Why a palette file could not be read or written.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PaletteFormatError {
    /// The `.gpl` reader refused the file.
    #[error(transparent)]
    Gpl(#[from] GplError),
    /// A colour line (or the `.pal` count line) cannot be read; 1-based
    /// line number.
    #[error("line {0}: not a colour")]
    BadLine(usize),
    /// The `.pal` file ends before its `0100` and count lines.
    #[error("not a JASC palette (missing `0100` or count line)")]
    MissingPalHeader,
    /// More entries than a palette can hold.
    #[error("palette has more than {MAX_PALETTE_ENTRIES} colours")]
    TooManyEntries,
    /// The file holds no colour entries.
    #[error("no colours found")]
    NoColours,
    /// A PNG with more than [`MAX_PALETTE_ENTRIES`] distinct colours.
    #[error("not a palette image (more than {MAX_PALETTE_ENTRIES} colours)")]
    NotPaletteImage,
    /// The PNG could not be decoded.
    #[error("png decode failed: {0}")]
    Decode(String),
    /// The PNG strip could not be encoded (empty palette or cell size 0).
    #[error("png encode failed: {0}")]
    Encode(String),
}

/// Detect the palette format from the file content: PNG signature → PNG;
/// first non-empty line `GIMP Palette` → `.gpl`; `JASC-PAL` → `.pal`;
/// anything else → `.hex`. A leading UTF-8 BOM and whitespace are ignored.
pub fn detect_palette_format(bytes: &[u8]) -> PaletteFormat {
    if bytes.starts_with(&PNG_SIGNATURE) {
        return PaletteFormat::Png;
    }
    let text = String::from_utf8_lossy(bytes);
    match lines(&text).map(|(_, l)| l).find(|l| !l.is_empty()) {
        Some("GIMP Palette") => PaletteFormat::Gpl,
        Some("JASC-PAL") => PaletteFormat::JascPal,
        _ => PaletteFormat::Hex,
    }
}

/// Read a palette file of any supported format, detected by content
/// ([`detect_palette_format`]).
///
/// # Errors
///
/// The format reader's error; [`PaletteFormatError::NoColours`] when the
/// file has no entries.
pub fn read_palette(bytes: &[u8]) -> Result<Palette, PaletteFormatError> {
    let format = detect_palette_format(bytes);
    if format == PaletteFormat::Png {
        return palette_from_png(bytes);
    }
    let text = String::from_utf8_lossy(bytes);
    match format {
        PaletteFormat::Gpl => non_empty(read_gpl(&text)?),
        PaletteFormat::JascPal => read_jasc_pal(&text),
        PaletteFormat::Hex | PaletteFormat::Png => read_hex(&text),
    }
}

/// Read a Lospec `.hex` palette: one `RRGGBB` (optionally `#RRGGBB`) per
/// line, opaque. Blank lines and lines starting with `;` or `//` are
/// skipped.
///
/// # Errors
///
/// [`PaletteFormatError::BadLine`] for a line that is not six hex digits,
/// [`PaletteFormatError::TooManyEntries`] above 256 entries,
/// [`PaletteFormatError::NoColours`] for an empty file.
pub fn read_hex(text: &str) -> Result<Palette, PaletteFormatError> {
    let mut colors = Vec::new();
    for (n, line) in lines(text) {
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        let digits = line.strip_prefix('#').unwrap_or(line);
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(PaletteFormatError::BadLine(n));
        }
        let v = u32::from_str_radix(digits, 16).map_err(|_| PaletteFormatError::BadLine(n))?;
        let [_, r, g, b] = v.to_be_bytes();
        push(&mut colors, Rgba::new(r, g, b, 255))?;
    }
    non_empty(Palette::from_entries(colors))
}

/// Write `palette` as Lospec `.hex`: one lowercase `rrggbb` line per entry,
/// alpha and names dropped.
pub fn write_hex(palette: &Palette) -> String {
    palette
        .colors
        .iter()
        .map(|e| format!("{:02x}{:02x}{:02x}\n", e.rgba.r, e.rgba.g, e.rgba.b))
        .collect()
}

/// Read a JASC `.pal` palette (`JASC-PAL`, `0100`, a count, then `R G B`
/// or `R G B A` lines). The colour lines win over the count.
///
/// # Errors
///
/// [`PaletteFormatError::BadLine`] for a colour line that is not three or
/// four numbers 0–255 (or a count line that is not a number),
/// [`PaletteFormatError::TooManyEntries`] above 256 entries,
/// [`PaletteFormatError::NoColours`] without colour lines.
pub fn read_jasc_pal(text: &str) -> Result<Palette, PaletteFormatError> {
    let mut rows = lines(text).filter(|(_, l)| !l.is_empty());
    if rows.next().map(|(_, l)| l) != Some("JASC-PAL") {
        return Err(PaletteFormatError::MissingPalHeader);
    }
    rows.next().ok_or(PaletteFormatError::MissingPalHeader)?;
    let (n, count) = rows.next().ok_or(PaletteFormatError::MissingPalHeader)?;
    count
        .parse::<usize>()
        .map_err(|_| PaletteFormatError::BadLine(n))?;
    let mut colors = Vec::new();
    for (n, line) in rows {
        let values = line
            .split_whitespace()
            .map(str::parse::<u8>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PaletteFormatError::BadLine(n))?;
        let rgba = match values[..] {
            [r, g, b] => Rgba::new(r, g, b, 255),
            [r, g, b, a] => Rgba::new(r, g, b, a),
            _ => return Err(PaletteFormatError::BadLine(n)),
        };
        push(&mut colors, rgba)?;
    }
    non_empty(Palette::from_entries(colors))
}

/// Write `palette` as a classic three-column JASC `.pal` (CRLF line ends),
/// alpha and names dropped.
pub fn write_jasc_pal(palette: &Palette) -> String {
    let mut out = format!("JASC-PAL\r\n0100\r\n{}\r\n", palette.len());
    for e in &palette.colors {
        out.push_str(&format!("{} {} {}\r\n", e.rgba.r, e.rgba.g, e.rgba.b));
    }
    out
}

/// Read a palette image: pixels top to bottom, left to right; fully
/// transparent pixels are skipped and the first occurrence of each distinct
/// RGBA value becomes one entry.
///
/// # Errors
///
/// [`PaletteFormatError::Decode`] for bytes that are not a PNG,
/// [`PaletteFormatError::NotPaletteImage`] above 256 distinct colours,
/// [`PaletteFormatError::NoColours`] for a fully transparent image.
pub fn palette_from_png(bytes: &[u8]) -> Result<Palette, PaletteFormatError> {
    let image = import_png(bytes).map_err(|e| PaletteFormatError::Decode(e.to_string()))?;
    let (pixels, _) = image.rgba.as_chunks::<4>();
    let mut seen = HashSet::new();
    let mut colors = Vec::new();
    for &[r, g, b, a] in pixels {
        if a == 0 || !seen.insert([r, g, b, a]) {
            continue;
        }
        if colors.len() >= MAX_PALETTE_ENTRIES {
            return Err(PaletteFormatError::NotPaletteImage);
        }
        colors.push(PaletteEntry::new(Rgba::new(r, g, b, a)));
    }
    non_empty(Palette::from_entries(colors))
}

/// Write `palette` as a one-row PNG strip, `entries × cell` by `cell`
/// pixels, each entry filling its cell with its RGBA value.
///
/// # Errors
///
/// [`PaletteFormatError::Encode`] for an empty palette or `cell == 0`.
pub fn palette_to_png(palette: &Palette, cell: u32) -> Result<Vec<u8>, PaletteFormatError> {
    if palette.is_empty() || cell == 0 {
        return Err(PaletteFormatError::Encode(
            "a strip needs at least one colour and a cell size of 1 px or more".to_owned(),
        ));
    }
    let entries = u32::try_from(palette.len())
        .map_err(|_| PaletteFormatError::Encode("palette too large".to_owned()))?;
    let width = entries
        .checked_mul(cell)
        .ok_or_else(|| PaletteFormatError::Encode("strip too large".to_owned()))?;
    let mut row = Vec::with_capacity(width as usize * 4);
    for e in &palette.colors {
        let Rgba { r, g, b, a } = e.rgba;
        for _ in 0..cell {
            row.extend_from_slice(&[r, g, b, a]);
        }
    }
    let pixels = row.repeat(cell as usize);
    encode_png(&pixels, width, cell).map_err(|e| PaletteFormatError::Encode(e.to_string()))
}

/// `text`'s lines with 1-based numbers, a leading BOM removed and each line
/// trimmed (`str::lines` already accepts CRLF).
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
}

/// Append `rgba`, refusing to grow past [`MAX_PALETTE_ENTRIES`].
fn push(colors: &mut Vec<PaletteEntry>, rgba: Rgba) -> Result<(), PaletteFormatError> {
    if colors.len() >= MAX_PALETTE_ENTRIES {
        return Err(PaletteFormatError::TooManyEntries);
    }
    colors.push(PaletteEntry::new(rgba));
    Ok(())
}

/// `palette`, or [`PaletteFormatError::NoColours`] when it is empty.
fn non_empty(palette: Palette) -> Result<Palette, PaletteFormatError> {
    if palette.is_empty() {
        Err(PaletteFormatError::NoColours)
    } else {
        Ok(palette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(p: &Palette) -> Vec<(u8, u8, u8)> {
        p.colors
            .iter()
            .map(|e| (e.rgba.r, e.rgba.g, e.rgba.b))
            .collect()
    }

    fn rgba(p: &Palette) -> Vec<Rgba> {
        p.colors.iter().map(|e| e.rgba).collect()
    }

    fn png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
        encode_png(pixels.as_flattened(), width, height).expect("encode")
    }

    fn palette(colors: &[Rgba]) -> Palette {
        Palette::from_entries(colors.iter().map(|c| PaletteEntry::new(*c)).collect())
    }

    #[test]
    fn detect_palette_format_by_content_in_spec_order() {
        let strip = png(1, 1, &[[1, 2, 3, 255]]);
        assert_eq!(detect_palette_format(&strip), PaletteFormat::Png);
        assert_eq!(
            detect_palette_format(b"\xef\xbb\xbfGIMP Palette\n0 0 0\n"),
            PaletteFormat::Gpl
        );
        assert_eq!(
            detect_palette_format(b"  JASC-PAL\r\n0100\r\n0\r\n"),
            PaletteFormat::JascPal
        );
        assert_eq!(detect_palette_format(b"ff0000\n"), PaletteFormat::Hex);
        assert_eq!(detect_palette_format(b""), PaletteFormat::Hex);
    }

    #[test]
    fn read_palette_ignores_extension_and_dispatches_on_content() {
        let hex = read_palette(b"ff0000\n00ff00\n").expect("hex");
        assert_eq!(rgb(&hex), [(255, 0, 0), (0, 255, 0)]);
        let pal = read_palette(b"JASC-PAL\n0100\n1\n1 2 3\n").expect("pal");
        assert_eq!(rgb(&pal), [(1, 2, 3)]);
        let gpl = read_palette(b"GIMP Palette\n4 5 6 x\n").expect("gpl");
        assert_eq!(rgb(&gpl), [(4, 5, 6)]);
        let img = read_palette(&png(1, 1, &[[7, 8, 9, 255]])).expect("png");
        assert_eq!(rgb(&img), [(7, 8, 9)]);
    }

    #[test]
    fn read_hex_skips_comments_blank_lines_bom_and_whitespace() {
        let text = "\u{feff}; Lospec\r\n// c\r\n\r\n  #FF0044  \r\n00ff00\n\t0000ff\n123456\n";
        let p = read_hex(text).expect("parses");
        assert_eq!(p.len(), 4);
        assert_eq!(
            rgb(&p),
            [(255, 0, 68), (0, 255, 0), (0, 0, 255), (0x12, 0x34, 0x56)]
        );
        assert!(p.colors.iter().all(|e| e.rgba.a == 255 && e.name.is_none()));
    }

    #[test]
    fn read_hex_bad_line_names_its_line_number() {
        assert_eq!(
            read_hex("ff0000\n\nxyzxyz\n"),
            Err(PaletteFormatError::BadLine(3))
        );
        assert_eq!(read_hex("ff0000ff\n"), Err(PaletteFormatError::BadLine(1)));
        assert_eq!(read_hex("fff\n"), Err(PaletteFormatError::BadLine(1)));
        assert_eq!(read_hex("+f0000\n"), Err(PaletteFormatError::BadLine(1)));
    }

    #[test]
    fn read_jasc_pal_keeps_alpha_and_lets_colour_lines_win() {
        let text = "\u{feff}JASC-PAL\r\n0100\r\n16\r\n 255 0 68 \r\n0 255 0 128\r\n";
        let p = read_jasc_pal(text).expect("parses");
        assert_eq!(
            rgba(&p),
            [Rgba::new(255, 0, 68, 255), Rgba::new(0, 255, 0, 128)]
        );
        let fewer = read_jasc_pal("JASC-PAL\n0100\n1\n1 1 1\n2 2 2\n3 3 3\n").expect("parses");
        assert_eq!(fewer.len(), 3);
    }

    #[test]
    fn read_jasc_pal_bad_line_names_its_line_number() {
        for (text, line) in [
            ("JASC-PAL\n0100\n2\n1 2 3\n1 2\n", 5),
            ("JASC-PAL\n0100\n1\n1 2 300\n", 4),
            ("JASC-PAL\n0100\n1\n1 2 3 4 5\n", 4),
            ("JASC-PAL\n0100\n1\nred green blue\n", 4),
            ("JASC-PAL\n0100\nmany\n1 2 3\n", 3),
        ] {
            assert_eq!(
                read_jasc_pal(text),
                Err(PaletteFormatError::BadLine(line)),
                "{text:?}"
            );
        }
        assert_eq!(
            read_jasc_pal("JASC-PAL\n"),
            Err(PaletteFormatError::MissingPalHeader)
        );
    }

    #[test]
    fn palette_from_png_reads_distinct_colours_in_reading_order() {
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        let clear = [9, 9, 9, 0];
        let half = [0, 255, 0, 128];
        let two = palette_from_png(&png(2, 1, &[red, blue])).expect("2x1");
        assert_eq!(two.len(), 2);
        let img = png(3, 2, &[clear, blue, red, red, half, blue]);
        let p = palette_from_png(&img).expect("3x2");
        assert_eq!(
            rgba(&p),
            [
                Rgba::new(0, 0, 255, 255),
                Rgba::new(255, 0, 0, 255),
                Rgba::new(0, 255, 0, 128)
            ]
        );
    }

    #[test]
    fn palette_from_png_scaled_strip_gives_each_colour_once() {
        let colors = [
            Rgba::new(1, 2, 3, 255),
            Rgba::new(4, 5, 6, 255),
            Rgba::new(7, 8, 9, 255),
        ];
        let strip = palette_to_png(&palette(&colors), 8).expect("strip");
        assert_eq!(rgba(&palette_from_png(&strip).expect("reads")), colors);
    }

    #[test]
    fn readers_refuse_more_than_256_entries() {
        let hex = "000000\n".repeat(257);
        assert_eq!(read_hex(&hex), Err(PaletteFormatError::TooManyEntries));
        let pal = format!("JASC-PAL\n0100\n257\n{}", "0 0 0\n".repeat(257));
        assert_eq!(read_jasc_pal(&pal), Err(PaletteFormatError::TooManyEntries));
        let pixels: Vec<[u8; 4]> = (0..257u32)
            .map(|i| [i as u8, (i >> 8) as u8, 0, 255])
            .collect();
        assert_eq!(
            palette_from_png(&png(257, 1, &pixels)),
            Err(PaletteFormatError::NotPaletteImage)
        );
        assert_eq!(
            PaletteFormatError::NotPaletteImage.to_string(),
            "not a palette image (more than 256 colours)"
        );
        let gpl = format!("GIMP Palette\n{}", "0 0 0\n".repeat(257));
        assert_eq!(
            read_palette(gpl.as_bytes()),
            Err(PaletteFormatError::Gpl(GplError::TooManyEntries))
        );
    }

    #[test]
    fn readers_refuse_files_without_colours() {
        assert_eq!(read_palette(b""), Err(PaletteFormatError::NoColours));
        assert_eq!(
            read_palette(b"; only a comment\n\n"),
            Err(PaletteFormatError::NoColours)
        );
        assert_eq!(
            read_palette(b"JASC-PAL\n0100\n0\n"),
            Err(PaletteFormatError::NoColours)
        );
        assert_eq!(
            read_palette(b"GIMP Palette\n# none\n"),
            Err(PaletteFormatError::NoColours)
        );
        let clear = png(2, 1, &[[0, 0, 0, 0], [255, 255, 255, 0]]);
        assert_eq!(read_palette(&clear), Err(PaletteFormatError::NoColours));
        assert_eq!(
            PaletteFormatError::NoColours.to_string(),
            "no colours found"
        );
    }

    #[test]
    fn write_hex_is_lowercase_rgb_without_alpha_or_names() {
        let mut p = palette(&[Rgba::new(0xAB, 0xCD, 0xEF, 10), Rgba::new(0, 1, 2, 255)]);
        p.colors[0].name = Some("Sky".into());
        assert_eq!(write_hex(&p), "abcdef\n000102\n");
    }

    #[test]
    fn write_jasc_pal_has_header_count_and_three_columns() {
        let p = palette(&[Rgba::new(255, 0, 68, 128), Rgba::new(0, 1, 2, 255)]);
        let text = write_jasc_pal(&p);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, ["JASC-PAL", "0100", "2", "255 0 68", "0 1 2"]);
    }

    #[test]
    fn palette_to_png_is_one_row_of_cells_in_palette_order() {
        let colors = [Rgba::new(1, 2, 3, 255), Rgba::new(4, 5, 6, 7)];
        for cell in [1u32, 8] {
            let img = import_png(&palette_to_png(&palette(&colors), cell).expect("strip"))
                .expect("decodes");
            assert_eq!((img.width, img.height), (2 * cell, cell));
            let (px, _) = img.rgba.as_chunks::<4>();
            for (i, p) in px.iter().enumerate() {
                let x = (i as u32 % img.width) / cell;
                let c = colors[x as usize];
                assert_eq!(*p, [c.r, c.g, c.b, c.a], "cell {cell} pixel {i}");
            }
        }
        assert!(palette_to_png(&Palette::default(), 8).is_err());
        assert!(palette_to_png(&palette(&colors), 0).is_err());
    }

    #[test]
    fn hex_pal_and_png_strip_round_trip() {
        let p = Palette::preset(crate::document::PalettePreset::Db32);
        assert_eq!(rgb(&read_hex(&write_hex(&p)).expect("hex")), rgb(&p));
        assert_eq!(
            rgb(&read_jasc_pal(&write_jasc_pal(&p)).expect("pal")),
            rgb(&p)
        );
        let mut q = p.clone();
        q.colors[3].rgba.a = 100;
        let strip = palette_to_png(&q, 1).expect("strip");
        assert_eq!(rgba(&palette_from_png(&strip).expect("png")), rgba(&q));
    }

    #[test]
    fn png_strip_collapses_duplicate_entries() {
        let c = Rgba::new(5, 5, 5, 255);
        let strip = palette_to_png(&palette(&[c, c, Rgba::new(6, 6, 6, 255)]), 1).expect("strip");
        assert_eq!(palette_from_png(&strip).expect("reads").len(), 2);
    }
}
