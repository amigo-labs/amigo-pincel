---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §3.2, pincel.md §3.3, pincel.md §3.6, pincel.md §4.2, pincel.md §9.2]
---

# Pixel-mode layers and timeline

## Context

A Pixel-mode sprite is a stack of layers over a sequence of frames (`pincel.md` §3.2, §3.3), with named frame ranges (tags, §3.6) that a game's animation state machine reads by name. This spec records, as built, how the user edits that structure: the shared layers panel (§9.2) in its Pixel-mode form — a tree with groups — the Layer and Frame menus, and the timeline strip with frame durations, tags, playback and onion-skin controls.

The wasm `Document` is the source of truth; every structural edit is an undoable command in `pincel-core`. Already decided (Decision Log, §15): `MoveLayerTo { layer, target, place: Above | Below | Into }` moves a layer with its subtree across groups (2026-10-12); the onion-skin tint math (2026-10-11).

## Scope

### In scope

- Layers panel in Pixel mode: add, add group, delete, duplicate, merge down, flatten, rename (inline and Layer ▸ Rename…), visibility, lock, opacity, blend mode, thumbnails, drag and keyboard reordering, moving into and out of groups
- Layer menu entries and the active layer as paint target
- Frames: add, remove, select, step, per-frame duration
- Tags: add, edit (name, range, direction), delete, the identifier-name warning
- Playback over all frames or one tag, and the onion-skin controls in the timeline

### Out of scope

- How layers, groups and the onion skin are composited (blend math, tint, group visibility inside `compose()`) — because the `pixel-compose` spec owns `compose()`; this spec only covers that the editor's controls reach it.
- Folding a group's opacity and blend mode into its children, and linked cels — because the planned `linked-cels-group-compose` spec covers them.
- Reading and writing layers, frames and tags in `.aseprite` files — because the `aseprite-round-trip` spec owns the codec.
- Undo-stack mechanics (depth, redo invalidation, the Undo / Redo buttons) — because the `pixel-undo-history` spec owns them; here only "one edit = one undo step" is stated per action.
- Collapsing a group's children in the panel — because it is not built; the Aseprite collapsed flag is only preserved from files.
- Inserting a frame anywhere but the end, duplicating frames, linked cels from the UI — because `AddFrame` is append-only today.
- Tag colour and repeat-count editing — because the timeline exposes neither.
- Exporting by tag — because the `pixel-export` spec owns export.

## Acceptance criteria

- [ ] AC-01 (event): When the user adds a layer (Layers panel "Add layer" or Layer ▸ New Layer), then an empty image layer appears at the top of the stack, and one undo removes it again (redo restores it).
- [ ] AC-02 (event): When the user adds a layer, then it is named `Layer {n}` (n = number of layers after adding), becomes the active row, and the next stroke paints into it.
- [ ] AC-03 (state): While an image layer is the active row, the pixel tools paint into that layer; while a group or tilemap row is active, or no row is, they paint into the lowest image layer of the stack.
- [ ] AC-04 (event): When the user adds a group (Layers panel "Add group" or Layer ▸ New Group), then an empty group named `Group {n}` (n = number of groups after adding) appears at the top of the list.
- [ ] AC-05 (event): When the user deletes the active layer (Layers panel "Delete" or Layer ▸ Delete Layer), then it is removed together with its cels in every frame — for a group, its whole subtree — and one undo restores everything at its original position.
- [ ] AC-06 (state): While the stack holds a single layer, deleting a layer is unavailable in the panel and the Layer menu.
- [ ] AC-07 (event): When the user chooses Layer ▸ Rename…, enters a name and confirms, then the active layer carries that name, and one undo restores the previous name.
- [ ] AC-08 (event): When the user renames a layer inline from its row's rename button, then Enter commits the trimmed name as one undo step, Escape cancels, and an empty or unchanged name changes nothing.
- [ ] AC-09 (always): The layers panel lists layers top-most first as a tree: a group's row sits directly above its children, children are indented one level per enclosing group, and group and tilemap rows carry a kind badge.
- [ ] AC-10 (event): When the user drags a layer's grip onto another row and releases, then the layer (with its subtree) moves to that row's slot, and the whole drag is one undo step.
- [ ] AC-11 (event): When the user drops a layer on the middle of a group's row, then it becomes that group's top child (indented one level deeper); when dropped on the top quarter of a row, it lands above that row, sharing its parent; each drop is one undo step.
- [ ] AC-12 (event): When a layer's grip has focus, then ↑ / ↓ move the layer one row up / down, Alt+→ moves it into the nearest group above at the same depth, and Alt+← moves it out of its group to just above that group.
- [ ] AC-13 (always): A group always moves together with its whole subtree, and a moved subtree stays contiguous in the stack.
- [ ] AC-14 (error): If a move would put a group into itself or its own subtree, or "into" a layer that is not a group, then the move is refused and the document is unchanged.
- [ ] AC-15 (event): When the user toggles a layer's visibility, then the layer disappears from or reappears on the canvas as one undo step; hiding a group hides every layer nested in it, at any depth.
- [ ] AC-16 (event): When the user locks a layer and tries to paint on it, then nothing is painted and the status bar shows `Layer “{name}” is locked`; after unlocking, painting works again.
- [ ] AC-17 (state): While a group is locked, every layer nested in it refuses pixel edits like a locked layer, and its lock icon shows locked with the hint "Locked by its group" while its own lock flag stays off.
- [ ] AC-18 (event): When the user drags the active layer's opacity slider, then the layer's opacity follows the slider and the whole drag is one undo step.
- [ ] AC-19 (event): When the user picks a blend mode for the active layer from the panel's list (every blend mode the core supports, labelled in the UI language), then the layer uses it, as one undo step.
- [ ] AC-20 (event): When the user duplicates an image or tilemap layer, then a copy named `{source} copy`, with the source's cels in every frame, is inserted directly above the source, as one undo step.
- [ ] AC-21 (error): If the layer to duplicate is a group, then duplicating is refused and the document is unchanged.
- [ ] AC-22 (event): When the user merges the active image layer down, then its pixels are composited onto the image layer directly below it in the same group with the upper layer's opacity and blend mode, the upper layer is removed, the lower one becomes the paint target, and one undo restores both layers.
- [ ] AC-23 (error): If the active layer is the bottom layer, a group, or has no image layer directly below it in the same group, then merge down is refused and the document is unchanged.
- [ ] AC-24 (event): When the user flattens the image, then the stack is replaced by one image layer holding the visible composite of every frame (hidden layers are dropped), it becomes the paint target, and one undo restores the previous stack and cels exactly.
- [ ] AC-25 (always): Each layer row shows a thumbnail of that layer at the current frame, also while the layer is hidden, scaled down to fit and never scaled up.
- [ ] AC-26 (event): When the user adds a frame (timeline "Add frame" or Frame ▸ New Frame), then an empty frame of 100 ms is appended after the last frame and becomes the current frame, and one undo removes it.
- [ ] AC-27 (event): When the user selects a frame (its cell in the timeline, Previous / Next frame, the `,` / `.` keys or the Frame menu), then the canvas shows that frame, the timeline reads `frame {n}/{total}`, and the next stroke paints into that frame.
- [ ] AC-28 (event): When the user removes the current frame, then later frames shift down by one, tags shrink or shift with them (a tag covering only that frame is dropped), slice keys shift with them, the current frame stays within range, and one undo restores the exact prior state.
- [ ] AC-29 (error): If the user tries to remove the only frame, then the removal is refused and the document is unchanged.
- [ ] AC-30 (event): When the user enters a frame duration between 1 and 65535 ms and commits it, then the frame's duration becomes that value, and one undo restores the previous duration.
- [ ] AC-31 (error): If the user enters a duration of 0, above 65535 or not a number, then the frame keeps its previous duration.
- [ ] AC-32 (event): When the user adds a tag, then a forward-playing tag covering only the current frame, with the next colour from a fixed six-colour cycle, appears as a bar in the timeline and its tag editor opens; one undo removes it.
- [ ] AC-33 (event): When the user edits a tag's name, first frame, last frame or direction (forward, reverse, ping-pong, ping-pong reverse) in the tag editor, or deletes it there, then the tag bar updates and each edit is one undo step.
- [ ] AC-34 (state): While a tag's name is not identifier-like (letters, digits and `_`, not starting with a digit), the tag editor shows "Use letters, digits and _ only" and the tag bar shows a warning mark; the name is kept.
- [ ] AC-35 (error): If a tag would get an empty name, a first frame after its last frame, or a range past the last frame, then the edit is refused and the tag is unchanged.
- [ ] AC-36 (event): When the user presses Play (timeline button, Enter or Frame ▸ Play), then the frames of the chosen loop range — all frames, or the frames of the tag picked under "Loop range" — play repeatedly until Pause.
- [ ] AC-37 (option): Where the loop range is a tag, playback follows its direction: forward wraps from last to first, reverse runs from last to first and wraps, ping-pong and ping-pong reverse bounce at both ends without showing an end frame twice.
- [ ] AC-38 (state): While the document has fewer than two frames, Play is unavailable.
- [ ] AC-39 (event): When the user ticks Onion skin, then the frames-back and frames-forward counts (0–8) and the onion opacity become editable and the canvas shows the neighbouring frames under the current one; unticking removes them from the canvas.
- [ ] AC-40 (always): Onion-skin settings are view state: changing them adds no undo step and does not mark the document as changed.
- [ ] AC-41 (state): While playback runs, each frame stays on screen for its own duration; when a frame takes longer to show, later frames are skipped to keep wall-clock time rather than slowing the animation down.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

**Core commands** (`crates/pincel-core/src/command/`): `AddLayer` (`AddLayer::on_top`), `RemoveLayer` (drops the subtree and its cels), `DuplicateLayer` (name `"<source> copy"`, id chosen on first apply and reused on redo, groups rejected), `MergeDown` (immediate lower sibling, image only, uses upper opacity / blend / cel opacity), `FlattenImage`, `MoveLayer` (sibling step), `ReorderLayer` (to another layer's slot within the same group), `MoveLayerTo` (`LayerPlacement::{Above, Below, Into}`), `SetLayerVisible`, `SetLayerEditable`, `SetLayerName`, `SetLayerOpacity` (merges until `end_stroke`), `SetLayerBlendMode`, `AddFrame` (append only, no cels seeded), `RemoveFrame` (re-keys cels, materialises links to the removed frame, shifts tags and slice keys), `SetFrameDuration` (merges per frame), `AddTag` / `RemoveTag` / `SetTagRange` / `SetTagName` / `SetTagDirection` (`tag_ops.rs`). `Sprite::layer_locked(id)` is true when the layer or any ancestor group has `editable == false`.

**wasm** (`crates/pincel-wasm/src/lib.rs`, `layer_ops.rs`, `timeline.rs`): `addLayer(name)`, `addGroup(name)`, `removeLayer(id)`, `renameLayer`, `setLayerVisible`, `setLayerEditable`, `layerEditable` / `layerLocked`, `layerDepth`, `layerParent`, `reorderLayer(id, targetId)`, `moveLayerTo(id, targetId, "above" | "below" | "into")`, `moveLayerUp` / `moveLayerDown`, `setLayerOpacity(id, 0..=255)`, `setLayerBlendMode(id, name)`, `blendModeNames()`, `duplicateLayer`, `mergeDown`, `flattenImage`, `layerThumbnail(id, frame, maxSize)`, `setActiveLayer` / `paintTargetLayer` (`u32::MAX` = none); `addFrame(durationMs)`, `removeFrame`, `setCurrentFrame`, `frameDuration` / `setFrameDuration` (0 rejected), `addTag(name, from, to, color)`, `removeTag`, `setTagRange`, `setTagName`, `setTagDirection("forward" | "reverse" | "pingpong" | "pingpong_reverse")`, `setOnionSkin(enabled, back, forward, prevAlpha, nextAlpha)` (no undo entry).

**UI**: shared panel `ui/src/lib/components/layers/LayersPanel.svelte` (`LayerRow`, `DropPlace`; drop place from pointer height: top quarter `above`, group middle `into`, bottom quarter `below`), Pixel mapping `ui/src/modes/pixel/components/LayersPanel.svelte` (tree walk, `canMergeDown`, `canDuplicate`), actions in `ui/src/modes/pixel/doc-ops.ts`, lock refusal in `pointer.ts` (`lockedEditTarget`), Layer / Frame menus in `menus.ts`, timeline `components/TimelinePanel.svelte`, playback stepping `playback.ts` (`startCursor`, `advance`, `isIdentifierLike`), defaults in `stores/editor.svelte.ts` (`onion: { enabled: false, back: 1, forward: 1, alpha: 76 }`). Keys: `prevFrame` `,`, `nextFrame` `.`, `togglePlay` Enter (`ui/src/lib/shell/shortcuts.ts`).

**e2e**: `ui/e2e/layers.spec.ts` (helpers `layerNames`, `dragLayer`, `dropOnRow`, `layerRow`), `ui/e2e/timeline.spec.ts`. UI unit tests (Vitest) are not set up, so `playback.ts` stepping can only be proven through e2e today.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Delete a group that holds the active layer | The subtree goes; the active-layer pointer is cleared and painting falls back to the lowest image layer |
| Undo of an "Add layer" that was the active row | The row disappears and the active-layer highlight is cleared |
| Remove the current frame when it is the last frame | The current frame moves to the new last frame |
| Remove a frame before the current frame | The same content stays selected (current frame index drops by one) |
| Remove a frame that a linked cel points at | The link is materialised into a copy of the removed cel |
| Tag "From" typed beyond "To" | "To" is raised to "From" (and vice versa), so the range never inverts |
| Frame number typed past the last frame in the tag editor | Clamped to the last frame |
| Merge down with no cel on the upper layer in some frame | Those frames are left untouched |
| Thumbnail of a 64×32 layer at 16 px | 16×8, nearest-neighbour; at 128 px it stays 64×32 |
| Deleting the tag that playback loops over | Loop range falls back to all frames |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-wasm: tests::add_layer_appends_and_returns_monotonic_id`, `tests::add_layer_is_undoable`; `pincel-core: command::add_layer::tests::apply_appends_layer_when_no_index_given`, `command::add_layer::tests::revert_removes_inserted_layer` | built | |
| AC-02 |      | open   |        |
| AC-03 | `pincel-wasm: tests::pencil_paints_into_active_layer_cel`, `tests::paint_target_resolves_active_image_layer`, `tests::paint_target_falls_back_for_non_image_or_unknown_active` | built | |
| AC-04 | `ui/e2e/layers.spec.ts › pixel: drag a layer into a group and back out` | built | |
| AC-05 | `pincel-core: command::remove_layer::tests::remove_leaf_layer_and_restore_it`, `command::remove_layer::tests::remove_layer_drops_its_cels_across_frames_and_revert_restores`, `command::remove_layer::tests::removing_a_group_takes_its_whole_subtree`; `pincel-wasm: tests::remove_layer_drops_it_clears_active_and_undoes` | built | |
| AC-06 |      | open   |        |
| AC-07 | `ui/e2e/layers.spec.ts › pixel: Layer ▸ Rename… renames the active layer` | built | |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 | `ui/e2e/layers.spec.ts › pixel: drag the grip to reorder layers`; `pincel-wasm: layer_ops::tests::reorder_layer_is_one_undo_step` | built | |
| AC-11 | `ui/e2e/layers.spec.ts › pixel: drag a layer into a group and back out`; `pincel-core: command::move_layer_to::tests::move_into_group_makes_it_the_top_child_and_undo_restores`, `command::move_layer_to::tests::move_out_of_group_above_a_root_layer` | built | |
| AC-12 |      | open   |        |
| AC-13 | `pincel-core: command::move_layer::tests::moving_a_group_carries_its_children`, `command::move_layer_to::tests::a_group_moves_with_its_children_into_another_group`, `command::reorder_layer::tests::apply_moves_down_past_a_group_as_a_block` | built | |
| AC-14 | `pincel-core: command::move_layer_to::tests::moving_into_own_subtree_or_a_non_group_errs_without_change`; `pincel-wasm: layer_ops::tests::move_layer_into_a_new_group_and_back_out_undoes_in_one_step_each` | built | |
| AC-15 | `pincel-wasm: tests::set_layer_visible_toggles_and_undoes`; `pincel-core: render::compose::tests::invisible_layer_is_skipped`, `render::compose::tests::hidden_group_hides_child_layers`, `render::compose::tests::hidden_grandparent_hides_nested_child` | built | |
| AC-16 | `ui/e2e/layers.spec.ts › pixel: a locked layer refuses paint`; `pincel-wasm: layer_ops::tests::locked_layer_refuses_paint_until_unlocked` | built | |
| AC-17 |      | open   |        |
| AC-18 | `pincel-wasm: layer_ops::tests::set_layer_opacity_round_trips_and_merges_slider_ticks`; `pincel-core: command::set_layer_opacity::tests::consecutive_edits_merge_into_one_undo_entry_restoring_the_first_prior` | built | |
| AC-19 | `pincel-wasm: layer_ops::tests::set_layer_blend_mode_by_name_and_reject_unknown`, `layer_ops::tests::blend_mode_names_lists_all_modes` | built | |
| AC-20 | `pincel-core: command::duplicate_layer::tests::apply_inserts_copy_above_source_with_cels_and_revert_removes_it`; `pincel-wasm: layer_ops::tests::duplicate_layer_returns_new_id_and_copies_pixels` | built | |
| AC-21 | `pincel-core: command::duplicate_layer::tests::group_layer_is_rejected` | built | |
| AC-22 | `pincel-wasm: layer_ops::tests::merge_down_combines_layers_and_moves_active_target`; `pincel-core: command::merge_down::tests::apply_uses_upper_layer_opacity`, `command::merge_down::tests::apply_uses_upper_blend_mode`, `command::merge_down::tests::revert_restores_both_layers_and_cels_exactly` | built | |
| AC-23 | `pincel-core: command::merge_down::tests::bottom_layer_group_and_cross_parent_are_rejected` | built | |
| AC-24 | `pincel-core: command::flatten_image::tests::apply_leaves_one_layer_with_the_visible_composite_per_frame`, `command::flatten_image::tests::undo_restores_layers_and_cels_exactly_and_redo_reflattens`; `pincel-wasm: layer_ops::tests::flatten_image_leaves_one_layer_with_the_composite` | built | |
| AC-25 | `pincel-wasm: layer_ops::tests::layer_thumbnail_fits_and_shows_hidden_layers` | built | |
| AC-26 | `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin`; `pincel-wasm: tests::add_frame_appends_and_is_undoable`, `tests::paint_auto_creates_cel_on_a_fresh_frame` | built | |
| AC-27 |      | open   |        |
| AC-28 | `pincel-core: command::remove_frame::tests::apply_middle_frame_shifts_later_cels_down`, `command::remove_frame::tests::apply_adjusts_tags_and_drops_single_frame_tag`, `command::remove_frame::tests::apply_shifts_slice_keys_and_keeps_the_removed_frames_key`, `command::remove_frame::tests::revert_restores_frames_and_cels_exactly`; `pincel-wasm: timeline::tests::remove_frame_clamps_the_current_frame`; `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin` | built | |
| AC-29 | `pincel-core: command::remove_frame::tests::apply_last_remaining_frame_returns_err`; `pincel-wasm: timeline::tests::remove_last_remaining_frame_errors` | built | |
| AC-30 | `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin`; `pincel-wasm: timeline::tests::set_frame_duration_round_trips_through_undo` | built | |
| AC-31 |      | open   |        |
| AC-32 |      | open   |        |
| AC-33 | `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin`; `pincel-wasm: timeline::tests::tag_lifecycle_through_the_surface`; `pincel-core: command::tag_ops::tests::set_tag_range_applies_and_reverts`, `command::tag_ops::tests::set_tag_name_applies_and_reverts`, `command::tag_ops::tests::set_tag_direction_applies_and_reverts`, `command::tag_ops::tests::remove_tag_round_trips` | built | |
| AC-34 |      | open   |        |
| AC-35 | `pincel-core: command::tag_ops::tests::set_tag_name_empty_returns_err`, `command::tag_ops::tests::set_tag_range_inverted_returns_err`, `command::tag_ops::tests::add_tag_with_range_past_last_frame_returns_err`, `command::tag_ops::tests::add_tag_with_empty_name_returns_err` | built | |
| AC-36 | `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin` | built | |
| AC-37 |      | open   |        |
| AC-38 |      | open   |        |
| AC-39 | `ui/e2e/timeline.spec.ts › timeline: frames, durations, tags, playback and onion skin`; `pincel-wasm: timeline::tests::onion_skin_toggles_and_composes`; `pincel-core: render::onion::tests::previous_frame_shows_red_tinted_under_a_hole`, `render::onion::tests::next_frame_is_blue_tinted_at_next_alpha` | built | |
| AC-40 |      | open   |        |
| AC-41 |      | open   |        |

## Decisions

