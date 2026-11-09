---
status: implemented
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6, pincel.md §9.4, pincel.md §15 (2026-10-12 effects honour the selection), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image mode: Fill Selection

## Context

paint.net has Edit ▸ Fill Selection (Backspace): it paints the selected area of the current layer with the primary color, the whole layer without a selection. Image mode has the opposite command, Edit ▸ Delete Selection (`delete_selection` in `pincel-image-core`, Delete and Backspace in `shortcuts.ts`), but no way to fill a selection short of a bucket fill with tolerance 255, which still depends on the layer's content and does not reach every selected pixel. Users coming from paint.net, Photoshop (Alt+Backspace) or GIMP expect the command.

This is the first of two specs for paint.net's Edit and options-bar extras. The second, `image-edit-extras-tool-options.md`, adds the Antialiasing and Blending (Normal / Overwrite) switches to the options bar.

Delete Selection already sets the pattern this spec follows: one undoable command over the active layer, "no selection = everything selected" (spec §5.4, §8.1 of the archived `fineliner.md`), and feathered coverage scales the change instead of cutting a hard edge (Decision Log 2026-10-12).

Decision Log 2026-11-05 "MCP tool parity" requires an MCP tool for every document-changing capability of the editor, so this spec adds one (AC-10 to AC-12).

"The foreground color" means the color of the foreground swatch, with its alpha once `color-picker.md` gives Image mode color alpha; until then it is opaque.

## Scope

### In scope

- Edit ▸ Fill Selection in Image mode, below Delete Selection
- Default shortcut: Backspace on Windows / Linux, Option+Backspace on macOS, where Backspace stays Delete Selection
- Filling the selection, or the whole active layer without one, with the foreground color, feathered and partial coverage blending proportionally, as one undo step
- The locked-layer and group refusals the other Edit commands have
- The MCP tool `select.fill` for Image documents

### Out of scope

- Fill Selection in Pixel mode — because Pixel mode's marquee is a hard-edged rectangle, indexed sprites need palette handling, and `pincel-core` has no fill-region command; it is cheap to add later as its own spec, and until then Pixel mode does not bind the action
- Filling with the background color, a pattern or a gradient (Photoshop's Fill dialog) — because paint.net fills with the primary color only, and the Gradient tool covers ramps
- Stroke Selection (outline the selection) — because paint.net has no such command; it belongs to a separate spec if wanted
- Honoring the options bar's Blending mode (Normal / Overwrite) — because Fill Selection is an Edit command, not a tool, and has its own fixed semantics (AC-03); the Blending switch arrives with `image-edit-extras-tool-options.md`
- Making Delete Selection's menu entry enabled without a selection — because that is existing behavior of another command; this spec only notes the asymmetry under Decisions

## Acceptance criteria

- [x] AC-01 (always): The Image editor's Edit menu has a Fill Selection entry directly below Delete Selection, labelled in every UI language, showing its shortcut hint from the keymap, and enabled whenever a document is open.
- [x] AC-02 (event): When the user chooses Edit ▸ Fill Selection or presses its shortcut while a selection exists, then every pixel of the active layer with selection coverage 255 becomes exactly the foreground color (its red, green, blue and alpha), and every pixel with coverage 0 is unchanged.
- [x] AC-03 (event): When Fill Selection runs over a pixel with partial selection coverage c (1–254, from feathering or an anti-aliased selection edge), then the pixel moves from its previous value toward the foreground color by c/255: its alpha equals previous alpha + (foreground alpha − previous alpha) × c/255 within ±1, and its color lies between the previous and the foreground color; for an opaque foreground over an opaque pixel, each color channel equals previous + (foreground − previous) × c/255 within ±1.
- [x] AC-04 (event): When the user runs Fill Selection without a selection, then every pixel of the active layer becomes exactly the foreground color.
- [x] AC-05 (event): When Fill Selection has changed the layer, then it is exactly one undo step: one Undo restores every pixel of the layer to its previous value, Redo repeats the fill, and the selection is the same before and after the fill.
- [x] AC-06 (event): When Fill Selection would leave every pixel as it was (the selected pixels already hold the foreground color), then no undo step is added and the document's unsaved-changes state is the same as before the command.
- [x] AC-07 (error): If the selection exists but covers no pixel, then Fill Selection changes nothing and adds no undo step.
- [x] AC-08 (error): If the active layer is locked, lies in a locked group, or is a group, then Fill Selection changes no pixel, adds no undo step, and the editor shows the same locked-layer notice as Delete Selection.
- [x] AC-09 (always): Fill Selection changes only the active layer: the pixels of every other layer, layer properties, the canvas size and the selection stay unchanged, also when the active layer is hidden or has an opacity below 100 %.
- [x] AC-10 (always): `tools/list` of `pincel-mcp` contains `select.fill`, meeting `mcp-headless-pixel` AC-02, with the arguments `doc`, `layer` (default: the active layer, as for `select.delete` in `mcp-parity-selection-transform`) and `color` (default `#000000`, color syntax of `mcp-headless-pixel` AC-21); it takes no `frame`, since Image documents have none.
- [x] AC-11 (event): When the client calls `select.fill` on an Image document, then the document's pixels equal the editor's after Fill Selection with that color on that layer and the same selection (the whole layer without a selection), the selection stays, the call is one undo step, a call that changes nothing adds none, and the result reports `changed` (`true` or `false`) as `select.delete` does, without the selection's bounds.
- [x] AC-12 (error): If `select.fill` is called on a Pixel document, names a layer that does not exist, targets a locked layer or a group, gets a `color` that is not a valid color, or gets a `frame` argument, then the call fails with `isError: true` (naming the document's mode for a Pixel document, the layer for a locked layer or group, otherwise the argument and its value), and the document is unchanged.
- [x] AC-13 (always): The `pincel://guide` resource names `select.fill`, that it works on Image documents only, and that it fills the whole layer without a selection.
- [x] AC-14 (always): Fill Selection's default shortcut is Backspace on Windows and Linux and Option+Backspace on macOS. On Windows and Linux, Delete alone runs Delete Selection; on macOS, Backspace and Delete both run Delete Selection, in both modes. In Pixel mode on Windows and Linux, Backspace runs neither Fill Selection nor Clear, and Delete still clears the marquee.

## Technical notes

- The core command mirrors `tools::delete_selection` (`crates/pincel-image-core/src/tools/delete.rs`): one `SetPixels` over the layer, so undo restores the layer exactly and the existing lock refusal in `CommandSpec::refused` applies once the new variant reports its target layer. Its history kind is `fill_selection` (the history panel names undo steps by kind ids from the core).
- `CommandSpec` gains a `fill_selection { layer, color }` variant; the ts-rs mirror under `ui/src/modes/image/core/generated/` is regenerated (`cargo test -p pincel-image-wasm export_command_spec`).
- The menu entry goes through `layerEdit(false, …)` in `ImageEditor.svelte`, as Delete Selection does, which shows the locked notice (AC-08).
- The tool name `select.fill` pairs with `select.delete` from `mcp-parity-selection-transform` (its AC-21 to AC-23); share its target resolution and lock / group refusal.
- The platform split of the default key uses the existing `isMac` check in `shortcuts.ts`; the keymap overview and menu hints print the platform's chord.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Selection extends to the canvas edge after Select All | Every pixel of the layer is filled, as without a selection |
| Feathered selection with radius 20 | Pixels in the feather band blend by coverage (AC-03); none outside the band changes |
| Foreground color with alpha 128 (after `color-picker.md`) over a transparent pixel at coverage 255 | The pixel becomes the foreground color with alpha 128 |
| Foreground fully transparent (alpha 0, after `color-picker.md`) | Every selected pixel loses alpha in proportion to its coverage, exactly as with Delete Selection (AC-02, AC-03) |
| A user keymap override from before this spec binds Backspace to Delete Selection | The override wins: Backspace keeps running Delete Selection, and Fill Selection has no effective key on that platform until the user rebinds it, because a stored user choice is never overridden by a new default |
| Fill Selection twice in a row with the same color | The second run changes nothing and adds no undo step (AC-06) |
| Active layer hidden | The fill happens on the hidden layer; the composite shows no change until it is shown |
| Fill Selection while the Move tool has lifted selected pixels mid-drag | Not reachable: the drag ends on pointer-up before a menu or key action runs |
| `select.fill` with `color: "#ff000080"` | Fills with alpha 128 as in AC-03 / AC-02, independent of the editor's color picker |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Fill Selection moves each pixel toward the foreground by its coverage, alpha included (fully covered pixels become exactly the foreground), instead of compositing over the old pixel | paint.net's Fill Selection replaces the selected pixels; the rule makes Fill with a transparent color equal Delete Selection, and it is the Overwrite rule of `image-edit-extras-tool-options.md` AC-11. For an opaque foreground over an opaque pixel the result equals compositing | Low: differs from compositing only over transparent pixels or once color alpha exists |
| `select.fill` defaults to `color: #000000` instead of requiring a color | The MCP server has no foreground swatch, and `draw.stroke`, `draw.shape` and `draw.text` default to `#000000` too | Low |
| The menu entry stays enabled without a selection | paint.net fills the whole layer then; AC-04 makes that behavior reachable from the menu | Low |

## Open questions

## Evidence

| AC    | Test | Status   | Commit  |
| ----- | ---- | -------- | ------- |
| AC-01 | `Edit ▸ Fill Selection sits below Delete Selection with its key, in English and German (AC-01)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 1bf117d |
| AC-02 | `fill_selection_full_coverage_takes_the_color_and_leaves_unselected_pixels`, `fill_selection_full_coverage_over_transparent_takes_the_exact_color` (`crates/pincel-image-core/src/tools/fill_selection.rs`); `Fill Selection paints the selected pixels exactly with the foreground color (AC-02)` (`ui/e2e/image-fill-selection.spec.ts`); `Fill Selection fills only the active layer, also hidden and translucent (AC-02, AC-04, AC-09)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-03 | `fill_selection_partial_coverage_interpolates_opaque_channels`, `fill_selection_partial_coverage_moves_alpha_and_keeps_color_between`, `fill_selection_transparent_color_scales_alpha_like_delete` (`crates/pincel-image-core/src/tools/fill_selection.rs`) | verified | 2c9f6d3 |
| AC-04 | `fill_selection_without_selection_fills_the_whole_layer_and_undoes` (`crates/pincel-image-core/src/tools/fill_selection.rs`); `Backspace without a selection fills the whole layer (AC-04, AC-14)` (`ui/e2e/image-fill-selection.spec.ts`); `Fill Selection fills only the active layer, also hidden and translucent (AC-02, AC-04, AC-09)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-05 | `fill_selection_is_one_undo_step_that_undoes_and_redoes_and_keeps_the_selection` (`crates/pincel-image-core/tests/fill_selection.rs`); `Fill Selection is one undo step that keeps the selection; Redo repeats it (AC-05)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-06 | `fill_selection_that_changes_nothing_returns_none` (`crates/pincel-image-core/src/tools/fill_selection.rs`); `fill_selection_that_changes_nothing_adds_no_undo_step` (`crates/pincel-image-core/tests/fill_selection.rs`); `a Fill Selection that changes nothing adds no undo step and no unsaved changes (AC-06)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-07 | `fill_selection_with_empty_selection_returns_none` (`crates/pincel-image-core/src/tools/fill_selection.rs`); `fill_selection_with_a_selection_that_covers_nothing_changes_nothing` (`crates/pincel-image-core/tests/fill_selection.rs`) | verified | 2c9f6d3 |
| AC-08 | `fill_selection_on_a_locked_layer_is_refused` (`crates/pincel-image-core/tests/fill_selection.rs`); `locked_group_refuses_edits_on_its_descendants`, `pixel_edits_on_a_group_are_refused` (`crates/pincel-image-core/src/command/spec.rs`); `Fill Selection on a locked layer or a group shows the locked notice and changes nothing (AC-08)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-09 | `fill_selection_changes_only_the_target_layer_also_when_hidden_and_translucent` (`crates/pincel-image-core/tests/fill_selection.rs`); `Fill Selection fills only the active layer, also hidden and translucent (AC-02, AC-04, AC-09)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 2c9f6d3 |
| AC-10 | `select_fill_is_listed_with_doc_layer_and_color_and_no_frame` (`crates/pincel-agent/src/tests/select_fill.rs`); `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | verified | a45b6d1 |
| AC-11 | `select_fill_matches_the_editor_with_a_feathered_selection`, `select_fill_defaults_to_black_on_the_active_layer_and_fills_it_whole_without_a_selection`, `select_fill_that_changes_nothing_reports_unchanged_and_adds_no_step` (`crates/pincel-agent/src/tests/select_fill.rs`); `image_calls_name_their_step_after_the_editor_action` (`crates/pincel-agent/src/tests/history.rs`) | verified | a45b6d1 |
| AC-12 | `select_fill_refuses_pixel_documents_bad_layers_colors_and_frames` (`crates/pincel-agent/src/tests/select_fill.rs`); `validate_missing_and_unknown_arguments_are_named` (`crates/pincel-agent/src/schema.rs`); `validate_unknown_argument_quotes_a_long_value_shortened` (`crates/pincel-agent/src/schema.rs`) | verified | a45b6d1 |
| AC-13 | `guide_names_the_modes_of_the_selection_and_transform_tools` (`crates/pincel-agent/src/resources.rs`) | verified | a45b6d1 |
| AC-14 | `Backspace without a selection fills the whole layer (AC-04, AC-14)`, `Delete runs Delete Selection; in Pixel mode Backspace does nothing (AC-14)`, `on macOS › ⌥Backspace fills; Backspace and Delete delete in both modes (AC-14)` (`ui/e2e/image-fill-selection.spec.ts`) | verified | 1bf117d |

## Summary

> Completed: 2026-11-09

- Edit ▸ Fill Selection in Image mode, directly below Delete Selection (en + de): fills the selection of the active layer with the foreground color, or the whole layer without a selection, as one undo step that keeps the selection.
- `fill_selection` in `pincel-image-core` and the `fill_selection` `CommandSpec`: partial coverage moves a pixel toward the color by c/255 (alpha a plain lerp, color weighted by alpha), a fill that changes nothing adds no undo step, and the lock / group refusal of the other pixel edits applies.
- Default key Backspace on Windows / Linux and ⌥Backspace on macOS, where Backspace and Delete stay Delete Selection; Pixel mode binds no Fill Selection.
- MCP: `select.fill` for Image documents (`doc`, `layer`, `color` default `#000000`), reporting `changed` like `select.delete`, and named in the `pincel://guide` resource.

## Decisions

- 2026-11-05: Delete Selection's menu entry is disabled without a selection although its key clears the whole layer then; Fill Selection's entry is enabled without one (AC-01, AC-04). The asymmetry is left for a separate small fix rather than widened here.
- 2026-11-05: Default key (owner's choice): Backspace runs Fill Selection on Windows / Linux; on macOS Backspace stays Delete Selection and Fill Selection gets Option+Backspace, because that gives paint.net parity where paint.net users are without taking the only delete key from Mac laptops (Photoshop's Mac fill key). Pixel mode does not bind Fill Selection (AC-14).
- 2026-11-05: MCP name (owner's choice): `select.fill`, not `draw.fill_selection`, because it pairs with `select.delete` from `mcp-parity-selection-transform` and takes the same arguments.
- 2026-11-05 (clarification pass): Fill Selection interpolates toward the foreground by coverage, alpha included, because then a transparent fill equals Delete Selection with no hard step at the feather edge and the rule matches Overwrite in `image-edit-extras-tool-options.md`. `select.fill` takes no `frame` (Image only) and reports `changed` like `select.delete`, because one result shape for the pair keeps clients simple. A stored keymap override keeps its chord over the new default, because user choices are never silently replaced.
- 2026-11-06 (build): Partial coverage mixes the color weighted by alpha (premultiplied) while alpha is a plain lerp, because a straight per-channel lerp would darken a fill over transparent pixels and change the color of a transparent fill, which the edge case wants equal to Delete Selection; for opaque over opaque both are the same (AC-03).
- 2026-11-06 (build): `fill_selection` in `pincel-image-core` returns no command when no pixel would change, because then the editor, `apply_command` and `select.fill` share the no-op rule of AC-06 / AC-07 / AC-11 without a dry run.
- 2026-11-06 (build): Fill Selection records the foreground in the recent colors (`notePainted`) like the painting tools, because it paints with that color.
- 2026-11-06 (build): The schema's unknown-argument error now quotes the value (``unknown argument `frame` (got 7; …)``), because AC-12 asks a `frame` refusal to name the argument and its value and `select.fill`'s closed schema refuses it before the tool runs.
- 2026-11-06 (build): On macOS, ⌥⌫ inside the live text field is not a key the field owns, so it runs Fill Selection like any other passed-on chord (`image-live-text` AC-23); left as is because the field's key set belongs to that spec.
- 2026-11-06 (build): `command/spec.rs`, `ImageEditor.svelte` and `controller.ts` were over 800 lines before this spec; the additions are kept to a few lines each and the new logic lives in new files (`tools/fill_selection.rs`, `tests/select_fill.rs`) rather than splitting those files here.
- 2026-11-06 (build), red runs: the core tests failed against a stub returning `None` (6 of 9); the three `None` tests failed once the no-change / empty-selection guards and the layer lookup were broken. The `tests/fill_selection.rs` tests (written after the command existed) failed with the spec arm on layer 0, without `FillSelection` in `edited_layer` and without the no-change guard (5 of 5). The `select_fill` tests failed before the tool existed (4 of 4); the no-change and refusal tests failed with `changed` forced true and the value dropped from the unknown-argument error; the guide test failed with the old `guide.md`. All 8 e2e tests failed against a build without the UI change.
- 2026-11-06 (spec-check rework): The editor's active layer is now pinned by an e2e test with two layers, each filled in turn while it is active, hidden and below 100 % opacity (AC-02, AC-04, AC-09). Red runs: with `layer: 0` in `fillSelection()` the test failed (the upper layer stayed empty); with the top layer hardcoded (`editor.layers.length - 1`) it failed too (the lower layer was not filled); green again after the revert.
- 2026-11-06 (spec-check rework): The unknown-argument error quotes at most 60 characters of the value, then `…`, because a large value (a pixel array) would bloat the message. Red run: `validate_unknown_argument_quotes_a_long_value_shortened` failed while the value was quoted whole.
- 2026-11-06 (spec-check rework): `select.fill` is pinned to the history kind `fill_selection` in `image_calls_name_their_step_after_the_editor_action`; it failed with the core kind changed to `delete_selection`, green after the revert.
- 2026-11-06 (spec-check rework): The Pixel-mode Backspace check (AC-14) now uses a blue foreground and checks the marquee's empty pixels too, so a fill would show. Pixel mode has no fill command, so no red run can produce one; the assertion is red for a fill by construction (an empty pixel would turn blue).
- 2026-11-06 (spec-check round 2): adding `fill_selection` to the test list `PIXEL_EDITS` in `command/spec.rs` shifted `translate_layer` from index 4 to 5, so `move_on_a_group_with_a_locked_descendant_is_refused` (image-layer-groups AC-18) tested Fill Selection on a group — refused for being a group, the test went hollow. The list is split into `PIXEL_ONLY_EDITS` and a named `TRANSLATE_LAYER`; the test now also asserts Move is allowed while nothing below is locked. Red against `refused()` checking only the group itself instead of its subtree, green after the revert.
