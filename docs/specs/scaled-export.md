---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §7.3, pincel.md §17.5]
---

# Scaled export in Pixel mode

## Context

A 32×32 sprite exported at 1:1 is too small to share, post or use as a store screenshot. Aseprite's export has a resize factor, and Pixelorama and Piskel export scaled; Pincel exports at original size only.

PNG frame export and sprite-sheet export get an **integer scale (×1–×10)**, upscaled with nearest-neighbour so pixels stay sharp. The sprite sheet's JSON data is scaled to match the image. The animated-export spec uses the same scale option and should share the upscale.

## Scope

### In scope

- A Scale choice (×1, ×2, ×3, ×4, ×5, ×6, ×8, ×10) for PNG frame export and for sprite-sheet export
- PNG frame export through a small dialog instead of exporting immediately
- Nearest-neighbour integer upscale of the exported image
- Scaling of the sprite sheet's JSON data (frame rects, slice rects, pivots)
- An output cap of 16384 px per edge with a clear error
- An optional `scale` argument on the wasm exports, without breaking the existing signatures
- Remembering the last used scale across sessions
- en + de strings

### Out of scope

- Non-integer or smoothing scale — because smoothing blurs pixel art, and Image mode's image scaling dialog covers free scaling.
- Scale2x / hqx filters — because `rotsprite.md` covers RotSprite / Scale2x.

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses File ▸ Export ▸ PNG (current frame) or presses its shortcut, then a dialog opens with a Scale choice of ×1, ×2, ×3, ×4, ×5, ×6, ×8 and ×10, and nothing is exported until the user confirms.
- [ ] AC-02 (state): While the sprite sheet dialog is open, it offers the same Scale choice.
- [ ] AC-03 (option): Where the scale is N, the exported PNG frame is `N × width` by `N × height` pixels (a 4×4 sprite at ×4 gives a PNG whose IHDR says 16×16).
- [ ] AC-04 (option): Where the scale is N, the exported sprite-sheet PNG is N times as wide and N times as high as the same sheet at ×1.
- [ ] AC-05 (always): In a scaled export every source pixel becomes an N×N block of exactly its RGBA value; the image contains no colour that is not in the unscaled export.
- [ ] AC-06 (option): Where the scale is N and the sheet's JSON data is exported, every frame rect (`frame`), every slice key's `bounds` and `center` and every `pivot` is multiplied by N, and `meta.size` equals the scaled sheet PNG's size.
- [ ] AC-07 (always): The wasm exports called without a scale argument produce exactly the same bytes as before this feature, and the same bytes as with scale 1.
- [ ] AC-08 (error): If a scale outside 1–10 is requested through the wasm API, then the export returns an error naming the valid range and no bytes.
- [ ] AC-09 (error): If the scaled output would be wider or higher than 16384 px, then the export returns an error naming the limit, no file is written, and in the editor the status bar shows the error.
- [ ] AC-10 (event): When the user exports with a scale and later opens either export dialog again, also after reloading the app, then that scale is preselected.
- [ ] AC-11 (always): On a profile with no stored scale, both dialogs preselect ×1.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**UI.** `ui/src/modes/pixel/file-ops.ts`: `exportPng()` (≈L297) calls `ed.doc.exportPng(ed.currentFrame)` then `saveExport(…)` with the name `<stem>[-<frame>].png`; `openSheetDialog()` (≈L317) and `exportSheet(options)` (≈L321) call `ed.doc.exportAtlas(columns, tag, imageName)` and write the PNG plus an optional `.json`. Errors go to `ed.status` via `t('status.failed', …)`. `ui/src/modes/pixel/components/SpriteSheetDialog.svelte`: `SpriteSheetOptions { columns, tag, data }`, a tag select and a columns input. Menu labels: `menu.exportPngFrame` = "PNG (current frame)" (Ctrl+E, `export` in `ui/src/lib/shell/shortcuts.ts`), `menu.exportSheet` (Ctrl+Shift+E). The macOS native menu has its own labels in `src-tauri/src/main.rs`.

**wasm.** `crates/pincel-wasm/src/lib.rs`: `exportPng(frame)` (≈L167), `exportAtlas(columns, tag, image_name)` (≈L190). Add `scale` as an optional trailing argument; the npm API is public (§17.5).

**Core.** `crates/pincel-core/src/codec/png.rs`: `export_frame_png` (≈L211), `export_atlas_png` + `AtlasOptions` (≈L84–104, L246), private `encode_rgba8` (≈L377). Proposed: `ExportScale(u8)` validated to 1–10; compose at 1×, then upscale by row and block copy; `AtlasOptions::with_scale(scale)`. Upscale helper alternative: `Raster::resample(w, h, Nearest)` in `crates/pincel-core/src/command/transform_support.rs` (≈L214). Share the upscale with the animated-export spec.

**Sheet JSON.** `crates/pincel-core/src/codec/sheet_data.rs` (`aseprite_sheet_json` ≈L18) writes `frame`, `spriteSourceSize`, `sourceSize`, `meta.size`, `meta.scale` (hard-coded `"1"`, ≈L83) and slice keys with `bounds`, optional `center` and `pivot` (≈L125–138). Compare with Aseprite's own `--scale` sheet output.

**Prefs.** IndexedDB `ui/src/lib/idb/prefs.ts` (`getPref` / `setPref`).

**e2e.** Export a 4×4 sprite at ×4 and read the PNG IHDR width and height (bytes 16–23) from the download, in the style of `ui/e2e/export.spec.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1×1 sprite at ×10 | 10×10 PNG |
| 1639 px wide sprite at ×10 (16390 px) | Error naming the 16384 px limit, no file |
| Output exactly 16384 px on an edge | Exported |
| Sprite sheet whose ×1 size fits but the scaled sheet exceeds 16384 px | Error, no PNG and no JSON written |
| Semi-transparent pixels | Alpha kept per pixel in the N×N block |
| Slice key without `center` or `pivot` | The missing keys stay absent in the scaled JSON |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| One remembered scale shared by the PNG frame and sprite-sheet dialogs (AC-10) | The legacy plan says "remember the last scale", singular | Low — a second pref is a small change |
| ×1 is preselected when no scale is stored (AC-11) | Keeps today's export output as the default | Low |

## Open questions

- The Decision Log line is not written yet: scaling the sheet JSON rects with the image, and the 16384 px per edge cap.
- Does the PNG export dialog also offer "all frames as separate files"? The legacy plan calls it optional.
- Do `sourceSize`, `spriteSourceSize` and `meta.scale` in the sheet JSON follow the scale, as in Aseprite's `--scale` output? The legacy plan asks to check Aseprite's JSON shape.

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

## Decisions

- 2026-10-28 (legacy plan): Integer nearest-neighbour scale only — pixels stay sharp, as Aseprite's resize factor does.
- 2026-10-28 (legacy plan): PNG frame export opens a small dialog with the scale instead of sharing the sprite-sheet dialog — the frame export has no columns, tag or JSON options.
- 2026-10-28 (legacy plan): `scale` is an optional trailing wasm argument — the npm API is public (§17.5) and must not break.
- 2026-10-28 (legacy plan): Output capped at 16384 px per edge — protects memory and browser canvas limits.
