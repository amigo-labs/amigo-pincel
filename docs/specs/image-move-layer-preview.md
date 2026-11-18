---
status: todo
created: 2026-11-18
size: M
mode: Image
design: [pincel.md §5.4, pincel.md §4.5, pincel.md §15 (2026-11-13 Transform drag preview), pincel.md §15 (2026-11-18 Move Layer drag preview)]
depends_on: []
---

# Image Move Layer: live drag preview

## Context

In Image mode, Move Layer (and Move Selected Pixels with no selection, which moves the whole layer, `image-editing` AC-21) shows nothing while the pointer is held: the canvas keeps the layer where it was, and the layer jumps to its new place only when the pointer is released. The user cannot see where the layer is while dragging, so they release early or guess, and a layer dragged past the canvas edge loses its pixels on that release (`image-editing` AC-21; `image-move-layer-offcanvas` changes that).

Owner report 2026-11-18: "while holding the image you don't see live where it is, only after releasing it". The Move tools' transform box already previews its drag (`image-transform-handles` AC-23, Decision Log 2026-11-13); Move Layer without a selection needs the same.

## Scope

### In scope

- A live preview of Move Layer drags, and of Move Selected Pixels drags with no selection, on raster layers and on groups
- The move applied once, on release, as today
- A performance budget for the preview frame

### Out of scope

- Drags of the transform box (with a selection) — because `image-transform-handles` already previews them
- Arrow-key nudging — because it is a separate gap shared with live objects (`image-live-objects-ui` out of scope)
- Keeping pixels moved past the canvas edge — because `image-move-layer-offcanvas` owns it
- Pixel mode — because its Move tool is a different code path with its own preview (`pixel-selection-move`)

## Acceptance criteria

- [ ] AC-01 (state): While a Move Layer drag is in progress, the canvas shows the active layer's content shifted by the offset between the press and the current pointer position in document pixels, each axis rounded to the nearest whole pixel (the same rounding the release uses), updated as the pointer moves, while the document stays as before the drag: its pixels, its undo history and its dirty state do not change until release.
- [ ] AC-02 (always): For a document whose layers all use the Normal blend mode at 100 % opacity and whose pixels are each fully opaque or fully transparent (any number of layers above and below the moved layer, the moved layer visible), at a zoom of 100 % or more, the canvas during a drag at offset (dx, dy) looks the same as the canvas after the drag is released at (dx, dy) — compared as element screenshots of the canvas at the same view.
- [ ] AC-03 (state): While a Move Layer drag is in progress, layers above the moved layer stay drawn above it and layers below stay below it: where an opaque Normal layer at 100 % opacity lies above the moved layer, the canvas shows that layer's pixels there during the drag, whatever the moved layer's blend mode; where the moved layer (Normal, 100 % opacity) has opaque pixels over a layer below, the canvas shows the moved layer's pixels there.
- [ ] AC-04 (event): When the pointer is released at a non-zero offset, then the layer moves by that offset as one undo step, and the result equals a single move by that offset, however many pointer moves the drag had.
- [ ] AC-05 (event): When the pointer leaves the canvas during a drag and comes back before release, then the result depends only on the release position: a drag that ends where it started changes nothing and adds no undo step.
- [ ] AC-06 (event): When a Move Layer drag is ended before release — by Escape, Undo, a cancelled pointer (`pointercancel`, a second touch pointer), a switch to another tool, another tab, another active layer, a menu command that changes the document or its selection, or the window losing focus — then the preview disappears, the canvas shows the document as before the drag, and no undo step is added; Undo during a drag only cancels the drag and does not undo an earlier step.
- [ ] AC-07 (state): While Move Selected Pixels is dragged with no selection, AC-01 – AC-06 hold as for Move Layer.
- [ ] AC-08 (state): While a group is the active layer and dragged with Move Layer, all of the group's visible content follows the pointer, and on release the group moves as one undo step (as today).
- [ ] AC-09 (error): If Move Layer refuses the press — the active layer is locked or inside a locked group, or the dragged group contains a locked layer — then no preview shows and the refusal notice appears as today.
- [ ] AC-10 (always): While the user drags a raster layer with Move Layer on a 3840×2160 document with 10 layers, the time from a pointer move to the next presented frame is no longer than the §4.5 budget row "Move Layer drag preview, Image 4K, 10 layers: ≤ 33 ms" (p95), measured by the browser performance script that measures `image-transform-handles` AC-20; the one apply on release is outside this budget because it is a single step after the gesture, as for the transform box (Decision Log 2026-11-13).

## Technical notes

- Model: the transform-box preview (Decision Log 2026-11-13). When the drag starts, the core hands the editor once, through the wasm adapter, what it needs to draw the drag: the composite of the layers below the moved layer, the moved layer's content (a group: its own composite) with its opacity and blend mode, and the composite of the layers above. Every frame the editor draws below, the moved content at the offset, and above, with a canvas transform; a view change (zoom, pan, resize) asks the core again. Canvas pixels are never read (AGENTS.md §9).
- Blend modes other than Normal, opacities below 100 % and semi-transparent pixels may be approximated during the drag (the browser blends in sRGB, the core in linear light; AC-03 still holds); the release gives the exact result. AC-02 pins the case the browser draws exactly.
- Once `image-move-layer-offcanvas` is built, the handed-over content includes the pixels outside the canvas of the moved layer (for a group: of every raster layer in it), so they come into view during the drag (its AC-09).
- e2e can count the core applies of AC-04 with `countCall`, as `image-transform-handles` AC-23 does.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Press and release without moving | No preview change, no undo step (AC-05) |
| Drag far past the canvas edge and back within one drag | Nothing is lost; result is the release offset (AC-05) |
| Hidden active layer | Move Layer behaves as today for a hidden layer; the preview shows what the canvas shows (nothing of the hidden layer) |
| Zoom or pan during the drag (mouse wheel, View menu or its keys) | The preview redraws at the new view and keeps following the pointer; the drag is not cancelled |
| Second pointer (touch pinch) during the drag | The pinch takes over and the move is cancelled (AC-06) |
| Empty (fully transparent) layer or empty group | The preview shows no change; the release behaves as Move Layer does today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 33 ms p95 is achievable with a one-time handover | The transform-box preview meets the same budget with the same approach (`image-transform-handles` AC-20) | Low |
| Approximating non-Normal blend modes during the drag is acceptable | The owner accepted the same for the transform-box preview (2026-11-13) | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | e2e `the layer follows the drag; the release shows the same picture as one step (AC-01 – AC-04)` (ui/e2e/image-move-layer-preview.spec.ts); `compose_split_parts_stack_to_the_composite` (crates/pincel-image-core/src/render/mod.rs) | built |        |
| AC-02 | e2e `the layer follows the drag; the release shows the same picture as one step (AC-01 – AC-04)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-03 | e2e `the layer above stays on top of the moved layer, whatever its blend mode (AC-03)` (ui/e2e/image-move-layer-preview.spec.ts); `compose_split_keeps_the_moved_layers_groups` (crates/pincel-image-core/src/render/mod.rs) | built |        |
| AC-04 | e2e `the layer follows the drag; the release shows the same picture as one step (AC-01 – AC-04)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-05 | e2e `a drag that comes back to its start changes nothing (AC-05)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-06 | e2e `Escape …`, `Undo …`, `a tool switch …`, `the window losing focus during the drag drops it unapplied (AC-06)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-07 | e2e `Move Selected Pixels without a selection previews the whole layer (AC-07)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-08 | e2e `a group follows the drag and moves as one step (AC-08)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-09 | e2e `a locked layer shows no preview and the notice (AC-09)` (ui/e2e/image-move-layer-preview.spec.ts) | built |        |
| AC-10 | perf `image 4K, 10 layers: Move Layer drag preview` (ui/e2e-perf/scenarios.perf.ts, metric `browser/image-move-layer-4k/dab-latency-p95`, budget in ui/e2e-perf/budgets.json) | built |        |

## Decisions

- 2026-11-18: Split from the owner's report together with `image-move-layer-offcanvas`; the preview is built first because it is independent and smaller (owner).
- 2026-11-18 (revision): AC-02 narrowed to fully opaque / fully transparent pixels at 100 % opacity and zoom ≥ 100 % (owner): the drag preview is drawn by the browser, which blends semi-transparent pixels in sRGB while the core blends in linear light; exact per-frame core composites would miss the AC-10 budget on 4K. Release stays exact.
- 2026-11-18 (spec-build): The core hands over three canvas-sized composites (below, moved, above) once per drag through `move_preview` (pincel-image-wasm, new public API) backed by `compose_split` (pincel-image-core, new public API), instead of view-rect composites as for the transform box — because they do not depend on the view, so a zoom or pan during the drag needs no second handover; canvases over 4096 px are downscaled for the preview only.
- 2026-11-18 (spec-build): A drag is tied to the document, its revision, the active layer and the tool it began with; any change of these drops it unapplied (AC-06). Menu commands and Undo change the revision, so they need no hook of their own; Undo during the drag is caught before it runs (`gestureControl.cancelLiveDrag`), so it does not undo an earlier step.
- 2026-11-18 (spec-build): Red run before the fix: AC-01/02/04, AC-06 (Undo, tool switch, window blur), AC-07 and AC-08 failed; AC-03, AC-05, AC-06 (Escape) and AC-09 were green before, because without a preview the canvas never changes during a drag — their tests guard the new code against regressions (a preview drawn above the upper layers fails AC-03).
- 2026-11-18 (spec-build): AC-10 is measured by `pnpm perf` on the reference machine; this container is not one, so the budget check of that run stays with the owner.
