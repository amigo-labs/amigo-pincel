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
- [ ] AC-02 (always): For a document whose layers all use the Normal blend mode (any opacity, any number of layers above and below the moved layer, the moved layer visible), the canvas during a drag at offset (dx, dy) looks the same as the canvas after the drag is released at (dx, dy) — compared as element screenshots of the canvas at the same view.
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
- Blend modes other than Normal on the moved layer or above it may be approximated during the drag (AC-03 still holds); the release gives the exact result. AC-02 pins the Normal case.
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
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |

## Decisions

- 2026-11-18: Split from the owner's report together with `image-move-layer-offcanvas`; the preview is built first because it is independent and smaller (owner).
