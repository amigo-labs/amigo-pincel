---
status: todo
created: 2026-11-08
size: L
mode: Image
design: [pincel.md §9.2, pincel.md §9.3, pincel.md §9.5, pincel.md §10.1, pincel.md §10.3, pincel.md §15 (2026-11-08 OCR as a Rust port of Tesseract's LSTM recognizer), pincel.md §15 (2026-11-05 effect dialog)]
depends_on: [ocr-page]
---

# Recognize Text dialog

## Context

`ocr-recognizer` and `ocr-page` give Pincel a pure Rust OCR engine in the crate `pincel-ocr`. This spec puts it in front of the user. Image mode gets a new **Tools** menu with **Recognize Text…**. The dialog reads the text in the selection or in the whole image and shows it in an editable field, and a Copy button puts it on the clipboard. The document never changes.

The owner chose all of the following (Decision Log 2026-11-08):

- Image mode only, because pixel art rarely carries text.
- A new Tools menu.
- A modal dialog.
- German and English.
- Models downloaded on first use from Pincel's website, in the PWA and the desktop app alike. One code path keeps both small for people who never use OCR.

Terms used below:

- **OCR dialog**: the modal dialog titled "Recognize Text" this spec adds. While it is open, the rest of the editor is dimmed and inert, as with the effect dialog (Decision Log 2026-11-05): no edits, menus, shortcuts, tab switches or closing the document.
- **Source area**: what is read. The pixels are the composite of the visible layers, the pixels of File ▸ Export ▸ PNG. With a selection, the area is the selection's bounding box clipped to the canvas. Pixels in it that are less than half selected count as transparent. Without a selection, the area is the whole canvas. Transparent pixels follow `ocr-page`'s flattening.
- **Page text**: as defined in `ocr-page`.
- **Language**: German or English, each backed by one model (`deu`, `eng` from `tessdata_fast` 4.1.0).
- **Stored model**: a model already downloaded, checked against its pinned SHA-256 and kept on this device, unless the browser or the user clears the site's storage.
- **Model URL**: the model's address on Pincel's website, in a path that names the `tessdata_fast` version: `/ocr/tessdata_fast-4.1.0/<lang>.traineddata` under `https://pincel.amigo-labs.dev`. The website build publishes the models committed with `ocr-recognizer` there.

## Scope

### In scope

- A Tools menu in Image mode's menu bar (German "Extras"), between Effects and Help, with "Recognize Text…" as its only entry for now
- The OCR dialog: language picker, download and recognition progress with Cancel, the result in an editable multi-line field, Copy and Close
- Downloading a language's model on first use, with progress, hash check, storage on the device and Retry
- Removing a stored model automatically once a newer pinned version of it is stored, so updates don't pile up old models
- Keeping the editor responsive while text is recognized
- The wasm surface Image mode needs
- Publishing the two models on the website at the model URL, outside the PWA's offline cache
- English and German strings

### Out of scope

- Pixel mode — because the owner chose Image mode only; Pixel mode's menu bar stays unchanged
- Putting the text into the document (text layer, text object) — because the owner chose dialog and copy; a later spec can add it once `image-live-text` exists
- Saving the text as a `.txt` file — because copying covers the owner's use; it can be added to the dialog later without engine work
- A keyboard shortcut — because no free, conventional key exists for OCR; `power-user-shortcuts-keymap` will let users bind one
- The Tools menu in the desktop app's native macOS menu — because the native menu mirrors only File, Edit, View and Help; Effects and Image are not in it either
- Languages other than German and English — because v1 keeps the catalog to two models; adding one later is a catalog entry with a pinned hash, not new code
- A button to remove stored models — because they are a few MB; clearing the site data (browser) or the app data (desktop) removes them
- MCP and CLI access to OCR — because they are separate entry points with their own specs
- Showing the recognized boxes over the canvas — because the first goal is the text; the page result already has the boxes for a later spec

## Acceptance criteria

- [ ] AC-01 (always): Image mode's menu bar has a Tools menu between Effects and Help, holding "Recognize Text…". Pixel mode's menu bar has no Tools menu.
- [ ] AC-02 (event): When the user picks Tools ▸ Recognize Text…, then the OCR dialog opens. The language is preselected: the language of the last recognition the user started, kept across sessions. On first use it is German if the interface language is German and English otherwise, the best guess for the text a user reads.
- [ ] AC-03 (event): When the dialog opens, or the user picks another language, and that language's model is stored, then recognition of the source area starts without further input. While the layout is analysed, the dialog shows "Finding text…". After that it shows "Recognizing line n of m". A Cancel button is shown throughout.
- [ ] AC-04 (event): When the language's model is not stored, then the dialog downloads it from the model URL and shows "Downloading German model…" (or English) with the bytes downloaded and the total, or only the bytes downloaded when the server sends no total. It then stores the model and starts recognition. Later opens of the dialog download nothing while the stored model is present.
- [ ] AC-05 (error): If the downloaded file does not match its pinned SHA-256, then it is discarded and not stored, and the dialog shows "The language model could not be verified." with Retry.
- [ ] AC-06 (error): If the download fails (offline, server error, aborted connection), then the dialog shows "The language model could not be downloaded. Check the connection and try again." with Retry. Nothing partial is stored, and Retry starts the download again.
- [ ] AC-07 (event): When recognition finishes, then the dialog shows the page text in an editable multi-line field with the cursor at its start, plus Copy and Close. If the page text is empty, the dialog shows "No text found." instead of the field.
- [ ] AC-08 (event): When the user presses Copy, then the field's current content, with the user's edits, goes to the clipboard as plain text. The dialog shows "Copied" until the next edit and stays open. Copy is disabled while the field is empty or holds only whitespace.
- [ ] AC-09 (error): If writing to the clipboard fails, then the dialog shows "Could not copy to the clipboard." and the text stays in the field.
- [ ] AC-10 (state): With a selection, text outside it is not recognized. In the test, a document holds three words with gaps of at least one word height between them, and a selection covers only the middle word. In a second case, a lasso selection excludes a word that lies inside the selection's bounding box. In both cases the result holds only the selected words.
- [ ] AC-11 (always): Hidden layers do not contribute to the source area. In the test, a word on a hidden layer is not in the result, and a word on a visible layer above it is.
- [ ] AC-12 (event): When the user presses Cancel during download or recognition, or presses Escape or Close at any time, then the work stops and the dialog closes. A download in progress is discarded, so that model is downloaded again next time. A model already stored stays stored.
- [ ] AC-13 (always): Opening, using and closing the OCR dialog leaves the document, its selection, its undo history and its dirty state unchanged.
- [ ] AC-14 (state): While recognition of the 1920 × 1080 screenshot from `ocr-page`'s corpus runs in the e2e test, a press on Cancel closes the dialog within 500 ms, and the progress line changes at least once per recognized line.
- [ ] AC-15 (always): The PWA's precache manifest lists no model file; installing or updating the PWA downloads no model.
- [ ] AC-16 (option): Where the interface language is German, the Tools menu reads "Extras" and its entry "Text erkennen…", and the dialog's title, labels, buttons and messages are German. The recognized text itself is never translated.
- [ ] AC-17 (always): Recognizing the 1920 × 1080 screenshot from `ocr-page`'s corpus in the e2e test gives exactly the page text that the native `pincel-ocr` test records for that image, which shows the wasm build computes the same as the native one.
- [ ] AC-18 (error): If recognition fails (the engine cannot start, runs out of memory, or stops with an error), then the dialog shows "Text recognition failed." with Retry, and the editor stays usable after Close.
- [ ] AC-19 (error): If the device storage cannot be read or written (quota exceeded, storage blocked), then a model that cannot be read counts as not stored. A downloaded model that cannot be stored is still used for this recognition, without an error message.
- [ ] AC-20 (error): If a stored model no longer matches its pinned SHA-256 when it is loaded, then it is removed and downloaded again as in AC-04.
- [ ] AC-21 (always, manual: reference machine): Recognizing the 1920 × 1080 corpus screenshot in the PWA's release build takes at most 8 s on the reference machine (pincel.md §4.5). That is wasm without SIMD threads at most about 2.5 × the native budget of `ocr-page` AC-13.

## Technical notes

- **Wasm surface**: `pincel-image-wasm` depends on `pincel-ocr` and exposes an OCR session created inside Rust from a document handle and the source area (no RGBA round trip through JavaScript). It takes the model bytes and reports progress. It runs in Image mode's job worker (`ui/src/modes/image/jobs`) on a document snapshot, so the main thread stays free. Cancel can end the worker. The generated TypeScript types follow the existing `export_command_spec` drift check.
- **Model storage**: an IndexedDB object store for model bytes keyed by language and `tessdata_fast` version, the same in the PWA and in the desktop app's webview. The last used language is a pref (`ui/src/lib/idb/prefs.ts`). No `localStorage` (AGENTS.md §9).
- **Model URL and website**: the PWA fetches the model URL on its own origin. The desktop app fetches the absolute URL, so the website serves the models with `Access-Control-Allow-Origin` for the desktop app's origin and long-lived cache headers. `vite-plugin-pwa`'s `globPatterns` lists the file types to precache, and `.traineddata` is not among them. e2e tests serve the committed models through `page.route` and need no network.
- **Concurrent downloads**: two windows that download the same model both store identical bytes; the second write replaces the first, without an error.
- **Dialog**: built like the effect dialog, with `pc-*` tokens and `t()` strings. The menu entry is data in Image mode's menu definition (`EditorFrame`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Very large image (e.g. 8000 × 8000 px) | Recognition runs with progress, and Cancel works (AC-03, AC-12) |
| Selection of 1 × 1 px, or a selection over an empty area | "No text found." |
| Selection that reaches past the canvas edge | Clipped to the canvas |
| Document fully transparent, no selection | "No text found." |
| Language switched during a download | The download is discarded; the other language's model is downloaded or used |
| Language switched while recognition runs | Recognition stops and restarts in the new language |
| Desktop app offline with the model stored | Recognition works without a network |
| An update pins a newer `tessdata_fast` version | The new version is downloaded on next use; the old one is removed once the new one is stored |
| Field cleared by the user | Copy is disabled (AC-08) |
| Browser evicted the stored model | Next use downloads it again (AC-04) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| IndexedDB holds a few MB per model | Both platforms grant at least this much without a prompt | Under storage pressure the browser may evict it; AC-04 downloads it again |
| "Tools" is the right home for future entries (vectorize, AI) | Owner chose a Tools menu for OCR | Later specs may move OCR or add entries; no AC depends on other entries |
| Ending the worker is an acceptable way to cancel | Recognition holds no state worth keeping | The next recognition pays the worker start again (about the time to instantiate the core) |

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
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |

## Decisions

- 2026-11-08: The owner chose Image mode only, a Tools menu, a dialog with Copy, German and English, and models downloaded on first use from Pincel's website.
- 2026-11-08 (clarification pass): The dialog is modal, the owner's choice, which removes edits and tab switches during recognition. Transparent and unselected pixels follow `ocr-page`'s flattening, also the owner's choice. "Nothing is stored" on cancel now means only the download in progress. AC-14 is measured in the e2e test. AC-17 to AC-21 were added: wasm equals native, recognition failure, storage failure, a damaged stored model, and the wasm time budget.
