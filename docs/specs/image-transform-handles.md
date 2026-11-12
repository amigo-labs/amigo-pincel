---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6.1, pincel.md §6.2, pincel.md §4.5, pincel.md §15 (2026-10-28 Image paste as a new layer), pincel.md §15 (2026-11-05 MCP tool parity), pincel.md §15 (2026-11-05 History entries carry a kind from the core)]
depends_on: [image-live-objects, image-transform-handles-rotate-zoom]
---

# Image transform handles: scale and rotate the selection

## Context

paint.net's Move Selected Pixels and Move Selection tools show a bounding box with handles around the selection: corner and edge handles scale it, a right-drag (or a drag outside the box) rotates it about a movable pivot, and the transform stays live — editable by further drags — until it is finished. Pincel's Image mode only translates: `CommandSpec::TranslateSelection` (pixels and outline, one `MoveSelection` command) and `TranslateSelectionMask` (outline only) take an integer `(dx, dy)` that the UI computes on pointer release; during the drag nothing on the canvas moves (`image-editing` AC-21). The owner wants paint.net parity for Image mode.

This spec adds the handles, scaling, rotation, a resampling quality option and a live transform that is committed as one undo step, for both Move variants that act on a selection. The resampler for rotation and scaling is built by its sibling spec `image-transform-handles-rotate-zoom` (Layer ▸ Rotate / Zoom), which can ship first because it needs no new architecture.

Two owner decisions (2026-11-05, see Decisions) shape this spec:

- `pincel.md` §5.4 says "transforming [a selection] is Phase 2 (§13)", and Decision Log 2026-10-28 deferred paint.net's floating paste for the same reason. Selection transforms are lifted into the current phase for Image mode only.
- A live transform follows the model of `image-live-objects` (shapes, text and gradient stay editable until commit): the core applies the live transform to the layer at every change, the transform's settings live beside the document on the command bus (never in the saved file), each handle drag is a normal undo step, commit folds those steps into one, cancel removes them, and any other command or a change of the active layer commits first. Escape cancels and Enter commits, the same rule for every live object.

## Scope

### In scope

- A bounding box with four corner and four edge handles and a pivot marker around the selection, for Move Selected Pixels and Move Selection
- Translate (drag inside), scale (corner / edge handles, Shift keeps the aspect ratio), rotate (right-drag anywhere, or left-drag outside the box; Shift snaps to 15°), moving the pivot
- A live transform: the core applies it to the layer at every change, each handle drag is an undo step while it is live
- Commit by Enter, by choosing another tool, by a click outside the box, and implicitly before any other command or active-layer change; cancel by Escape
- One undo step per committed transform (the drag steps folded), for pixels and outline together
- A resampling quality option (Nearest / Bilinear / Bicubic) in the options bar of both tools
- Move Selected Pixels' live translate now following the pointer during the drag (today the content jumps on release)
- en + de strings for every new label and the history step kinds

### Out of scope

- Layer ▸ Rotate / Zoom — because the sibling spec `image-transform-handles-rotate-zoom` covers it
- Handles for Move Layer (no selection) — because paint.net's Move Selected Pixels without a selection moves the whole layer without handles, and the topic asks for the selection tools
- Flipping by dragging a handle past the opposite side — because Layer ▸ Flip already mirrors, and a negative scale complicates the pivot and readout; the box stops at 1 px
- Skew / perspective / free distort handles — because paint.net's Move tools offer none
- Typed numeric entry of position, size and angle in the options bar — because the topic asks for handles; a read-only readout is enough for this spec
- Pasting into the current layer as a floating, transformable selection — because Decision Log 2026-10-28 chose paste as a new layer; switching that is a separate product decision this spec makes possible but does not take
- MCP tools for the handle transform — because `mcp-parity-selection-transform` owns selection and transform tools (Decision Log 2026-11-05 "MCP tool parity"); this spec delivers the core command that tool will call
- Pixel mode — because Pixel mode has its own Move tool (`pixel-selection-move`) and RotSprite (`rotsprite`); arbitrary transforms of pixel art are a separate decision
- Arrow-key nudging — because it is not built for any Move tool yet

## Acceptance criteria

- [ ] AC-01 (state): While Move Selected Pixels or Move Selection is the active tool and a selection with at least one selected pixel exists, the canvas shows a box around the selection's bounds with four corner handles, four edge handles and a pivot marker at its center; while a transform is live the box is drawn turned by the transform's angle around its content; with no selection, or with the other tools, no box is shown.
- [ ] AC-02 (event): When the user drags inside the box, then the selection's content (Move Selected Pixels) or outline (Move Selection) follows the pointer during the drag, not only on release, and the pivot moves with it.
- [ ] AC-03 (event): When the user drags a corner handle, then the box scales along its own two axes (turned with the box after a rotation) with the opposite corner fixed; an edge handle scales along its axis only with the opposite edge fixed; the box never becomes narrower or shorter than 1 px; the pivot keeps its position relative to the box (a centered pivot stays centered).
- [ ] AC-04 (option): Where Shift is held (the key or the on-screen Shift toggle) while a corner handle is dragged, the box keeps the width-to-height ratio it had when the transform started.
- [ ] AC-05 (event): When the user right-drags anywhere on the stage, or left-drags starting outside the box farther than a handle's hit area, then the content turns about the pivot by the angle the pointer sweeps around it, positive clockwise; with Shift held the angle snaps to multiples of 15°.
- [ ] AC-06 (event): When the user drags the pivot marker, then the pivot moves and no pixel or outline changes; later rotations turn about the new pivot; the pivot drag is an undo step like any other drag.
- [ ] AC-07 (state): While a transform is live (from the first handle, inside, rotation or pivot drag until commit or cancel), the layer holds the transformed content with transparency where the lifted pixels were, the selection is the transformed outline, the options bar shows the box's position, size and angle, and every finished drag and every quality change has added one undo step, consecutive quality changes with no other step between them forming one.
- [ ] AC-08 (event): When the user presses Enter, chooses another tool, clicks outside the box (presses and releases with the pointer moving less than 4 screen pixels), runs any other command (a menu command, a layer property change, Deselect, an effect, an MCP call), begins another live object, or changes the active layer, then the live transform is committed first and the other action runs on the committed result.
- [ ] AC-09 (event): When a live transform is committed, then its steps are replaced by exactly one undo step, however many drags it took; Undo of that step restores the layer's pixels and the selection outline as they were before the transform, and Redo reapplies the same result.
- [ ] AC-10 (event): When the user presses Escape while a transform is live, then the transform is cancelled: its steps are removed, and pixels, outline and undo history (undo and redo depth included) are as they were before it started; Escape without a live transform keeps deselecting as before (`image-editing` AC-31).
- [ ] AC-11 (always): The committed result depends only on the final box, angle, pivot and quality, not on the sequence of drags that reached them: scaling to 25 % and back to 100 %, or rotating by +30° and then −30°, and committing leaves every pixel and the outline as before the transform (no accumulated resampling loss).
- [ ] AC-12 (option): Where the quality is Nearest and the selection has no feathered edge, every pixel of the committed result has a color (alpha included) that occurs in the lifted pixels or in the active layer's pixels the transform does not cover, or is fully transparent; Bilinear (the default) and Bicubic smooth the edges; changing the quality while a transform is live re-applies the transform with the new quality.
- [ ] AC-13 (always): Transformed pixels that lie beyond the canvas edge while the transform is live are not lost: dragging them back onto the canvas shows them again exactly as if they had never left; on commit they are dropped and the layer keeps the canvas size.
- [ ] AC-14 (always): Move Selected Pixels commits the transformed pixels composited over what lies at the target in the same layer (as the plain move does, `image-editing` AC-21), and the selection becomes the transformed outline, so a second transform carries the transformed patch on.
- [ ] AC-15 (event): When the user commits a Move Selection transform, then only the selection outline changes — scaled, rotated and moved as shown — and every layer keeps its pixels.
- [ ] AC-16 (error): If the active layer is locked (itself or through its group) or is a group, then Move Selected Pixels shows the locked-layer notice on the first press and starts no transform; Move Selection still works there, because it changes no pixels.
- [ ] AC-17 (error): If the selection selects no pixel (e.g. after Contract removed everything), then no box is shown and nothing starts; if a transform is committed whose pixels and outline equal those before it started (only the pivot moved, a full 360° turn, or every drag undone), then its steps are removed and no step is added, as `image-live-objects` AC-12 does for a live object.
- [ ] AC-18 (event): When the user switches to another tab while a transform is live, then the transform is committed in its document before the switch. Saving, exporting, autosave and copying while a transform is live contain the transformed pixels and outline as currently applied, do not commit it and do not change the undo history, and never contain the transform's settings; a project saved while live reopens with plain pixels and no live transform.
- [ ] AC-19 (always): Each committed transform appears in the history as one step named "Move Selected Pixels" or "Move Selection" in English and German, each live step is named the same, and the committed pixels and outline equal those the core produces for the same box, angle, pivot and quality without the UI.
- [ ] AC-20 (always): One live update — the core applying a changed box, angle, pivot or quality to the layer — of a 2048×2048 selection on a 3840×2160 document with 10 layers takes no longer than the §4.5 budget row "Live transform update, Image 4K, 2048² selection: ≤ 33 ms" (Decision Log 2026-11-05), measured by the browser performance script.
- [ ] AC-21 (event): When the user presses Undo (Ctrl+Z / ⌘Z) while a transform is live, then only the last step is undone and the transform stays live with the box, angle, pivot and quality from before that step; undoing the first drag leaves the layer and outline as before the transform and no transform live; Redo reapplies a step undone this way.
- [ ] AC-22 (always): The undo capacity never drops a live transform's steps and Escape still restores the pixels and outline from before the transform, following `image-live-objects` AC-14: while the history is full the oldest steps below the transform are dropped first, and when its own steps exceed the capacity its oldest steps fold into the first one.

## Technical notes

> Paths are hints from 2026-11-05 and drift — grep for the names.

**Current state**

- `crates/pincel-image-core/src/tools/move_tool.rs` — `Move::translate`, `Move::translate_selection` (lifts by coverage: feathered edges move their share).
- `crates/pincel-image-core/src/command/selection.rs` — `MoveSelection` (pixels + mask, one step), `SetSelection`; `SelectionMask::translated` in `selection/mod.rs`.
- `crates/pincel-image-core/src/command/spec.rs` — `CommandSpec::TranslateSelection` / `TranslateSelectionMask`; `edited_layer` / `refused` gate the locked-layer notice.
- `crates/pincel-image-core/src/transform/mod.rs` — `Interpolation { Nearest, Bilinear, Bicubic }` and `scale`; its doc comment mentions rotation by arbitrary angles, but no rotation function exists yet. `pincel-effects` `distort::edge::remap` / `sample` and `photo::straighten` already do an inverse-mapped bilinear rotation; the sibling spec generalises that to an affine resampler.
- UI: `ui/src/modes/image/tools/pointer.ts` (`MOVE_TOOLS`, the move commit in `onUp`), `core/controller.ts` (`moveSelectionContent`, `moveSelectionMask`), `components/toolbar/ToolOptions.svelte` (`opt.moveHint`), the selection outline overlay in `components/canvas/MainCanvas.svelte`.
- `CommandBus` / `UndoStack` in `crates/pincel-image-core/src/command/mod.rs` — today a plain stack of `Box<dyn Command>`; the live-object model adds settings beside the document and a fold of the live steps into one.

**Suggested shape (owner decision 2026-11-05, shared with `image-live-objects`)**

- A live transform on `CommandBus`: the lifted original pixels and outline, the layer, and the current settings (offset, scale, angle, pivot, interpolation). Not part of `Document`, so neither `.ora` nor autosave carries it (AC-18).
- Each handle drag is a normal command that applies the new settings to the layer, resampling from the lifted original (AC-11), and goes on the undo stack. Commit folds the live steps into one `TransformSelection` / `TransformSelectionMask` step; cancel reverts and removes them; any other command or an active-layer change commits first (AC-08).
- Cost: every change re-applies the transform to the layer inside the core, including undo of a drag step; AC-20's ≤ 33 ms budget applies to that per-change apply, and undo memory grows per live drag until the fold.
- The plain translate is the special case scale 1, angle 0. The commit command is what MCP's tool in `mcp-parity-selection-transform` calls.
- History kinds `move_selection` / `move_selection_mask` per `history-panel`.
- The live transform is one more kind of live object in the sense of `image-live-objects` (at most one per document, its AC-03 / AC-04 / AC-08 / AC-12 – AC-14 apply); that spec lists Line, Gradient and box shapes today, so this spec extends its kind set.

**Geometry**

- Settings: the box (the selection's bounds at the start), scale `(sx, sy)` along the box's own axes, angle, pivot (relative to the box, default its center) and offset. A source point maps by: scale about the dragged handle's opposite corner or edge in the box frame, then rotate about the pivot, then translate by the offset. The pivot follows the box on translate and scale.
- The resampler from `image-transform-handles-rotate-zoom` is used with the lifted patch as the source: a buffer of any size, read as transparent outside its pixels, under an inverse affine map with sub-pixel pivot and offset. Keeping the lifted patch (not the canvas-clipped layer) as the source is what AC-11 and AC-13 require.
- Hit areas in screen pixels, independent of zoom: handles 8 px squares with a 12 px hit radius, pivot 10 px; "outside the box" (AC-05) means farther than 12 px from the box and every handle.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Feathered selection scaled 200 % | Partly selected pixels lift and land with their coverage share, as the plain move does; the outline's feather scales with it |
| Rotation by exactly 90° with Nearest on a square selection | Pixels equal the content turned with Layer ▸ Rotate 90° CW inside that square |
| Edge handle dragged past the opposite edge | The box stops at 1 px; it does not flip |
| Pen or touch input (no right button) | A drag starting outside the box rotates (AC-05); Shift comes from the on-screen toggle |
| Selection touching the canvas edge rotated 45° | Corners beyond the canvas stay while live (AC-13), are dropped on commit |
| Escape after five handle drags | Everything as before the first drag; the five drag steps are gone (AC-10) |
| Ctrl+D while a transform is live | Commits, then deselects: two undo steps (AC-08) |
| Undo twice, then a new drag | The two undone drags' redo branch is dropped, as for any new edit; the transform stays live |
| Undo past the first drag, then Redo | The transform is live again with the first drag's settings (AC-21) |
| Transform live, then autosave fires | The snapshot holds the layer's current (transformed) pixels and outline; recovering it gives plain pixels, no live transform (AC-18) |
| Switching the active layer while live | Commits to the original layer first, then switches (AC-08) |
| Tiny selection (1×1) scaled to 400×400 | Works; the content is the single color resampled (Nearest: one flat color) |
| A live shape or gradient exists, user picks Move Selected Pixels | The tool change commits the live object first (AC-08); then the box appears |
| 150 handle drags (capacity 100), then Escape | Pixels and outline as before the first drag (AC-22) |
| Only the pivot moved, then Enter | No step remains (AC-17) |
| Rotate after a 45° turn, then drag a corner handle | Scales along the turned box axes (AC-03) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A press outside the box that moves less than 4 screen pixels before release is a click (commit), more is a rotation drag | 4 px is Windows' default drag threshold; screen pixels keep it independent of zoom | Medium — pen jitter may need more |
| Handle and pivot hit areas as in Technical notes (12 px hit radius) | Big enough for touch-free mouse and pen use; on-canvas sizes stay constant across zoom | Low |
| Shift snaps rotation to 15° | paint.net and most editors snap to 15° | Low |
| The quality option is shared by both Move tools and persists per session like other tool options | One option per tool pair keeps the options bar small | Low |
| Default quality Bilinear | `Interpolation::default()` is Bilinear | Low |
| The outline of Move Selection is resampled with the same quality | A raster coverage mask has to be resampled; Nearest keeps a hard edge, Bilinear softens it | Medium |
| A drag is updated during the pointer move, and its undo step is recorded when the pointer is released | A step per pointer move would flood the history; the owner's model makes each drag one step | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

- AC-20 (budget): a live update of a 2048² selection on the 4K, 10-layer document takes 381 ms (p95, pointer event to the canvas draw) on the reference machine, against ≤ 33 ms. The core alone needs about 147 ms natively for one update after the one-pass Nearest / Bilinear path (Decisions, 2026-11-12) — 4.2 M resampled pixels on one core — and the browser adds the recompose of the changed area and the outline. Re-applying the transform in full resolution on the CPU at every pointer move cannot reach 33 ms. Options: (a) during a drag the editor shows the lifted patch transformed by the GPU / canvas (no core work), and the core applies the transform once per drag on release — which changes the owner decision "the core applies the live transform to the layer at every change"; (b) the core applies a reduced-resolution update during the drag and the full one on release; (c) relax the budget for this row. Recommendation: (a).

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | e2e `a selection shows the box with eight handles and a pivot for both Move tools only (AC-01)` (ui/e2e/transform-handles.spec.ts); `empty_selection_starts_nothing` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-02 | e2e `dragging inside moves the content during the drag; the readout follows (AC-02, AC-07)` (ui/e2e/transform-handles.spec.ts); `begin_transform_translate_moves_pixels_and_outline` (crates/pincel-image-core/tests/live_transform.rs); `the pivot keeps its place relative to the box when it scales or moves (AC-02, AC-03)` (ui/e2e/transform-box.spec.ts) | built |        |
| AC-03 | `a corner drag keeps the opposite corner and scales along the box axes (AC-03)`, `a box never flips and never gets below 1 px (AC-03)`, `an edge drag scales along its axis only, the opposite edge fixed (AC-03)` (ui/e2e/transform-box.spec.ts); e2e `a corner handle scales and a right-drag turns; the readout says so (AC-03, AC-05, AC-07)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-04 | `Shift keeps the starting width-to-height ratio on a corner drag (AC-04)` (ui/e2e/transform-box.spec.ts); the drag reads Shift through `shiftHeld` (key or on-screen toggle) | built |        |
| AC-05 | `a rotation turns about the pivot, clockwise positive; Shift snaps to 15° (AC-05, AC-06)`, `hit-testing in screen pixels: pivot, handles, inside, outside (AC-05)` (ui/e2e/transform-box.spec.ts); e2e `a corner handle scales and a right-drag turns …` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-06 | e2e `moving the pivot changes no pixel (AC-06)` (ui/e2e/transform-handles.spec.ts); `a rotation turns about the pivot …` (ui/e2e/transform-box.spec.ts) | built |        |
| AC-07 | e2e `dragging inside moves the content during the drag; the readout follows (AC-02, AC-07)` (ui/e2e/transform-handles.spec.ts); `consecutive_quality_changes_merge`, `commit_transform_folds_its_steps_into_one` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-08 | e2e `Enter commits as one undo step; Undo restores the pixels (AC-08, AC-09)`, `a click outside the box commits; another tool commits (AC-08)` (ui/e2e/transform-handles.spec.ts); `another_command_commits_first` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-09 | `commit_transform_folds_its_steps_into_one` (crates/pincel-image-core/tests/live_transform.rs); e2e `Enter commits as one undo step; Undo restores the pixels (AC-08, AC-09)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-10 | `cancel_transform_restores_everything` (crates/pincel-image-core/tests/live_transform.rs); e2e `Escape cancels: pixels and the undo state as before (AC-10)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-11 | `round_trips_leave_no_loss_and_no_step` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-12 | `nearest_uses_only_existing_colours` (crates/pincel-image-core/tests/live_transform.rs); e2e `changing the quality while live re-applies the transform (AC-12)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-13 | `content_beyond_the_canvas_returns_unchanged` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-14 | `a_second_transform_carries_the_moved_patch`, `begin_transform_translate_moves_pixels_and_outline` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-15 | `move_selection_transform_changes_only_the_outline` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-16 | `locked_layer_refuses_pixels_but_not_the_outline`, `command_spec_drives_the_transform` (crates/pincel-image-core/tests/live_transform.rs); e2e `a locked layer refuses Move Selected Pixels, not Move Selection (AC-16)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-17 | `empty_selection_starts_nothing`, `round_trips_leave_no_loss_and_no_step` (crates/pincel-image-core/tests/live_transform.rs) | built |        |
| AC-18 | `saving_while_live_keeps_the_transform_and_writes_plain_pixels` (crates/pincel-image-core/tests/live_transform.rs); e2e `saving while live keeps the transform; switching tabs commits it (AC-18)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-19 | `steps_carry_the_move_history_kinds`, `command_spec_drives_the_transform` (crates/pincel-image-core/tests/live_transform.rs); the names come from `history.move_selection` / `history.move_selection_mask` (en, de) | built |        |
| AC-20 | `ui/e2e-perf/scenarios.perf.ts` › "image 4K, 10 layers: live transform of a 2048² selection" (`browser/image-transform-4k/dab-draw-latency-p95`, budget 33 ms in `budgets.json`): measured 381 ms on the reference machine — not met, see Open questions | open |        |
| AC-21 | `undo_while_live_steps_back_one_drag` (crates/pincel-image-core/tests/live_transform.rs); e2e `Undo while live steps back one drag and keeps it live (AC-21)` (ui/e2e/transform-handles.spec.ts) | built |        |
| AC-22 | `capacity_folds_live_steps_and_cancel_still_restores` (crates/pincel-image-core/tests/live_transform.rs) | built |        |

## Decisions

- 2026-11-05: The live transform follows the `image-live-objects` model — owner's choice, because one model for every live object keeps undo, MCP and save behaving alike: the core applies the transform to the layer at every change, its settings live beside the document on `CommandBus` (not in `.ora`), each drag is a normal undo step, commit folds them into one, cancel removes them, and any other command or an active-layer change commits first. This replaces the UI-held state with a stateless preview proposed in the first draft (Decision Log 2026-11-05).
- 2026-11-05: Escape cancels, Enter commits — owner's choice, because every live object uses the same keys.
- 2026-11-05: §5.4's "transforming a selection is Phase 2" is lifted for Image mode only — owner's choice, because paint.net parity is an Image-mode priority; Pixel mode keeps its own Move tool and RotSprite (Decision Log 2026-11-05).
- 2026-11-05: §4.5 gains the budget row "Live transform update, Image 4K, 2048² selection: ≤ 33 ms", owned by this spec — owner's choice, because the per-change apply in the core sits on the input path; AC-20 keeps it (Decision Log 2026-11-05).
- 2026-11-05: Clarification pass — aligned with `image-live-objects`: a commit that changes nothing leaves no step (AC-17, its AC-12), quality changes are steps that merge like style edits (AC-07, its AC-03), the undo capacity keeps the transform's steps (AC-22, its AC-14), save / export / autosave neither commit nor carry settings (AC-18, its AC-13; this replaces "saves or exports" as a commit trigger), and at most one live object exists (AC-08). The box turns with the content and scales along its own axes, the pivot follows the box (AC-01 – AC-03), because that is how paint.net's handles behave and it keeps a handle under the corner it moves. Click threshold 4 screen px.
- 2026-11-05: Split from Layer ▸ Rotate / Zoom (`image-transform-handles-rotate-zoom`) — because together they exceed size L; the dialog needs no live-edit architecture and builds the shared resampler, so it goes first.

- 2026-11-12 (spec-build): the live transform is its own step type beside the live object's (`command/live_transform.rs`): a begin step lifts the selection once — the original layer, the layer with the lifted pixels taken out, the lifted patch and the selection's coverage — and every step re-applies the transform from that patch, so the result depends only on the final box (AC-11) and content beyond the canvas is kept (AC-13). Settings are the box centre, size along its own axes, angle and the pivot relative to the box (the canonical form of "scale about the opposite handle, rotate about the pivot, translate": the UI turns each drag into it, `ui/src/modes/image/tools/transform-box.ts`). Settings equal to the start render the original exactly, so a round trip loses nothing and commits no step (AC-11, AC-17). `commit_if_live` commits a live transform too, so every other command, `set_active_layer` and a new live object commit it first (AC-08); the history capacity folds its oldest steps like a live object's (AC-22).
- 2026-11-12 (spec-build): the transform commands are `CommandSpec` variants with flat fields (`begin_transform`, `edit_transform`, `commit_transform`, `cancel_transform`) and the query `live_transform` (`LiveTransformSpec`); the UI mirrors them as `SelectionTransformCommand` (`TransformCommand` was taken by the layer transform). Saving, export and autosave do not commit (AC-18): the save flow's `gestureControl.commitLive` leaves the transform alone, and a separate `gestureControl.commitTransform` runs on a tab switch and before an MCP call.
- 2026-11-12 (spec-build): live updates take a one-pass path for Nearest and Bilinear (content and coverage from the lifted bytes, written straight into the layer and a canvas-sized mask buffer; integer casts instead of `floor` / `round`, which are library calls on baseline x86-64); Bicubic keeps the shared `pincel_effects::transform::resample`. Measured natively on 3840×2160 with a 2048² selection: 270 ms per update before, 147 ms after; in the browser 742 → 381 ms p95 (AC-20 still open).
- 2026-11-12 (spec-build): a press with a Move tool while the box shows always goes to the box (`tools/transform-drag.ts`): inside translates, a handle scales, the pivot moves, a right-button drag rotates, a left press outside rotates once it moves 4 screen px and is a click (commit) otherwise. Without a selection Move Selected Pixels keeps moving the whole layer on release as before. Red runs: the core tests failed to compile before `live_transform.rs` existed; the e2e tests ran red against the editor before the box (no `[data-transform-box]`); the core tests for AC-12, AC-14, AC-18 and AC-19 were written after the implementation and green at once (the properties hold by construction).
