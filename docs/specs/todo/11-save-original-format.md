---
id: todo-save-original-format
title: Image mode — Save back to the original format (paint.net behaviour)
status: todo
size: M (one session)
mode: Image
depends_on: []
created: 2026-10-28
---

# Image mode: Save back to the original format

## Goal

Open `screenshot.png`, edit it, press **Ctrl+S**, and the **PNG is overwritten**, as in paint.net:
- If the document has **more than one layer**, Save first asks **"Flatten image?"** with three choices:
  - **Flatten** writes the PNG, flattened in the file. The open document keeps its layers.
  - **Save as .ora…** keeps the layers in an OpenRaster file.
  - **Cancel.**
- **JPEG** asks for the quality once per document, with the existing JPEG dialog.
- **WebP** is lossless only (ADR-007).
- BMP, GIF and TIFF: save back where the image codec can encode them. Otherwise Save falls back to Save As with the format choice.

## Why

Screenshot editing is a main Image-mode use case (spec §1.1). Today the first Save after opening a PNG asks for a **`.ora` location**. That surprises everyone coming from paint.net, Paint or Preview, and it creates a second file instead of updating the one they opened.

## Current state (read these first)

**`ui/src/modes/image/core/controller.ts`**
- `projectTarget: SaveTarget` (≈L492).
- `saveProject(forceAs)` (≈L520–527) calls `saveBytes(…, projectTarget, { forceAs, format: 'ora' })`.
- `openBytes` (≈L551–572) sets `projectTarget` to the opened file only for ORA. **For other formats it sets `{ name: `${stem}.ora`, handle: null, path: null }`** (≈L567). That throws away the opened file's FSA handle or Tauri path for saving.
- `exportImage(format, quality)` (≈L860) already encodes PNG / JPEG / WebP from the composite.
- `markSaved` / `markUnsaved` drive the dirty state.

**`ui/src/lib/fs/index.ts`**
- `saveBytes(bytes, target, { forceAs, format?: ProjectFormat })` (≈L468) handles FSA handle reuse, Tauri paths, and the download fallback.
- `ProjectFormat` is only the project formats today. Image formats go through `saveExport`.

**wasm.** `crates/pincel-image-wasm/src/lib.rs`: `export_png` / `export_jpeg` / `export_webp` (≈L1563–1581) encode the flattened composite.

**Encoders.** `crates/pincel-image-core/src/codec/mod.rs`: `encode_png`, `encode_jpeg`, `encode_bmp`, `encode_webp` (lossless).
- No GIF or TIFF encoder is wired up, although the `image` crate has the `gif` / `tiff` features enabled (root `Cargo.toml` ≈L34).
- **Check** whether encoding through them needs extra features.

**Dialogs.** The image `ExportDialog.svelte` asks for JPEG quality. `Modal.svelte` supports three buttons (`secondary`, from PR #87).

**Recents / autosave.** Recents store a handle or path per file (`ui/src/lib/idb/recent-files.ts`); autosave writes ORA snapshots (`ui/src/lib/idb/autosave-session.svelte.ts`). Autosave stays ORA: it is crash recovery, not the file.

## Design

**Save target per document.** `projectTarget` keeps the **opened file** with its `name`, `handle` / `path` and a `format`: `png | jpeg | webp | bmp | gif | tiff | ora`.
- **Save:**
  - `ora` saves the project exactly as today.
  - An image format checks flatten first (more than one layer, or any layer with opacity or blend that would be lost), then encodes the **flattened composite** in that format and writes it to the same handle / path.
- **Save As…** offers every format, with ORA preselected when there is more than one layer.
- **Flatten prompt.** Wording as in paint.net: "Flatten image? Saving as PNG merges all layers." with **Flatten** (default), **Save as .ora…** and Cancel.
  - Add "Don't ask again for this document" for the session.
  - The open document **keeps its layers**. Only the file is flat, which differs from paint.net, which flattens the open document. Record that choice in the Decision Log.
- **Quality.** JPEG asks once per document and remembers the value. WebP is lossless (no dialog).
- **Formats without an encoder** (GIF / TIFF, until checked): Save routes to Save As with a notice.
- **`fs` layer.** Widen `saveBytes`' format option to accept the image formats, or route image saves through a shared helper that reuses the handle / path logic. **The FSA handle from the open must be kept**, and `ensureReadWritePermission` is already there for recents.
- **Window title / dirty marker** show the real file name and extension.

## Tasks (one M session)

1. **`docs(spec)`.** Add the save semantics to §7 File I/O (Image mode). Decision Log entry: save back to the original format, flatten in the file only, autosave stays ORA, GIF / TIFF fallback.
2. **`feat(ui)`** (plus `feat(pincel-image-core)` if GIF or TIFF encoding is added).
   - Target-format tracking, the flatten prompt, the quality memory, and the `fs` changes.
   - e2e `ui/e2e/save-original.spec.ts`:
     - open a 2-layer-free PNG through the file chooser, paint, Save → the download name is `*.png` and the bytes start with the PNG signature;
     - add a layer, Save → the flatten prompt appears; Flatten → the PNG is written and the layers panel still shows 2 layers;
     - "Save as .ora…" → an `.ora` download;
     - a JPEG: Save asks the quality once, and a second Save doesn't ask again.

## Out of scope

- Writing a layered TIFF or PSD.
- Keeping EXIF or color profiles from the original. **Note it**: re-encoding drops metadata. Mention it in the prompt for JPEG if metadata was present; that's a follow-up.
