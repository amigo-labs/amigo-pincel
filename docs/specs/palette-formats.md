---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §3.7, pincel.md §17.5, pincel.md §15 (2026-10-12 palette presets and .gpl)]
---

# Palette formats: Lospec `.hex`, JASC `.pal`, PNG strip

## Context

Most pixel-art palettes are shared on Lospec, which offers `.hex`, `.pal`, `.png` and `.gpl`. Aseprite and Pixelorama read all of these; Pincel reads and writes only the GIMP `.gpl` format (§3.7, Decision Log 2026-10-12).

Palette ▸ Import / Export gains three formats next to `.gpl`:

- **`.hex`** — Lospec's plain list, one `RRGGBB` per line.
- **`.pal`** — JASC-PAL, from Paint Shop Pro, Aseprite and Pyxel.
- **`.png`** — a palette image: every distinct colour in reading order becomes an entry, which covers Lospec's 1× and scaled strips.

Import recognizes the format by content, not by file extension alone. As with `.gpl`, an import replaces the whole palette through one undoable `ReplacePalette`.

## Scope

### In scope

- Reading and writing `.hex`, JASC `.pal` and PNG palette images in `pincel-core`
- Format detection by content for import
- One Import palette… entry for all formats, and an Export palette submenu (GIMP `.gpl`, Lospec `.hex`, JASC `.pal`, PNG strip)
- wasm functions for format-agnostic import and export; `importGpl` / `exportGpl` keep working
- File pickers and save filters for the new formats (browser and Tauri)
- en + de strings

### Out of scope

- Adobe `.ase` / `.aco` swatch files — because they are binary and rarely used for pixel art (backlog).
- Generating a palette from the current sprite's colours — because it is a separate feature, a good follow-up.
- Palette import on indexed sprites — because `ReplacePalette` refuses it on purpose until indexed painting can remap pixels (§3.7).

## Acceptance criteria

- [ ] AC-01 (event): When the user opens the Palette panel's menu, then it offers one Import palette… entry and an Export palette submenu with GIMP `.gpl`, Lospec `.hex`, JASC `.pal` and PNG strip.
- [ ] AC-02 (always): Import detects the format from the file content in this order: PNG signature → PNG image; first line `GIMP Palette` → `.gpl`; first line `JASC-PAL` → `.pal`; anything else → `.hex`. The file name's extension does not change the result (a `.txt` file with hex lines imports as `.hex`).
- [ ] AC-03 (event): When the user imports a `.hex` file, then each line `RRGGBB` (optionally prefixed with `#`) becomes one opaque entry in file order; blank lines and lines starting with `;` or `//` are skipped, and CRLF line ends are accepted (a 4-colour `.hex` gives 4 swatches).
- [ ] AC-04 (event): When the user imports a JASC `.pal` file (`JASC-PAL`, `0100`, a count, then `R G B` lines), then each colour line becomes one entry in file order; a fourth number on a line is kept as the entry's alpha, and when the count disagrees with the number of colour lines, the colour lines win.
- [ ] AC-05 (event): When the user imports a PNG image, then its pixels are read top to bottom, left to right, fully transparent pixels (alpha 0) are skipped, and the first occurrence of each distinct RGBA value becomes one entry (a 2×1 PNG with two colours gives 2 swatches; a strip scaled 8× gives each colour once).
- [ ] AC-06 (error): If a `.hex` or `.pal` colour line cannot be read (for `.hex` not six hex digits, for `.pal` not three or four numbers 0–255), then the import fails with an error naming the 1-based line number, and the palette is unchanged.
- [ ] AC-07 (error): If an imported file has more than 256 entries, or a PNG has more than 256 distinct non-transparent colours, then the import fails with an error (for PNG: "not a palette image") instead of truncating, and the palette is unchanged.
- [ ] AC-08 (error): If the sprite is indexed, then palette import is refused for every format, as `.gpl` import is today, and the palette is unchanged.
- [ ] AC-09 (event): When an import succeeds, then the whole palette is replaced in one undo step and the status bar reports the number of imported colours.
- [ ] AC-10 (event): When the user exports Lospec `.hex`, then the file has one lowercase `rrggbb` line per entry in palette order, without alpha.
- [ ] AC-11 (event): When the user exports JASC `.pal`, then the file starts with the lines `JASC-PAL`, `0100` and the entry count, followed by one `R G B` line (three columns) per entry.
- [ ] AC-12 (option): Where the user exports a PNG strip with a cell size of 1 or 8 px, the image is one row of `entries × cell` by `cell` pixels, each entry filling its cell with its RGBA value, in palette order.
- [ ] AC-13 (always): Writing a palette as `.hex` or `.pal` and reading it back gives the same RGB values in the same order; writing a palette without duplicate or fully transparent entries as a PNG strip and reading it back gives the same RGBA entries in the same order.
- [ ] AC-14 (always): The wasm functions `importGpl` and `exportGpl` keep their signatures and behavior.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**Codec template.** `crates/pincel-core/src/codec/gpl.rs`: `read_gpl(text) -> Result<Palette, GplError>` (≈L34; skips the BOM, comments, `Name:` / `Columns:`; alpha 255; capped at `MAX_PALETTE_ENTRIES` = 256 from `document/palette.rs` ≈L154), `write_gpl(palette, name)` (≈L77), `GplError { MissingHeader, BadLine(usize), TooManyEntries }` (≈L21), tests at the bottom. Re-exported from `codec/mod.rs` (≈L14/L21). Proposed: a new `codec/palette_formats.rs` with `read_hex` / `write_hex`, `read_jasc_pal` / `write_jasc_pal`, `palette_from_png` / `palette_to_png(palette, cell)`, one thiserror `PaletteFormatError` (AGENTS.md §5.1), and a content detector; `gpl.rs` stays as it is.

**PNG decode.** `import_png(bytes) -> ImportedImage { width, height, rgba }` (`crates/pincel-core/src/codec/png.rs` ≈L443) normalizes every PNG type, indexed included, to RGBA8. `encode_rgba8` (≈L377) can write the strip.

**Command.** `ReplacePalette` (`crates/pincel-core/src/command/palette_ops.rs` ≈L208) refuses indexed sprites (`require_rgba` ≈L109) and returns `PaletteFull` above the cap.

**wasm.** `crates/pincel-wasm/src/lib.rs`: `importGpl(text) -> u32` (≈L1465), `exportGpl(name) -> String` (≈L1474), private `replace_palette(palette)` (≈L1479; seal, execute, seal) to reuse. Proposed: `importPalette(bytes) -> Result<u32, String>` and `exportPalette(format, name) -> Vec<u8>`; mark `importGpl` / `exportGpl` deprecated in their doc comments (public npm API, §17.5).

**UI.** `ui/src/modes/pixel/components/PalettePanel.svelte`: `importGpl()` (≈L78), `exportGpl()` (≈L102), menu rows (≈L236–241). `ui/src/lib/fs/index.ts`: `PickKind = 'document' | 'gpl'` (≈L91), `GPL_TYPES` (≈L93), `ExportFormat` (≈L143), `EXPORT_FILTERS` (≈L153), `pickAndOpen` (≈L333), `saveExport`. Proposed: `PickKind 'palette'` with `gpl`, `hex`, `pal`, `png`; `ExportFormat` gains `'hex' | 'pal' | 'palette-png'`.

**i18n.** `palette.importGpl` / `palette.exportGpl` / `palette.imported` / `palette.exported` in `ui/src/lib/i18n/en.ts` (≈L686) and `de.ts` (≈L693).

**Fixtures.** Proposed under `crates/pincel-core/tests/fixtures/palettes/`. `.gitattributes` lists `*.png` as binary already; add `*.hex` and `*.pal` as text (AGENTS.md §6.4).

**e2e.** `ui/e2e/palette.spec.ts` (≈L52) imports via a file chooser with `setFiles({ name: 'grey.gpl', … })`. Proposed: import a 4-colour `.hex` → 4 swatches; export `.pal` → the download starts with `JASC-PAL`; import a 2×1 PNG → 2 swatches.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `.hex` file with a UTF-8 BOM or trailing whitespace | Not specified by the legacy plan — see Open questions |
| `.hex` line with eight digits (`RRGGBBAA`) | Bad line error (only six digits are the format) |
| `.pal` written by a tool with a fourth alpha column | Alpha kept per entry |
| `.pal` whose count line says 16 but 8 colour lines follow | 8 entries |
| PNG strip scaled 8× with grid lines between swatches in a seventh colour | That colour becomes an entry too — every distinct non-transparent colour counts |
| PNG with semi-transparent pixels | Kept as entries with their alpha |
| Palette with named entries exported as `.hex` / `.pal` | Names are dropped — the formats have no names |
| Palette entries with alpha below 255 exported as `.hex` / `.pal` | Alpha dropped — exported opaque |
| Palette with duplicate entries exported as a PNG strip and re-imported | Duplicates collapse to one entry |

## Open questions

- The Decision Log entry is not written yet: the content detection order, the PNG distinct-colour rule (error above 256 instead of truncating), and deprecating `importGpl` / `exportGpl`; the §3.7 palette paragraph also needs the new formats.
- What does importing a file with no colour entries do (an empty `.hex`, a `.pal` with only the header, a fully transparent PNG) — an error with the palette unchanged, or an empty palette?
- Does `.hex` import skip a UTF-8 BOM and surrounding whitespace, as `read_gpl` skips the BOM?
- How does the user choose the PNG strip's cell size of 1 or 8 px — two entries in the Export palette submenu, or a choice in a dialog?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- 2026-10-28 (legacy plan): Detection by content (PNG → gpl → pal → hex) — a mislabeled file still imports, and one picker serves all formats.
- 2026-10-28 (legacy plan): A PNG with more than 256 distinct colours is an error, not truncated — silently dropping colours would hide that the image is not a palette.
- 2026-10-28 (legacy plan): `.pal` is written in the classic three-column form — the most compatible variant; alpha is read but not written.
- 2026-10-28 (legacy plan): `gpl.rs` stays unchanged and the new formats live in their own module — keeps the existing codec and its tests stable.
- 2026-10-28 (legacy plan): `importGpl` / `exportGpl` stay and are only marked deprecated — the npm API is public (§17.5).
