---
status: todo
created: 2026-11-05
size: M
mode: both
design: [mcp.md, pincel.md §5.2, pincel.md §5.4, pincel.md §6, pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-selection-transform]
---

# MCP parity: moving content and the clipboard

## Context

Second part of `mcp-parity-selection-transform`, split off because the topic exceeds size L. The first part gives both modes the `select.*` tools, Delete and the transforms. This part covers what the editors do with the Move tools and the clipboard. No MCP tool reaches any of it today:

- **Move**: the Image editor moves a layer's content (Move tool without a selection), the selected pixels (Move Selected Pixels) and the selection outline alone (Move Selection). The Pixel editor moves the selected pixels (Move tool with a selection).
- **Clipboard**: both editors copy the selection (Copy, or Copy Merged for the composite) as a PNG, cut it, and paste a PNG as a new layer. The Pixel editor also imports a PNG file as a layer. A headless agent has no system clipboard. Its equivalents are a PNG file in the workspace: "copy" writes one, "paste" reads one.

Decision Log 2026-11-05 "MCP tool parity" sets the naming rule: one tool name per capability, working on every mode whose editor offers it, and the mode error of `mcp-headless-image` AC-05 elsewhere. "The editor", the conventions (coordinates, layer ids, frame indexes, the `layer` / `frame` defaults) and "the selection's bounds" mean the same as in `mcp-parity-selection-transform`.

New tools: `layer.translate`, `select.move_content`, `select.move`, `layer.import`, `export.selection`. No tool is renamed or removed.

## Scope

### In scope

- `layer.translate`: move a layer's content by `dx`, `dy` (Image documents)
- `select.move_content`: move the selected pixels and the selection with them (both modes)
- `select.move`: move only the selection outline (Image documents)
- `layer.import`: a workspace image file becomes a new layer at `x`, `y`, optionally growing the canvas (both modes)
- `export.selection`: the selection's pixels of a layer, or of the composite, written to a workspace PNG, optionally cut (both modes)
- `pincel://guide` documents the tools as the headless clipboard

### Out of scope

- Moving layer content or the selection outline alone in Pixel documents — because the Pixel editor offers neither: its Move tool without a selection pans the view, and it has no outline-only move
- The system clipboard (reading or writing it from `pincel-mcp`) — because the headless server touches only workspace files (`mcp-headless-pixel` Decisions), and live clipboard access belongs to `mcp-live-desktop`
- Paste as a new document, and opening a PNG as a Pixel document — because `doc.open` / `doc.new` own new documents, and `mcp-parity-export` covers PNG as a Pixel document
- Free transform (scale or rotate the moved pixels while moving) — because the transforms are the menu commands of `mcp-parity-selection-transform`
- Cutting from the composite (a "Cut Merged") — because no editor offers it

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains `layer.translate`, `select.move_content`, `select.move`, `layer.import` and `export.selection`, each meeting `mcp-headless-pixel` AC-02.
- [ ] AC-02 (error): If `layer.translate` or `select.move` is called on a Pixel document, then the call fails with `isError: true`, the message names the document's mode, and the document is unchanged.

### Moving

- [ ] AC-03 (event): When the client calls `layer.translate` with `dx`, `dy` on an Image document, then the target layer's pixels equal the editor's after the Move tool drags that layer by the same offset. Pixels pushed off the canvas are gone, the uncovered area is transparent, and a group moves its whole subtree.
- [ ] AC-04 (event): When the client calls `select.move_content` with `dx`, `dy` while a selection exists, then the target layer's pixels at the target frame and the selection equal the editor's after Move Selected Pixels by that offset. The selected pixels land at the offset, transparency stays behind, the selection's shape moves with them, and pixels moved off the canvas are dropped.
- [ ] AC-05 (state): While no selection exists, `select.move_content` on an Image document moves the whole target layer, as the Image editor does. On a Pixel document it fails with `isError: true` saying a selection is needed (`select.*` first), and the document is unchanged.
- [ ] AC-06 (event): When the client calls `select.move` with `dx`, `dy` on an Image document with a selection, then the selection's coverage is shifted by the offset (coverage leaving the canvas is dropped), and no pixel changes. Without a selection the call succeeds and changes nothing.
- [ ] AC-07 (always): A successful `layer.translate`, `select.move_content` or `select.move` call that changes pixels or the selection is exactly one undo step, and one `doc.undo` restores both the pixels and the selection, in Pixel documents too: the selection moved by `select.move_content` is part of that step, unlike the selection tools of `mcp-parity-selection-transform` AC-11. A call that changes neither adds no undo step and reports `changed: false`. Examples: `dx = dy = 0`, `select.move` without a selection, or `layer.translate` of a fully transparent layer. `select.move_content` of fully transparent selected pixels with a non-zero offset still moves the selection and is one step.
- [ ] AC-08 (error): If `layer.translate` or `select.move_content` targets a locked layer, a layer in a locked group, or (`layer.translate`, Image) a group containing a locked layer, or `select.move_content` targets a group (either mode; the Image core refuses it because a group has no pixels) or a tilemap layer (Pixel), then the call fails with `isError: true` naming the layer and the reason, and the document is unchanged.

### Import (paste)

- [ ] AC-09 (event): When the client calls `layer.import` with a workspace-relative `path`, then the decoded image lands on a new image layer, with its top-left at canvas `(x, y)` (default `0, 0`). In a Pixel document it lands on the target frame. The layer sits directly above the active layer, as the editor's Paste places it, and is named `Layer N`, N being the number of layers after adding, as both editors name a pasted layer (Pixel: `pincel-wasm` `paste_layer`; Image: the core `PasteLayer`). The layer becomes active, the result carries its id, and the call is one undo step.
- [ ] AC-10 (option): Where `layer.import` gets `expand: true`, the canvas first grows, anchored top-left, just enough to cover the image at `(x, y)`, as the editor's Paste with "expand canvas" does. It grows only to the right and down: parts left of or above the canvas origin (negative `x` / `y`) are cut off in both cases. Without `expand`, the parts outside the canvas are cut off and the canvas size stays; an image wholly outside the canvas adds an empty layer, as the editor's Paste does.
- [ ] AC-11 (event): When `layer.import` reads a file, then Pixel documents accept PNG, and Image documents accept PNG, JPEG, WebP, BMP, GIF and TIFF, recognized by content as the editor's Paste decodes them.
- [ ] AC-12 (error): If the `layer.import` file does not exist, cannot be decoded, is in a format the document's mode does not paste, or the document is an indexed Pixel sprite, then the call fails with `isError: true` stating the reason, and the document is unchanged.
- [ ] AC-13 (error): If `layer.import` with `expand: true` would grow the canvas beyond the mode's size limit (Pixel 4096 px per side; Image 16384 px per side or 64 Mpx), then the call fails with `isError: true` naming the limit and the size it would need, and the document is unchanged.

### Export (copy / cut)

- [ ] AC-14 (event): When the client calls `export.selection` with `path`, then a PNG is written there, whatever the path's extension, equal to the editor's Copy of the target layer at the target frame. The PNG covers the selection's bounds clipped to the canvas, or the whole canvas without a selection. Pixels outside the selection's shape are transparent, and in Image documents alpha is scaled by coverage. The result reports the exported area as `{x, y, w, h}`, the bounds shape of `mcp-parity-structure` AC-24. The document is unchanged. The target layer is `layer` or, without it, the layer the editor's Copy reads: the active layer whatever its kind (Pixel: else the paint target).
- [ ] AC-15 (option): Where `export.selection` gets `merged: true`, the PNG equals the editor's Copy Merged: the composite of the visible layers at the target frame, over the same area.
- [ ] AC-16 (option): Where `export.selection` gets `cut: true`, the PNG is written as for Copy, and afterwards the target layer equals the editor's after Cut. The copied pixels are cleared, the whole layer when there is no selection. The cut is one undo step, and the file stays when it is undone.
- [ ] AC-17 (event): When `export.selection` writes a PNG from a document that `layer.import` accepts (not an indexed Pixel sprite, AC-12) and `layer.import` reads it back at the reported `x`, `y` into a document of the same mode with the same size, then the new layer's pixels equal the PNG's.
- [ ] AC-18 (error): If `export.selection` gets `cut` together with `merged`, `merged: true` together with `layer` (the composite has no layer), a `cut` on a locked, group or tilemap layer, or (Image) a selection with no selected pixel, then the call fails with `isError: true` stating the reason, nothing is written, and the document is unchanged.
- [ ] AC-19 (error): If the file of `export.selection` cannot be written (`E007` outside the workspace, `--read-only`, an I/O error), then the call fails as `export.png` does with that path, and a `cut` leaves the document unchanged.

### Shared rules

- [ ] AC-20 (error): If a tool of this spec names a `layer` or `frame` that does not exist, or `dx` / `dy` / `x` / `y` is not an integer, then the call fails with `isError: true` naming the argument, and the document is unchanged.
- [ ] AC-21 (error): If the core command the editor issues for a tool of this spec refuses the call (for example a Pixel cel in a color mode Move Selected Pixels does not support), then the call fails with `isError: true` carrying the command's reason, and the document is unchanged.
- [ ] AC-22 (always): The `pincel://guide` resource names every tool of this spec and the modes it works in, and describes `export.selection` + `layer.import` as the headless Copy / Cut / Paste.

## Technical notes

Every capability exists already. Grep for these names, because line numbers drift.

| Tool | Pixel | Image |
| ---- | ----- | ----- |
| `layer.translate` | — | `CommandSpec::TranslateLayer` (groups: `group_edits`) |
| `select.move_content` | `MoveSelectionContent` (wasm `apply_move_selection`); needs a selection | `TranslateSelection` (`MoveSelection`; whole layer without selection) |
| `select.move` | — | `TranslateSelectionMask` |
| `layer.import` | wasm `paste_layer` / `paste_image` over `pincel_core::{import_png, PasteLayer}`; refused for indexed sprites | wasm `paste_layer` over `codec::decode` + `PasteLayer::at` |
| `export.selection` | `pincel_core::{copy_pixels, CopySource, encode_png}`; cut: wasm `cut_selection` (`ClearRegion`, masked with a selection) | `tools::copy::copy_selection`; cut: wasm `cut_selection` (`delete_selection`) |

`paste_image`, the copy-layer rule (`copy_layer`: the selected layer whatever its kind, else the paint target) and `cut_selection` live in the wasm crates today. They move into plain Rust that `pincel-agent` can call, as in the phase-0 refactor of `mcp-headless-pixel`, with the wasm wrappers and tests unchanged.

`layer.import` reads through the host's `FileHost` and the sandbox (`E007`), with the size cap of `doc.open`. `export.selection` writes through it, so `--read-only` refuses it (`mcp-headless-pixel` AC-36). For a cut, the tool writes the file first and applies the cut only after the write succeeded (AC-19).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `select.move_content` that moves every selected pixel off the canvas | Succeeds as in the editor: pixels gone, selection empty or clipped, one undo step restores both |
| `select.move_content` with a shaped (non-rectangular) selection | Only the pixels inside the shape move; pixels in a hole stay (as in the Pixel editor's test `delete_and_move_honour_the_shaped_mask`) |
| `layer.translate` on an Image group | The whole subtree moves; one undo step |
| `layer.import` at negative `x`, `y` without `expand` | The image is cropped to the canvas |
| `layer.import` of an image larger than the canvas without `expand` | The image is cropped to the canvas; canvas size unchanged |
| `layer.import` of a file larger than `doc.open` accepts | Fails like `doc.open` with an oversized file |
| `layer.import` of a JPEG into a Pixel document | Fails (AC-12), naming PNG as the accepted format |
| `export.selection` onto an existing file or into a missing directory | Same as `export.png` with that path |
| `export.selection` of a group layer in a Pixel document | The group's rendered pixels, as the Pixel editor's Copy |
| `export.selection` with `cut: true` and no selection | Whole target layer written, then cleared; one undo step |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `layer.import` follows the editor's Paste: above the active layer, named as Paste names it, not Pixel File ▸ Import PNG as Layer (top of the stack, file name, origin) | Paste is the action both editors share. `layer.add` + `layer.move` (`mcp-parity-structure`) and `layer.set` reach the other placement and name | Low |
| `export.selection` always writes PNG | Both editors copy PNG to the clipboard; other formats are `mcp-parity-export`'s concern | Low |
| `layer.import` with `layer` / `frame` arguments: `frame` picks the Pixel frame; there is no `layer` argument, and the new layer goes above the active layer | Paste has no target-layer choice in either editor | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built  |        |
| AC-02 | `layer_translate_and_select_move_on_a_pixel_document_fail_naming_the_mode` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-03 | `layer_translate_equals_the_editors_move_tool` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-04 | `select_move_content_equals_the_editors_move_selected_pixels`, `select_move_content_moves_the_target_layer_at_the_target_frame` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-05 | `select_move_content_without_selection_follows_each_editor` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-06 | `select_move_shifts_only_the_selection` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-07 | `moves_are_one_undo_step_restoring_pixels_and_selection` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-08 | `moves_on_locked_group_or_tilemap_layers_fail_with_the_reason` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-09 | `layer_import_pastes_above_the_active_layer_like_the_editor`, `layer_import_lands_on_the_target_frame` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-10 | `layer_import_expands_or_crops_like_the_editors_paste` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-11 | `layer_import_accepts_each_modes_formats_by_content` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-12 | `layer_import_failures_state_the_reason_and_change_nothing`, `layer_import_accepts_each_modes_formats_by_content` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-13 | `layer_import_with_expand_beyond_the_limit_fails_naming_it` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-14 | `export_selection_writes_the_editors_copy`, `export_selection_copies_the_target_layer_at_the_target_frame` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-15 | `export_selection_merged_writes_the_editors_copy_merged` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-16 | `export_selection_cut_clears_like_the_editors_cut` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-17 | `export_selection_round_trips_through_layer_import` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-18 | `export_selection_bad_combinations_write_nothing` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-19 | `export_selection_that_cannot_write_leaves_the_document_unchanged` (`crates/pincel-mcp/tests/stdio.rs`), `export_selection_cut_with_a_failing_write_fails_like_export_png` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-20 | `clipboard_tools_naming_a_missing_layer_or_frame_or_a_fractional_offset_fail` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-21 | `a_refusing_move_command_fails_with_its_reason` (`crates/pincel-agent/src/tests/clipboard.rs`) | built  |        |
| AC-22 | `read_resource_guide_covers_workflow_and_both_modes`, `guide_names_the_modes_of_the_clipboard_tools` (`crates/pincel-agent/src/resources.rs`) | built  |        |

## Decisions

- 2026-11-05: The headless clipboard is a workspace PNG file: `export.selection` writes it, `layer.import` reads it. The headless server has no system clipboard and touches only workspace files, and a file lets the agent also hand the image to other tools.
- 2026-11-05: Cut is the option `cut: true` on `export.selection` (AC-16), not a separate tool and not export + `select.delete`. Owner's choice, because one call then equals the editor's Cut (one undo step, the whole layer without a selection in Pixel mode too), and no extra tool name is needed.
- 2026-11-05: A move that changes nothing adds no undo step (AC-07), although the Pixel `apply_move_selection` lets a zero offset join the undo bus. `mcp-parity-structure` AC-25 sets this rule for every MCP tool, and the Pixel UI never sends a zero move (`pointer.ts` checks the offset first).
- 2026-11-05: `merged: true` with `layer` is an error, not silently ignored (AC-18), because the agent asked for two different sources and should learn which one it gets.
- 2026-11-05: `select.move_content` on a Pixel document without a selection is an error, not a whole-layer move. The Pixel editor offers no whole-layer move (its Move tool pans), and parity forbids inventing one.
- 2026-11-05: `select.move` and `layer.translate` are Image-only (AC-02), per Decision Log 2026-11-05 "MCP tool parity", because only the Image editor offers them.
- 2026-11-05 (spec-build): Paste, Copy and Cut were not moved out of the wasm crates: they are thin compositions of public core API (`import_png` + `PasteLayer`, `copy_pixels` + `encode_png`, `ClearRegion`; Image `codec::decode` + `PasteLayer::at`, `copy_selection`, `DeleteSelection`), which `pincel-agent` calls directly; only the Pixel copy-layer rule (the selected layer, else the paint target) is repeated, four lines.
- 2026-11-05 (spec-build): `export.selection` of an Image layer that is not the active one runs the editor's `copy_selection` on a copy of the document with that layer active, because the core's Copy reads the active layer only.
- 2026-11-05 (spec-build): A Pixel `select.move_content` on a layer without a cel at the target frame adds an empty cel in the same undo step, as the editor's paint target does, so a move of transparent pixels still moves the selection (AC-07).
- 2026-11-05 (spec-build): `export.selection` reports `{doc, path, area: {x, y, w, h}, cut, changed}`, `layer.import` `{doc, layer, width, height}` (the canvas size after an `expand`).
- 2026-11-06 (spec-build): Acceptance found test gaps in AC-08, AC-09, AC-10, AC-11, AC-18 and AC-22; the behavior existed, so the added checks were green at once. Counter-checks, all reverted: a Pixel lock check that ignores the group's lock and an Image lock message without the layer id made `moves_on_locked_group_or_tilemap_layers_fail_with_the_reason` fail (AC-08, which now covers a layer inside a locked group in both modes); `layer.import` ignoring `frame` made `layer_import_lands_on_the_target_frame` fail (AC-09); clamping negative paste offsets to 0 made `layer_import_expands_or_crops_like_the_editors_paste` fail (AC-10, which now compares every case with the editor's `PasteLayer` in both modes and checks the landed, clipped and dropped pixels and the empty layer of an image wholly outside the canvas); refusing GIF in Image documents made `layer_import_accepts_each_modes_formats_by_content` fail (AC-11, now PNG, JPEG, WebP, BMP, GIF and TIFF under a file name that hides the format, compared with the editor's Paste; Pixel refuses the five non-PNG formats); skipping the paint-target check for a tilemap `cut` made `export_selection_bad_combinations_write_nothing` fail, the cut then writing the file (AC-18); dropping "Both modes;" from the clipboard bullet made `guide_names_the_modes_of_the_clipboard_tools` fail (AC-22).
- 2026-11-06 (spec-build): An Image `export.selection` of a group (the active layer or `layer`) fails naming the group, because the Image core's Copy (`copy_selection`, as the editor's Edit ▸ Copy) has no pixels to copy from a group and the editor refuses it too; the tool said "nothing is selected" before, which `export_selection_copies_the_target_layer_at_the_target_frame` showed red. A Pixel group still copies its rendered pixels (edge cases).
- 2026-11-06 (spec-build): Clause-by-clause audit of every AC. Added: the copied layer and frame (AC-14: `layer` 0 while layer 1 is active, the active non-zero layer, an active Pixel group copying its rendered pixels, `frame` 0 while frame 1 is active, Image `layer` ≠ active layer with the document, active layer included, unchanged), `select.move_content` with `layer` and `frame` ≠ the active ones (AC-04), `select.move` without a selection adding no step and changing nothing (AC-06, AC-07), the Copy Merged area (AC-15), the Image cut's PNG, the file surviving undo and a cut without a selection clearing the whole layer (AC-16), the Image combinations and a cut on a locked layer, a group and a layer in a locked group writing nothing (AC-18), an I/O error failing exactly as `export.png` with the document unchanged in both modes (AC-19), fractional `dx` / `dy` / `x` / `y` for every tool and the Image document unchanged (AC-20), the Image paste's undo step and id (AC-09). Green at once, each went red against a temporary break that was then reverted: Image `export.selection` skipping `check_paintable` for a cut (AC-18: the locked-group row wrote the file), the Pixel copy default using the paint target instead of the active layer (AC-14: "the active group"), Image Copy ignoring `layer` (AC-14: "`layer` 0"), Pixel `select.move_content` ignoring `frame` (AC-04), a failed write ignored (AC-19: the call succeeded and cut), the Copy Merged area reported as the whole canvas (AC-15).
