---
status: todo
created: 2026-11-01
size: L
mode: both
design: [pincel.md §9.2, pincel.md §9.3, pincel.md §15 (2026-11-01 Vectorizer core as pincel-trace), pincel.md §15 (2026-11-01 SVG export dialog), vectorize-core.md]
depends_on: [vectorize-core]
---

# Export as SVG

## Context

`vectorize-core` ported the owner's Vectorizer into the crate `pincel-trace`: an RGBA image goes in, an SVG comes out, in a colour mode, a monochrome mode or a pixel-exact mode for pixel art (Decision Log 2026-11-01). Nothing in the app calls it yet. This spec puts it behind File ▸ Export in both editors, so the Vectorizer app can be deleted: the user picks the mode and its options in a dialog, sees the result before saving, and saves an `.svg` file. The only product is the file; the document never changes (Decision Log 2026-11-01).

Terms used below:

- **Export dialog**: the dialog titled "Export as SVG" this spec adds.
- **Source image**: what the trace reads. Pixel mode: the current frame composed at 1× from the visible layers, without onion skin, grid, selection outline or other overlays — the pixels of File ▸ Export ▸ PNG (current frame). Image mode: the composite of the visible layers at full size — the pixels of File ▸ Export ▸ PNG.
- **Summary line**: the dialog's text that names the number of layers, the number of outlines and the file size.
- **Modes** and their options are those of `vectorize-core`: *Colours* (2 – 16 colours), *Monochrome* (automatic or manual threshold, invert, fill colour), *Pixel-exact*; the Potrace options (minimum area, corner threshold, curve optimisation and its tolerance) and despeckle apply to Colours and Monochrome.

## Scope

### In scope

- File ▸ Export ▸ SVG… in both editors, including the macOS native menu
- The export dialog: mode, the mode's options, a live preview and the summary line
- Saving the SVG through the existing export path (save dialog on desktop and in browsers with the File System Access API, download otherwise)
- The wasm surface both editors need to trace the source image
- English and German strings

### Out of scope

- A pinned palette, per-layer visibility, colour editing or "drop the background" in the dialog — because the first goal is replacing the Vectorizer's export; the core already supports a pinned palette, so a later spec can add it without core changes
- The turn-policy option — because its effect is subtle and its default (*minority*) is what the Vectorizer UI used too; it stays at the default
- Placing the result into the document, vector layers, SVG import — because the owner decided an export is the only output
- Tracing off the main thread — because `performance-background-jobs` owns moving work off the main thread; until then a large image can make the preview lag
- An MCP tool or CLI command for vectorizing — because they are separate entry points with their own specs
- Downscaling before tracing — because the export should match the document's pixels; the user can resize the document first

## Acceptance criteria

- [ ] AC-01 (always): In both modes the File menu's Export submenu holds "SVG…" as its last entry, disabled while no document is open.
- [ ] AC-02 (event): When the user picks File ▸ Export ▸ SVG…, then the export dialog opens titled "Export as SVG" with the mode preselected — Pixel-exact in Pixel mode, Colours in Image mode — and every option at the `vectorize-core` default (6 colours, automatic threshold, invert off, fill black, despeckle off, minimum area 2, corner threshold 1.0, curve optimisation on, tolerance 0.2).
- [ ] AC-03 (state): While the export dialog is open, it shows only the options of the selected mode — colour count for Colours; threshold (automatic or a value 0 – 255), invert and fill colour for Monochrome; nothing extra for Pixel-exact — plus despeckle and the Potrace options for Colours and Monochrome.
- [ ] AC-04 (state): While the export dialog is open, it shows a preview of the SVG the current options produce and the summary line (for example "4 layers · 12 outlines · 3.2 KB"); both update within a second after an option changes; the document, its undo history and the canvas stay unchanged.
- [ ] AC-05 (event): When the user presses Export, then the SVG of the current options, traced from the source image, is saved under the document's file name with the extension `.svg` — for an unsaved document the name the editor's PNG export uses, `pincel.svg` in Pixel mode and `image.svg` in Image mode — and the type `image/svg+xml`, the dialog closes, and Pixel mode's status bar reads "exported SVG · <n> bytes".
- [ ] AC-06 (event): When the user presses Cancel or Escape, then the dialog closes, no file is saved, and the document and its undo history are unchanged.
- [ ] AC-07 (error): If the selected mode cannot trace the source image — Pixel-exact with more than 256 colours — then the dialog shows the reason with the colour count in place of the preview and disables Export, and choosing another mode or option recovers.
- [ ] AC-08 (error): If the user cancels the save dialog, then nothing is saved, no error is shown, and the document is unchanged.
- [ ] AC-09 (always): The exported file traces the source image only: hidden layers, the onion skin and the selection outline do not appear in it.
- [ ] AC-10 (always, manual: native macOS menu): On macOS the desktop app's native File ▸ Export menu holds "Export as SVG…" (German "Als SVG exportieren…"), which opens the same dialog.
- [ ] AC-11 (option): Where the interface language is German, the menu entry, the dialog title, its labels and buttons and the summary line are German.

## Technical notes

- **Wasm surface** (Decision Log 2026-11-01 "SVG export dialog"): `pincel-wasm` and `pincel-image-wasm` both depend on `pincel-trace` and expose one small class each, `Vectorizer`, created from the source image inside Rust (no RGBA round-trip through JavaScript) — Pixel: `Document.vectorizer(frame)`; Image: `vectorizer(handle)`. `vectorizer.trace(mode, params: Float64Array)` returns the SVG plus the counts or an error string. The flat number vector mirrors the effects' wire format (Decision Log 2026-10-11). The class keeps the core's session caches, so a preview that only changes Potrace options skips the colour reduction.
- **Source image**: Pixel mode composes like `exportPng` (`Visible` layers, zoom 1, no onion skin, no overlays); Image mode like `composite(handle)`.
- **Saving**: `saveExport(bytes, name, 'svg')` in `ui/src/lib/fs` with a new `svg` export format (`image/svg+xml`, `.svg`).
- **Dialog**: one shared component under `ui/src/lib/components/`, mounted by both editors, modal like the animation export dialog; the preview is an `<img>` showing the SVG, which keeps the canvas untouched (AGENTS.md §9: no canvas pixel reads). Option changes are coalesced (about 150 ms) before re-tracing.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Empty (fully transparent) document | Preview is empty, summary "0 layers · 0 outlines · …", Export saves a valid SVG without paths |
| Pixel document with more than 256 colours, Pixel-exact | AC-07: reason shown, Export disabled; Colours mode works |
| Multi-frame Pixel document | The current frame is traced; the file name has no frame suffix |
| Hidden layer holding the only content | Empty result (AC-09) |
| Monochrome with an all-white image | Zero layers, still exportable |
| Changing options quickly | Only the latest options' result is shown and exported |
| Export pressed while a re-trace is pending | The file holds the result of the options shown when Export was pressed |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Pixel mode preselects Pixel-exact, Image mode Colours (AC-02) | Pixel art loses its look under Potrace smoothing; photos and drawings are what the Vectorizer's colour mode was built for | Low |
| The preview lives in the dialog, not on the canvas | A preview on the canvas would have to overlay vector output on the raster view in both renderers; an `<img>` of the SVG shows exactly the file | Low |
| The macOS native menu gets the entry; other platforms use the in-window menu | The native menu is macOS-only (Decision Log 2026-10-27) | Low |
| No frame suffix in the Pixel file name | The SVG export is one image like the Image-mode export; the PNG frame export's suffix exists to tell frames apart in a sequence | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `both modes list SVG… last under File ▸ Export`, `File ▸ Export is disabled until the document is loaded` (`ui/e2e/export-svg.spec.ts`) | built |        |
| AC-02 | `pixel: the dialog opens pixel-exact with defaults and exports the frame`, `image: the dialog opens in colour mode, shows each mode's options and updates the summary` (`ui/e2e/export-svg.spec.ts`) | built |        |
| AC-03 | `image: the dialog opens in colour mode, shows each mode's options and updates the summary` (`ui/e2e/export-svg.spec.ts`; each mode asserts what it shows and what it hides) | built |        |
| AC-04 | `image: the dialog opens in colour mode, …` (summary and preview `src` follow invert within 1 s), `pixel: the dialog opens pixel-exact …` (preview, undo unchanged), `Cancel and Escape close the dialog without saving or editing` (canvas screenshot unchanged) (`ui/e2e/export-svg.spec.ts`) | built |        |
| AC-05 | `pixel: the dialog opens pixel-exact with defaults and exports the frame` (`pincel.svg`), `image: the dialog opens in colour mode, …` (`image.svg`) (`ui/e2e/export-svg.spec.ts`); the type `image/svg+xml` is the `svg` export format in `ui/src/lib/fs/index.ts` | built |        |
| AC-06 | `Cancel and Escape close the dialog without saving or editing` (`ui/e2e/export-svg.spec.ts`, both modes) | built |        |
| AC-07 | `pixel-exact refuses more than 256 colours until another mode is chosen` (`ui/e2e/export-svg.spec.ts`) | built |        |
| AC-08 | `cancelling the save picker saves nothing and shows no error` (`ui/e2e/export-svg.spec.ts`) | built |        |
| AC-09 | `a hidden layer is not exported`, `onion skin and the selection outline are not exported` (`ui/e2e/export-svg.spec.ts`); Rust: `vectorize::tests::vectorizer_traces_the_visible_frame_pixel_exact` (`crates/pincel-wasm/src/vectorize.rs`) | built |        |
| AC-10 |      | open   |        |
| AC-11 | `menu entry, dialog and summary follow the German language` (`ui/e2e/export-svg.spec.ts`) | built |        |

## Decisions

- 2026-11-01 (revision after spec-check) — AC-05 named `pincel.svg` for every unsaved document, while Image mode names an unsaved document's exports `image.*` (its PNG export saves `image.png`). The criterion now follows each editor's existing name instead of giving Image mode a second default; the code and its test already did. The acceptance check also found clauses without a test — the disabled Export submenu before a document has loaded (AC-01), the options each mode hides (AC-03), the preview update within a second and the unchanged canvas (AC-04), Image-mode Cancel / Escape (AC-06), onion skin and the selection outline (AC-09); the added tests passed at once because the behaviour existed, and the AC-01 test fails when the submenu's no-document flag is removed.

- 2026-11-01 (spec-build) — UI behaviour is test-after (AGENTS.md §2.3): the e2e spec was written against the finished dialog; the wasm bindings have native Rust tests. The summary uses singular and plural forms ("1 layer", "4 layers") in both languages. The preview `<img>` scales the SVG to the preview box. The macOS native menu item (AC-10) is built in `src-tauri/src/main.rs` and checked by CI's Tauri job; its behaviour needs a Mac.

- 2026-11-01 — Written autonomously under the owner's goal "Vectorizer completely integrated", following their earlier decisions (Rust port, export only, Vectorizer repository deleted afterwards); every choice not settled by them is listed under Assumptions.
