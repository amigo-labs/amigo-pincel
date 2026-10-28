---
id: todo-scaled-export
title: Scaled export in Pixel mode (PNG frame and sprite sheet, ×1–×10)
status: todo
size: S–M (one session)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Scaled export in Pixel mode

## Goal

PNG frame and sprite-sheet export take an **integer scale (×1–×10)**, upscaled with nearest-neighbour, so pixels stay sharp. The sheet's JSON data (frame rects) is scaled to match.

## Why

A 32×32 sprite exported at 1:1 is too small to share, post or use as a store screenshot. Aseprite's export has a resize factor, and Pixelorama and Piskel export scaled. Pincel exports at original size only.

## Current state (read these first)

**UI.** `ui/src/modes/pixel/file-ops.ts`
- `exportPng()` (≈L297) calls `ed.doc.exportPng(ed.currentFrame)` and then `saveExport(…png)`.
- `openSheetDialog()` (≈L317) and `exportSheet(options)` (≈L321) call `ed.doc.exportAtlas(columns, tag, …)`, and write PNG plus optional `.json`.
- `ui/src/modes/pixel/components/SpriteSheetDialog.svelte` has `SpriteSheetOptions { columns, tag, data }` (≈L8), a tag select and a columns input.

**wasm.** `crates/pincel-wasm/src/lib.rs`: `exportPng(frame)` (≈L167), `exportAtlas(...)` (≈L190).

**Core.** `crates/pincel-core/src/codec/png.rs`
- `export_frame_png` (≈L211)
- `export_atlas_png` + `AtlasOptions` (≈L84–104, L246)
- the private `encode_rgba8` (≈L377)

The sheet JSON is `crates/pincel-core/src/codec/sheet_data.rs` (`aseprite_sheet_json`, ≈L18).

**Upscale helper.** `Raster::resample(w, h, Nearest)` in `crates/pincel-core/src/command/transform_support.rs` (≈L214), or a direct block copy, which is faster for integer factors.

## Design

**Core.**
- `ExportScale(u8)`, validated to 1–10. Cap the output at 16384 px per edge, to protect memory and browser canvas limits, with a clear error.
- `export_frame_png(sprite, frame, scale)` composes at 1×, then integer-upscales with a row and block copy (each source pixel becomes a `scale × scale` block).
- `AtlasOptions::with_scale(scale)`: upscale the packed sheet, **and multiply every frame rect, slice rect and pivot in the JSON** by `scale`.
  - Aseprite's own `--scale` sheet export does the same. Check with its JSON shape.

**wasm.** An extra `scale` argument, defaulting to 1: `exportPng(frame, scale?)`, `exportAtlas(..., scale?)`. The npm API is public (§17.5): add an optional argument, don't break the existing signature.

**UI.**
- The sprite sheet dialog gets a **Scale** select: ×1, ×2, ×3, ×4, ×5, ×6, ×8, ×10.
- PNG frame export goes through a small dialog, or shares the sheet dialog with a "single frame" choice. **Recommendation:** File ▸ Export ▸ PNG… opens a small dialog with scale and "current frame / all frames as separate files". All-frames is optional.
- Remember the last scale in the IndexedDB prefs.
- i18n: `export.scale` etc.
- **`06-animated-export.md`** uses the same scale option. Share the upscale function from step 1.

## Tasks (one session)

1. **`docs(spec)`.** Add scale to §7.3 PNG export, plus a Decision Log line covering the JSON rects scaling and the 16384 cap.
2. **`feat(pincel-core)`.** Upscale and options.
   - Tests:
     - `export_frame_scale_3_triples_dimensions_nearest`
     - `atlas_scale_2_doubles_rects_in_json`
     - `scale_out_of_range_is_error`
     - `scale_over_edge_cap_is_error`
3. **`feat(pincel-wasm)` + `feat(ui)`.**
   - The arguments, the dialog and the prefs.
   - e2e: export a 4×4 sprite at ×4. Read the download's PNG IHDR width and height (bytes 16–23), which must equal 16×16, in the style of `ui/e2e/export.spec.ts`.

## Out of scope

- Non-integer or smoothing scale (the image scaling dialog covers that in Image mode).
- Scale2x / hqx filters (`08-pixel-tools.md` covers RotSprite / Scale2x internals).
