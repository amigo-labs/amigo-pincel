---
id: todo-palette-formats
title: Palette formats — Lospec .hex, JASC .pal, PNG strip
status: todo
size: M (one session)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Palette formats: Lospec `.hex`, JASC `.pal`, PNG strip

## Goal

Palette ▸ Import / Export handles these formats in addition to `.gpl`:
- **`.hex`**: Lospec's plain list, one `RRGGBB` per line.
- **`.pal`**: JASC-PAL, from Paint Shop Pro, Aseprite and Pyxel.
- **`.png`**: a palette image. Every distinct color in reading order becomes an entry, which covers Lospec's 1× and scaled strips.

Import works by content, not by file extension alone.

## Why

Most pixel-art palettes are shared on Lospec, which offers `.hex`, `.pal`, `.png` and `.gpl`. Aseprite and Pixelorama read all of these. Pincel reads `.gpl` only.

## Current state (read these first)

**Codec.** `crates/pincel-core/src/codec/gpl.rs`
- `read_gpl(text) -> Result<Palette, GplError>` (≈L34): skips the BOM, comments, `Name:` and `Columns:`; entries get alpha 255; capped at `MAX_PALETTE_ENTRIES`.
- `write_gpl(palette, name)` (≈L77).
- `GplError { MissingHeader, BadLine(usize), TooManyEntries }`.
- Tests ≈L96–125. **Use this file as the template.**
- Exported from `codec/mod.rs`.

**PNG decode.** `import_png(bytes) -> ImportedImage { width, height, rgba }` (`crates/pincel-core/src/codec/png.rs` ≈L426–443) normalizes every PNG type to RGBA8, including indexed ones.

**Command.** `ReplacePalette` (`crates/pincel-core/src/command/palette_ops.rs` ≈L208–240) is refused on indexed sprites (`require_rgba`) and returns `PaletteFull` above the cap.

**wasm.** `crates/pincel-wasm/src/lib.rs`
- `importGpl(text) -> u32` (count) at ≈L1465, `exportGpl(name) -> String` at ≈L1474.
- The private `replace_palette(palette)` (seal, execute, seal) at ≈L1479. **Reuse it.**

**UI.** `ui/src/modes/pixel/components/PalettePanel.svelte`
- `importGpl()` (≈L78–100): `pickAndOpen('gpl')`, decode the text, `edit(..., d => count = d.importGpl(text))`, then a status message.
- `exportGpl()` (≈L102–118).
- Menu items ≈L236–241.

**File layer.** `ui/src/lib/fs/index.ts`: `PickKind = 'document' | 'gpl'` (≈L91), `GPL_TYPES`, `PICK.gpl`, `ExportFormat` (≈L143), `EXPORT_FILTERS.gpl` (≈L158), `pickAndOpen` (≈L333), `saveExport` (≈L561).

**i18n.** `palette.importGpl` / `palette.exportGpl` / `palette.imported` / `palette.exported` (`en.ts` ≈L676–689, `de.ts` ≈L683–696).

**e2e.** `ui/e2e/palette.spec.ts` (≈L31–64) uses a file chooser with `setFiles({ name: 'grey.gpl', … })`.

## Design

**New codec module** `crates/pincel-core/src/codec/palette_formats.rs`. Keep `gpl.rs` as it is and re-export everything from `codec/mod.rs`.

**`read_hex(text)`**
- One color per line, `RRGGBB` with an optional `#`.
- Ignore blank lines and lines starting with `;` or `//`.
- Accept CRLF.
- Error `BadLine(n)`.

**`write_hex(palette)`**: lowercase `rrggbb`, one per line, alpha dropped (the format has none).

**`read_jasc_pal(text)`**
- Header lines `JASC-PAL` and `0100`, then the count, then `R G B` lines.
- Some writers add a 4th alpha column: accept it and keep it.
- If the count disagrees with the number of lines, trust the lines.
- Error variants: `MissingHeader` and `BadLine`.

**`write_jasc_pal(palette)`**: the classic 3-column form, for compatibility.

**`palette_from_png(bytes)`**
- Decode with `import_png`.
- Walk pixels top to bottom, left to right.
- Skip fully transparent pixels and keep the first occurrence of each RGBA. That handles scaled swatches and grids.
- Error if there are more than `MAX_PALETTE_ENTRIES` distinct colors ("not a palette image") instead of truncating silently.

**`palette_to_png(palette, cell: u32)`**: a one-row strip, `cell` px per entry. Offer cell 1 (Lospec 1×) and 8.

**Errors.** One `PaletteFormatError` enum with thiserror (CLAUDE.md §5.1).

**Format detection on import.** One picker for all palette files.
- Content check order:
  1. PNG magic → PNG.
  2. First line `GIMP Palette` → gpl.
  3. `JASC-PAL` → pal.
  4. Otherwise try hex.
- So a mislabeled `.txt` still works.

**wasm.**
- `importPalette(bytes: &[u8]) -> Result<u32, String>` does the detection, then `replace_palette`.
- `exportPalette(format: &str, name: &str) -> Vec<u8>`.
- Keep `importGpl` / `exportGpl` working, and **mark them deprecated in their doc comments** (the npm API is public, §17.5).

**UI.**
- `PickKind 'palette'` with the extensions `gpl`, `hex`, `pal` and `png` (and the Tauri filters).
- `ExportFormat` gains `'hex' | 'pal' | 'palette-png'`.
- The Palette ⋯ menu: **Import palette…** (one entry, any format) and an **Export palette** submenu (GIMP `.gpl`, Lospec `.hex`, JASC `.pal`, PNG strip).
- i18n keys in en + de.

## Tasks (one M session)

1. **`docs(spec)`.** Update the §3.7 Palette paragraph (formats) and add a Decision Log entry (the detection order, PNG distinct-color rule, deprecating `importGpl` / `exportGpl`).
2. **`feat(pincel-core)`.** The codec and its tests:
   - `hex_round_trip`
   - `hex_accepts_hash_crlf_and_comments`
   - `jasc_round_trip`
   - `jasc_accepts_alpha_column`
   - `jasc_count_mismatch_uses_lines`
   - `png_strip_scaled_8x_yields_each_color_once`
   - `png_skips_transparent`
   - `png_over_256_colors_is_error`
   - `detect_format_by_content`

   Add small fixtures under `crates/pincel-core/tests/fixtures/palettes/`, and **add `*.hex`, `*.pal` to `.gitattributes` as text and `*.png` as binary** if they aren't listed yet (CLAUDE.md §6.4).
3. **`feat(pincel-wasm)` + `feat(ui)`.**
   - The wasm functions, pickers, menu and i18n.
   - e2e in `ui/e2e/palette.spec.ts`: import a 4-color `.hex` → the palette has 4 swatches; export `.pal` → the download starts with `JASC-PAL`; import a 2×1 PNG → 2 swatches.

## Out of scope

- Adobe `.ase` / `.aco` swatch files (binary, rarely used for pixel art; backlog).
- Palette from the current sprite's colors ("Generate palette from sprite"); a good follow-up.
- Palette import on indexed sprites, which `ReplacePalette` refuses on purpose.
