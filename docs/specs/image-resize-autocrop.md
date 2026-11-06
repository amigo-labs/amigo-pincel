---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6.1, pincel.md §9.2, pincel.md §15 (2026-09-06 Two cores, one shell), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image mode: Resize by percent and print size, Supersampling, Auto Crop

## Context

Image mode should reach paint.net parity (owner, 2026-11-05). Today the Image menu has **Resize Canvas…** (width / height in pixels plus a 9-grid anchor) and **Scale Image…** (width / height in pixels, a "Constrain proportions" toggle, resampling Nearest / Bilinear / Bicubic), both accepted in `image-editing` AC-25, with oversized requests reduced to the size limits by `image-files` AC-16. paint.net's Resize and Canvas Size dialogs also take a percentage and a print size (inches or centimeters at a resolution), and its resampling list has a Supersampling filter that averages the source area for clean downscales. paint.net 5 also has Image ▸ Auto Crop, which trims a uniform border from the image. Pincel has none of these.

This spec adds them to Image mode: percentage and print-size input in both dialogs, a Supersampling filter, and Auto Crop. Following Decision Log 2026-11-05 "MCP tool parity", the new capabilities (Supersampling, Auto Crop) also reach agents through `pincel-agent`.

The document model already carries a resolution, `DocumentMetadata.dpi` (default 96), but no codec reads or writes it and no UI shows it. The owner decided (Decisions, 2026-11-05) that the resolution field here is only a conversion factor of the dialog, starting at that value; reading and writing a file's resolution is a later spec.

"The editor" in the MCP ACs means the result of the core command the UI issues for the action, as in `mcp-parity-structure`.

## Scope

### In scope

- Scale Image… dialog: "By percentage" or "By absolute size"; absolute size in pixels, percent, inches or centimeters; a resolution field in pixels per inch; the aspect-ratio toggle; a live readout of the resulting pixel size
- Resize Canvas… dialog: the same size input (percentage, absolute size with units and resolution, aspect-ratio toggle), keeping its anchor grid
- A fourth resampling mode, Supersampling (area average), as the dialog's default
- Image ▸ Auto Crop: crop the canvas to the content inside a uniform border, one undo step
- MCP: `image.scale` accepts `supersampling`; a new tool `image.auto_crop`
- en + de strings for every new label, option and notice

### Out of scope

- Changing the document's resolution, and reading / writing it in PNG (pHYs), JPEG (JFIF density), TIFF or OpenRaster — because it is a file-format change the owner moved to a later spec (Decisions, 2026-11-05); here the resolution only converts print size to pixels
- Pixel mode (its Resize Canvas / Scale Image, an Aseprite-style Trim) — because Pixel mode follows Aseprite and pixel art scales by integer factors; Pixel-mode trim needs its own spec
- Renaming the menu items to paint.net's "Resize…" / "Canvas Size…" — because the labels `menu.resizeCanvas` / `menu.scaleImage` are shared with Pixel mode, and e2e specs and other specs (`image-files` AC-16, `power-user-shortcuts-palette`) refer to them
- Default keyboard shortcuts for the two dialogs and Auto Crop — because paint.net's Ctrl+R / Ctrl+Shift+R collide with desktop Rotate and browser reload (Decision Log 2026-10-29); the keymap specs own default chords
- A tolerance option for Auto Crop — because paint.net's Auto Crop has none and an exact match keeps the result predictable; noisy JPEG borders are a follow-up if users ask
- Further filters (Lanczos, Fant, "Bicubic smooth") — because the topic asks for Supersampling only; each extra filter is a separate parity decision
- Percent or print-size arguments for the MCP tools — because agents can compute pixel sizes; parity covers capabilities, not input conveniences of a dialog
- Linear-light resampling — because the existing three filters resample gamma-encoded values and all four should agree on brightness (Assumptions)

## Acceptance criteria

### Size input (both dialogs)

- [ ] AC-01 (always): Every time Scale Image… or Resize Canvas… opens, it shows the choice "By percentage" / "By absolute size" with "By percentage" selected at 100; a single percentage field; for the absolute size a width and a height field with a unit list (Pixels, Percent, Inches, Centimeters) set to Pixels and filled with the document's size; a "Maintain aspect ratio" toggle, on; a resolution field in pixels per inch holding the document's resolution (96 if that is zero, negative or not finite, as `image-view-aids` AC-21); and a readout of the resulting size in pixels that updates on every input. No value from an earlier opening is kept.
- [ ] AC-02 (event): When the user picks "By percentage" and enters p, then the resulting size is round(width × p / 100) × round(height × p / 100) pixels, each edge at least 1 px, and Apply produces exactly that size (Scale Image: the image is resampled to it; Resize Canvas: the canvas is resized to it at the chosen anchor). The percentage scales both edges, so the aspect-ratio toggle is disabled in this mode.
- [ ] AC-03 (event): When the user picks "By absolute size" with a unit of inches or centimeters at resolution r, then an edge of value v becomes round(v × r) pixels for inches and round(v × r / 2.54) pixels for centimeters, each at least 1 px; with the unit percent, round(edge × v / 100) pixels; with pixels, v pixels.
- [ ] AC-04 (event): When the user changes the unit, then width and height show the current resulting pixel size in the new unit (inches, centimeters and percent to two decimals, pixels as integers), and the resulting pixel size does not change by more than 1 px per edge. When the user switches between "By percentage" and "By absolute size", each mode keeps the values it last held and the readout follows the selected mode.
- [ ] AC-05 (event): When the user changes the resolution while the unit is inches or centimeters, then the entered width and height stay and the resulting pixel size follows AC-03; while the unit is pixels or percent, or "By percentage" is selected, a resolution change leaves the resulting pixel size unchanged.
- [ ] AC-06 (state): While "By absolute size" is selected and "Maintain aspect ratio" is on, editing width sets height to the value that keeps the document's width-to-height ratio (rounded to the unit's precision), and editing height sets width likewise; while it is off, each edge changes alone.
- [ ] AC-07 (error): If a field the selected mode uses is invalid — empty, not a number, zero or negative; a fraction in Pixels; a resolution above 10000 — then that field is marked invalid, the readout shows no size, Apply is disabled, and Enter does not apply; the document is unchanged. The resolution counts only while the unit is inches or centimeters, and the absolute fields only while "By absolute size" is selected; an invalid field the selected mode does not use blocks nothing.
- [ ] AC-08 (error): If the resulting size exceeds the Image-mode limits (16384 px per edge, 8192 × 8192 px in area), then the readout shows the size reduced as in `image-files` AC-16 and Apply produces that reduced size.

### Supersampling

- [ ] AC-09 (always): The Scale Image… resampling list offers Supersampling, Nearest, Bilinear and Bicubic, with Supersampling selected when the dialog opens.
- [ ] AC-10 (event): When an image is scaled with Supersampling to a size not larger than the original in either edge, then each target pixel is the mean of the source area it covers, each source pixel weighted by the fraction of that area it covers: alpha is the weighted mean of the alphas, RGB the weighted mean weighted additionally by alpha, every channel within ±1 of the exact value for integer and non-integer factors alike (for an integer factor k, the mean of its k × k block; for 3 → 2 px, weights 1 and ½). A 2 × 1 image of opaque red and fully transparent black scales to one pixel of red (255, 0, 0) at alpha 127 or 128, not a darkened red; a target pixel whose whole source area has alpha 0 is (0, 0, 0, 0).
- [ ] AC-11 (event): When an image is scaled with Supersampling to a size larger than the original in at least one edge, then the result equals Bicubic at the same size.
- [ ] AC-12 (always): Scaling with Supersampling is one undo step that, like the other filters, resamples every image layer, clears the selection and restores layers, canvas size and selection on Undo (`image-editing` AC-25).

### Auto Crop

- [ ] AC-13 (event): When the user chooses Image ▸ Auto Crop, then the border color is the top-left pixel of the composite as the canvas shows it (visibility including hidden groups, opacity and blend modes, `image-editing` AC-01), and the canvas shrinks to the smallest rectangle that contains every composite pixel differing from it; two pixels match when all four channels are equal, or when both have alpha 0. Every layer, hidden ones included, is cropped to that rectangle in the same place.
- [ ] AC-14 (always): Auto Crop is one undo step; it clears the selection, and Undo restores every layer's pixels, the canvas size and the selection.
- [ ] AC-15 (error): If none of the outermost row and column on any of the four edges consists only of pixels matching the border color, or every composite pixel matches it, then Auto Crop changes nothing, adds no undo step, and the editor shows the notice "Nothing to crop".
- [ ] AC-16 (state): While an Image document is open, Image ▸ Auto Crop is enabled, with or without a selection; the selection does not limit the area Auto Crop examines.

### MCP

- [ ] AC-17 (event): When the client calls `image.scale` with `interpolation: "supersampling"`, then the document equals the editor's after Scale Image with Supersampling at the same size; without `interpolation` the default stays `bilinear`.
- [ ] AC-18 (event): When the client calls `image.auto_crop` on an Image document, then its layers, canvas size and selection equal the editor's after Auto Crop, the call is one undo step, and the result is `{ "cropped": true, "width": <new width>, "height": <new height>, "rect": { "x", "y", "w", "h" } }`, `rect` being the kept area in the old canvas's pixels.
- [ ] AC-19 (error): If `image.auto_crop` finds nothing to crop (AC-15), then the call succeeds with the result `{ "cropped": false }`, the document is unchanged and no undo step is added; if it is called on a Pixel document, it fails with `isError: true` naming the document's mode, as `mcp-headless-image` AC-05 does.
- [ ] AC-20 (always): `tools/list` contains `image.auto_crop` meeting `mcp-headless-pixel` AC-02, the `image.scale` schema lists `supersampling`, and the `pincel://guide` resource names `image.auto_crop` and the Supersampling filter.

### Strings

- [ ] AC-21 (always): Every new label, option, unit, tooltip and notice of this spec has an English and a German string; no new UI text is hard-coded.
- [ ] AC-22 (option): Where the UI language is German, decimal values in the size fields and the readout show a comma as decimal separator; in English a point. In either language the fields accept both a comma and a point as decimal separator.

## Technical notes

- Core today: `ScaleImage` (`crates/pincel-image-core/src/command/transform.rs`) calls `transform::scale` with `Interpolation::{Nearest, Bilinear, Bicubic}`; `ResizeCanvas` (`command/resize.rs`) takes width, height and `Anchor`; `CropToSelection` snapshots the document for undo and clips layers (ADR-010). `CommandSpec` tags `scale_image` (`interpolation` string, unknown values fall back to bilinear in `parse_interpolation`) and `resize_canvas`. Auto Crop fits beside `CropToSelection` as a `CommandSpec` tag (e.g. `auto_crop`) whose TS mirror is regenerated (`cargo test -p pincel-image-wasm export_command_spec`).
- UI today: `ui/src/modes/image/components/dialogs/ScaleDialog.svelte` and `ResizeDialog.svelte` hold their own width / height fields and clamp with `clampCanvasSize` from `stores/editor.svelte.ts`. A shared size-input block for both dialogs avoids two copies of the unit logic. The notice uses `editor.notice`.
- The dialog's starting resolution comes from `DocumentMetadata.dpi`; the parallel spec `image-view-aids` adds a read-only wasm getter for it, which this spec reuses instead of adding its own (soft note, not a dependency: until that getter exists the value is 96 for every document).
- MCP today: `image.scale` and `canvas.resize` in `crates/pincel-agent/src/image/canvas.rs`; `image.auto_crop` sits beside `image.crop_to_selection`.
- If `history-panel` lands first, the new step (Auto Crop) needs its kind id and en / de name there (`history-panel` AC-04, AC-15); Supersampling reuses the Scale Image kind.
- Existing e2e specs address the dialogs by their titles and the fields by "Width" / "Height" (`ui/e2e/image-export.spec.ts`, `ui/e2e/symmetry-tiled.spec.ts`); the new block keeps these accessible names.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1 % of a 50 × 50 image | 1 × 1 px (each edge at least 1 px, AC-02) |
| 1000 % of a 4000 × 3000 image | Reduced to the limits as in `image-files` AC-16 (AC-08) |
| Percentage 100 with Supersampling | Same size; pixels unchanged, still one undo step as Scale Image is today |
| Downscale 3 → 2 px (non-integer factor) with Supersampling | Each target pixel weighs the source pixels by the area it covers (1 and ½) |
| Supersampling shrinking width, growing height | Bicubic result (AC-11) |
| Unit switch px → in → px at 96 DPI | Back to the original pixel size within 1 px per edge (AC-04) |
| Resolution 300 DPI, 2 in × 1 in | 600 × 300 px |
| Aspect toggle off, then on again | Height is recomputed from the current width when width is next edited, not at the moment of toggling |
| Auto Crop on a fully transparent document | Nothing to crop: notice, no undo step (AC-15) |
| Auto Crop on a photo whose top-left pixel differs from its neighbors | Only edge rows / columns equal to that pixel are trimmed, usually none: notice (AC-15) |
| Auto Crop with content only on a hidden layer | The composite is uniform: nothing to crop (AC-15) |
| Auto Crop where content touches one edge only | Only the other three edges are trimmed |
| Auto Crop with transparent border pixels of different RGB values | All match, because alpha 0 matches alpha 0 (AC-13) |
| Auto Crop on a locked layer | Crops it too, as Crop to Selection does — a canvas operation, not a pixel edit of that layer |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The resolution field starts at the document's `DocumentMetadata.dpi`, which is 96 for every document until a later spec reads it from files | No codec reads a file's resolution today | Users who print expect the file's resolution until that spec lands |
| Supersampling averages gamma-encoded values, weighted by alpha | Nearest / Bilinear / Bicubic resample encoded values; a fourth filter with different brightness would surprise when switching | A fine black / white pattern downscales to 128 gray, not the linear-light 188 |
| Supersampling is the dialog default; MCP's default stays bilinear | It is paint.net's best-quality choice for shrinking and falls back to Bicubic when enlarging; existing agent calls must keep their results | Users who relied on Bilinear as the dialog default see a different result |
| Auto Crop reads the composite of visible layers | It crops what the user sees, as paint.net crops the flattened image | Content on hidden layers outside the rectangle is cut (Undo restores it) |
| The border color is the composite's top-left pixel | One reference color keeps the result predictable; per-edge colors would trim mixed borders unexpectedly | A frame whose corners differ from its sides is not trimmed |

## Open questions


## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/size-input.spec.ts` "AC-01: every opening starts…"; `ui/e2e/image-resize-autocrop.spec.ts` "both dialogs open fresh with the size input" | built | |
| AC-02 | `ui/e2e/size-input.spec.ts` "AC-02: a percentage scales both edges…"; `ui/e2e/image-resize-autocrop.spec.ts` "a percentage and a print size apply exactly", "anchor › Resize Canvas by percentage places the image at the chosen anchor" | built | |
| AC-03 | `ui/e2e/size-input.spec.ts` "AC-03: absolute sizes in each unit"; `ui/e2e/image-resize-autocrop.spec.ts` "a percentage and a print size apply exactly" | built | |
| AC-04 | `ui/e2e/size-input.spec.ts` "AC-04: a unit change shows the size in the new unit and keeps it"; `ui/e2e/image-resize-autocrop.spec.ts` "each size mode keeps its values and the readout follows the selected mode" (both dialogs) | built | |
| AC-05 | `ui/e2e/size-input.spec.ts` "AC-05: the resolution moves only print sizes"; `ui/e2e/image-resize-autocrop.spec.ts` "a percentage and a print size apply exactly" | built | |
| AC-06 | `ui/e2e/size-input.spec.ts` "AC-06: the aspect lock follows the document ratio…"; `ui/e2e/image-resize-autocrop.spec.ts` "German shows a decimal comma…" | built | |
| AC-07 | `ui/e2e/size-input.spec.ts` "AC-07: invalid used fields block, unused ones do not"; `ui/e2e/image-resize-autocrop.spec.ts` "an invalid used field blocks Apply and Enter" | built | |
| AC-08 | `ui/e2e/size-input.spec.ts` "AC-08: a size beyond the limits is reduced…"; `ui/e2e/image-export.spec.ts` "Resize Canvas and Scale Image reduce an oversized request to the limits" (readout before Apply, then the applied size) | built | |
| AC-09 | `ui/e2e/image-resize-autocrop.spec.ts` "both dialogs open fresh with the size input" | built | |
| AC-10 | `crates/pincel-image-core/src/transform/mod.rs` `scale_supersampling_integer_factor_is_block_mean`, `scale_supersampling_three_to_two_weighs_partial_pixels_by_area`, `scale_supersampling_weights_color_by_alpha`, `scale_supersampling_fully_transparent_area_is_transparent_black`, `scale_supersampling_matches_exact_area_mean` (proptest) | built | |
| AC-11 | `crates/pincel-image-core/src/transform/mod.rs` `scale_supersampling_growing_an_edge_equals_bicubic` | built | |
| AC-12 | `crates/pincel-image-core/src/command/auto_crop.rs` `scale_image_supersampling_is_one_undo_step`, `scale_image_spec_parses_supersampling`; `ui/e2e/image-resize-autocrop.spec.ts` "a percentage and a print size apply exactly" | built | |
| AC-13 | `crates/pincel-image-core/src/command/auto_crop.rs` `auto_crop_rect_trims_a_uniform_border`, `auto_crop_rect_trims_only_the_free_edges`, `auto_crop_rect_matches_transparent_pixels_of_any_rgb`, `auto_crop_rect_reads_the_visible_composite`, `auto_crop_rect_hides_the_content_of_a_hidden_group`, `auto_crop_rect_follows_the_blend_mode`, `auto_crop_crops_every_layer_and_undo_restores`; `ui/e2e/image-resize-autocrop.spec.ts` "Auto Crop trims the border in one undo step" | built | |
| AC-14 | `crates/pincel-image-core/src/command/auto_crop.rs` `auto_crop_crops_every_layer_and_undo_restores`; `ui/e2e/image-resize-autocrop.spec.ts` "Auto Crop trims the border in one undo step" | built | |
| AC-15 | `crates/pincel-image-core/src/command/auto_crop.rs` `auto_crop_rect_uniform_or_framed_is_none`, `auto_crop_with_nothing_to_crop_adds_no_undo_step`; `ui/e2e/image-resize-autocrop.spec.ts` "Auto Crop trims the border in one undo step" | built | |
| AC-16 | `ui/e2e/image-resize-autocrop.spec.ts` "Auto Crop trims the border in one undo step" (runs with a selection) | built | |
| AC-17 | `crates/pincel-agent/src/tests/auto_crop.rs` `image_scale_supersampling_equals_the_editor`; default bilinear: `crates/pincel-agent/src/tests/transform.rs` `resize_and_scale_defaults_follow_each_editor` | built | |
| AC-18 | `crates/pincel-agent/src/tests/auto_crop.rs` `image_auto_crop_equals_the_editor_in_one_undo_step` | built | |
| AC-19 | `crates/pincel-agent/src/tests/auto_crop.rs` `image_auto_crop_with_nothing_to_crop_changes_nothing`, `image_auto_crop_on_a_pixel_document_names_its_mode` | built | |
| AC-20 | `crates/pincel-agent/src/tests/auto_crop.rs` `auto_crop_and_supersampling_are_listed_and_documented`; `crates/pincel-agent/src/tests/doc.rs` `tool_schemas_are_valid` | built | |
| AC-21 | `pnpm check` (`de.ts` is typed against `en.ts`); `ui/e2e/image-resize-autocrop.spec.ts` "German shows a decimal comma and accepts a point" | built | |
| AC-22 | `ui/e2e/size-input.spec.ts` "AC-22: decimals follow the language, both separators are accepted"; `ui/e2e/image-resize-autocrop.spec.ts` "German shows a decimal comma and accepts a point" | built | |

## Decisions

- 2026-11-05: The resolution field is only a conversion factor of the dialog, starting at the document's `DocumentMetadata.dpi` (default 96); it does not change the document, and reading / writing a file's resolution is a later spec — owner's choice, because storing it is a file-format change (PNG pHYs needs a writer that `image` 0.25 lacks) that would push this spec past size L, while the conversion alone already gives paint.net's print-size input.
- 2026-11-05: Supersampling is an area average only when no edge grows; otherwise it gives the Bicubic result — because area averaging has nothing to average when enlarging, and paint.net's best-quality mode switches the same way.
- 2026-11-05: Both dialogs share one size input (percentage, units, resolution, aspect toggle) — because paint.net's Resize and Canvas Size offer the same input, and one block keeps the unit arithmetic in one place.
- 2026-11-05: Auto Crop with nothing to crop is a no-op with a notice — because an empty undo step or a 0 × 0 canvas would be a silent surprise; `image-editing` already treats no-op commands as adding no undo step.
- 2026-11-05 (clarification): The percentage is one field that scales both edges, so the aspect toggle only applies to the absolute size — because paint.net's "By percentage" works that way and two percent fields duplicate the Percent unit of the absolute size.
- 2026-11-05 (clarification): The dialogs open fresh every time (100 %, Pixels, document size) and the unit does not follow View ▸ Units (`image-view-aids`) — because remembering values needs a new pref, and the dialog's unit list includes Percent, which View ▸ Units lacks; following it can be added once both specs have landed.
- 2026-11-05 (clarification): An invalid field blocks Apply only if the selected mode uses it; fractions in Pixels and resolutions above 10000 ppi are invalid — because a field without effect must not block the user, a fractional pixel size cannot be applied without silent rounding, and 10000 ppi is far beyond any print use.
- 2026-11-05 (clarification): A stored resolution that is zero, negative or not finite opens the dialog at 96 — the same fallback as `image-view-aids` AC-21, so both show the same physical sizes.
- 2026-11-05 (clarification): Resolution only in pixels per inch, no pixels-per-centimeter choice — because the owner asked for a DPI field, centimeters still convert from it (AC-03), and `image-view-aids` uses the same DPI value for its units.
- 2026-11-05 (clarification): The selection does not limit Auto Crop — because Crop to Selection already crops to a selection, and paint.net's Auto Crop also examines the whole image.
- 2026-11-05 (clarification): Auto Crop's composite is the one the canvas shows (groups, opacity, blend modes) — because the crop should match what the user sees.
- 2026-11-05 (clarification): Size fields use the UI language's decimal separator and accept both — because German users type a comma, and pasted values often use a point.
- 2026-11-05: Menu labels stay "Resize Canvas…" / "Scale Image…" — because they are shared with Pixel mode and referenced by tests and other specs (Out of scope).
- 2026-11-06 (spec-build): A unit change keeps the unrounded value behind the two-decimal text, so a px → in → px round trip stays within 1 px at any resolution (AC-04); the text is what the user sees and edits.
- 2026-11-06 (spec-build): Auto Crop's "nothing to crop" check runs before the command reaches the bus (`CommandSpec::AutoCrop` dispatch, `auto_crop_rect`), so it adds no undo step; the UI shows the notice when the canvas size did not change, which a real crop always changes.
- 2026-11-06 (spec-build): The canvas limits (`MAX_CANVAS_DIM`, `MAX_CANVAS_AREA`, `clampCanvasSize`) moved to the rune-free `ui/src/modes/image/canvas-limits.ts` (re-exported by the editor store) so the node-side size-input spec can import them.
- 2026-11-06 (spec-build): The read-only wasm getter `document_dpi` (planned by `image-view-aids`) is added in this spec's series, because AC-01 needs it first; `image-view-aids` reuses it.
- 2026-11-06 (spec-build): `Modal` gained `applyDisabled`, which disables Apply and makes Enter apply nothing (AC-07).
- 2026-11-06 (spec-build): The size-input arithmetic module and its node-side spec were written together, so that spec had no separate red run; the dialog e2e spec was red before the dialogs changed.
- 2026-11-06 (spec-build): Audit round. New tests for clauses no test pinned: AC-04's "each mode keeps the values it last held and the readout follows the selected mode" (both dialogs), AC-08's readout of the reduced size before Apply, AC-13's hidden groups and blend modes (Rust), and AC-02's "at the chosen anchor" for Resize Canvas (Copy Merged read test-side). All were green at once and shown red against temporary breaks, reverted afterwards: "By percentage" resetting its field to 100 when selected (AC-04), the readout showing the size before the limits (AC-08), `auto_crop_rect` composing hidden groups as visible and, separately, every layer as Normal (AC-13), and Resize Canvas always applying the center anchor (AC-02).
