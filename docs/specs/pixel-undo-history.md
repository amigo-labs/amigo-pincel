---
status: implemented
created: 2026-10-29
size: S
mode: Pixel
design: [pincel.md §6.1, pincel.md §6.2, pincel.md §6.3]
---

# Pixel undo history

## Summary

> Completed: 2026-10-29

- Linear Pixel-mode undo / redo on the `pincel-core` command bus, capped at 100 entries in the editor; a new edit drops the redo branch, a failed edit leaves the history untouched.
- Gesture merging: one press-drag-release stroke is one entry (`endStroke` seals), consecutive edits of the same layer opacity, palette entry, frame duration, tag range or tag name merge; undo / redo seal against merging; shape, fill, move and tile commands never merge.
- Every document-content edit (layers, frames, palette incl. presets and `.gpl`, selection delete, slices, tilesets, tiles) is undoable; selection, eyedropper, frame / layer switching and onion skin stay outside the history.
- History starts empty on create / open and survives save; undo of an added frame clamps the current frame.
- Pixel editor controls: toolbar buttons disabled at depth 0, Edit menu, Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z and Ctrl/Cmd+Y; modal dialogs keep the shortcut from the editor.

## Context

Every Pixel-mode edit to document content runs through a command bus in `pincel-core`. The bus applies the command, records it in a linear undo history, and can revert or re-apply it (pincel.md §6). Undo and redo must restore exactly what an edit changed. One press-drag-release gesture must be one undo step, not hundreds. Transient editor state such as the selection deliberately stays outside the history. This spec records the behavior as built on 2026-10-29.

## Scope

### In scope

- The linear undo / redo history: depth, cap, the redo branch being dropped by a new edit
- Merging several commands into one undo step, and the stroke boundary that stops merging
- Which Pixel-mode actions create a history entry and which do not
- The Pixel editor's Undo / Redo controls: toolbar buttons, Edit menu, keyboard shortcuts

### Out of scope

- Image-mode undo — because Image mode has its own history in `pincel-image-core`; the `image-editing` spec owns it
- Branching history or a history panel — because §6.2 rules out branching for the MVP
- Undo history that survives closing the app or reloading the file — because §6.2 defers it to Phase 2
- The "modified since last save" marker, which reads the undo depth — because the `file-access` spec owns it
- Rolling back a stroke cut short by a second finger (pinch) — because the `tablet-gestures` / `pen-input` specs own gestures
- The behavior of each individual command — because the specs for those features (layers, palette, tilemaps, slices, drawing tools, …) own it; this spec only covers how commands enter and leave the history

## Acceptance criteria

- [x] AC-01 (event): When the latest edit is undone, the document's pixels and structure return to their state before that edit.
- [x] AC-02 (event): When an undone edit is redone, the document returns to its state after that edit.
- [x] AC-03 (event): When a new edit is made after one or more undos, the undone edits can no longer be redone.
- [x] AC-04 (always): The history holds at most its configured number of entries. An edit beyond that drops the oldest entry, and only the remaining entries can be undone.
- [x] AC-05 (always): The Pixel editor's history holds 100 entries. The 101st edit drops the oldest, and the undo depth stays at 100.
- [x] AC-06 (event): When the user paints a pencil or eraser stroke in one press-drag-release, the whole stroke is one undo entry, and undoing it clears every pixel of the stroke.
- [x] AC-07 (event): When a second stroke starts after the pointer was released, it becomes a separate undo entry.
- [x] AC-08 (event): When an edit follows an undo or a redo, it never merges into the entry that was just restored.
- [x] AC-09 (state): While the user paints, pixel writes to a different layer or a different frame never merge into the same undo entry.
- [x] AC-10 (event): When the user edits one layer's opacity, one palette entry's color, one frame's duration or one tag's frame range several times in a row within one gesture, the edits form one undo entry. Undoing that entry restores the value from before the first edit. Edits to a different layer, palette entry, frame or tag start a new entry.
- [x] AC-11 (event): When the user renames the same tag several times in a row within one gesture, the renames form one undo entry. Undoing it restores the name from before the first rename.
- [x] AC-12 (always): Each line, rectangle, ellipse, bucket fill, move of the selection content and tile placement is its own undo entry, even when two of the same kind follow each other without a stroke boundary.
- [x] AC-13 (always): These actions add a history entry and can be undone: adding, removing, reordering and renaming a layer; toggling a layer's visibility; adding a frame; editing the palette (add, recolor, move, delete, apply a preset, import `.gpl`); deleting the selection's pixels; adding and removing a slice; adding a tileset, a tile or a tilemap layer; placing a tile; editing a tile's pixels.
- [x] AC-14 (state): The selection is not part of the history. Setting, replacing or clearing a selection adds no entry, and undoing an edit leaves the current selection as it is.
- [x] AC-15 (always): Picking a color with the eyedropper adds no history entry.
- [x] AC-16 (always): Switching the current frame, choosing the active layer and changing the onion-skin settings add no history entry.
- [x] AC-17 (event): When an undo removes the frame that is currently shown, the current frame moves to the last remaining frame.
- [x] AC-18 (error): If an edit fails, no history entry is added and the redo history is kept. Examples of failing edits are a pixel outside the canvas and an unknown layer.
- [x] AC-19 (state): While there is nothing to undo, Undo does nothing and the Undo button is disabled. The same holds for Redo. After an undo, the Redo button is enabled.
- [x] AC-20 (event): When a document is created or opened, its history starts empty.
- [x] AC-21 (event): When a document is saved, its history is kept, and edits made before the save can still be undone.
- [x] AC-22 (event): When the user presses Ctrl/Cmd+Z the last edit is undone. Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y redoes it.
- [x] AC-23 (state): While a modal dialog is open, the undo shortcut does not reach the editor.

## Technical notes

Hints that drift; check the code before relying on them.

- **Command trait** (`crates/pincel-core/src/command/mod.rs`). Each command reports a `dirty_region()` (default `DirtyRegion::Canvas`), which the wasm layer turns into a render event.

  ```rust
  pub trait Command {
      fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError>;
      fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap);
      fn merge(&mut self, _next: &Self) -> bool where Self: Sized { false }
      fn dirty_region(&self) -> DirtyRegion { DirtyRegion::Canvas }
  }
  ```

- **Bus** (`command/bus.rs`): `Bus::new()` uses `DEFAULT_HISTORY_CAP = 100`, and `Bus::with_capacity(0)` turns history off.
  - `execute` applies the command, clears the redo stack, then merges into the top entry unless sealed, or pushes and drops the oldest past the cap.
  - `undo` and `redo` seal implicitly.
  - `seal()` marks a gesture boundary.
  - `AnyCommand` dispatches without trait objects.
- **Commands with a real `merge`:**
  - `SetPixel`: same layer and frame; appends the writes.
  - `SetLayerOpacity`: same layer.
  - `SetPaletteColor`: same index.
  - `SetFrameDuration`: same frame.
  - `SetTagRange` and `SetTagName`: same tag index.

  Every other command keeps the default `false`.
- **wasm** (`crates/pincel-wasm/src/lib.rs`):
  - `undo() -> bool`, `redo() -> Result<bool, String>`, `endStroke()` (calls `Bus::seal`), and the getters `undoDepth` and `redoDepth`.
  - After an undo or redo, `clamp_current_frame()` runs. Then `Event::from_dirty(bus.last_dirty_region())` queues a `dirty-rect` or `dirty-canvas` event.
  - The selection (`setSelection`, `select*`, `clearSelection`), `setCurrentFrame`, `setActiveLayer`, `setOnionSkin` and `pickColor` bypass the bus.
- **UI wiring** (`ui/src/modes/pixel/`):
  - `doc-ops.ts` `undo()` / `redo()` call the wasm methods, bump `docRev`, reconcile the active slice, layer and stamp, and on failure show `status.failed`.
  - `pointer.ts` calls `endStroke()` on pointer-up. `cancelGesture()` undoes back to the depth recorded at the start of the gesture.
  - `PixelEditor.svelte` provides the toolbar `IconButton`s (disabled at depth 0), the Edit menu items from `menus.ts`, the native menu events `menu:undo` / `menu:redo`, and the status-bar counter `px.undoRedo`.
  - Keys come from `ui/src/lib/shell/shortcuts.ts`: `undo: [{ key: 'z', mod: true }]`, `redo: [{ key: 'z', mod: true, shift: true }, { key: 'y', mod: true }]`.
  - Panels that edit continuously (palette color input, timeline durations) call `endStroke()` on commit so each commit is one entry.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Undo on an empty history | Returns `false`, changes nothing, queues no event |
| Two pencil stroke calls on the same layer and frame without `endStroke` in between | They merge into one entry, because the stroke boundary is the caller's job |
| Redo after the document was reopened | Not possible; the history starts empty |
| Undo of an "add frame" while viewing the added frame | The current frame clamps to the last remaining frame |
| Capacity 0 | Edits still apply, but nothing is recorded |
| Selection changed between two paint strokes, then undo | Only the paint is reverted; the selection stays |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `command_bus::set_pixel_undo_restores_buffer`, `command_bus::add_layer_and_undo_restores_layer_count` (crates/pincel-core/tests/command_bus.rs) | verified | 8550715 |
| AC-02 | `command_bus::redo_replays_undone_command` (crates/pincel-core/tests/command_bus.rs) | verified |  |
| AC-03 | `command_bus::execute_clears_redo_stack` (crates/pincel-core/tests/command_bus.rs) | verified |  |
| AC-04 | `command_bus::history_cap_drops_oldest_entries` (crates/pincel-core/tests/command_bus.rs) | verified |  |
| AC-05 | `undo_history::document_history_holds_100_entries_and_the_101st_drops_the_oldest` (crates/pincel-wasm/tests/undo_history.rs) | verified | 1cb73cc |
| AC-06 | `command_bus::execute_merges_consecutive_set_pixels_into_one_undo_entry` (crates/pincel-core/tests/command_bus.rs), `pincel_wasm::tests::apply_tool_drag_then_end_stroke_is_single_undo_entry` (crates/pincel-wasm/src/lib.rs), `ui/e2e/smoke.spec.ts › pixel mode: a pencil stroke is undoable` | verified |  |
| AC-07 | `command_bus::seal_prevents_merge_across_strokes` (crates/pincel-core/tests/command_bus.rs), `pincel_wasm::tests::two_strokes_are_two_undo_entries` (crates/pincel-wasm/src/lib.rs) | verified |  |
| AC-08 | `command_bus::undo_seals_history_against_merging`, `command_bus::edit_after_undo_does_not_merge_into_the_restored_top`, `command_bus::edit_after_redo_does_not_merge_into_the_restored_entry` (crates/pincel-core/tests/command_bus.rs) | verified | 8550715 |
| AC-09 | `pincel_core::command::set_pixel::tests::merge_rejects_different_layer_or_frame` (crates/pincel-core/src/command/set_pixel.rs) | verified |  |
| AC-10 | `pincel_core::command::set_layer_opacity::tests::consecutive_edits_merge_into_one_undo_entry_restoring_the_first_prior`, `…::edits_to_different_layers_do_not_merge` (crates/pincel-core/src/command/set_layer_opacity.rs), `pincel_core::command::palette_ops::tests::set_palette_color_applies_reverts_and_merges`, `…::set_palette_color_different_index_does_not_merge` (crates/pincel-core/src/command/palette_ops.rs), `pincel_core::command::set_frame_duration::tests::merge_same_frame_keeps_the_original_prior_value`, `…::merge_different_frame_is_rejected` (crates/pincel-core/src/command/set_frame_duration.rs), `pincel_core::command::tag_ops::tests::set_tag_range_merge_keeps_the_first_prior_range`, `…::set_tag_range_different_tag_does_not_merge` (crates/pincel-core/src/command/tag_ops.rs) | verified | 8550715 |
| AC-11 | `pincel_core::command::tag_ops::tests::set_tag_name_consecutive_renames_merge_into_one_undo_entry_restoring_the_first_name` (crates/pincel-core/src/command/tag_ops.rs) | verified | 216dce0 |
| AC-12 | `pincel_core::command::draw_line::tests::does_not_merge_with_another_draw_line`, `pincel_core::command::draw_rectangle::tests::does_not_merge_with_another_draw_rectangle`, `pincel_core::command::draw_ellipse::tests::does_not_merge_with_another_draw_ellipse`, `pincel_core::command::fill_region::tests::does_not_merge_with_another_fill_region`, `pincel_core::command::move_selection_content::tests::does_not_merge_with_another_move`, `pincel_core::command::place_tile::tests::does_not_merge_with_other_place_tile` (crates/pincel-core/src/command/) | verified |  |
| AC-13 | `pincel_wasm::tests::add_layer_is_undoable`, `…::remove_layer_drops_it_clears_active_and_undoes`, `…::move_layer_round_trips_through_undo`, `…::rename_layer_updates_name_and_undoes`, `…::set_layer_visible_toggles_and_undoes`, `…::add_frame_appends_and_is_undoable`, `…::palette_edits_route_through_undo`, `…::palette_presets_and_gpl_round_trip_through_undo`, `…::delete_selection_clears_pixels_and_undoes`, `…::add_slice_joins_undo_bus`, `…::remove_slice_drops_and_undo_restores`, `…::add_tileset_joins_undo_bus`, `…::add_tile_joins_undo_bus`, `…::add_tilemap_layer_joins_undo_bus`, `…::place_tile_joins_undo_bus`, `…::set_tile_pixel_joins_undo_bus` (crates/pincel-wasm/src/lib.rs), `undo_history::gpl_import_is_one_undoable_entry` (crates/pincel-wasm/tests/undo_history.rs), `ui/e2e/palette.spec.ts › palette: DB32 by default, add / recolor / move / delete with undo` | verified | 96040aa |
| AC-14 | `pincel_wasm::tests::selection_is_not_undoable_in_the_m7_8_slice` (crates/pincel-wasm/src/lib.rs), `undo_history::selection_changes_add_no_history_entry` (crates/pincel-wasm/tests/undo_history.rs) | verified | 96040aa |
| AC-15 | `pincel_wasm::tests::pick_color_does_not_disturb_command_bus` (crates/pincel-wasm/src/lib.rs) | verified |  |
| AC-16 | `undo_history::frame_layer_and_onion_skin_changes_add_no_history_entry` (crates/pincel-wasm/tests/undo_history.rs) | verified | 1cb73cc |
| AC-17 | `pincel_wasm::timeline::tests::undo_of_add_frame_clamps_the_current_frame` (crates/pincel-wasm/src/timeline.rs) | verified |  |
| AC-18 | `command_bus::failed_execute_adds_no_entry_and_keeps_redo_stack` (crates/pincel-core/tests/command_bus.rs), `undo_history::failed_edits_add_no_entry_and_keep_redo_history` (crates/pincel-wasm/tests/undo_history.rs) | verified | 216dce0 |
| AC-19 | `pincel_wasm::tests::undo_on_empty_stack_returns_false_and_emits_nothing`, `…::redo_on_empty_stack_returns_false_and_emits_nothing` (crates/pincel-wasm/src/lib.rs), `ui/e2e/smoke.spec.ts › pixel mode: a pencil stroke is undoable` | verified | 7668bf9 |
| AC-20 | `paint_save_open_roundtrip::paint_save_open_roundtrip_preserves_undo_target_state` (crates/pincel-wasm/tests/paint_save_open_roundtrip.rs) | verified |  |
| AC-21 | `undo_history::save_keeps_history_and_edits_before_it_stay_undoable` (crates/pincel-wasm/tests/undo_history.rs) | verified | 1cb73cc |
| AC-22 | `ui/e2e/smoke.spec.ts › pixel mode: Ctrl+Z undoes the last edit, Ctrl+Shift+Z and Ctrl+Y redo it` | verified | 617d168 |
| AC-23 | `ui/e2e/help.spec.ts › pixel mode: the About dialog holds the keyboard` | verified |  |

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering).
- 2026-10-29: The new tests for AC-05, AC-11, AC-16, AC-18, AC-21 and AC-22 were green at once because the behavior already existed. They were instead shown red against temporary mutations: history cap 101 (AC-05), `SetTagName::merge` overwriting the first prior name (AC-11), `set_onion_skin` calling `bus.undo` (AC-16), `Bus::execute` clearing the redo stack before `apply` (AC-18), and the Ctrl+Shift+Z binding removed from `shortcuts.ts` (AC-22). AC-21 has no mutation: `Document::save_aseprite` takes `&self`, so the compiler already rules out a save that touches the history, and the test pins that contract.
- 2026-10-29: The AC-13 `.gpl` and AC-14 selection gap tests were green at once. They go red when `import_gpl` assigns the palette directly instead of via `replace_palette`, and when `clear_selection` calls `bus.undo`.
- 2026-10-29: AC-08 and AC-10 gap tests were green at once (behavior exists). `edit_after_undo_does_not_merge_into_the_restored_top` goes red when `sealed = true` is removed from `Bus::undo` alone, and the two `*_does_not_merge` tests go red when the index guard is removed from `SetPaletteColor::merge` / `SetTagRange::merge`. `edit_after_redo_does_not_merge_into_the_restored_entry` stays green when only the seal in `Bus::redo` is removed, and goes red only with both seals removed. Through the public `Bus` API the redo seal is redundant: `sealed` is cleared only by a successful `execute`, which also empties the redo stack, so after any `undo` the flag stays set through every later `redo`. The test still pins the observable AC-08 behavior; it just cannot single out that one line.
- 2026-10-29: For AC-19, `smoke.spec.ts › pixel mode: a pencil stroke is undoable` now also asserts that the Redo button is disabled on a fresh document and after redoing to the end. It was green at once, and goes red when the Redo button's `disabled` is forced to `false` in `PixelEditor.svelte`.
- 2026-10-29: AC-16 and AC-21 are tested at the wasm surface (`crates/pincel-wasm/tests/undo_history.rs`), not in e2e, because the UI's frame, layer, onion-skin and save paths call those wasm methods directly and never touch the history themselves.
