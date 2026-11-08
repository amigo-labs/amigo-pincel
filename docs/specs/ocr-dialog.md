---
status: todo
created: 2026-11-08
size: L
mode: Image
design: [pincel.md §9.2, pincel.md §9.3, pincel.md §9.5, pincel.md §10.1, pincel.md §10.3, pincel.md §15 (2026-11-08 OCR as a Rust port of Tesseract's LSTM recognizer)]
depends_on: [ocr-page]
---

# Recognize Text dialog

## Context

`ocr-recognizer` and `ocr-page` give Pincel a pure Rust OCR engine in the crate `pincel-ocr`. This spec puts it in front of the user. In Image mode, a new **Tools** menu holds **Recognize Text…**. It reads the text in the selection or in the whole image, shows it in an editable field, and copies it to the clipboard. The document never changes. The owner chose Image mode only (pixel art rarely carries text), a new Tools menu, German and English as the languages, and a model download on first use from Pincel's own website (Decision Log 2026-11-08).

Terms used below:

- **OCR dialog**: the dialog titled "Recognize Text" this spec adds.
- **Source area**: what is read. With a selection in the document, the selection's bounding box, where pixels outside the selection's shape count as white. Without a selection, the whole canvas. In both cases the pixels are the composite of the visible layers, i.e. the pixels of File ▸ Export ▸ PNG, cropped to the area.
- **Language**: German or English, each backed by one model (`deu`, `eng` from `tessdata_fast` 4.1.0).
- **Stored model**: a model already downloaded, checked against its pinned SHA-256, and kept on this device across sessions and updates.
- **Model URL**: the model's address on Pincel's website (`https://pincel.amigo-labs.dev/…`), in a path that names the `tessdata_fast` version. The website build publishes the models committed with `ocr-recognizer`.

## Scope

### In scope

- A Tools menu in Image mode's menu bar (German "Extras"), between Effects and Help, with "Recognize Text…" as its only entry for now
- The OCR dialog: language picker, progress with Cancel, the result in an editable multi-line field, Copy and Close
- Downloading a language's model on first use from the model URL, with progress, hash check, storage on the device and Retry
- Running the recognition off the main thread, so the editor stays responsive
- The wasm surface Image mode needs to read the source area
- Publishing the two models on the website at the model URL, outside the PWA's offline cache
- English and German strings

### Out of scope

- Pixel mode — because the owner chose Image mode only; Pixel mode keeps its menu bar unchanged
- Putting the text into the document (text layer, text object) — because the owner chose dialog and copy; a later spec can add it once `image-live-text` exists
- Saving the text as a `.txt` file — because copy covers the owner's use; it can be added to the dialog later without engine work
- A keyboard shortcut — because no free, conventional key exists for OCR; `power-user-shortcuts-keymap` will let users bind one
- The Tools menu in the desktop app's native macOS menu — because the native menu mirrors only File, Edit, View and Help; Effects and Image are not in it either
- Languages other than German and English, and removing stored models — because v1 keeps the catalog to two models of a few MB each; clearing the site data (browser) or the app data (desktop) removes them
- MCP and CLI access to OCR — because they are separate entry points with their own specs
- Showing the recognized boxes over the canvas — because the first goal is getting the text; the page result already has the boxes for a later spec

## Acceptance criteria

- [ ] AC-01 (always): Image mode's menu bar has a Tools menu between Effects and Help with the entry "Recognize Text…", disabled while no document is open. Pixel mode's menu bar has no Tools menu.
- [ ] AC-02 (event): When the user picks Tools ▸ Recognize Text…, then the OCR dialog opens with the language preselected: the one used last, or on first use German if the interface language is German and English otherwise.
- [ ] AC-03 (event): When the dialog opens, or the user picks another language, and that language's model is stored, then recognition of the source area starts at once. While it runs, the dialog shows "Recognizing text…", the lines done out of the lines found, and a Cancel button.
- [ ] AC-04 (event): When the language's model is not stored, then the dialog downloads it from the model URL, shows "Downloading German model…" (or English) with bytes downloaded and the total, then stores the model and starts recognition. A second open of the dialog, in this or a later session, downloads nothing.
- [ ] AC-05 (error): If the downloaded file does not match the pinned SHA-256, then it is discarded and not stored, and the dialog shows "The language model could not be verified." with Retry.
- [ ] AC-06 (error): If the download fails (offline, server error, aborted connection), then the dialog shows "The language model could not be downloaded. Check the connection and try again." with Retry; nothing partial is stored. Retry starts the download again.
- [ ] AC-07 (event): When recognition finishes, then the dialog shows the page text in an editable multi-line field with the cursor at its start, plus Copy and Close. If the text is empty, the field is replaced by "No text found." and Copy is disabled.
- [ ] AC-08 (event): When the user presses Copy, then the field's current content, including the user's edits, goes to the clipboard as plain text, and the dialog shows "Copied" until the next edit. The dialog stays open.
- [ ] AC-09 (error): If writing to the clipboard fails, then the dialog shows "Could not copy to the clipboard." and the text stays in the field.
- [ ] AC-10 (state): With a selection in the document, only text inside the selection is recognized: text outside the selection's bounding box, and outside its shape within that box, does not appear in the result.
- [ ] AC-11 (always): Hidden layers do not contribute to the source area. The text of a hidden layer does not appear in the result, and the text of a visible layer above a hidden one does.
- [ ] AC-12 (event): When the user presses Cancel during download or recognition, or presses Escape or Close at any time, then the work stops, the dialog closes, and nothing is stored. A model whose download was cancelled is downloaded again next time.
- [ ] AC-13 (always): Opening, using and closing the OCR dialog leaves the document, its selection, its undo history and its dirty state unchanged.
- [ ] AC-14 (state): While recognition runs, the editor stays responsive: the dialog's Cancel button reacts within 200 ms, and the progress updates at least after every line.
- [ ] AC-15 (always): The PWA's offline cache does not contain the models; installing or updating the PWA downloads no model.
- [ ] AC-16 (option): Where the interface language is German, the Tools menu reads "Extras" and its entry "Text erkennen…", and the dialog's title, labels, buttons and messages are German. The recognized text itself is never translated.

## Technical notes

- **Wasm surface**: `pincel-image-wasm` depends on `pincel-ocr` and exposes an OCR session created inside Rust from a document handle and the source area (no RGBA round-trip through JavaScript). It takes the model bytes and returns the page text plus progress through a callback. The session runs in Image mode's job worker (`ui/src/modes/image/jobs`), on a snapshot of the document taken when the dialog starts recognition, so later edits do not leak in. The generated TypeScript types follow the existing `export_command_spec` drift check.
- **Model storage**: one IndexedDB store for model bytes keyed by language and `tessdata_fast` version, in both the PWA and the desktop app's webview. No `localStorage` (AGENTS.md §9).
- **Model URL and website**: the website build copies `crates/pincel-ocr/models/*.traineddata` to a versioned path, served with long-lived cache headers and an `Access-Control-Allow-Origin` that admits the desktop app's origin. `vite-plugin-pwa`'s `globPatterns` already excludes `.traineddata`, so the models stay out of the precache by construction.
- **Dialog**: built like the other shell dialogs, with `pc-*` tokens and `t()` strings; the menu entry is data in Image mode's menu definition (`EditorFrame`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Very large image (e.g. 8000 × 8000 px) | Recognition runs with progress; Cancel works (AC-14) |
| Selection of 1 × 1 px, or a selection over an empty area | "No text found." |
| Language switched while recognition runs | The running recognition stops; recognition in the new language starts (after its download if needed) |
| Document edited in another tab while the dialog is open | The dialog keeps the result of the snapshot it read; the other tab's document is not involved |
| Two Image documents open, dialog opened on the second | The active document is read |
| Desktop app offline with the model already stored | Recognition works without a network |
| A stored model from an older `tessdata_fast` version after an update that pins a newer one | The new version is downloaded on next use; the old entry is deleted after the new one is stored |
| Text field edited and then Copy pressed twice | The same edited text is copied each time |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| IndexedDB holds a few MB per model reliably | Both platforms grant at least this much without a prompt | A browser under storage pressure may evict it; the next use downloads it again (AC-04) |
| "Tools" is the right home for future entries (vectorize, AI) | Owner chose a Tools menu for OCR | Later specs may move OCR or add entries; no AC depends on other entries |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |

## Decisions

- 2026-11-08: Image mode only, Tools menu, dialog plus copy, German and English, models downloaded on first use from Pincel's website — owner's choices.
