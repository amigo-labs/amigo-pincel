---
status: todo
created: 2026-11-05
size: L
mode: both
design: [mcp.md, pincel.md §5.4, pincel.md §6, pincel.md §13.1, pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-structure]
---

# MCP parity: selection and transform

## Context

`mcp-headless-pixel` and `mcp-headless-image` give agents a headless `pincel-mcp` server. Their selection and transform tools cover only part of what the editors offer:

- Pixel documents have only `selection.set_rect` and `selection.clear`. The Pixel editor also selects ellipses, lassos / polygons and magic-wand areas, combines shapes with add / subtract / intersect, and offers Select All, Invert, Expand and Contract.
- Image documents have `select.rect`, `select.ellipse`, `select.wand` and `select.clear`. The Image editor also has the lasso / polygon select, Select All, Invert, Expand, Contract and Feather.
- No tool reads the selection's shape; `doc.info` reports only its bounding box (`mcp-parity-structure` AC-24).
- `draw.clear` is Pixel-only and clears a rectangle; it ignores the selection's shape. Both editors' Delete clears the selected pixels.
- `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip` and `image.rotate` are Image-only, although the Pixel editor has all five (Image menu, Canvas Size, Scale Image).
- No tool flips or rotates a single layer, although both editors do.

This spec closes these gaps. It is the second of five `mcp-parity-*` specs and is split in two parts. This part covers selections, Delete and transforms. `mcp-parity-selection-transform-content` covers moving content and the clipboard (`layer.import`, selection export). Every capability exists already in `pincel-core` / `pincel-image-core` and is used by the editor. The tools expose these capabilities; they add no new editing behavior.

Decision Log 2026-11-05 "MCP tool parity" sets the naming rule. A capability gets one tool name. The tool works on every mode whose editor offers the capability, and it fails with the mode error of `mcp-headless-image` AC-05 elsewhere. No aliases are kept. **Renamed / removed by this spec:**

| Before | After |
| ------ | ----- |
| `selection.set_rect` (Pixel) | `select.rect` (both modes); removed without alias |
| `selection.clear` (Pixel) | `select.clear` (both modes); removed without alias |
| `select.rect`, `select.ellipse`, `select.wand`, `select.clear` (Image-only) | Same names, now on both modes |
| `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip`, `image.rotate` (Image-only) | Same names, now on both modes |
| — | New: `select.polygon`, `select.all`, `select.invert`, `select.expand`, `select.contract`, `select.feather`, `select.get`, `select.delete`, `layer.flip`, `layer.rotate` |

`draw.clear` stays as it is (Pixel-only, rectangle). This spec supersedes the `selection.*` wording of `mcp-headless-pixel` AC-25 and AC-43 and the `selection.*` entry of `mcp-headless-image` AC-05.

**"The editor" in this spec** means the result the editor's UI produces for the same action in a document of that mode. That is the result of the core command the UI issues for it, as in `mcp-parity-structure`. A test compares the tool's result with that command applied directly to an equal document, and also checks the concrete values this spec states.

**Conventions** inherited from `mcp-headless-pixel` and `mcp-parity-structure`: coordinates are canvas pixels with the origin top-left, layers are addressed by the ids `layer.list` reports, frames by 0-based index, and `layer` / `frame` default to the active layer and frame (`mcp-headless-pixel` AC-43). "The selection's bounds" is the bounding box `{x, y, w, h}` in canvas pixels that `doc.info` reports for the selection (`mcp-parity-structure` AC-24, which changes Image documents' current `width` / `height` keys to `w` / `h`). Selection changes are not undo steps in Pixel documents. In Image documents they are, as in each editor.

## Scope

### In scope

- `select.rect`, `select.ellipse`, `select.polygon`, `select.wand` with the combine modes `replace`, `add`, `subtract` and `intersect` on both modes, plus `feather` on Image documents
- `select.all`, `select.clear`, `select.invert`, `select.expand`, `select.contract` on both modes; `select.feather` on Image documents
- `select.get`: the selection's bounds, the selected-pixel count and a coverage grid of up to 64×64
- `select.delete`: the editor's Delete on the target layer
- `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip`, `image.rotate` on Pixel documents
- `layer.flip` and `layer.rotate` on both modes
- Removal of `selection.set_rect` and `selection.clear`
- `pincel://guide` documents the tools and the per-mode differences

### Out of scope

- Moving layer content, moving selected pixels, moving the selection outline, `layer.import` and selection export — because `mcp-parity-selection-transform-content` covers them
- Feather in Pixel documents, and rounded-rectangle or regular-polygon selections — because no editor offers them as selection tools. The core builders `SelectionMask::rounded_rect` / `regular_polygon` have no tool in the Pixel UI, and parity means reaching what the editor can do
- Free rotation by an arbitrary angle, skew, perspective, and RotSprite rotation of a selection — because the transforms here are the editors' menu commands. RotSprite has its own spec (`rotsprite`), and the selection transform of its UI is not an MCP target yet
- Selections stored in the file or kept across `doc.close` — because both cores keep the selection as editor state outside the file format
- Fill Selection (`select.fill`, the sibling of `select.delete`) — because `image-edit-extras` adds it
- Changing `draw.clear` — because it is an existing tool with its own semantics (a rectangle, Pixel-only); `select.delete` is the editor's Delete
- View state (marching ants, the selection overlay) — because it does not change the document (Decision Log 2026-11-05)

## Acceptance criteria

### Tool list and naming

- [ ] AC-01 (always): `tools/list` contains `select.rect`, `select.ellipse`, `select.polygon`, `select.wand`, `select.all`, `select.clear`, `select.invert`, `select.expand`, `select.contract`, `select.feather`, `select.get`, `select.delete`, `layer.flip` and `layer.rotate`, each meeting `mcp-headless-pixel` AC-02, and contains neither `selection.set_rect` nor `selection.clear`.
- [ ] AC-02 (error): If `tools/call` names `selection.set_rect` or `selection.clear`, then the server answers as for any unknown tool (`mcp-headless-pixel` AC-37).

### Selecting

- [ ] AC-03 (event): When the client calls `select.rect` or `select.ellipse` with `x`, `y`, `width`, `height` and a `mode` (`replace` by default, `add`, `subtract` or `intersect`) on a document of either mode, then the document's selection equals the one the editor's Rectangle or Ellipse Select makes from the same rectangle and mode.
- [ ] AC-04 (event): When the client calls `select.polygon` with `points` (at least three `[x, y]` pairs, the polygon implicitly closed) and a `mode`, then the document's selection equals the one the editor's Lasso / Polygonal Lasso makes from the same points and mode.
- [ ] AC-05 (event): When the client calls `select.wand` with a seed `x`, `y`, `tolerance`, `contiguous`, `sample` (`current_layer`, the target layer at the target frame, or `all_layers`, the composed frame) and a `mode`, then the document's selection equals the one the editor's Magic Wand makes with the same settings. Omitted settings take the defaults of the document's editor: `tolerance` 0 in Pixel documents and 32 in Image documents, `contiguous: true`, `sample: current_layer`, `mode: replace`.
- [ ] AC-06 (event): When a `subtract` or `intersect` shape is applied while no selection exists, then the result follows each editor: in a Pixel document no selection exists afterwards; in an Image document, where no selection counts as everything selected, `subtract` selects the canvas minus the shape and `intersect` selects the shape. In both modes `add` without a selection selects the shape.
- [ ] AC-07 (option): Where `select.rect`, `select.ellipse` or `select.polygon` on an Image document gets `feather` (0–250 px, default 0), the selection's coverage equals the editor's for that feather radius.
- [ ] AC-08 (event): When the client calls `select.all`, `select.invert`, `select.expand` (`radius`) or `select.contract` (`radius`), then the selection equals the editor's after Select All, Invert Selection, Expand or Contract by that radius. `select.invert` without a selection selects nothing in an Image document and the whole canvas in a Pixel document, as each editor does. `select.expand` / `select.contract` without a selection succeed and change nothing.
- [ ] AC-09 (event): When the client calls `select.feather` with `radius` on an Image document, then the selection's coverage equals the editor's after Feather Selection by that radius.
- [ ] AC-10 (event): When the client calls `select.clear`, then no selection exists afterwards, and `draw.fill`, `adjust.apply` and `effect.apply` act on the whole target layer again; `select.delete` acts as without a selection (AC-22): it clears the whole target layer in an Image document and changes nothing in a Pixel document.
- [ ] AC-11 (always): Every successful call of a selection tool (`select.rect`, `select.ellipse`, `select.polygon`, `select.wand`, `select.all`, `select.clear`, `select.invert`, `select.expand`, `select.contract`, `select.feather`) returns the selection's bounds, or `null` when nothing is selected, and the selected-pixel count. In an Image document, such a call that changes the selection's coverage is exactly one undo step, and one that leaves it unchanged (for example `select.clear` without a selection, or a second `select.all`) adds none. In a Pixel document no selection tool adds an undo step, and `doc.undo` does not change the selection a selection tool set. Pixel edits that move the selection with the pixels (`select.move_content` in `mcp-parity-selection-transform-content`) are undo steps of their own.
- [ ] AC-12 (state): While a shaped selection (ellipse, polygon, wand, or a combination) exists, `draw.fill`, `adjust.apply` and `effect.apply` change no pixel outside the shape in either mode, not only no pixel outside its bounding box.

### Selection errors

- [ ] AC-13 (error): If `select.feather` is called on a Pixel document, or `select.rect` / `select.ellipse` / `select.polygon` gets a `feather` above 0 on a Pixel document, then the call fails with `isError: true`, the message names `feather` and the document's mode, and the selection is unchanged.
- [ ] AC-14 (error): If `select.polygon` gets fewer than three points, a point that is not a pair of numbers, or (on a Pixel document) a non-integer coordinate, then the call fails with `isError: true` naming `points` and the offending entry, and the selection is unchanged.
- [ ] AC-15 (error): If the `select.wand` seed lies outside the canvas, then the call fails with `isError: true` naming the seed and the canvas size, and the selection is unchanged.
- [ ] AC-16 (error): If a `mode`, `sample`, `direction`, `anchor` or `interpolation` value is not one of the listed values, `angle` is not 90, 180 or 270, `tolerance` is outside 0–255, `radius` is outside 1–250, `feather` is outside 0–250, `width` / `height` is below 1, or `x`, `y`, `width` or `height` of `select.rect` / `select.ellipse` (or the `select.wand` seed in a Pixel document) is not an integer, then the call fails with `isError: true` naming the argument and its allowed values or range, and the document is unchanged.

### Reading the selection

- [ ] AC-17 (event): When the client calls `select.get` with a `region` (`{x, y, width, height}`, the argument form of `doc.read_pixels`) of at most 64×64 inside the canvas, then the result carries the selection's bounds (or `null`), the selected-pixel count over the whole canvas, the `region` read, and `rows`: one text row per pixel row of the region, as in `doc.read_pixels`. Each row holds one two-digit lowercase hex coverage value per pixel, separated by single spaces: `00` unselected and `ff` selected, and in an Image document every value 00–ff the coverage has.
- [ ] AC-18 (option): Where `select.get` gets no `region`, the region is the selection's bounds. Without a selection, the result carries `null` bounds, a count of 0 and no rows.
- [ ] AC-19 (error): If the `select.get` region (given, or the bounds when omitted) is larger than 64×64 in either dimension, or lies wholly or partly outside the canvas (it is not clipped), then the call fails with `isError: true`, and the message states the 64×64 limit (suggesting `region`) or the canvas size.
- [ ] AC-20 (always): `select.get` never changes the document, the selection or the undo history.

### Delete

- [ ] AC-21 (event): When the client calls `select.delete` while a selection exists, then the target layer's pixels at the target frame equal the editor's after Delete. Every selected pixel becomes transparent; in an Image document, a feathered pixel's alpha is scaled down by its coverage. No pixel outside the selection changes, the selection stays, and the result is `{doc, layer, changed}`. The call is one undo step when a pixel changed. When every selected pixel was already transparent, it reports `changed: false` and adds no undo step (AC-36).
- [ ] AC-22 (state): While no selection exists, `select.delete` acts as each editor's Delete does: in an Image document it clears the whole target layer (one undo step when a pixel changed, AC-36); in a Pixel document it changes nothing, adds no undo step and reports `changed: false`.
- [ ] AC-23 (error): If `select.delete` targets a locked layer (or a layer in a locked group), a group, or a tilemap layer, then the call fails with `isError: true` naming the layer and the reason, and the document is unchanged.

### Canvas and image transforms on Pixel documents

- [ ] AC-24 (event): When the client calls `image.flip` (`direction`: `horizontal` or `vertical`) or `image.rotate` (`angle`: 90, 180 or 270 degrees clockwise) on a Pixel document, with the arguments these tools take on Image documents today, then its size, every layer's cels in every frame, and its slices equal the editor's after the matching Flip Canvas / Rotate Canvas command; 90 and 270 swap width and height.
- [ ] AC-25 (event): When the client calls `canvas.resize` (`width`, `height`, `anchor`) or `image.scale` (`width`, `height`, `interpolation`: `nearest`, `bilinear` or `bicubic`) on a Pixel document, then its size and pixels in every frame equal the editor's after Canvas Size or Scale Image with the same values.
- [ ] AC-26 (option): Where `anchor` or `interpolation` is omitted, the default of the document's editor applies: `center` and `nearest` in Pixel documents, `top_left` and `bilinear` in Image documents (unchanged from `mcp-headless-image`). `anchor` takes the nine values `canvas.resize` accepts today (`top_left` … `bottom_right`) in both modes.
- [ ] AC-27 (event): When the client calls `image.crop_to_selection` on a Pixel document with a selection, then the canvas equals the editor's after Crop to Selection, which crops to the selection's bounds clipped to the canvas.
- [ ] AC-28 (error): If `image.crop_to_selection` is called on a Pixel document without a selection, then it fails as in `mcp-headless-image` AC-14, and the document is unchanged.
- [ ] AC-29 (error): If `canvas.resize` or `image.scale` on a Pixel document gets a `width` or `height` outside 1–4096, then the call fails with `isError: true` and the message "width must be 1–4096" (or "height must be 1–4096"), and the document is unchanged.
- [ ] AC-30 (error): If `image.flip`, `image.rotate` or `image.scale` is called on a Pixel document that has a tilemap layer, then the call fails with `isError: true` saying that tilemap layers cannot be transformed, and the document is unchanged.

### Layer transforms

- [ ] AC-31 (event): When the client calls `layer.flip` (`direction`: `horizontal` or `vertical`) or `layer.rotate` (`angle`: 90, 180 or 270 degrees clockwise) on a layer, then the document's pixels equal the editor's after the matching Flip / Rotate Layer command on that layer, and the canvas size is unchanged. In a Pixel document this applies to the target frame's cel. In an Image document a group's whole subtree turns, as in the editor. The result is `{doc, layer, changed}`.
- [ ] AC-32 (state): While a selection exists in a Pixel document, `layer.flip` and `layer.rotate` turn only the pixels inside the selection's bounds, about the centre of those bounds, as the Pixel editor does. In an Image document they turn the whole layer whatever is selected, as the Image editor does.
- [ ] AC-33 (error): If `layer.flip` or `layer.rotate` targets a locked layer, a layer in a locked group, a group containing a locked layer (Image), or a group or tilemap layer (Pixel), then the call fails with `isError: true` naming the layer and the reason, and the document is unchanged.

### Shared rules

- [ ] AC-34 (error): If a tool of this spec names a `layer` or `frame` that does not exist, then the call fails with `isError: true` naming the argument and its value, and the document is unchanged.
- [ ] AC-35 (error): If the core command the editor issues for a tool of this spec refuses the call (for example a cel color mode the command does not support), then the call fails with `isError: true` carrying the command's reason, and the document is unchanged.
- [ ] AC-36 (always): Every successful call of `select.delete`, `layer.flip`, `layer.rotate`, `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip` or `image.rotate` that changes the document is exactly one undo step (`mcp-headless-pixel` AC-09); one that changes nothing adds no undo step. `canvas.resize` and the `image.*` tools report `{doc, width, height, changed}` in both modes, as on Image documents today.
- [ ] AC-37 (always): The `pincel://guide` resource names every tool of this spec and the modes it works in, and states the per-mode differences: selection undo, `feather` and `select.feather` only in Image mode, the per-mode defaults of AC-05 and AC-26, and Delete and layer transforms with and without a selection.
- [ ] AC-38 (event): When `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip` or `image.rotate` succeeds on a Pixel document, then no selection exists afterwards, and one `doc.undo` restores the selection the document had before the call, together with its size and pixels, as the Pixel editor's commands do.

## Technical notes

Every capability exists already. Grep for these names, because line numbers drift.

| Tool | Pixel (`pincel-core`, used by `pincel-wasm`) | Image (`pincel-image-core` `CommandSpec`) |
| ---- | -------------------------------------------- | ----------------------------------------- |
| `select.rect` / `.ellipse` / `.polygon` | `SelectionMask::{rect, ellipse, polygon}` + wasm `combine_selection` (`selection_ops.rs`) | `SelectRectangle`, `SelectEllipse`, `SelectPolygon` (`feather`) |
| `select.wand` | `SelectionMask::wand` over wasm `active_layer_canvas` / `composite_buffer` (`sample` `layer` / `composite`) | `SelectWand` (`sample` `current_layer` / `all_layers`) |
| `select.all` / `.clear` / `.invert` | wasm `select_all`, `clear_selection`, `invert_selection` | `SelectAll`, `Deselect`, `InvertSelection` |
| `select.expand` / `.contract` / `.feather` | `SelectionMask::{expand, contract}`; no feather | `ExpandSelection`, `ContractSelection`, `FeatherSelection` |
| `select.get` | `Sprite::selection_as_mask` (0 / 255) | `Document::selection` coverage (0–255) |
| `select.delete` | wasm `delete_selection`: `ClearRegion::new(..).masked()` on the selection rect; no-op without selection | `DeleteSelection` (`delete_selection`, coverage-scaled; whole layer without selection) |
| `image.flip` / `image.rotate` | `TransformCanvas` (`Orientation`), rejects tilemap layers | `FlipCanvas`, `RotateCanvas` |
| `canvas.resize` | `ReframeCanvas::resize` (`Anchor`: `top`, `left`, `right`, `bottom` for the edge centres) | `ResizeCanvas` |
| `image.scale` | `ScaleImage` (`Interpolation`), rejects tilemap layers | `ScaleImage` |
| `image.crop_to_selection` | `ReframeCanvas::new(selection ∩ canvas)` | `CropToSelection` |
| `layer.flip` / `layer.rotate` | `TransformCel::new(..).within(selection rect)` (`flip_horizontal`, `flip_vertical`, `rotate_90_cw`, `rotate_90_ccw`, `rotate_180`) | `TransformLayer` (`flip_h`, `flip_v`, `rotate_180`), `RotateLayer90 { ccw }` |

The Pixel combine rule (`combine_selection`) and the wand sample buffers live in `pincel-wasm`'s `Document` today, so a native caller cannot reach them. Moving them into `pincel-core` next to `SelectionMask` follows the "phase 0" precedent of `mcp-headless-pixel`, where `Sprite::paint_target_layer` moved. `pincel-wasm` keeps thin wrappers and its tests stay green, as pure `refactor(...)` commits. The new `pincel-core` public API is recorded in `pincel.md` §15 when it is built, as the 2026-10-29 build rows of `mcp-headless-pixel` were.

The tool's `canvas.resize` anchor names map onto the Pixel `Anchor` names (`top_center` → `top`, `center_left` → `left`, `center_right` → `right`, `bottom_center` → `bottom`). `image.rotate` 90 / 180 / 270 maps onto `rotate_90_cw` / `rotate_180` / `rotate_90_ccw`. `layer.rotate` 270 maps onto Pixel `rotate_90_ccw` and Image `RotateLayer90 { ccw: true }`.

`SelectionMask::{expand, contract}` in `pincel-core` dilate / erode by a square in two separable passes, O(w·h) whatever the radius (since 2026-11-06; before, a window scan per pixel cost O(w·h·r²)). See Decisions.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `select.rect` partly outside the canvas | Clipped to the canvas as in the editor; bounds lie within the canvas |
| `select.rect` wholly outside the canvas with `replace` | No selection afterwards, as in the editor; result bounds `null`, count 0 |
| `select.ellipse` 4×4 on a 4×4 Pixel canvas | 12 pixels selected (the four corners stay out), as the editor's Ellipse Select |
| `select.rect` `add` that completes a rectangle | `select.get` rows show the full rectangle, with no hole |
| `select.wand` on a uniform layer | The whole canvas is selected |
| `select.wand` with a transparent seed | The transparent area around the seed is selected, as in the editor |
| `select.contract` larger than the selection | No selection afterwards; result bounds `null` |
| `select.invert` of a full-canvas selection | No selection afterwards |
| `select.expand` with a `radius` of at least the canvas's longer side (e.g. 50 on a 10×10 canvas) while any pixel is selected | The whole canvas is selected in both modes; `select.contract` with such a radius leaves no selection |
| `select.get` of a region with no selected pixel | All values `00` |
| `select.delete` in a Pixel document with a hole in the selection | Pixels in the hole keep their color |
| `image.rotate` 90 on a 32×16 Pixel document | 16×32 afterwards; one `doc.undo` restores 32×16 and the pixels |
| `layer.rotate` 90 of a whole 32×16 layer | Canvas stays 32×16; content that no longer fits is cut off as in each editor |
| `canvas.resize` to the current size | Succeeds; document unchanged, no undo step |
| `image.flip` on a Pixel document with a selection | Pixels flipped, selection cleared; one `doc.undo` restores pixels and selection (AC-38) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `select.wand` keeps the Image tool's `sample` values (`current_layer`, `all_layers`) for both modes and maps them onto the Pixel editor's `layer` / `composite` | One tool, one vocabulary; existing Image clients keep working | Low |
| `select.polygon` takes integer points on Pixel documents and accepts fractional points on Image documents | Pixel sprite coordinates are integers in the Pixel core; the Image core takes `f32` points | Low |
| Fewer than three polygon points and an off-canvas wand seed are errors, not the editor's silent no-op or clear | The editor reaches these states only through stray pointer gestures; for an agent they are mistakes it should see | Low |
| `radius` for expand, contract and feather is 1–250 px in both modes | 250 is the Image editor's feather slider maximum, and no editor offers a larger radius | Low: measured 2026-11-06, a 4096×4096 Pixel document at radius 250 expands or contracts in about 0.2 s (release) after the separable rewrite (Decisions) |
| Selection changes in Pixel documents set no dirty flag | The selection is not part of the Pixel file and not an undo step (`mcp-headless-pixel` build decision 2026-10-29) | Low |
| The error cases listed here are all the tools must check; where the core command does not check one (lock checks live in `pincel-wasm`), the tool checks it | As in `mcp-parity-structure` | Medium |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built  |        |
| AC-02 | `selection_set_rect_and_selection_clear_are_unknown_tools` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-03 | `select_shapes_combine_like_the_editor_in_both_modes` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-04 | `select_shapes_combine_like_the_editor_in_both_modes` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-05 | `select_wand_matches_the_editor_with_each_modes_defaults` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-06 | `pixel_subtract_or_intersect_without_selection_selects_nothing`, `image_subtract_or_intersect_without_selection_follows_the_editor` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-07 | `image_feather_matches_the_editors_coverage` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-08 | `select_all_invert_expand_contract_match_the_editor` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-09 | `select_feather_matches_the_editor_on_image_documents` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-10 | `select_clear_lets_edits_act_on_the_whole_layer` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-11 | `selection_undo_follows_each_editor` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-12 | `shaped_selections_limit_fill_adjust_and_effect_to_the_shape` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-13 | `feather_on_a_pixel_document_fails_naming_feather_and_the_mode` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-14 | `select_polygon_with_bad_points_fails_naming_the_entry` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-15 | `select_wand_seed_outside_the_canvas_fails_naming_seed_and_canvas` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-16 | `bad_selection_arguments_fail_naming_the_argument` (`crates/pincel-agent/src/tests/select.rs`), `bad_transform_arguments_fail_naming_the_argument` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-17 | `select_get_reports_bounds_count_and_coverage_rows` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-18 | `select_get_without_selection_returns_null_bounds_and_no_rows` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-19 | `select_get_refuses_large_or_off_canvas_regions` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-20 | `select_get_changes_nothing` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-21 | `select_delete_clears_the_selected_pixels_like_the_editor` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-22 | `select_delete_without_selection_follows_each_editor` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-23 | `select_delete_on_a_locked_group_or_tilemap_layer_fails_with_the_reason` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-24 | `pixel_flip_and_rotate_equal_the_editors_canvas_commands` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-25 | `pixel_resize_and_scale_equal_the_editors_commands` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-26 | `resize_and_scale_defaults_follow_each_editor` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-27 | `pixel_crop_to_selection_crops_to_the_clipped_bounds` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-28 | `pixel_crop_without_selection_fails_and_changes_nothing` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-29 | `pixel_sizes_outside_1_to_4096_fail` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-30 | `pixel_transforms_refuse_tilemap_documents` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-31 | `layer_flip_and_rotate_equal_the_editors_layer_commands` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-32 | `layer_transforms_follow_each_editors_selection_rule` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-33 | `layer_transforms_refuse_locked_group_and_tilemap_targets`, `layer_transforms_refuse_a_layer_inside_a_locked_pixel_group` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-34 | `selection_tools_naming_a_missing_layer_or_frame_fail_with_argument_and_value` (`crates/pincel-agent/src/tests/select.rs`), `layer_transforms_naming_a_missing_layer_or_frame_fail` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-35 | `a_refusing_core_command_fails_with_its_reason` (`crates/pincel-agent/src/tests/select.rs`) | built  |        |
| AC-36 | `transforms_record_one_undo_step_and_none_without_a_change` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |
| AC-37 | `read_resource_guide_covers_workflow_and_both_modes`, `guide_names_the_modes_of_the_selection_and_transform_tools` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-38 | `pixel_canvas_transforms_clear_the_selection_and_undo_restores_it` (`crates/pincel-agent/src/tests/transform.rs`) | built  |        |

## Decisions

- 2026-11-05: The topic is split into this spec (selection, Delete, transforms) and `mcp-parity-selection-transform-content` (moving content, clipboard), because together they exceed size L (AGENTS.md §3.2).
- 2026-11-05: Selections follow each editor's undo model: undo steps in Image documents, transient state in Pixel documents (AC-11). Unifying them would change one core's selection model, which is architecture beyond parity.
- 2026-11-05: Delete is the new tool `select.delete`, and `draw.clear` keeps its rectangle semantics. Delete acts on the selection, so it belongs with `select.*`; changing `draw.clear` would break its existing clients.
- 2026-11-05: Mode-only options fail with the mode error on the mode that lacks them (`feather` and `select.feather` on Pixel documents, AC-13), per Decision Log 2026-11-05 "MCP tool parity". They are not silently ignored, because a silently unfeathered selection would surprise the agent.
- 2026-11-05: Layer transforms follow each editor's selection rule (AC-32): Pixel turns the selection's bounds, Image the whole layer. The tools reproduce the editor and do not unify behavior that the cores implement differently.
- 2026-11-05: `canvas.resize`, `image.scale` and `select.wand` default to each editor's values: Pixel `center` / `nearest` / tolerance 0, Image `top_left` / `bilinear` / 32 (AC-05, AC-26). Owner's choice, because an agent should get what a user of that mode gets, and `bilinear` would blur pixel art.
- 2026-11-05: Selection bounds use the `{x, y, w, h}` shape of `mcp-parity-structure` AC-24 in every `select.*` result and in `select.get`. Owner's choice, because one bounds shape across `doc.info` and the selection tools keeps clients simple.
- 2026-11-05: Calls that change nothing add no undo step (AC-11, AC-21, AC-36). This deviates from the editors: the Image command bus records every selection command, even `Deselect` without a selection, and Pixel Delete records a step when nothing was opaque. The tool compares before and after and skips the step, because `mcp-parity-structure` AC-25 sets this rule for every MCP tool, and an empty step would make one `doc.undo` appear to do nothing.
- 2026-11-05: `select.get` rejects a region that lies partly outside the canvas instead of clipping it (AC-19), because `doc.read_pixels` does the same (`mcp-headless-pixel` AC-14), so both region reads follow one rule.
- 2026-11-05: `select.rect` / `select.ellipse` take integer coordinates in both modes, and the wand seed is an integer in Pixel documents (AC-16), because the cores take integers there (`SelectRectangle` `i32`, Pixel `select_wand` `i32`). Only the Image wand seed and Image polygon points are fractional.
- 2026-11-05: `select.get` returns a hex coverage grid capped at 64×64, like `doc.read_pixels` (AC-17 – AC-19). One limit and one text form for all region reads, and Image coverage needs more than one bit.
- 2026-11-05 (spec-build): Image selection tools predict the new selection with the core's own builders (`SelectionMask::{rectangle, ellipse, polygon}`, `magic_wand`, `apply_mode`, the modifiers) and run the editor's `CommandSpec` only when the prediction differs from the current selection, because the Image command bus records every selection command and offers no way to drop a recorded step (AC-11).
- 2026-11-05 (spec-build): Image canvas and layer transforms decide "no change" (AC-36) by a dry run of the same `CommandSpec` on a copy of the document; Pixel tools use the dry run of `mcp-parity-structure` (`Doc::commit_if_changed`).
- 2026-11-05 (spec-build): A Pixel `layer.flip` / `layer.rotate` on a layer without a cel at the target frame reports `changed: false`, because an empty cel turns into itself; the editor creates an empty cel first (`ensure_paint_cel`), which changes nothing visible.
- 2026-11-05 (spec-build): Pixel canvas sizes are checked as integers (1–4096) by the tool, not by the schema's `minimum`, so every out-of-range value gets AC-29's message "width must be 1–4096"; Image sizes keep `mcp-headless-image`'s messages.
- 2026-11-05: Owner decision on AC-06 (open question of the 2026-11-05 spec-build): the tools follow each editor. Pixel documents keep "no selection afterwards" for `subtract` / `intersect` without a selection; Image documents follow `pincel-image-core` `apply_mode`, where no selection counts as everything selected (`subtract` selects the canvas minus the shape, `intersect` the shape); `add` selects the shape in both. AC-06 reworded per mode (owner-approved reword); neither core's rule changes.
- 2026-11-06 (spec-build): The Image half of AC-06 (`image_subtract_or_intersect_without_selection_follows_the_editor`) was green at once, because the tools already predict with the Image core's `apply_mode`; no red run was possible. Counter-check: with `apply_mode` temporarily starting Subtract / Intersect from an empty mask instead of a full one, the test failed at `subtract` (bounds `null` instead of the whole canvas); reverted.
- 2026-11-06 (spec-build): The Assumptions risk was measured with a release-build timing probe on a 4096×4096 Pixel mask with a centred 2048×2048 rectangle at radius 250. The window scan of `SelectionMask::{expand, contract}` did not finish within 120 s; at 512×512 its `expand` alone took 29.6 s (256×256: 5.4 s), so 4096×4096 (64 times the pixels) extrapolates to about half an hour. Fixed at the cause in `pincel-core`: the square is separable, so `morph` now runs a row pass over prefix counts and a column pass with running column sums, O(w·h) whatever the radius — 0.20 s for `expand` and 0.19 s for `contract` at 4096×4096, radius 250 (release). `expand_and_contract_equal_the_window_scan` pins the output to the old window scan (kept in the test module as the oracle) over 540 cases: four fixed masks (empty, full, one pixel, an ellipse) and five noise densities on six canvases from 1×1 to 24×17, at ten radii from 0 to beyond the canvas. It was green on the old code by construction (the oracle is that code); counter-check: dropping the top-edge clip from the new contract made it fail (`contract 13x9 r1`); reverted. No public API changed. The only difference from the old code is a radius above `i32::MAX`, where the old cast wrapped negative; the tools limit the radius to 1–250.
- 2026-11-05 (spec-build): New public `pincel-core` API: `Sprite::combine_selection`, `selection::{layer_canvas, composite_canvas}` (Decision Log row 2026-11-05 in `pincel.md` §15).
- 2026-11-06: Owner decision on the contradiction between AC-10 and AC-22 found at acceptance: AC-22 and the code are right, so in a Pixel document `select.delete` after `select.clear` changes nothing (the Pixel editor's Delete without a selection), while in an Image document it clears the whole layer. AC-10 reworded to defer to AC-22; `draw.fill`, `adjust.apply` and `effect.apply` still act on the whole layer in both modes.
- 2026-11-06 (spec-build): Acceptance found test gaps in AC-10, AC-12, AC-16, AC-23, AC-33, AC-34 and AC-37; the behavior existed, so the added checks were green at once. Counter-checks, all reverted: Pixel `select.delete` without a selection clearing the whole layer made `select_clear_lets_edits_act_on_the_whole_layer` fail (AC-10; it now also compares `adjust.apply` and `effect.apply` after `select.clear` with the same edits on a document that never had a selection); Pixel `effect.apply` / `adjust.apply` ignoring the selection's shape made `shaped_selections_limit_fill_adjust_and_effect_to_the_shape` fail (AC-12, which now also covers Pixel `adjust.apply` in an ellipse and Image `draw.fill` in a polygon); argument messages without the allowed values or range made `bad_selection_arguments_fail_naming_the_argument` and `bad_transform_arguments_fail_naming_the_argument` fail (AC-16); a Pixel lock check that ignores the group's lock made `select_delete_on_a_locked_group_or_tilemap_layer_fails_with_the_reason`, `layer_transforms_refuse_locked_group_and_tilemap_targets` and `layer_transforms_refuse_a_layer_inside_a_locked_pixel_group` fail, and an Image lock message without the layer id failed the Image cases (AC-23, AC-33); clearing the selection or the slices before the layer / frame is resolved made `selection_tools_naming_a_missing_layer_or_frame_fail_with_argument_and_value` and `layer_transforms_naming_a_missing_layer_or_frame_fail` fail (AC-34); renaming the guide lines "Selecting, both modes:" and "Transforms, both modes:" made `guide_names_the_modes_of_the_selection_and_transform_tools` fail (AC-37).
