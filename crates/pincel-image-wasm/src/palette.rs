//! Image mode's palette files (`docs/specs/image-palette-gradient.md`):
//! load and save the Colors panel palette through `pincel-palette`, the
//! codecs Pixel mode uses too, so one file reads the same in both modes.
//! The palette itself is app state in the UI, never document state, so
//! these functions take and return entries and touch no document.

use pincel_palette::{PaletteEntry, Rgba};
use wasm_bindgen::prelude::*;

/// A palette read from a file: packed `0xRRGGBBAA` colors and their names
/// (`""` for an unnamed entry), in file order.
#[wasm_bindgen]
pub struct PaletteFile {
    colors: Vec<u32>,
    names: Vec<String>,
}

#[wasm_bindgen]
impl PaletteFile {
    /// The entries' colors as packed `0xRRGGBBAA`.
    #[wasm_bindgen(getter)]
    pub fn colors(&self) -> Vec<u32> {
        self.colors.clone()
    }

    /// The entries' names, `""` where an entry has none.
    #[wasm_bindgen(getter)]
    pub fn names(&self) -> Vec<String> {
        self.names.clone()
    }
}

fn read(bytes: &[u8]) -> Result<PaletteFile, String> {
    let entries = pincel_palette::read_palette(bytes).map_err(|e| e.to_string())?;
    Ok(PaletteFile {
        colors: entries.iter().map(|e| e.rgba.to_u32()).collect(),
        names: entries
            .into_iter()
            .map(|e| e.name.unwrap_or_default())
            .collect(),
    })
}

fn write(
    colors: &[u32],
    names: &[String],
    format: &str,
    name: &str,
    cell: u32,
) -> Result<Vec<u8>, String> {
    let entries: Vec<PaletteEntry> = colors
        .iter()
        .enumerate()
        .map(|(i, &c)| match names.get(i).filter(|n| !n.is_empty()) {
            Some(n) => PaletteEntry::with_name(Rgba::from_u32(c), n.clone()),
            None => PaletteEntry::new(Rgba::from_u32(c)),
        })
        .collect();
    match format {
        "gpl" => Ok(pincel_palette::write_gpl(&entries, name).into_bytes()),
        "hex" => Ok(pincel_palette::write_hex(&entries).into_bytes()),
        "pal" => Ok(pincel_palette::write_jasc_pal(&entries).into_bytes()),
        "png" => pincel_palette::palette_to_png(&entries, cell).map_err(|e| e.to_string()),
        other => Err(format!("unknown palette format `{other}`")),
    }
}

/// Read a palette file — GIMP `.gpl`, Lospec `.hex`, JASC `.pal` or a PNG
/// palette image, detected from the content, not the file name. Errors
/// carry the reader's reason (a line number, a missing header, the
/// 256-entry limit, "no colours found", a PNG decode failure, "not a
/// palette image").
#[wasm_bindgen]
pub fn read_palette_file(bytes: &[u8]) -> Result<PaletteFile, JsError> {
    read(bytes).map_err(|e| JsError::new(&e))
}

/// Write a palette as a file: `format` is `gpl` (a GIMP palette called
/// `name`, keeping entry names), `hex` (Lospec), `pal` (JASC) or `png` (a
/// one-row strip of `cell` × `cell` px swatches). `colors` are packed
/// `0xRRGGBBAA`; `names` lines up with them, `""` for an unnamed entry.
/// Errors for an unknown format, or a PNG strip of an empty palette or
/// with `cell` 0.
#[wasm_bindgen]
pub fn write_palette_file(
    colors: &[u32],
    names: Vec<String>,
    format: &str,
    name: &str,
    cell: u32,
) -> Result<Vec<u8>, JsError> {
    write(colors, &names, format, name, cell).map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(colors: &[u32]) -> (Vec<u32>, Vec<String>) {
        (colors.to_vec(), vec![String::new(); colors.len()])
    }

    #[test]
    fn read_detects_the_format_from_the_content() {
        let hex = read(b"\xef\xbb\xbf; Lospec\r\n#FF0044\r\n00ff00\r\n").expect("hex in a .txt");
        assert_eq!(hex.colors, [0xff00_44ff, 0x00ff_00ff]);
        let pal = read(b"JASC-PAL\n0100\n2\n1 2 3\n4 5 6 0\n").expect("pal");
        assert_eq!(pal.colors, [0x0102_03ff, 0x0405_0600]);
        let gpl = read(b"GIMP Palette\n0 0 0 Black\n255 255 255\n").expect("gpl");
        assert_eq!(gpl.colors, [0x0000_00ff, 0xffff_ffff]);
        assert_eq!(gpl.names, ["Black", ""]);
        let strip = write(&[0x1122_3380, 0x4455_66ff], &[], "png", "", 1).expect("strip");
        assert_eq!(
            read(&strip).expect("png").colors,
            [0x1122_3380, 0x4455_66ff]
        );
    }

    #[test]
    fn read_unreadable_file_names_the_reason() {
        assert_eq!(
            read(b"ff0000\nnope\n").err().as_deref(),
            Some("line 2: not a colour")
        );
        assert_eq!(
            read(b"GIMP Palette\n1 2 300\n").err().as_deref(),
            Some("line 2: expected `R G B [name]` with values 0–255")
        );
        assert_eq!(
            read(b"JASC-PAL\n").err().as_deref(),
            Some("not a JASC palette (missing `0100` or count line)")
        );
        assert_eq!(
            read("000000\n".repeat(257).as_bytes()).err().as_deref(),
            Some("palette has more than 256 colours")
        );
        assert_eq!(read(b"").err().as_deref(), Some("no colours found"));
        let strip = write(&[0x0102_03ff], &[], "png", "", 1).expect("strip");
        let truncated = read(&strip[..20]).err().unwrap_or_default();
        assert!(truncated.starts_with("png decode failed"), "{truncated}");
    }

    #[test]
    fn write_then_read_round_trips_every_format() {
        let (colors, names) = entries(&[0xff00_44ff, 0x1122_3380, 0x0000_00ff]);
        for (format, cell, alpha_kept) in [
            ("gpl", 1, false),
            ("hex", 1, false),
            ("pal", 1, false),
            ("png", 1, true),
            ("png", 8, true),
        ] {
            let bytes = write(&colors, &names, format, "palette", cell).expect(format);
            let back = read(&bytes).expect(format);
            let expected: Vec<u32> = colors
                .iter()
                .map(|&c| if alpha_kept { c } else { c | 0xff })
                .collect();
            assert_eq!(back.colors, expected, "{format} {cell}");
        }
    }

    #[test]
    fn write_gpl_keeps_entry_names() {
        let names = vec!["Black".to_owned(), String::new()];
        let bytes = write(&[0x0000_00ff, 0xffff_ffff], &names, "gpl", "palette", 1).expect("gpl");
        let text = String::from_utf8(bytes).expect("utf-8");
        assert!(text.starts_with("GIMP Palette\nName: palette\n"), "{text}");
        assert!(text.contains("  0   0   0\tBlack\n"), "{text}");
        // An unnamed entry is labelled with its hex code, as GIMP (and
        // Pixel mode's export) does.
        assert_eq!(
            read(text.as_bytes()).expect("reads").names,
            ["Black", "#ffffff"]
        );
    }

    #[test]
    fn write_refuses_unknown_format_and_empty_strip() {
        assert!(write(&[0xff], &[], "aco", "", 1).is_err());
        assert!(write(&[], &[], "png", "", 8).is_err());
        assert!(write(&[0xff], &[], "png", "", 0).is_err());
    }
}
