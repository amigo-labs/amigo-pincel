---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §3.5, pincel.md §5.2, pincel.md §9.2]
---

# Slices

## Context

Slices are named rectangles over the sprite that a game engine reads for UI scaling (9-patch), hit boxes and anchor points (pivot) (`pincel.md` §3.5). A slice has a colour for the editor overlay and one or more keys; a key applies from its frame until the next key's frame (Aseprite semantics). This spec records, as built, how the Pixel-mode user creates and edits slices with the Slice tool (§5.2) and the Slices panel (§9.2), and how slice keys follow frame edits.

The UI edits only the key at the first frame; keys at other frames arrive from opened files or the wasm API and are preserved.

## Scope

### In scope

- Slice tool: drag out a new slice, redraw the active slice's bounds
- Slices panel: add, activate, remove, edit bounds, 9-patch center and pivot of the first-frame key
- The active slice's overlay on the canvas
- Per-frame key semantics in the document (upsert by frame, frame-ordered keys) and keys following frame removal
- Undo of every slice edit

### Out of scope

- Editing keys at frames other than the first in the UI — because the panel and the Slice tool only address the first-frame key today (multi-frame slice editing is in the backlog).
- Renaming a slice or changing its colour after creation — because no command or UI for it exists.
- Resizing a slice by dragging its corners and a double-click properties dialog (§5.2) — because the Slice tool only redraws the whole rectangle today.
- Reading and writing slice chunks in `.aseprite` — because the `aseprite-round-trip` spec owns the codec.
- Slices in sprite-sheet JSON and in canvas transforms — because the `pixel-export` and image-operation areas own them.

## Acceptance criteria

- [ ] AC-01 (event): When the user drags on the canvas with the Slice tool while no slice is active, then a slice named `Slice {n}` (n = number of slices after adding), in the current foreground colour, covering the dragged rectangle including both corner pixels, is created and becomes the active slice; one undo removes it.
- [ ] AC-02 (event): When the user clicks the canvas with the Slice tool without dragging while no slice is active, then a 1 × 1 slice is created at that pixel.
- [ ] AC-03 (always): A newly created slice has exactly one key, at the first frame, with the given bounds and neither a 9-patch center nor a pivot.
- [ ] AC-04 (event): When the user drags with the Slice tool while a slice is active, then that slice's first-frame bounds become the dragged rectangle, its 9-patch center and pivot keep their coordinates relative to the bounds origin (so they move with the slice), and one undo restores the previous bounds.
- [ ] AC-05 (event): When the user submits the Slices panel's Add slice form, then a slice with the entered name and colour covering the whole canvas is created and becomes the active slice; one undo removes it.
- [ ] AC-06 (always): The Add slice form opens with the name `Slice` and the colour `#3b82f6`.
- [ ] AC-07 (error): If the Add slice form's name is empty or only spaces, then the form shows "name is required" and no slice is created.
- [ ] AC-08 (event): When the user clicks a slice's row, then that slice becomes the active slice (highlighted) and the Slice tool becomes the active tool; clicking the active slice's row again deactivates it.
- [ ] AC-09 (state): While a slice is active, the canvas shows marching ants around its first-frame bounds, an outline of its 9-patch center in the slice's colour when it has one, and a crosshair at its pivot when it has one — center and pivot drawn relative to the slice bounds' origin; none of this becomes part of the sprite's pixels.
- [ ] AC-10 (event): When the user commits a new X, Y, W or H value in a slice's row, then the slice's first-frame bounds take that value, as one undo step.
- [ ] AC-11 (error): If the user enters a width or height (of the bounds or the 9-patch center) below 1, then the row shows "{field} must be a positive integer" and the slice is unchanged.
- [ ] AC-12 (event): When the user ticks 9-patch on a slice without a center, then a center inset by 1 px on every side of the bounds is set — `(1, 1, w − 2, h − 2)` relative to the slice bounds, at least 1 × 1 — as one undo step; its X, Y, W and H are then shown and edited relative to the slice bounds, and unticking removes the center.
- [ ] AC-13 (event): When the user ticks Pivot, then a pivot at the slice's top-left corner — `(0, 0)` relative to the slice bounds — is set, as one undo step; its X and Y are then shown and edited relative to the slice bounds, negative values included, and unticking removes the pivot.
- [ ] AC-14 (error): If a key would get empty bounds, only part of a 9-patch center (fewer than all of x, y, width, height), or only one pivot coordinate, then the edit is refused and the slice is unchanged.
- [ ] AC-15 (event): When the user removes a slice from its row, then it disappears from the panel and the canvas, an active slice is deactivated, and one undo restores it at its previous position in the list.
- [ ] AC-16 (event): When undo removes the active slice, then the active-slice overlay and highlight disappear.
- [ ] AC-17 (always): A slice's keys are kept sorted by frame: setting a key for a frame that already has one replaces it, and setting one for a frame without a key inserts it at its frame position; each key applies from its frame until the next key's frame.
- [ ] AC-18 (event): When a frame is removed, then keys at later frames move one frame earlier, and a key on the removed frame moves to the frame that takes its place unless that frame already has its own key, in which case the removed frame's key is dropped; one undo restores the original keys.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

**Document model** (`crates/pincel-core/src/document/slice.rs`): `Slice { id: SliceId, name, color: Rgba, keys: Vec<SliceKey> /* sorted by frame */, user_data }`, `SliceKey { frame: FrameIndex, bounds: Rect, center: Option<Rect>, pivot: Option<(i32, i32)> }`. `center` and `pivot` are relative to the key's bounds origin, as the Aseprite format stores them (`crates/aseprite-writer/src/file.rs`); the codec copies them unchanged. The core doc comment still says the pivot is "in sprite coordinates" and the panel and overlay still treat both as absolute — both are fixed to the relative space (the panel seeds center `(1, 1, w − 2, h − 2)` and pivot `(0, 0)`, the overlay adds the bounds origin).

**Commands** (`crates/pincel-core/src/command/`): `AddSlice` (appends; rejects duplicate ids, empty key lists, empty bounds), `RemoveSlice` (re-inserts at the prior index on undo), `SetSliceKey` (upsert by frame), `RemoveFrame` (shifts slice keys). None of the slice commands merge, so every edit is its own undo entry.

**wasm** (`crates/pincel-wasm/src/lib.rs`, "M9.4"): `sliceCount`, `sliceIdAt(i)`, `sliceName`, `sliceColor` (`0xRRGGBBAA`), `sliceKeyCount`, `sliceKeyFrame / X / Y / Width / Height`, `sliceKeyHasCenter`, `sliceKeyCenterX / Y / Width / Height`, `sliceKeyHasPivot`, `sliceKeyPivotX / Y`, `addSlice(name, x, y, w, h, color)` (one key at frame 0; id = max + 1; zero size rejected), `removeSlice(id)`, `setSliceKey(id, frame, x, y, w, h, cx?, cy?, cw?, ch?, px?, py?)` (center all-or-none, pivot both-or-none).

**UI**: `ui/src/modes/pixel/components/SlicesPanel.svelte` (form, rows, `commit`, `toggleCenter`, `togglePivot`, `UNSIGNED_FIELDS` guard), Slice tool drag in `ui/src/modes/pixel/pointer.ts` (`commitSliceDrag`), overlay in `overlay-painter.ts` (`paintActiveSlice`), stale active slice cleared in `doc-ops.ts` (`reconcileActiveSlice`); the panel is collapsed by default in `PixelEditor.svelte`. The slice overlay and the Slice tool ignore layer locks.

**e2e**: `ui/e2e/slices.spec.ts`; slice state is read from the panel rows, the overlay test-side from the overlay canvas next to the `Pincel canvas`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Drag from bottom-right to top-left | Same slice as the opposite drag: bounds start at the smaller corner |
| Slice whose only key is at a later frame (from a file) | The panel and overlay fall back to its first stored key |
| Clearing a row's X or Y field | Treated as 0 and applied |
| Clearing a row's W or H field | Treated as 0, refused with "must be a positive integer" |
| Redrawing the active slice so its 9-patch center lies outside the new bounds | Accepted; the center keeps its relative coordinates |
| Slice with a 9-patch center and pivot opened from an Aseprite file | Overlay and panel show them at the same place as Aseprite does; saving writes the values back unchanged |
| Removing the active slice | The overlay stops and no slice is active |
| Undo of a slice creation after other edits to it | Each edit undoes on its own first, then the creation |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

- Canvas operations still treat a key's 9-patch center and pivot as sprite coordinates: `ReframeCanvas` shifts them by the canvas offset along with the bounds, and `TransformCanvas` / `ScaleImage` orient or scale them against the canvas size, not the key's bounds. Under the bounds-relative model (Decisions 2026-10-29) a resize or flip moves them twice or to the wrong place. Out of scope here (image operations own canvas transforms); needs a decision there.
- After a refused W / H (or 9-patch width / height) entry the field keeps showing the refused value (e.g. `0`) until that size changes in the document, so the row reads differently from the slice. Should a refused entry snap back to the stored value?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/slices.spec.ts › a Slice-tool drag creates the active slice "Slice n" in the foreground color` | built | |
| AC-02 | `ui/e2e/slices.spec.ts › a Slice-tool click creates a 1 × 1 slice at that pixel` | built | |
| AC-03 | `pincel-wasm: tests::add_slice_appends_with_single_key_at_frame_zero`, `tests::add_slice_joins_undo_bus` | built | |
| AC-04 | `ui/e2e/slices.spec.ts › redrawing the active slice moves its bounds, 9-patch and pivot along` | built | |
| AC-05 | `ui/e2e/slices.spec.ts › the Add slice form creates a whole-canvas slice` | built | |
| AC-06 | `ui/e2e/slices.spec.ts › the Add slice form creates a whole-canvas slice` | built | |
| AC-07 | `ui/e2e/slices.spec.ts › the Add slice form creates a whole-canvas slice` | built | |
| AC-08 | `ui/e2e/slices.spec.ts › clicking a row activates the slice and the Slice tool, clicking again deactivates` | built | |
| AC-09 | `ui/e2e/slices.spec.ts › overlay › the active slice shows ants, its 9-patch center and pivot, off the sprite`; `ui/e2e/compose.spec.ts › pixel mode: the tile-stamp hover and the active slice stay off the sprite` | built | |
| AC-10 | `ui/e2e/slices.spec.ts › row X / Y / W / H commits are one undo step each, sizes below 1 are refused` | built | |
| AC-11 | `ui/e2e/slices.spec.ts › row X / Y / W / H commits are one undo step each, sizes below 1 are refused`, `› the slice rows follow the UI language` | built | |
| AC-12 | `ui/e2e/slices.spec.ts › ticking 9-patch insets a center by 1 px, relative to the bounds` | built | |
| AC-13 | `ui/e2e/slices.spec.ts › ticking Pivot sets it at the top-left, relative to the bounds` | built | |
| AC-14 | `pincel-wasm: tests::set_slice_key_rejects_empty_bounds`, `tests::set_slice_key_rejects_partial_center_quartet`, `tests::set_slice_key_rejects_partial_pivot_pair`, `tests::add_slice_rejects_zero_size`; `pincel-core: command::set_slice_key::tests::empty_bounds_rect_is_rejected`, `command::add_slice::tests::empty_bounds_rect_is_rejected` | built | |
| AC-15 | `pincel-wasm: tests::remove_slice_drops_and_undo_restores`; `pincel-core: command::remove_slice::tests::revert_reinserts_at_same_index` | built | |
| AC-16 | `ui/e2e/slices.spec.ts › overlay › undo of the active slice clears its overlay and highlight` | built | |
| AC-17 | `pincel-core: command::set_slice_key::tests::apply_replaces_existing_key_for_same_frame`, `command::set_slice_key::tests::apply_inserts_new_key_at_sorted_position`, `command::set_slice_key::tests::revert_after_replace_restores_prior_key`, `command::set_slice_key::tests::revert_after_insert_removes_inserted_key`; `pincel-wasm: tests::set_slice_key_replaces_existing_frame_zero_key`, `tests::set_slice_key_inserts_new_frame_in_sorted_order` | built | |
| AC-18 | `pincel-core: command::remove_frame::tests::apply_shifts_slice_keys_and_keeps_the_removed_frames_key`, `command::remove_frame::tests::apply_drops_removed_frames_key_when_next_frame_has_one`, `command::remove_frame::tests::revert_restores_frames_and_cels_exactly` | built | |

## Decisions

- 2026-10-29: A key's 9-patch center and pivot are relative to the key's bounds origin in the core, the panel and the overlay (AC-04, AC-09, AC-12, AC-13 changed; the core doc comment gets fixed) — this matches the Aseprite format, so files round-trip correctly and a redrawn slice keeps its 9-patch and pivot attached (owner accepted the recommendation).
- 2026-11-03: Bug found by the new tests: the Slices panel still seeded a new center at `(x + 1, y + 1)` and a new pivot at `(x, y)`, and the overlay drew both at their stored values, i.e. in sprite space — the AC-04, AC-09, AC-12 and AC-13 tests were red (seeded `cX` 3 for `1`, pivot `pX` 5 for `0`, no crosshair at the bounds-relative point). Fixed in the panel and `paintActiveSlice` (`fix(ui)`); the core and wasm doc comments now say "relative to the bounds origin".
- 2026-11-03: Bug: the panel's field labels (X, Y, W, H, cX…pY), the bounds legend and the key-info line were hard-coded English, and the AC-11 message named the internal key (`w must be…` next to a field labelled W). They now go through `t()` (German width is `B`), and the message names the field as labelled; the test `the slice rows follow the UI language` and the AC-11 message checks were red before (`fix(ui)`).
- 2026-11-03: The other open ACs (01, 02, 05–08, 10, 11, 16) describe behaviour that already existed, so their tests were green at once. Each was shown to fail against a temporary, reverted break: the auto name counting the slices before adding (AC-01), a 1-px wider rect (AC-02), the redraw shifting `cX` by one (AC-04), the form not activating its slice (AC-05), the default colour `#3b82f5` (AC-06), the name not trimmed (AC-07), activation not selecting the Slice tool (AC-08), row commits writing frame 1 (AC-10), W missing from the unsigned guard (AC-11) and `reconcileActiveSlice` doing nothing (AC-16).
