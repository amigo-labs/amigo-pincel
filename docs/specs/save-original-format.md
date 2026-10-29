---
status: todo
created: 2026-10-28
size: M
mode: Image
design: [pincel.md §1.1, pincel.md §7, pincel.md §15 (2026-10-11 OpenRaster project format)]
---

# Save back to the original format in Image mode

## Context

Screenshot editing is a main Image-mode use case (§1.1). Today the first Save after opening a PNG asks for a **`.ora` location**. That surprises everyone coming from paint.net, Paint or Preview, and it creates a second file instead of updating the one they opened.

This spec makes Save behave like paint.net: open `screenshot.png`, edit it, press Ctrl+S, and the PNG is overwritten. A document with layers asks whether to flatten first; JPEG asks for its quality once per document; WebP stays lossless. `.ora` stays the format that keeps layers, and autosave keeps writing ORA.

## Scope

### In scope

- Save writes back to the opened file in its own format: PNG, JPEG, WebP, BMP, and GIF / TIFF where an encoder is available
- The "Flatten image?" prompt with Flatten / Save as .ora… / Cancel and a per-document "Don't ask again" option
- JPEG quality asked once per document; WebP lossless without a dialog
- Save As… offering every format
- Fallback to Save As with a notice for a format Pincel cannot encode
- The editor's title / dirty marker showing the real file name and extension

### Out of scope

- Writing layered TIFF or PSD — because Pincel's layered format is `.ora` (Decision Log 2026-10-11) and those writers are separate codecs
- Keeping EXIF data or color profiles of the original — because re-encoding from the composite drops metadata; warning about it in the JPEG prompt when metadata was present is a follow-up
- Pixel mode — because `.aseprite` already saves in place; this is an Image-mode gap

## Acceptance criteria

- [ ] AC-01 (event): When the user opens a PNG, JPEG, WebP or BMP file, edits it and saves (File ▸ Save or Ctrl/⌘+S) with a single layer at full opacity and Normal blend, then the opened file is written in its own format under its own name — no `.ora` dialog — and the document is marked saved.
- [ ] AC-02 (event): When the user saves such a document while it has more than one layer, or a layer whose opacity or blend mode would be lost by flattening, then a "Flatten image?" prompt appears with *Flatten* (default), *Save as .ora…* and *Cancel*.
- [ ] AC-03 (event): When the user chooses *Flatten*, then the file is written as the flattened composite in its original format, and the open document keeps all its layers.
- [ ] AC-04 (event): When the user chooses *Save as .ora…*, then the `.ora` Save As flow runs and the layers are saved in an OpenRaster file.
- [ ] AC-05 (event): When the user chooses *Cancel* in the flatten prompt, then nothing is written and the document stays marked unsaved.
- [ ] AC-06 (option): Where "Don't ask again for this document" was checked in the flatten prompt, later Saves of that document in the same session flatten without asking.
- [ ] AC-07 (event): When the user saves a document opened from a JPEG for the first time, then the JPEG quality dialog appears; later Saves of the same document reuse that quality without asking.
- [ ] AC-08 (event): When the user saves a document opened from a WebP, then it is written lossless without a quality dialog.
- [ ] AC-09 (error): If the document's original format has no encoder in Pincel, then Save opens Save As with the format choice and shows a notice saying the original format cannot be written.
- [ ] AC-10 (event): When the user chooses Save As…, then every format Pincel can write is offered, with `.ora` preselected when the document has more than one layer.
- [ ] AC-11 (always): The editor's title and dirty marker show the opened file's real name and extension (e.g. `screenshot.png`), not an `.ora` name.
- [ ] AC-12 (always): Documents created with New, and documents opened from `.ora`, save exactly as before (to `.ora`).
- [ ] AC-13 (always): Autosave and crash recovery keep storing OpenRaster snapshots, so recovery restores every layer even after a flattening Save.

## Technical notes

Hints from the legacy plan (2026-10-28); paths verified 2026-10-29, line numbers drift.

**`ui/src/modes/image/core/controller.ts`**

- `projectTarget: SaveTarget` (≈L492); `projectName()` (≈L495) feeds the editor title (`ImageEditor.svelte` ≈L741).
- `saveProject(forceAs)` (≈L520–527) calls `saveBytes(…, projectTarget, { forceAs, format: 'ora' })`.
- `openBytes` (≈L551–572) keeps the opened file's handle / path only for ORA; for other formats it sets `{ name: `${exportStem}.ora`, handle: null, path: null }` (≈L567–569), throwing away the FSA handle or Tauri path.
- `exportImage(format, quality)` (≈L860) already encodes PNG / JPEG / WebP from the composite.
- `markSaved` / `markUnsaved` drive the dirty state.

**`ui/src/lib/fs/index.ts`.** `saveBytes(bytes, target, { forceAs, format?: ProjectFormat })` (≈L468) handles FSA handle reuse (with `ensureReadWritePermission`, ≈L320; a denied permission falls through to the save picker), Tauri paths and the download fallback. `ProjectFormat` (≈L127) is `'aseprite' | 'ora'`; image formats go through `saveExport`. Widen the format option or route image saves through a shared helper that reuses the handle / path logic.

**Design.** `projectTarget` keeps the opened file with `name`, `handle` / `path` and a `format`: `png | jpeg | webp | bmp | gif | tiff | ora`. Save on `ora` behaves as today; an image format checks flatten, then encodes the flattened composite and writes it to the same handle / path.

**wasm.** `crates/pincel-image-wasm/src/lib.rs`: `export_png` / `export_jpeg` / `export_webp` (≈L1563–1581) encode the flattened composite.

**Encoders.** `crates/pincel-image-core/src/codec/mod.rs`: `encode_png`, `encode_jpeg`, `encode_bmp`, `encode_webp` (lossless). No GIF or TIFF encoder is wired up, although the `image` crate's `gif` / `tiff` features are enabled (root `Cargo.toml` ≈L34); check whether encoding needs extra features.

**Dialogs.** `ui/src/modes/image/components/dialogs/ExportDialog.svelte` asks for JPEG quality (1–100, default 90); `ui/src/modes/image/components/dialogs/Modal.svelte` supports three buttons (`secondary`).

**Recents / autosave.** Recents store a handle or path per file (`ui/src/lib/idb/recent-files.ts`); autosave writes ORA snapshots (`ui/src/lib/idb/autosave-session.svelte.ts`).

**e2e** `ui/e2e/save-original.spec.ts` (download pattern from `ui/e2e/ora.spec.ts`): open a single-layer PNG through the file chooser, paint, Save → download name `*.png`, bytes start with the PNG signature; add a layer, Save → flatten prompt; Flatten → PNG written, layers panel still shows 2 layers; *Save as .ora…* → `.ora` download; JPEG: first Save asks the quality, the second doesn't.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| File opened without a writable handle or path (drag and drop, or a browser without the File System Access API) | Save offers the save picker, or downloads, under the original name and extension |
| Write permission on the opened file's handle is denied | Falls through to the save picker, as `saveBytes` does today |
| Writing the file fails | The error is reported as for an `.ora` save today; the document stays marked unsaved |
| JPEG with transparent pixels | Flattened the same way the existing JPEG export flattens transparency |
| Cancel in the JPEG quality dialog | Nothing is written; the quality is not remembered; the document stays unsaved |
| `photo.jpg` (not `.jpeg`) | Written back as `photo.jpg` |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| After *Save as .ora…* or a Save As to another file, later Saves go to that new file and format | Standard Save As semantics; the plan keeps one save target per document | Low |
| When the document has one layer, Save As preselects the original format | The plan only says `.ora` is preselected with more than one layer | Low |
| The target format is the format detected from the file's content at open, the name stays as opened | `openBytes` routes by content sniffing | Low — a misnamed file is written in its real format under its old name |

## Open questions

- The Decision Log entry (save back to the original format; flatten in the file only while the document keeps its layers — unlike paint.net; autosave stays ORA; GIF / TIFF fallback) and the §7 File I/O text for Image-mode save semantics are not written yet.
- GIF and TIFF: wire up encoders in this spec (the `image` features are enabled; possibly extra features needed) or route their Save to Save As with a notice for now?
- After a flattening Save the file no longer holds the layers: is the document then marked saved (no dirty marker, no "unsaved changes" warning on close) or does it stay dirty?

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

## Decisions

- Flattening happens in the file only; the open document keeps its layers — because losing layers on Save would be destructive and undoable only through the undo history (the plan records this as a deliberate difference to paint.net).
- Autosave stays ORA — because it is crash recovery, not the user's file, and must keep every layer.
- WebP is written lossless only, without a quality dialog — because the existing WebP encoder is lossless (Fineliner ADR-007).
- JPEG asks its quality once per document and remembers it — because asking on every Save interrupts the Ctrl+S habit.
- The flatten prompt's default button is *Flatten*, and "Don't ask again" lasts for the document's session — because that matches paint.net's flow while keeping the choice per document.
