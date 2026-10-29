---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §7.3, pincel.md §7.4, pincel.md §4.1]
---

# Pixel PNG frame and sprite-sheet export

## Context

Pixel mode exports what `compose()` shows (`pincel.md` §4.1, spec `pixel-compose`) in two ways (§7.3):

- **PNG (current frame)** — the current frame, full canvas, 1:1, as an RGBA8 PNG.
- **Sprite sheet** — the frames of the document (or of one tag) packed into a fixed-cell grid PNG, with an optional sidecar JSON in Aseprite's `--data` "array" layout (frame rects and durations, tags, slices), so engines and tools that read Aseprite sheets read Pincel's.

`pincel-core` additionally returns a small manifest of per-frame source rects with every atlas. Pincel's engine contract is still the `.aseprite` file itself (§7.4); export is for everything else.

This spec records the behavior as built.

## Scope

### In scope

- PNG export of the current frame: pixels, file name, menu entry and shortcut, errors
- Sprite-sheet export: grid packing, column clamping, tag filter, the manifest, the Aseprite `--data` JSON, the Export Sprite Sheet dialog and the files it saves
- Error reporting for both

### Out of scope

- Scaled export (×2 … ×10) — because the `scaled-export` spec covers it
- Animated GIF / APNG / WebP and other formats — because the `animated-export` spec covers them
- Saving back to the opened image format — because the `save-original-format` spec covers it
- Trimming, rotation, bin-packing and the JSON "hash" layout — because the grid is the contract today (§7.3) and no engine integration has asked for more
- How a file reaches the disk (File System Access, download fallback, Tauri dialogs) — because the `file-access` spec owns it
- Image-mode export (PNG / JPEG / WebP) — because the `image-files` spec owns it

## Acceptance criteria

- [ ] AC-01 (always): An exported PNG frame decodes to an RGBA8 image of the canvas size whose pixels equal `compose()` of that frame at zoom 1 over the full canvas with the visible layers — partial alpha, every blend mode, group visibility and tilemap layers included.
- [ ] AC-02 (state): While the onion skin is enabled or hidden layers exist, an exported PNG frame and every sprite-sheet cell contain neither onion-skin pixels nor hidden layers.
- [ ] AC-03 (event): When File ▸ Export ▸ PNG (current frame) is chosen or Ctrl/Cmd+E is pressed, the current frame is saved as `<document name>.png` for a one-frame document, or `<document name>-<n>.png` with the 1-based frame number otherwise, and the status bar reports the export with its byte size.
- [ ] AC-04 (error): If the frame index is past the last frame, then the PNG export fails with an error.
- [ ] AC-05 (error): If the document's color mode is not RGBA, then the PNG frame export fails with the composer's `UnsupportedColorMode` error instead of writing an image.
- [ ] AC-06 (error): If an export fails in the UI, then the status bar shows the failure with its reason and no file is saved.
- [ ] AC-07 (always): A sprite sheet places the exported frames row-major in playback order into `columns` × `ceil(frames / columns)` cells, each cell the full canvas size, so the sheet is `columns · width` × `rows · height` pixels.
- [ ] AC-08 (always): Cells after the last exported frame in the final row are fully transparent.
- [ ] AC-09 (always): A requested column count larger than the number of exported frames is clamped down to that number.
- [ ] AC-10 (option): Where a tag is selected, the sheet holds exactly the frames from the tag's first to its last frame, and every frame is labeled with that tag even when other tags overlap it.
- [ ] AC-11 (always): The manifest lists sheet size, columns, rows, cell size and, per exported frame, its source frame index, cell rect and tag (`null` outside every tag) as JSON with camelCase keys and escaped tag names.
- [ ] AC-12 (always): The sheet's data JSON follows Aseprite's `--data` array layout: per frame `filename` (`<image stem> <frame>`), `frame` rect, `rotated: false`, `trimmed: false`, `spriteSourceSize`, `sourceSize` and `duration` in ms; `meta` with `image`, `format: "RGBA8888"`, `size`, `scale: "1"`, `frameTags` (name, from, to, direction, color) and `slices` (name, color, keys with bounds and optional center and pivot).
- [ ] AC-13 (option): Where the sheet holds one tag's frames, tag ranges in the data JSON are clipped and renumbered to sheet frame indices, tags outside the sheet are left out, and slice keys are re-emitted at the first sheet frame where their geometry applies.
- [ ] AC-14 (error): If the column count is 0, the tag is unknown, a tag's frame range reaches past the last frame, the selection holds no frames, or the sheet would be too large to address, then the sprite-sheet export fails with the matching error and produces no image.
- [ ] AC-15 (event): When File ▸ Export ▸ Sprite Sheet… is chosen or Ctrl/Cmd+Shift+E is pressed, the Export sprite sheet dialog opens with "All frames (n)" selected, one option per tag with its frame count, the columns field at `ceil(√frames)`, and the JSON checkbox checked.
- [ ] AC-16 (state): While the dialog is open, it shows the resulting grid and size as `<columns> × <rows> cells · <W>×<H> px` for the current columns and frame selection.
- [ ] AC-17 (event): When Export is pressed with the JSON checkbox checked, `<document name>.png` and `<document name>.json` are saved, and the JSON's `meta.image` names that PNG.
- [ ] AC-18 (option): Where a tag is selected in the dialog, only its frames are exported and both file names become `<document name>-<tag>`.
- [ ] AC-19 (option): Where the JSON checkbox is unchecked, only the PNG is saved.
- [ ] AC-20 (event): When Cancel or Escape is pressed in the dialog, it closes and nothing is exported.
- [ ] AC-21 (error): If any exported frame cannot be composed (non-RGBA color mode, linked cel, corrupt cel), then the whole sprite-sheet export fails with the composer's error and produces no image.

## Technical notes

> Paths are hints from 2026-10-29 and drift — grep for the names.

- Core (`crates/pincel-core/src/codec/png.rs`): `export_frame_png(sprite, cels, frame) -> Result<Vec<u8>, ExportError>` (composes `ComposeRequest::full`), `export_atlas_png(sprite, cels, &AtlasOptions) -> Result<AtlasOutput, ExportError>`, `AtlasOptions { columns: u32, tag: Option<String> }`, `AtlasOutput { png, manifest: AtlasManifest }`, `AtlasManifest { width, height, columns, rows, frame_width, frame_height, frames: Vec<AtlasFrame> }`, `AtlasFrame { frame, x, y, w, h, tag }`, `AtlasManifest::to_json()`. `ExportError::{Render(RenderError), Encode, NoFrames, InvalidColumns, TagNotFound, TagRangeInvalid, AtlasTooLarge}`.
- Data JSON (`crates/pincel-core/src/codec/sheet_data.rs`): `aseprite_sheet_json(sprite, &manifest, image) -> String`; `meta.app` is `https://pincel.amigo-labs.dev/`, `meta.layers` is always `[]`.
- wasm (`crates/pincel-wasm/src/lib.rs`): `exportPng(frame) -> Uint8Array`; `exportAtlas(columns, tag?, imageName?) -> AtlasExport { png, manifest, data }` (`imageName` defaults to `sheet.png`).
- UI: `exportPng`, `openSheetDialog`, `exportSheet` in `ui/src/modes/pixel/file-ops.ts`; `ui/src/modes/pixel/components/SpriteSheetDialog.svelte` (`SpriteSheetOptions { columns, tag, data }`); menu entries in `ui/src/modes/pixel/menus.ts`; shortcuts `export` (Mod+E) and `exportSheet` (Mod+Shift+E) in `ui/src/lib/shell/shortcuts.ts`. `<document name>` is the save target's name without `.aseprite` / `.ase` (`pincel` for a new document).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| One-frame document, sprite sheet | 1 × 1 cell; the PNG equals the frame export |
| Columns field emptied or set to 0 in the dialog | Treated as 1 column |
| Columns field above the number of selected frames | Treated as that number |
| Frame outside every tag in an all-frames sheet | Manifest tag `null`; not listed in any `frameTags` range |
| Overlapping tags, all-frames sheet | Manifest labels each frame with the first tag in document order that covers it |
| Slice with no key at or before the first sheet frame | Its keys start at the first sheet frame they apply to; a slice with no applicable key is left out |
| Tag or slice name with quotes or backslashes | Escaped in both JSON outputs |
| Save picker cancelled for the PNG | Nothing is saved, the JSON is not offered |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-core: codec::png::tests::export_frame_png_1x1_round_trips_to_composed_pixels`, `export_frame_png_preserves_partial_alpha_and_layer_stack`, `export_exports_non_normal_blend_mode_like_compose`, `export_composes_group_layers_like_compose`, `export_atlas_png_composes_tilemap_layers_like_compose`; `pincel-wasm: tests::export_png_matches_core_export` | built | |
| AC-02 |      | open   | |
| AC-03 |      | open   | |
| AC-04 | `pincel-core: codec::png::tests::export_frame_png_rejects_frame_past_end`; `pincel-wasm: tests::export_png_rejects_unknown_frame` | built | |
| AC-05 | `pincel-core: codec::png::tests::export_rejects_non_rgba_color_mode_with_render_error` | built | |
| AC-06 |      | open   | |
| AC-07 | `pincel-core: codec::png::tests::export_atlas_png_packs_four_frames_into_two_by_two_grid`; `pincel-wasm: tests::export_atlas_packs_every_frame_into_the_requested_grid` | built | |
| AC-08 | `pincel-core: codec::png::tests::export_atlas_png_leaves_unused_trailing_cells_transparent` | built | |
| AC-09 | `pincel-wasm: tests::export_atlas_clamps_columns_to_frame_count` | built | |
| AC-10 | `pincel-core: codec::png::tests::export_atlas_png_with_tag_exports_only_that_tags_frames`, `export_atlas_png_with_overlapping_tags_labels_frames_with_the_requested_tag`; `pincel-wasm: tests::export_atlas_with_tag_exports_only_that_tags_frames` | built | |
| AC-11 | `pincel-core: codec::png::tests::manifest_to_json_emits_camel_case_keys_and_null_tags`, `manifest_to_json_escapes_tag_names`, `export_atlas_png_untagged_frames_report_no_tag` | built | |
| AC-12 | `pincel-core: codec::sheet_data::tests::full_sheet_lists_frames_durations_tags_and_slices`; `pincel-wasm: tests::export_atlas_data_is_aseprite_sheet_json` | built | |
| AC-13 | `pincel-core: codec::sheet_data::tests::tag_sheet_renumbers_tags_and_slice_keys` | built | |
| AC-14 | `pincel-core: codec::png::tests::export_atlas_png_rejects_zero_columns`, `export_atlas_png_rejects_unknown_tag`, `export_atlas_png_rejects_tag_range_past_last_frame`, `export_atlas_png_rejects_sprite_without_frames`, `export_atlas_png_reports_too_large_instead_of_overflowing_the_byte_count` | built | |
| AC-15 |      | open   | |
| AC-16 | `ui/e2e/export.spec.ts › sprite sheet export writes the PNG and the Aseprite JSON` | built | |
| AC-17 | `ui/e2e/export.spec.ts › sprite sheet export writes the PNG and the Aseprite JSON` | built | |
| AC-18 |      | open   | |
| AC-19 |      | open   | |
| AC-20 |      | open   | |
| AC-21 |      | open   | |

## Decisions

