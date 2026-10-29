---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §5.2, pincel.md §9.4, pincel.md §13.1]
---

# Pixel selection and Move tool

## Context

Pixel mode has a selection that is either a plain rectangle or a canvas-sized coverage mask. It is made with the Rectangle / Ellipse marquee, the Lasso, the Polygonal Lasso and the Magic Wand, combined with replace / add / subtract / intersect, and changed with Select All, Deselect, Invert, Expand and Contract. With a selection active, Delete clears the selected pixels, the Move tool drags them, Layer flip / rotate turn only the selected area, and Crop trims the canvas to it. This spec records that behavior as it exists on 2026-10-29.

`pincel.md` §5.2 lists only the rectangle marquee; ellipse, lasso, wand and the combine modes, which §13.1 calls Phase 2, already exist in code. Selection state is transient and outside the undo stack (M7.8 slice, §5.2); commands that move pixels (`MoveSelectionContent`) do restore the selection on undo.

## Scope

### In scope

- Rectangle and Ellipse marquee, Lasso, Polygonal Lasso, Magic Wand (tolerance, contiguous, sample source)
- Combine modes from Shift / Alt (keyboard or latched on-screen toggles)
- Select All, Deselect, Invert, Expand, Contract (menu and keyboard)
- Delete / Backspace clearing the selected pixels
- Move tool: dragging the selected pixels, and panning when nothing is selected
- Selection-bounded Layer flip / rotate and Crop to selection
- The selection staying out of the undo history

### Out of scope

- Copy, cut and paste — because the planned `copy-cut-paste` spec covers them
- Free transform, scale or rotate by arbitrary angles of the selection — because they are not built (Phase 2, §13)
- Rotsprite rotation — because the planned `rotsprite` spec covers it
- Effects and adjustments confined to the selection — because the `effects-adjustments` spec covers them
- Clipping Pencil, Eraser, Bucket, Line and shapes to the selection — because they ignore the selection today (only Text clips; see `pixel-drawing-tools`)
- Space-drag panning and zoom — because `pen-input` covers the viewport
- Arrow-key nudging — because it is not built

## Acceptance criteria

- [ ] AC-01 (event): When the Rectangle marquee is dragged from pixel A to pixel B and released, the selection becomes the rectangle with A and B as inclusive corners and its outline is shown as marching ants.
- [ ] AC-02 (event): When the Ellipse marquee is dragged over a box, the selection becomes the ellipse inscribed in that box; for a 4×4 box the four corner pixels are not selected.
- [ ] AC-03 (event): When the Lasso is dragged along a path and released, the selection becomes the interior of the polygon the path forms.
- [ ] AC-04 (event): When the Polygonal Lasso is clicked repeatedly, each click adds a vertex; a double-click or Enter closes the polygon and selects its interior; Escape discards the vertices without changing the selection.
- [ ] AC-05 (event): When the Magic Wand is clicked on a pixel, the selection becomes the pixels whose every RGBA channel is within the tolerance (0–255) of the clicked pixel — only those 4-connected to it while Contiguous is on, all of them on the canvas while it is off — sampled from the active layer or from the composite as chosen; all fully transparent pixels count as one color.
- [ ] AC-06 (event): When a new selection shape is combined, replace discards the old selection, add unites them, subtract removes the shape from it, and intersect keeps only their overlap.
- [ ] AC-07 (state): While Shift is held (key or latched toggle) at the press of a selection tool the mode is add, while Alt is held it is subtract, while both are held it is intersect, otherwise replace.
- [ ] AC-08 (error): If subtract or intersect is used with nothing selected, then the selection stays empty.
- [ ] AC-09 (event): When a Rectangle or Ellipse marquee is clicked without dragging, or a Lasso gesture has fewer than three points, in replace mode, then the selection is cleared.
- [ ] AC-10 (event): When Select All, Deselect or Invert is chosen, the selection becomes the whole canvas, nothing, or the canvas minus the current selection (the whole canvas when nothing was selected).
- [ ] AC-11 (event): When Select ▸ Expand… or Contract… is chosen, a dialog asks for a radius in pixels, and applying it grows or shrinks the selection by that radius; contracting past its size clears it.
- [ ] AC-12 (event): When Ctrl/Cmd+A, Ctrl/Cmd+D, Ctrl/Cmd+Shift+I or Escape (no polygon in progress, focus not in a text field) is pressed, the editor runs Select All, Deselect, Invert and Deselect respectively; Delete or Backspace runs Delete while a selection exists.
- [ ] AC-13 (always): Selection changes are not undo entries: undoing an edit made after a selection change leaves the selection as it is.
- [ ] AC-14 (event): When Delete runs with a selection, every selected pixel of the paint target layer at the current frame becomes transparent, unselected pixels inside the selection's bounding box stay unchanged, the selection stays, and the change is one undo entry.
- [ ] AC-15 (state): While nothing is selected, Delete changes no pixel and adds no undo entry.
- [ ] AC-16 (event): When the Move tool is dragged with a selection active and released at a non-zero offset, the selected pixels of the paint target layer move by that offset, their old place becomes transparent, the selection moves with them, pixels moved off the canvas are dropped, and the move is one undo entry whose undo restores both the pixels and the selection position.
- [ ] AC-17 (event): When the Move tool is released at the press position, no pixel changes and no undo entry is added.
- [ ] AC-18 (state): While nothing is selected, a Move-tool drag pans the view and leaves the document unchanged.
- [ ] AC-19 (state): While a selection is active, Layer flip horizontal / vertical and rotate 90° / 180° transform only the pixels inside the selection's bounding box, as one undo entry.
- [ ] AC-20 (event): When Crop is chosen with a selection, the canvas becomes the selection's bounding box clipped to the canvas and the selection is cleared; with no selection on the canvas nothing changes.

## Technical notes

> Paths and line numbers are hints from 2026-10-29 and drift — grep for the names.

**Model** (`crates/pincel-core/src/selection.rs`, `Sprite` in `document/`): `Sprite::selection: Option<Rect>` holds the bounding box, `Sprite::selection_mask: Option<SelectionMask>` is `None` for a plain rectangle. `SelectionMask::{rect, ellipse, polygon, wand, empty}`, `combine(&shape, SelectionMode)`, `invert()`, `expand(r)`, `contract(r)`; `SelectionMode` ∈ `Replace | Add | Subtract | Intersect` (wire names `replace | add | subtract | intersect`). A combined result that is a rectangle collapses back to a plain rect.

**wasm** (`crates/pincel-wasm/src/selection_ops.rs`, `lib.rs`, `transforms.rs`):

- `selectRect(x, y, w, h, mode)`, `selectEllipse(…)`, `selectPolygon(points: Int32Array, mode)`, `selectWand(x, y, tolerance, contiguous, sample: "layer" | "composite", mode)`, `selectAll()`, `invertSelection()`, `expandSelection(r)`, `contractSelection(r)` — each emits one `selection-changed` event with the new bounds (zeros when empty).
- `setSelection(x, y, w, h)` (raw rect, not clipped to the canvas), `clearSelection()`, getters `hasSelection`, `selectionX/Y/Width/Height`, `selectionIsRect`, `selectionMask()` (canvas-sized `0` / `255` bytes).
- `deleteSelection() -> bool` runs `ClearRegion::new(layer, frame, sel).masked()`; `false` without a selection.
- `applyMoveSelection(dx, dy)` runs `MoveSelectionContent` (stores the prior selection for undo; errors "no active selection" without one).
- `transformLayer(orientation)` wraps `TransformCel::new(…).within(sel)` with the selection's bounding rect; `cropToSelection() -> bool` runs `ReframeCanvas::new(sel ∩ canvas)`.

**UI** (`ui/src/modes/pixel/`):

- `pointer.ts`: `selectionModeFor(e)` maps `shiftHeld` / `altHeld` to the mode; `commitSelection` / `commitEllipseSelection` turn the drag corners into an inclusive rect and clear on a no-move click in replace mode; `commitPolygonPoints` clears for fewer than three points in replace mode; `commitWand` clamps the tolerance to 0–255. Move-tool press with `ed.selection` sets `toolState.moveSelStart`, without one starts a pan; `onPointerUp` calls `applyMoveSelection` only for a non-zero delta.
- `doc-ops.ts`: `selectAction('all' | 'none' | 'invert' | 'expand' | 'contract', radius)`, `deselect()`, `deleteSelectionPixels()`.
- `PixelEditor.svelte` `onKeyDown`: Escape cancels an open polygon / lasso first, then deselects; Enter closes a polygon; the shared `SHORTCUTS` map (`ui/src/lib/shell/shortcuts.ts`) binds `selectAll` Ctrl+A, `deselect` Ctrl+D, `invertSelection` Ctrl+Shift+I, `deleteSelection` Delete / Backspace (only consumed while a selection exists).
- `render.ts` `syncSelection()` re-reads bounds and the mask after a `selection-changed` event; `overlay-painter.ts` draws the marching ants.
- Rail: group `select` (`M`: Rectangle Select, Ellipse Select), `lasso` (`Q`: Lasso, Polygonal Lasso), `wand` (`W`), `move` (`V`). Wand options: Tolerance (default 0), Contiguous (default on), Sample (default layer).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Marquee dragged partly off the canvas | The selection covers only on-canvas pixels (`selectRect` clips to the canvas mask) |
| Expand / Contract with nothing selected | No-op, no event |
| Contract by more than half the selection's size | The selection becomes empty |
| Move drag whose destination is entirely off the canvas | The selected pixels are cleared; the selection moves off canvas; undo restores both |
| Delete with a shaped selection that has a hole | Pixels in the hole keep their value |
| Invert with nothing selected | The whole canvas is selected |
| Move with a selection on a locked layer | The press is refused with the "is locked" notice |
| Wand clicked outside the canvas | The selection becomes empty |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |  | open | |
| AC-02 | `crates/pincel-wasm/src/selection_ops.rs › tests::select_ellipse_sets_bounds_and_a_shaped_mask`; `crates/pincel-core/src/selection.rs › tests::ellipse_drops_the_corners` | built | |
| AC-03 | `crates/pincel-wasm/src/selection_ops.rs › tests::select_polygon_lasso_triangle`; `crates/pincel-core/src/selection.rs › tests::polygon_fills_a_triangle_and_ignores_degenerate_input` | built | |
| AC-04 |  | open | |
| AC-05 | `crates/pincel-core/src/selection.rs › tests::wand_contiguous_vs_global_and_tolerance`, `tests::wand_treats_all_transparent_pixels_as_one_colour`; `crates/pincel-wasm/src/selection_ops.rs › tests::select_wand_on_layer_and_composite` | built | |
| AC-06 | `crates/pincel-wasm/src/selection_ops.rs › tests::modes_combine_and_a_rectangular_result_collapses_to_a_rect`; `crates/pincel-core/src/selection.rs › tests::combine_modes` | built | |
| AC-07 |  | open | |
| AC-08 | `crates/pincel-wasm/src/selection_ops.rs › tests::subtract_or_intersect_with_nothing_selected_stays_empty` | built | |
| AC-09 |  | open | |
| AC-10 | `crates/pincel-wasm/src/selection_ops.rs › tests::select_all_invert_expand_contract`; `crates/pincel-wasm/src/lib.rs › tests::clear_selection_drops_the_rect_and_emits_zeros` | built | |
| AC-11 | `ui/e2e/tools.spec.ts › pixel mode: Select ▸ Expand asks for a radius`; `crates/pincel-wasm/src/selection_ops.rs › tests::select_all_invert_expand_contract` | built | |
| AC-12 |  | open | |
| AC-13 | `crates/pincel-wasm/src/lib.rs › tests::selection_is_not_undoable_in_the_m7_8_slice` | built | |
| AC-14 | `crates/pincel-wasm/src/lib.rs › tests::delete_selection_clears_pixels_and_undoes`; `crates/pincel-wasm/src/selection_ops.rs › tests::delete_and_move_honour_the_shaped_mask` | built | |
| AC-15 | `crates/pincel-wasm/src/lib.rs › tests::delete_selection_without_selection_returns_false` | built | |
| AC-16 | `crates/pincel-wasm/src/lib.rs › tests::apply_move_selection_translates_pixels_and_selection`, `tests::apply_move_selection_joins_the_undo_bus`; `crates/pincel-wasm/src/selection_ops.rs › tests::delete_and_move_honour_the_shaped_mask`; `crates/pincel-core/src/command/move_selection_content.rs › tests::destination_outside_cel_drops_pixels` | built | |
| AC-17 |  | open | |
| AC-18 |  | open | |
| AC-19 | `crates/pincel-wasm/src/transforms.rs › tests::transform_layer_flips_and_respects_selection`; `crates/pincel-core/src/command/transform_cel.rs › tests::within_rect_leaves_outside_pixels_alone` | built | |
| AC-20 | `crates/pincel-wasm/src/transforms.rs › tests::resize_canvas_and_crop_change_dimensions_and_clear_selection` | built | |

## Decisions

- 2026-10-29: Extracted from the code as it stands; differences to `pincel.md` §5.2 / §13.1 are not ACs — see the extraction report.
