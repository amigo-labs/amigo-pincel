---
status: todo
created: 2026-10-29
size: M
mode: Image
design: [pincel.md §1.1, pincel.md §15 (2026-09-06 Two cores, one shell), pincel.md §15 (2026-10-11 Image-mode project format is OpenRaster)]
---

# Image-mode file formats: open, OpenRaster save, export

## Context

Image mode opens ordinary raster files — PNG, JPEG, WebP, BMP, GIF, TIFF — and its own layered project format, OpenRaster (`.ora`), chosen so a layered Pincel image opens directly in GIMP, Krita and MyPaint (Decision Log 2026-10-11). Save and Save As write `.ora`; File ▸ Export writes the flattened image as PNG, JPEG or WebP. The decoders and encoders sit in `pincel-image-core` on the `image` crate (Decision Log 2026-09-06) and the `zip` crate (2026-10-11). This spec records what those formats do today, including their current limits.

## Scope

### In scope

- Decoding PNG, JPEG, WebP, BMP, GIF and TIFF into a one-layer document
- The OpenRaster project: what Save writes, what Open reads (Pincel's own files and other applications')
- Export ▸ PNG / JPEG / WebP of the flattened composite, the JPEG quality dialog, export file names
- Document size limits for opened and resized images, and large-canvas rendering

### Out of scope

- File pickers, download fallbacks, Save writing in place, Open Recent, drag-and-drop and read-failure notices — because the `file-access` spec covers them
- Routing a file to Pixel or Image mode by its format, and the New dialog's size defaults and clamping — because the `app-shell` spec covers them
- Autosave snapshots (which store the same `.ora` bytes) — because the `autosave-recovery` spec covers them
- Saving back to the opened format (PNG stays PNG) — because the planned `save-original-format` spec adds it
- Layer groups in `.ora` — because the planned `image-layer-groups-ora` spec adds them; today nested stacks are flattened on read
- PSD import — because the planned `psd-import` spec adds it
- Lossy WebP export — because the pure-Rust `image` encoder writes lossless WebP only and system codecs are excluded (core rule: no platform dependencies)

## Acceptance criteria

- [ ] AC-01 (event): When the user opens a PNG, JPEG, WebP, BMP, GIF or TIFF file in Image mode, then a document opens with one layer named "Background" holding the image's pixels, at the image's width and height.
- [ ] AC-02 (always): Opening a raster file keeps its first frame only (an animated GIF opens as its first frame), converts higher bit depths to 8 bits per channel, and drops embedded metadata such as EXIF.
- [ ] AC-03 (always): PNG, BMP and WebP files written by Image mode decode back to exactly the pixels that were written, for any pixel content including fully transparent pixels and a 1 × 1 image.
- [ ] AC-04 (event): When the user exports PNG, then the file holds the composite of the visible layers: after opening a PNG, painting and exporting, the painted pixels are in the file and the untouched ones are unchanged; after undoing the paint, the export equals the original.
- [ ] AC-05 (event): When the user chooses File ▸ Export ▸ JPEG, then a dialog asks for the quality (1–100, default 90) before the file is written; PNG and WebP export without a dialog.
- [ ] AC-06 (always): A JPEG export flattens transparent areas onto white, and at quality 90 stays perceptually close to the source (mean absolute error per channel below 8 of 255).
- [ ] AC-07 (event): When the user exports, then the file is named after the opened file's name without its extension plus `.png`, `.jpg` or `.webp` (`image.*` for a new document), and the document's unsaved state does not change.
- [ ] AC-08 (always): A saved `.ora` file is a ZIP whose first entry is the uncompressed `mimetype` with the content `image/openraster`, and it contains `stack.xml`, `mergedimage.png`, `Thumbnails/thumbnail.png` and one PNG per layer.
- [ ] AC-09 (event): When the user saves an Image document and opens the `.ora` again, then the canvas size, the number and order of layers and every layer's name, pixels, opacity (within 0.001), blend mode, visibility and lock are the same, and the same layer is active.
- [ ] AC-10 (always): A saved `.ora`'s `mergedimage.png` is the composite of the visible layers, and its thumbnail is at most 256 px on its longer side with the canvas's aspect ratio.
- [ ] AC-11 (event): When the user opens an `.ora` written by another application, then nested layer stacks are flattened into one list in document order, a layer image stored at an `x` / `y` offset or at a different size lands at that offset on a canvas-sized layer, an unknown `composite-op` reads as Normal, and a layer without a name is called "Layer <n>" by its position in the file.
- [ ] AC-12 (error): If a `.ora` file's `mimetype` entry is not `image/openraster`, then opening fails with "not an OpenRaster file" and no document is created from it.
- [ ] AC-13 (error): If the bytes of an opened file are not a decodable image, then opening fails and no document is created from it.
- [ ] AC-14 (error): If a document would be 0 px or more than 32767 px wide or tall, then it is refused.
- [ ] AC-15 (state): While the canvas is wider than 8192 px, the image opens, renders through a render target no wider than the viewport, accepts paint, and re-renders at 1:1 zoom.
- [ ] AC-16 (event): When the user asks Resize Canvas or Scale Image for a size beyond 16384 px on an edge or 8192 × 8192 px in area, then the size is reduced to fit — each edge to at most 16384 px, both edges scaled down together when the area is too large.

## Technical notes

Hints, verified 2026-10-29; paths and line numbers drift.

**Codecs** (`crates/pincel-image-core/src/codec/mod.rs`)

- `decode(bytes) -> Result<ImageBuffer, CodecError>` — `image::load_from_memory`, format by signature, `to_rgba8()`. `CodecError::{Decode, Encode, Buffer}`.
- Encoders write to any `Write`: `encode_png(buf, w, compression 0–9)` (0–2 Fast, 3–6 Default, 7–9 Best, adaptive filter), `encode_jpeg(buf, w, quality 1–100)` (over white in linear light, then RGB8), `encode_bmp` (32-bit RGBA, not offered in the UI), `encode_webp` (lossless, ADR-007). `to_*_bytes` wrappers return `Vec<u8>`.
- `Document::from_pixels(buffer)` → one layer `"Background"`; the canvas is `CanvasSize::new(w, h)` (`MIN_CANVAS_DIM = 1`, `MAX_CANVAS_DIM = 32767`, `DocumentError::InvalidCanvasSize`).

**OpenRaster** (`codec/ora.rs`)

- `to_ora_bytes(doc, compression) -> Result<Vec<u8>, OraError>`; `from_ora_bytes(bytes) -> Result<Document, OraError>`; `OraError::{Zip, MissingEntry, NotOpenRaster, Stack, Codec, Document}`; `ORA_MIMETYPE = "image/openraster"`.
- Writes every entry `Stored`: `mimetype`, `data/layer{index}.png` (index = bottom-up storage index), `stack.xml`, `mergedimage.png`, `Thumbnails/thumbnail.png` (bilinear, ≤ 256 px).
- `stack.xml`: `<image version="0.0.6" w h xres="72" yres="72"><stack>` with one `<layer name src x="0" y="0" opacity="%.3f" visibility="visible|hidden" composite-op="svg:…" [edit-locked="true"] [selected="true"] />` per layer, top-most first. Blend modes map to `svg:src-over`, `svg:multiply`, `svg:screen`, `svg:overlay`, `svg:darken`, `svg:lighten`, `svg:color-dodge`, `svg:color-burn`, `svg:hard-light`, `svg:soft-light`, `svg:difference`, `svg:exclusion`.
- Read: a small attribute scanner (no XML crate); `<image w h>` required; per `<layer>` `src` required; `opacity` default 1 (clamped), `visibility` default visible, `edit-locked="true"` → locked, `selected="true"` → active (else the top layer); offset layers via `ImageBuffer::offset_copy`. `mergedimage.png` and the thumbnail are ignored on read.

**wasm** (`crates/pincel-image-wasm/src/lib.rs`): `open_image(data, _mime_type)`, `open_project(data)`, `save_project(handle, compression)`, `export_png(handle, compression)`, `export_jpeg(handle, quality)`, `export_webp(handle)` — every export encodes `compose(layers)`.

**UI** (`ui/src/modes/image/core/controller.ts`): `openBytes` sniffs `.ora` (`sniffFormat`) and decodes before closing the current document; `exportStem` = opened name without extension or `image`; `projectTarget` = the opened `.ora`, else `<stem>.ora`; Save uses PNG level 6, autosave level 1 (`projectBytes(fast)`). `exportImage(format, quality)` clamps quality to 1–100 and calls `saveExport(bytes, "<stem>.<png|jpg|webp>", format)`. JPEG dialog: `components/dialogs/ExportDialog.svelte`. Size limits: `MAX_CANVAS_DIM = 16384`, `MAX_CANVAS_AREA = 8192 * 8192`, `clampCanvasSize` in `stores/editor.svelte.ts`, used by New, Resize, Scale and paste-expand. The viewport renderer calls `composite_region(handle, x, y, w, h, out_w, out_h)` for just the visible rect.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `.ora` without a `stack.xml` or with a layer `src` missing from the archive | Opening fails with a missing-entry error |
| `.ora` whose `<image>` lacks a numeric `w` or `h` | Opening fails with an invalid-`stack.xml` error |
| `.ora` with an empty `<stack>` | Opens as a blank canvas with one transparent layer |
| `.ora` with no `selected` layer | The top layer becomes active |
| Layer opacity outside 0–1 in `stack.xml` | Clamped to 0–1 |
| Attribute values with `&amp;` etc. | Unescaped on read, escaped on write |
| Opened raster image larger than 8192 × 8192 px in area but within 32767 px per edge | Opens at full size; the 8192 × 8192 px area limit is not applied on open |
| Corrupt file while another document is open | Opening fails, the open document stays |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 | `pincel-image-core (tests/codec_proptest.rs): png_round_trip_is_lossless`, `bmp_round_trip_is_lossless`; `pincel-image-core: codec::tests::webp_lossless_round_trip_is_pixel_exact`, `codec::tests::png_preserves_full_transparency`, `codec::tests::png_one_by_one_round_trips` | built | |
| AC-04 | `pincel-image-core (tests/pencil_round_trip.rs): open_png_paint_pencil_export_png_is_pixel_correct`, `paint_then_undo_then_export_matches_original` | built | |
| AC-05 |      | open   |        |
| AC-06 | `pincel-image-core: codec::tests::jpeg_flattens_transparency_over_white`, `codec::tests::jpeg_round_trip_is_perceptually_close` | built | |
| AC-07 |      | open   |        |
| AC-08 | `pincel-image-core: codec::ora::tests::to_ora_bytes_stores_the_mimetype_first_and_uncompressed`; `ui/e2e/ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` | built | |
| AC-09 | `pincel-image-core: codec::ora::tests::to_ora_bytes_then_from_ora_bytes_round_trips_layers`; `pincel-image-wasm: project_tests::save_project_then_open_project_keeps_the_layers`; `ui/e2e/ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` | built | |
| AC-10 |      | open   |        |
| AC-11 | `pincel-image-core: codec::ora::tests::from_ora_bytes_places_offset_layers_and_flattens_nested_stacks` | built | |
| AC-12 | `pincel-image-core: codec::ora::tests::from_ora_bytes_rejects_other_mimetypes` | built | |
| AC-13 | `pincel-image-core: codec::tests::decode_garbage_errors` | built | |
| AC-14 | `pincel-image-core: document::canvas::tests::new_rejects_zero_dimension`, `document::canvas::tests::new_rejects_oversized_dimension`, `document::canvas::tests::new_accepts_boundary_values` | built | |
| AC-15 | `ui/e2e/large-image.spec.ts › image mode: a canvas wider than 8192 px opens, renders and paints` | built | |
| AC-16 |      | open   |        |

Partial coverage (status stays `open`): AC-01 — `pencil_round_trip.rs › open_png_paint_pencil_export_png_is_pixel_correct` opens a PNG and checks its size after export; JPEG / WebP / BMP / GIF / TIFF opens and the "Background" name are untested. AC-05 — no e2e drives Export ▸ JPEG. AC-10 — `to_ora_bytes_stores_the_mimetype_first_and_uncompressed` checks the entries exist, not their content or the thumbnail size. AC-16 — `large-image.spec.ts › image mode: the New dialog clamps an oversized canvas to the area limit` covers the same clamp through the New dialog (owned by `app-shell`), not Resize Canvas / Scale Image.

## Decisions

{Filled during revision and implementation — decision plus reason.}
