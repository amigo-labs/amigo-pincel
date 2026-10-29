---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §3.7, pincel.md §9.2, pincel.md §9.4]
---

# Pixel palette and foreground / background colors

## Context

A Pixel-mode document carries a palette of up to 256 entries (`pincel.md` §3.7). In RGBA documents — the only ones Pincel paints today — it is a swatch set: no pixel references an entry, so editing it never changes the image. The Palette panel shows the entries as swatches, picks the foreground or background color from them, and edits the palette through undoable commands: recolor, append, remove, reorder, load a built-in preset, import and export a GIMP `.gpl` file.

The foreground / background pair (shared `ColorPair` component, §9.2) holds the two colors the tools paint with, plus a foreground alpha; X swaps them and D resets them (§9.4).

This spec records the behavior as built. Already decided in §3.7: a new document starts with DB32; remove, reorder and whole-palette replacement are refused on indexed documents until indexed painting lands (Phase 2).

## Scope

### In scope

- The palette model: entries with RGBA and optional name, the 256-entry cap, the DB32 default, the five presets
- The Palette panel: swatches, picking foreground / background, the entry editor (recolor, move, delete), drag-to-reorder, add, the options menu (presets, `.gpl` import / export)
- Undo of every palette edit
- The foreground / background pair: swatch pickers, hex field, foreground alpha, swap, reset, persistence across sessions
- The wasm palette surface (`paletteCount`, `paletteColor`, `paletteName`, edits, presets, `.gpl`)

### Out of scope

- Palette formats other than `.gpl` (Lospec `.hex`, JASC `.pal`, PNG strip) — because the `palette-formats` spec covers them
- Pincel's own color picker (HSV square, background alpha, recent colors) — because the planned `color-picker` spec replaces the native picker
- Painting with indexed colors and palette-index pixels — because indexed painting is Phase 2 (§3.7)
- Reading and writing the palette in `.aseprite` files — because the `aseprite-round-trip` spec owns the codec
- The Eyedropper and which color a stroke paints with — because the `pixel-drawing-tools` spec owns the tools

## Acceptance criteria

- [ ] AC-01 (event): When a new Pixel document is created, its palette is DB32: 32 opaque entries, entry 0 black and entry 21 white, shown as 32 swatches.
- [ ] AC-02 (event): When the add button of the Palette panel is pressed, an entry with the current foreground color and foreground alpha is appended as the last entry and becomes the selected entry.
- [ ] AC-03 (error): If an entry is added to a palette that already holds 256 entries, then the add is refused with an error and the palette is unchanged.
- [ ] AC-04 (state): While the palette holds 256 entries, the add button is disabled.
- [ ] AC-05 (event): When a swatch is clicked, the foreground color and foreground alpha take that entry's color and alpha, and the entry editor opens for that entry, showing its index.
- [ ] AC-06 (event): When a swatch is right-clicked, the background color and background alpha take that entry's color and alpha, and no context menu opens.
- [ ] AC-07 (state): While the foreground color's RGB equals a swatch's RGB, that swatch is marked active (pressed); alpha is not compared.
- [ ] AC-08 (event): When the entry editor's color is changed, the selected entry takes the new RGB; all changes made before the picker closes form one undo step.
- [ ] AC-09 (event): When "Move entry left" or "Move entry right" is pressed, the selected entry moves one position in that direction, stays selected, and the entries in between shift by one.
- [ ] AC-10 (event): When a swatch is dragged onto another swatch, the dragged entry moves to the target's position.
- [ ] AC-11 (event): When "Delete entry" is pressed, the selected entry is removed and the entry editor closes.
- [ ] AC-12 (always): Every palette edit — add, recolor, move, delete, preset, `.gpl` import — is one undo step, and undo restores the palette as it was before the edit, including entry names.
- [ ] AC-13 (event): When a preset is chosen from the Palette options menu, the whole palette is replaced by it in one undo step; the presets are DawnBringer 32 (32 entries), DawnBringer 16 (16), PICO-8 (16), Game Boy (4) and NES (54).
- [ ] AC-14 (event): When a GIMP `.gpl` file is imported, the palette is replaced by its colors in file order — alpha 255, names kept, `Name:` / `Columns:` headers, `#` comments and blank lines skipped — and the status bar reports how many colors were imported.
- [ ] AC-15 (error): If an imported file lacks the `GIMP Palette` header, has a line that is not `R G B [name]` with values 0–255, or holds more than 256 colors, then the import is refused with an error and the palette is unchanged.
- [ ] AC-16 (event): When the palette is exported as `.gpl`, a file named after the document (`<name>.gpl`) is saved that starts with `GIMP Palette`, carries a `Name:` line and one `R G B<TAB>name` row per entry, alpha dropped; importing it reproduces the colors and names.
- [ ] AC-17 (error): If the document is indexed, then removing, moving or replacing the palette (preset, import) is refused with an error and the palette is unchanged.
- [ ] AC-18 (error): If a recolor, move or remove names an index past the last entry, then it is refused with an error and the palette is unchanged.
- [ ] AC-19 (always): Reading an entry returns its color as `0xRRGGBBAA` and its name, an unnamed entry returning an empty name; reading an index past the last entry is an error.
- [ ] AC-20 (event): When D is pressed, the foreground becomes opaque black and the background opaque white; when X is pressed, foreground and background (color and alpha) swap.
- [ ] AC-21 (event): When a full `#RRGGBB` or `RRGGBB` is typed into the hex field, the foreground color takes it; an incomplete value changes nothing, and leaving the field shows the current foreground again.
- [ ] AC-22 (event): When the alpha slider is moved, the foreground alpha takes its value (0–255) and the value is shown next to the slider.
- [ ] AC-23 (event): When the Pixel editor is reopened in a later session, the foreground and background colors from the previous session are restored (RGB only; alphas start at 255).

## Technical notes

> Paths are hints from 2026-10-29 and drift — grep for the names.

- Model (`crates/pincel-core/src/document/palette.rs`): `Palette { colors: Vec<PaletteEntry> }`, `PaletteEntry { rgba: Rgba, name: Option<String> }`, `MAX_PALETTE_ENTRIES = 256`, `Palette::db32()`, `Palette::preset(PalettePreset)`, `PalettePreset::{Db32, Db16, Pico8, GameBoy, Nes}` with ids `db32`, `db16`, `pico8`, `gameboy`, `nes`.
- Commands (`crates/pincel-core/src/command/palette_ops.rs`): `SetPaletteColor` (merges consecutive edits of the same index), `AddPaletteEntry` (`PaletteFull`), `RemovePaletteEntry`, `MovePaletteEntry`, `ReplacePalette` (`UnsupportedColorMode` on non-RGBA; `UnknownPaletteEntry(i)` for bad indices). All report `DirtyRegion::None`.
- Codec (`crates/pincel-core/src/codec/gpl.rs`): `read_gpl(text) -> Result<Palette, GplError>`, `write_gpl(palette, name) -> String`; `GplError::{MissingHeader, BadLine(usize), TooManyEntries}`.
- wasm (`crates/pincel-wasm/src/lib.rs`): `paletteCount`, `paletteColor(i)`, `paletteName(i)`, `setPaletteColor(i, rgba)` (merges until `endStroke()`), `addPaletteEntry(rgba) -> index`, `removePaletteEntry(i)`, `movePaletteEntry(from, to)`, `setPalettePreset(id)`, `importGpl(text) -> count`, `exportGpl(name) -> string`, free function `palettePresetNames()`. Add / remove / move / replace seal the undo bus on both sides.
- UI: `ui/src/modes/pixel/components/PalettePanel.svelte` (props `doc`, `rev`, `activeColor`, `addColor`, `onPick`, `onPickBg`, …), `ui/src/lib/components/ColorPair.svelte`, `swapColors` / `resetColors` in `ui/src/modes/pixel/doc-ops.ts`, color state `colors { color, alpha, bgColor, bgAlpha }` in `ui/src/modes/pixel/stores/editor.svelte.ts` (initial foreground `#f87171`), prefs `COLOR_PREF` / `BG_COLOR_PREF` in `PixelEditor.svelte`. Keys `swapColors: x`, `resetColors: d` in `ui/src/lib/shell/shortcuts.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Opened file with an empty palette | The panel shows the "Empty palette" hint; `+` still appends |
| Recolor of an entry with alpha < 255 | The entry keeps its alpha; only RGB changes |
| Move left on the first entry / right on the last | Button disabled; nothing happens |
| Drag a swatch onto itself | No change, no undo step |
| `.gpl` file with a UTF-8 BOM or Windows line endings | Imported like any other |
| `.gpl` import cancelled in the file picker | Palette and status unchanged |
| Preset chosen while an entry is selected | Selection is cleared |
| Two swatches share the foreground RGB | Both are marked active |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-wasm: tests::fresh_document_seeds_the_db32_palette`; `pincel-core: document::palette::tests::db32_has_32_opaque_entries_from_black_to_olive`; `ui/e2e/palette.spec.ts › palette: DB32 by default, add / recolor / move / delete with undo` | built | |
| AC-02 |      | open   | |
| AC-03 | `pincel-core: command::palette_ops::tests::add_palette_entry_to_full_palette_returns_err` | built | |
| AC-04 |      | open   | |
| AC-05 |      | open   | |
| AC-06 |      | open   | |
| AC-07 |      | open   | |
| AC-08 | `ui/e2e/palette.spec.ts › palette: DB32 by default, add / recolor / move / delete with undo`; `pincel-wasm: tests::palette_edits_route_through_undo`; `pincel-core: command::palette_ops::tests::set_palette_color_applies_reverts_and_merges` | built | |
| AC-09 | `ui/e2e/palette.spec.ts › palette: DB32 by default, add / recolor / move / delete with undo`; `pincel-core: command::palette_ops::tests::move_palette_entry_shifts_between_and_reverts` | built | |
| AC-10 |      | open   | |
| AC-11 | `ui/e2e/palette.spec.ts › palette: DB32 by default, add / recolor / move / delete with undo` | built | |
| AC-12 | `pincel-wasm: tests::palette_edits_route_through_undo`, `tests::palette_presets_and_gpl_round_trip_through_undo`; `pincel-core: command::palette_ops::tests::add_palette_entry_appends_and_reverts`, `remove_palette_entry_round_trips_with_name`, `replace_palette_swaps_and_reverts` | built | |
| AC-13 | `ui/e2e/palette.spec.ts › palette presets and .gpl › a preset replaces the palette in one undo step`; `pincel-core: document::palette::tests::preset_sizes_and_names_round_trip` | built | |
| AC-14 | `ui/e2e/palette.spec.ts › palette presets and .gpl › .gpl import and export`; `pincel-core: codec::gpl::tests::read_skips_headers_comments_and_blank_lines` | built | |
| AC-15 |      | open   | |
| AC-16 | `ui/e2e/palette.spec.ts › palette presets and .gpl › .gpl import and export`; `pincel-core: codec::gpl::tests::write_then_read_round_trips_colours_and_names` | built | |
| AC-17 | `pincel-core: command::palette_ops::tests::remove_and_move_refuse_indexed_documents`, `replace_palette_refused_on_indexed_sprites` | built | |
| AC-18 |      | open   | |
| AC-19 | `pincel-wasm: tests::palette_getters_read_colors_and_names`, `tests::palette_getters_error_out_of_range` | built | |
| AC-20 |      | open   | |
| AC-21 |      | open   | |
| AC-22 |      | open   | |
| AC-23 |      | open   | |

## Decisions

