---
status: todo
created: 2026-11-17
size: L
mode: Pixel
design: [pincel.md §3.3, pincel.md §3.6, pincel.md §7.3]
---

# Sprite-sheet import

## Context

Game assets are often handed around as sprite sheets: one PNG with the frames of a character laid out in a grid, one animation per row (idle, walk, punch, …), rows of different lengths, empty cells at the row ends and often an opaque background colour instead of transparency. Pincel exports such sheets (`pixel-export.md`: fixed-cell grid, Aseprite `--data` JSON) but cannot read one back; today a sheet opens as a single huge frame.

File ▸ Import Sprite Sheet… opens a PNG sheet in a dialog with a preview, lets the user set the grid (with a detected suggestion), turns the sheet's background colour transparent, and creates a **new** Pixel-mode document: the cell size becomes the canvas, every used cell a frame in reading order, every row a tag.

Owner decisions of 2026-11-17: new document (not appended to the open one); grid with an automatic suggestion (not free bounding-box detection); one tag per row, on by default; empty cells at the row ends are skipped; background colour to transparent and one frame duration for all frames are options of the dialog; the Aseprite JSON sidecar is not read.

## Scope

### In scope

- File ▸ Import Sprite Sheet… in the File menu of both editors; the result opens as a new Pixel-mode document tab
- PNG sheets (any colour type and bit depth the Pixel core's PNG reader accepts, converted to 8-bit RGBA; of an APNG the default image)
- Dialog with a preview of the whole sheet, scaled to fit the dialog with nearest-neighbour sampling, and the grid drawn over it
- Grid fields: cell width, cell height, offset X / Y (from the sheet's top-left to the first cell), spacing X / Y (gap between cells)
- A grid suggestion computed when the sheet is loaded, and a "Detect" button that applies it again
- "Make background transparent" with a colour (hex field and colour picker)
- Frame duration in ms for every imported frame
- "Rows as tags"

### Out of scope

- Appending the frames to the open document — because the owner chose a new document; the open document's canvas size would rarely match the cells
- Free (bounding-box) sprite detection for irregular sheets — because the owner chose the grid; irregular sheets can still be imported with a grid larger than the biggest sprite
- Reading an Aseprite `--data` JSON sidecar — because the owner left it out for now; it stays in the backlog
- Sheet formats other than PNG (GIF, WebP, JPEG, …) — because the Pixel core decodes PNG only, and sprite sheets are distributed as PNG
- Columns as tags, per-row frame durations, tag naming in the dialog — because tags and durations are editable in the timeline after the import
- Trimming or re-aligning sprites inside their cells — because the cell is the frame; the importer keeps every cell's pixels where they are
- Picking the background colour from the preview — because the app never reads canvas pixels (AGENTS.md §9); the colour comes from the core and is changed through the hex field or picker
- Agent / MCP tools for the import — because agents already build frames and cels with the existing tools; a separate spec if wanted

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses File ▸ Import Sprite Sheet… in either mode and picks a PNG file, then the Import sprite sheet dialog opens with a preview of the sheet, the grid fields filled with the suggested grid (or, when none is detected, the values of AC-10) and the grid drawn over the preview.
- [ ] AC-02 (always): The grid has cells of `cell width × cell height`, the first cell's top-left at `(offset X, offset Y)`, and each next cell `cell width + spacing X` to the right and `cell height + spacing Y` below; only cells that lie completely inside the sheet count. A sheet row is one row of grid cells.
- [ ] AC-03 (event): When the user changes a grid field or an option, then the grid over the preview and the dialog's summary ("n frames, m tags") follow the new values without closing the dialog.
- [ ] AC-04 (event): When the user confirms the dialog, then a new Pixel-mode document opens in its own tab whose canvas is `cell width × cell height`, with one image layer named "Layer 1" and one frame per used cell, in row-major order (left to right, top to bottom), each frame holding exactly its cell's pixels.
- [ ] AC-05 (always): A cell is empty when every pixel in it is fully transparent after the background step. A cell is used unless it is empty and no non-empty cell follows it in its row; so empty cells at a row's end and fully empty rows produce no frame, while an empty cell between non-empty cells of a row becomes an empty frame.
- [ ] AC-06 (option): Where "Rows as tags" is on (default), every sheet row with at least one used cell becomes one tag spanning that row's frames, direction Forward (Aseprite's default), named "Row k" where k is the row's 1-based index in the grid (document content, not translated); where it is off, the document has no tags.
- [ ] AC-07 (option): Where "Make background transparent" is on, every sheet pixel exactly equal in all four RGBA channels to the chosen colour becomes fully transparent before cells are cut and tested for emptiness. When the dialog opens, the colour is the sheet's top-left pixel, and the option is on if that pixel is fully opaque and off otherwise.
- [ ] AC-08 (always): Every imported frame has the duration from the dialog's frame-duration field, an integer from 1 to 65535 ms, default 100 ms (the duration of a new frame in Pixel mode).
- [ ] AC-09 (always): Where the sheet (after the background step) is made of sprites placed in a grid of P × Q px cells with offset (X, Y) and spacing 0, each sprite's non-transparent pixels (alpha > 0) inside its cell at the same position within the cell for every sprite, at least one fully transparent column and row between neighbouring sprites, and at least one pair of sprites in horizontally adjacent cells and one pair in vertically adjacent cells, then the suggested grid is P × Q with spacing 0 and an offset at which every sprite lies inside one cell.
- [ ] AC-18 (always): A fully transparent sheet, a sheet with a single sprite, and a sheet with no fully transparent column or no fully transparent row after the background step get no suggestion. For sheets that fit neither this nor AC-09, any suggestion or none is acceptable.
- [ ] AC-10 (state): While no grid is suggested, the dialog shows "No grid detected" and the grid fields hold the values of the last confirmed import in this app session, or 32 × 32, offset 0, spacing 0 when there was none (a common pixel-art cell size). Frame duration and "Rows as tags" likewise start with the last confirmed import's values, or 100 ms and on; the background option and colour always come from the sheet (AC-07).
- [ ] AC-11 (event): When the user presses "Detect", then the fields are set to the suggestion for the current background setting, or "No grid detected" is shown and the fields stay as they are.
- [ ] AC-12 (error): If the grid yields no complete cell, all cells are empty, it would create more than 65535 frames, or a field is out of range or not a whole number (cell size 1–4096; offset X 0 to sheet width − 1, offset Y 0 to sheet height − 1; spacing X 0 to sheet width, spacing Y 0 to sheet height; duration 1–65535), then Import is disabled and the dialog says which of these applies; no document is created.
- [ ] AC-13 (error): If the chosen file is not a decodable PNG, the sheet is wider or taller than 16384 px or has more than 67 108 864 pixels, or memory runs out while decoding or creating the document, then a notice says the file cannot be read as a sprite sheet (naming the size limit when it was exceeded), no document is created and the open documents are unchanged.
- [ ] AC-14 (event): When the user cancels the file picker or the dialog, then no document is created and the open documents are unchanged.
- [ ] AC-15 (always): The new document is unsaved, named after the sheet's file name with its extension replaced by `.aseprite` (`hero.png` → `hero.aseprite`), has the RGBA colour mode and the palette a document from File ▸ New gets, and its first Save asks for a location. A name already open in another tab is allowed, as for any open.
- [ ] AC-16 (always): A sheet written by Pincel's Export Sprite Sheet (all frames, no tag filter, scale 1) and imported with cell size = the exported canvas size, offset 0, spacing 0 and "Make background transparent" off gives back the same frames with the same pixels in the same order, provided no exported frame is fully transparent.
- [ ] AC-17 (always): Every user-visible string of the dialog and the menu entry has a translated key in every UI language's string table (the existing key-parity check passes); the tag names of AC-06 are document content and not translated.

## Technical notes

- The cutting and the grid suggestion belong in the Pixel core and are reached through the Pixel wasm adapter; the dialog gets the suggestion, the top-left colour and the summary from there and never reads canvas pixels (AGENTS.md §9). The preview shows the RGBA pixels the core decoded (written to a canvas, never read back), so the preview, the background colour and the cut frames agree even for APNG or colour-managed PNGs the browser would show differently.
- Menu: File menu of both editors, next to the import entries (`menu-structure.md` order).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Sheet width not a multiple of the cell width | Partial cells at the right / bottom edge are ignored (AC-02) |
| A row in the middle of the sheet is completely empty | No frames and no tag for it; later rows keep their grid row number in the tag name ("Row 13" after an empty row 12) |
| A sprite pokes one pixel into the neighbouring cell | That pixel lands in the neighbour's frame; the neighbour cell is not empty, so it is a used cell (AC-05) |
| Background option on and a sprite uses the background colour itself (white eyes on a white sheet) | Those pixels become transparent too — exact colour match, no flood fill; the user can switch the option off |
| Top-left pixel is semi-transparent | The option starts off; if switched on, exact RGBA matching still applies |
| Cell size equals the sheet size | One frame, one tag "Row 1" (with rows as tags) |
| 1 × 1 cells on a 4096 × 4096 sheet | More than 65535 frames → Import disabled with that reason (AC-12) |
| Exported sheet whose frames touch their cell edges | No suggestion (AC-09 does not hold); the user types the canvas size, as in AC-16 |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| At most 65535 frames per import | Aseprite stores the frame count as 16 bit, so a larger document could not be saved | Low; real sheets have dozens to hundreds of frames |
| Cell size capped at 4096 per side | Same cap as File ▸ New for Pixel documents | Low |
| Sheet capped at 16384 px per side and 67 108 864 pixels | Same per-side cap as the exports; the area cap keeps the decoded sheet at 256 MiB inside the 4 GiB wasm32 address space next to the new document | Low |
| Exact RGBA match for the background colour, no tolerance | Pixel art uses exact colours; a tolerance would eat anti-aliased edges | Medium for JPEG-born sheets; a tolerance can be added later |
| Dialog values of the last confirmed import are kept for the app session, not stored (AC-10) | No owner request for persistence; avoids a new IndexedDB pref | Low |
| Tags get the default tag colour | Tag colours are editable in the timeline | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode as a new tagged document`, `imports from Image mode, keeps a name already open and drops the tags when asked` | built |  |
| AC-02 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `grid_columns_and_rows_count_only_complete_cells`, `cut_offset_and_spacing_grid_frames_hold_their_cell_pixels` | built |  |
| AC-03 | `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode …` (grid overlay `data-grid` / `data-cells`, screenshot changes, summary); `crates/pincel-wasm/src/sheet_import.rs`: `summary_counts_frames_and_tags_or_names_the_error` | built |  |
| AC-04 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `cut_offset_and_spacing_grid_frames_hold_their_cell_pixels`; `crates/pincel-wasm/src/sheet_import.rs`: `build_creates_one_frame_per_used_cell_with_its_pixels`; `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode …` | built |  |
| AC-05 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `plan_skips_trailing_empty_cells_and_empty_rows_keeps_inner_gaps` | built |  |
| AC-06 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `row_tags_name_rows_by_grid_index_and_span_their_frames`; `crates/pincel-wasm/src/sheet_import.rs`: `build_names_one_forward_tag_per_row_or_none`; `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode …`, `imports from Image mode …` | built |  |
| AC-07 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `background_exact_match_turns_transparent_before_cutting`; `ui/e2e/sprite-sheet-import.spec.ts`: `the background option starts from the top-left pixel and changes the cut` | built |  |
| AC-08 | `crates/pincel-wasm/src/sheet_import.rs`: `build_creates_one_frame_per_used_cell_with_its_pixels`; `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode …` (frame duration fields); AC-12 test for the range | built |  |
| AC-09 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `suggest_grid_finds_the_grid_of_aligned_sprites` (300 generated sheets), `suggest_grid_works_after_the_background_step` | built |  |
| AC-10 | `ui/e2e/sprite-sheet-import.spec.ts`: `without a suggestion the dialog starts from the last confirmed import` | built |  |
| AC-11 | `ui/e2e/sprite-sheet-import.spec.ts`: `without a suggestion the dialog starts from the last confirmed import`; `crates/pincel-wasm/src/sheet_import.rs`: `suggest_reports_the_grid_for_the_background_setting` | built |  |
| AC-12 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `plan_reports_each_grid_error`; `ui/e2e/sprite-sheet-import.spec.ts`: `Import is disabled with the reason for an unusable grid` | built |  |
| AC-13 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `decode_rejects_garbage_oversized_sheets_and_reports_out_of_memory`; `crates/pincel-wasm/src/sheet_import.rs`: `decode_maps_errors_to_codes_and_preview_resamples`; `ui/e2e/sprite-sheet-import.spec.ts`: `an unreadable or oversized file gives a notice and changes nothing` | built |  |
| AC-14 | `ui/e2e/sprite-sheet-import.spec.ts`: `cancelling the picker or the dialog creates nothing` | built |  |
| AC-15 | `crates/pincel-wasm/src/sheet_import.rs`: `build_creates_one_frame_per_used_cell_with_its_pixels` (RGBA, DB32 palette, Layer 1); `ui/e2e/sprite-sheet-import.spec.ts`: `imports a sheet from Pixel mode …` (name, Save asks), `imports from Image mode …` (name already open) | built |  |
| AC-16 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `exported_sheet_reimports_to_the_same_frames` | built |  |
| AC-17 | `node scripts/ui-lint.mjs` (en/de key parity) and `pnpm check` (de, es, fr, ja, pt-br typed as `Record<MessageKey, string>`) | built |  |
| AC-18 | `crates/pincel-core/src/codec/sprite_sheet/tests.rs`: `suggest_grid_gives_none_for_blank_single_and_gapless_sheets` | built |  |

## Decisions

- 2026-11-17: New document, grid with suggestion, rows as tags (default on), empty cells at row ends skipped, background-to-transparent and one frame duration as dialog options; no JSON sidecar (owner).
- 2026-11-17: Clarification pass: empty cells inside a row stay as empty frames (pauses), only trailing empty cells and empty rows are dropped; the background option starts on when the top-left pixel is opaque, so the suggestion works on sheets with a solid background.
- 2026-11-17 (spec-build): The import dialog lives in the shell (`ui/src/lib/sheet-import/`, loaded on demand by `App.svelte`) and both editors reach it through a new `EditorProps.onImportSheet`; the cut document crosses to the Pixel editor as an unsaved `.aseprite` file through the normal open path, so no new editor-session API is needed.
- 2026-11-17 (spec-build): "Unsaved" (AC-15) means no save location, as for a PNG opened in Pixel mode: the tab has no unsaved-changes marker until edited, and its first Save asks where to save.
- 2026-11-17 (spec-build): Tags use `Tag::new`'s default colour, as the spec's Assumptions say, not the timeline's colour cycle.
- 2026-11-17 (spec-build): An invalid hex in the background colour field disables Import with its own message, so the colour used is always the one shown.
- 2026-11-17 (spec-build): The grid suggestion takes, per axis, the smallest distance between the starts of consecutive runs of occupied columns / rows as the cell size and the smallest offset that puts every run inside one complete cell; an axis with a single run (a one-row strip) takes the whole sheet as its cell. AC-09 is covered by 300 generated sheets in a deterministic test instead of `proptest`, which is not a dependency of `pincel-core`.
- 2026-11-17 (spec-build): Memory exhaustion (AC-13) is caught by fallible allocation in the PNG decoder and the frame cut (`try_reserve`, tested with an impossible size) and by catching a wasm trap in the UI; e2e cannot exhaust memory.
- 2026-11-17 (spec-build): The grid crosses the wasm boundary as one six-value `Uint32Array`, so the exported functions stay below clippy's argument limit without a suppression.
