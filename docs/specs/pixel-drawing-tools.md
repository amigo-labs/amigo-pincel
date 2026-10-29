---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §5.1, pincel.md §5.2, pincel.md §6.1, pincel.md §9.3, pincel.md §9.4]
---

# Pixel drawing tools

## Context

Pixel mode ships a set of drawing tools on the tool rail: Pencil, Eraser, Bucket, Eyedropper, Line, the Shapes group (Rectangle, Rectangle filled, Ellipse, Ellipse filled, Rounded Rectangle, Polygon) and Text. This spec records how they behave today, reverse-engineered from the code on 2026-10-29. The tool table in `pincel.md` §5.2 is the design reference; where the code differs (no brush sizes, no Pencil Shift constrain, no Alt eyedropper, no all-matching Bucket), this spec documents the code, and the differences are tracked outside it (the planned `pixel-pencil` spec covers brush sizes and Shift constrain).

Already decided: a press-drag-release Pencil / Eraser stroke is one undo entry, delimited by `Bus::seal()` / wasm `endStroke()` (Decision Log 2026-07-09). Pixel-mode tools stay pressure-free (§17.4). Selections, the Move tool and selection-bounded edits are in `pixel-selection-move`; pen, touch and viewport input in `pen-input`.

## Scope

### In scope

- Pencil, Eraser, Bucket, Eyedropper, Line, the six Shapes variants and Text in Pixel mode
- Foreground / background color use, including the right mouse button
- Shift constraint for Line and the Shapes, from the key and the latched on-screen toggle
- Shape options: stroke width, outline / fill / fill + outline mode, corner radius, polygon sides
- Paint target resolution, automatic cel creation and the locked-layer refusal
- Tool selection by single keys and variant cycling

### Out of scope

- Brush sizes, Pencil Shift constrain, pixel-perfect — because they are not built; the planned `pixel-pencil` spec covers them
- Pen pressure in Pixel mode — because §17.4 keeps Pixel tools pressure-free; the planned `pixel-pressure` spec covers it
- Selection tools, Move tool, Delete — because `pixel-selection-move` covers them
- Tilemap Stamp and Slice tools — because the `tilemaps` and `slices` specs cover them
- Undo / redo mechanics beyond "one gesture is one entry" — because `pixel-undo-history` covers them
- Color pickers, palette and the D / X color keys — because `pixel-palette` covers them
- Indexed / grayscale painting — because indexed painting is Phase 2 (§3.7); all tools write RGBA

## Acceptance criteria

- [ ] AC-01 (event): When the Pencil is dragged, every pixel on the straight line between consecutive pointer samples is painted in the foreground color, so a stroke whose pointer jumps between two samples still has no gaps.
- [ ] AC-02 (always): One press-drag-release Pencil or Eraser stroke is exactly one undo entry: undo removes the whole stroke, and a stroke made before it stays intact.
- [ ] AC-03 (event): When the Eraser passes over a pixel of the paint target layer, that pixel becomes fully transparent (RGBA 0, 0, 0, 0), whatever the foreground and background colors are.
- [ ] AC-04 (event): When a Pencil or Eraser stroke crosses the canvas edge, the on-canvas pixels are painted, the off-canvas part is skipped, and no error message appears in the status bar.
- [ ] AC-05 (event): When the right mouse button is pressed with the Pencil, Bucket, Line or a Shapes tool, the tool paints with the background color instead of the foreground color; with the Eyedropper it sets the background color; every other tool ignores the right button.
- [ ] AC-06 (event): When the Bucket is clicked on a pixel, every pixel 4-connected to it whose RGBA equals the clicked pixel's RGBA exactly is set to the color, pixels of any other value (and pixels connected only diagonally) stay unchanged, and the fill is one undo entry.
- [ ] AC-07 (event): When the Line tool is dragged from A to B and released, a 1-pixel line from A to B with both end pixels included is drawn in the color as one undo entry.
- [ ] AC-08 (state): While Shift is held when a Line drag is released, the committed line runs horizontally, vertically or at 45° from the start point — whichever is nearest to the pointer.
- [ ] AC-09 (state): While Shift is held when a Rectangle, Rectangle filled, Ellipse, Ellipse filled, Rounded Rectangle or Polygon drag is released, the shape's bounding box is a square whose side is the larger of the drag's width and height.
- [ ] AC-10 (always): A latched on-screen Shift toggle changes a Pixel-mode Line exactly as the held Shift key does.
- [ ] AC-11 (option): Where a Rectangle, Ellipse, Rounded Rectangle or Polygon is drawn in Outline mode with stroke width N, only a band N pixels wide inside the shape's edge is painted, in the stroke color, and the interior stays untouched.
- [ ] AC-12 (option): Where the shape mode is Fill + Outline, the edge band is painted in the stroke color and the interior in the fill color; where it is Fill, the whole shape is painted in the fill color.
- [ ] AC-13 (event): When a Shapes tool is drawn with the left button, the stroke color is the foreground color and the fill color is the background color; with the right button the two are swapped.
- [ ] AC-14 (option): Where the shape is a Rounded Rectangle (radius ≥ 1), an Ellipse or a Polygon (3–64 sides), the four corner pixels of the bounding box stay unpainted while the centre is painted.
- [ ] AC-15 (event): When Rectangle filled or Ellipse filled is dragged between two corners, the full rectangle or the full disk inscribed in it is painted in the drag color as one undo entry, and swapping the two corners gives the identical shape.
- [ ] AC-16 (event): When a Line or a shape extends beyond the canvas, the on-canvas pixels are painted, the rest is skipped, and no error is raised.
- [ ] AC-17 (event): When the Eyedropper is clicked on the canvas, the foreground color becomes the composited color of the visible layers at that pixel; a fully transparent pixel yields alpha 0.
- [ ] AC-18 (event): When the Text tool is clicked on the canvas, a text dialog opens anchored at the clicked pixel.
- [ ] AC-19 (event): When text is typed in the dialog, a live preview shows it on the canvas without changing the document or the undo history; when it is applied, the text is drawn in the foreground color with the chosen size, bold, italic, anti-alias and alignment as one undo entry.
- [ ] AC-20 (error): If applied text lands entirely outside the canvas, then no pixel changes and no undo entry is added.
- [ ] AC-21 (state): While a selection is active, the Text tool paints only pixels inside the selection.
- [ ] AC-22 (event): When a tool paints into a layer / frame that has no cel yet, a canvas-sized cel is created and the paint lands in it.
- [ ] AC-23 (always): Drawing tools paint into the active layer when it is an image layer; when the active layer is a group or tilemap layer, they paint into the lowest image layer instead.
- [ ] AC-24 (error): If the paint target layer or a group it is nested in is locked, then a drawing-tool press changes no pixel and the status bar shows "“{layer name}” is locked" until the layer is unlocked.
- [ ] AC-25 (event): When a tool-group key is pressed without Ctrl / Cmd / Alt and focus is not in a text field, that group's tool becomes active, and pressing the same key again cycles through the group's variants.

## Technical notes

> Paths and line numbers are hints from 2026-10-29 and drift — grep for the names.

**Tool ids** (`ui/src/modes/pixel/tools.ts`, `type Tool`): `'pencil' | 'eraser' | 'eyedropper' | 'bucket' | 'line' | 'rectangle' | 'rectangle-fill' | 'ellipse' | 'ellipse-fill' | 'rounded-rect' | 'polygon-shape' | … | 'text' | 'move' | 'tilemap-stamp' | 'slice'`. Rail groups and keys come from `toolGroups()` and `TOOL_GROUP_KEYS` in `ui/src/lib/shell/shortcuts.ts`: `B` pencil, `E` eraser, `G` fill, `I` eyedropper, `L` line, `U` shapes (cycles Rectangle → Rectangle filled → Ellipse → Ellipse filled → Rounded Rectangle → Polygon), `T` text. Default tool: `pencil`; default colors `#f87171` / `#ffffff` (`toolInit()` / `colorsInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts`).

**Pointer pipeline** (`ui/src/modes/pixel/pointer.ts`):

- `onPointerDown` → Pencil / Eraser set `live.painting` and call `paintAt`; Bucket calls `commitBucket` once per click; drag-shape tools (`isDragShapeTool`) record `live.dragStart` and commit on `onPointerUp`; Text calls `openTextAt`.
- `paintAt` rasterizes between samples with the TS Bresenham `linePoints()` and calls wasm `applyTool(tool, x, y, color)` per pixel; errors whose message contains `pixel out of bounds` are swallowed, others go to the status bar.
- `BG_TOOLS` = pencil, bucket, eyedropper, line and the six shapes; `strokeColor()` / `shapeFillColor()` pick fg / bg by `live.useBg` (right button).
- `constrainEndpoint()` (`tools.ts`) implements the Shift constraint; `shiftHeld(e)` (`ui/src/lib/shell/modifiers.svelte.ts`) merges key and latched toggle.
- `lockedEditTarget()` refuses the press and sets `t('layers.locked', { name })`.
- `onPointerUp` calls `endStroke()` for paint strokes; `pointercancel` is bound to `onPointerUp`.

**wasm** (`crates/pincel-wasm/src/lib.rs`, `shapes.rs`, `text.rs`), colors packed `0xRRGGBBAA`:

- `applyTool(tool_id, x, y, color)` — `"pencil"` → `SetPixel`, `"eraser"` → `SetPixel` with `Rgba::TRANSPARENT`; consecutive calls merge until `endStroke()` (`bus.seal()`).
- `applyLine` → `DrawLine`; `applyRectangle(…, fill)` → `DrawRectangle`; `applyEllipse(…, fill)` → `DrawEllipse` (the `-fill` tools); `applyShape(kind, x0, y0, x1, y1, mode, stroke_width, stroke_color, fill_color, param)` → `DrawShape` (outline tools; `kind` ∈ `rectangle | rounded_rectangle | ellipse | polygon`, `mode` ∈ `outline | fill | fill_outline`, polygon sides clamped 3–64).
- `applyBucket` → `FillRegion` (4-connected, tolerance 0); `pickColor(frame, x, y)` composes a 1×1 viewport with the visible-layer filter.
- `drawText(font_id, text, x, y, size, color, bold, italic, anti_alias, align)` → `DrawText`, returns `false` and undoes the empty entry when nothing landed; `previewText` composes without touching the bus.
- `paint_target_layer()` / `edit_target_layer()` / `ensure_paint_cel()` resolve the target, refuse locked layers and seed an empty canvas-sized RGBA cel outside the undo bus.

**Core commands** (`crates/pincel-core/src/command/`): `set_pixel.rs`, `draw_line.rs` (`BresenhamIter`), `draw_rectangle.rs`, `draw_ellipse.rs` (midpoint), `draw_shape.rs` (`ShapeKind`, `ShapeMode`, `ShapeStyle`; stroke = fill mask minus mask contracted by the stroke width), `fill_region.rs`, `draw_text.rs` (clips to `doc.selection_contains`).

**Options bar** (`{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte`): Shapes get Stroke (1–64), Mode, Radius (rounded rect, 0–256), Sides (polygon, 3–64); other tools show their label and, for `BG_TOOLS`, the right-button hint.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pointer jumps across the whole canvas in one sample | The stroke is still connected (AC-01) |
| Stroke leaves and re-enters the canvas | Only on-canvas pixels change; no error (AC-04) |
| Bucket clicked on a pixel already in the fill color | No pixel changes; the click still adds an undo entry |
| Bucket clicked outside the canvas | No pixel changes; the click still adds an undo entry |
| Eyedropper outside the canvas | Picks transparent |
| Zero-size drag with a shape tool | A 1-pixel shape at the press point |
| Text applied with an empty string or fully off canvas | Nothing committed, status "nothing placed" (AC-20) |
| Paint on a freshly added layer or frame | A cel is created on the fly (AC-22) |
| Document whose only layers are tilemaps / groups | Paint tools fail with "document has no paintable image layer" in the status bar |
| Mid-drag tool switch to a shape tool | `paintAt` ignores drag-shape tools, so no stray pixels |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/pencil.spec.ts › pixel mode: a fast pencil stroke stays connected` | built | |
| AC-02 | `crates/pincel-wasm/src/lib.rs › tests::apply_tool_drag_then_end_stroke_is_single_undo_entry`, `tests::two_strokes_are_two_undo_entries`; `ui/e2e/smoke.spec.ts › pixel mode: a pencil stroke is undoable` | built | |
| AC-03 | `crates/pincel-wasm/src/lib.rs › tests::apply_tool_eraser_clears_a_previously_painted_pixel`, `tests::apply_tool_eraser_ignores_the_color_argument` | built | |
| AC-04 |  | open | |
| AC-05 |  | open | |
| AC-06 | `crates/pincel-core/src/command/fill_region.rs › tests::fill_stops_at_a_color_boundary`, `tests::fill_is_four_connected_not_eight`; `crates/pincel-wasm/src/lib.rs › tests::apply_bucket_joins_the_undo_bus` | built | |
| AC-07 | `crates/pincel-core/src/command/draw_line.rs › tests::apply_writes_horizontal_pixels`, `tests::apply_writes_diagonal_pixels`; `crates/pincel-wasm/src/lib.rs › tests::apply_line_joins_the_undo_bus` | built | |
| AC-08 |  | open | |
| AC-09 |  | open | |
| AC-10 | `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key` | built | |
| AC-11 | `crates/pincel-core/src/command/draw_shape.rs › tests::outline_rectangle_with_width_two_leaves_the_centre` | built | |
| AC-12 | `crates/pincel-core/src/command/draw_shape.rs › tests::fill_outline_uses_both_colours_and_fill_mode_only_the_fill`; `crates/pincel-wasm/src/shapes.rs › tests::apply_shape_fill_outline_rectangle` | built | |
| AC-13 |  | open | |
| AC-14 | `crates/pincel-core/src/command/draw_shape.rs › tests::rounded_rectangle_ellipse_and_polygon_skip_corners`; `crates/pincel-wasm/src/shapes.rs › tests::apply_shape_polygon_and_rounded_rectangle_and_errors` | built | |
| AC-15 | `crates/pincel-core/src/command/draw_rectangle.rs › tests::fill_writes_every_pixel_in_the_bbox`, `tests::reversed_endpoints_produce_the_same_rectangle`; `crates/pincel-core/src/command/draw_ellipse.rs › tests::fill_includes_the_center_and_the_rim`, `tests::reversed_endpoints_produce_the_same_ellipse` | built | |
| AC-16 | `crates/pincel-core/src/command/draw_line.rs › tests::apply_skips_pixels_outside_cel_silently`; `crates/pincel-core/src/command/draw_shape.rs › tests::shape_partially_off_cel_is_clipped_not_an_error`; `crates/pincel-core/src/command/draw_rectangle.rs › tests::outline_clips_to_cel_bounds`; `crates/pincel-core/src/command/draw_ellipse.rs › tests::outline_clipped_to_cel_bounds` | built | |
| AC-17 | `ui/e2e/pencil.spec.ts › pixel mode: a fast pencil stroke stays connected`; `crates/pincel-wasm/src/lib.rs › tests::pick_color_returns_zero_on_transparent_pixel` | built | |
| AC-18 |  | open | |
| AC-19 | `crates/pincel-wasm/src/text.rs › tests::draw_text_paints_and_undoes_and_preview_leaves_state` | built | |
| AC-20 | `crates/pincel-wasm/src/text.rs › tests::draw_text_off_canvas_returns_false_and_leaves_no_undo_entry` | built | |
| AC-21 | `crates/pincel-core/src/command/draw_text.rs › tests::selection_clips_and_off_canvas_is_a_noop` | built | |
| AC-22 | `crates/pincel-wasm/src/lib.rs › tests::paint_auto_creates_cel_on_a_fresh_layer`, `tests::paint_auto_creates_cel_on_a_fresh_frame` | built | |
| AC-23 | `crates/pincel-wasm/src/lib.rs › tests::pencil_paints_into_active_layer_cel`, `tests::apply_tool_pencil_skips_group_layer_to_find_paintable_image`, `tests::paint_target_falls_back_for_non_image_or_unknown_active` | built | |
| AC-24 | `ui/e2e/layers.spec.ts › pixel: a locked layer refuses paint`; `crates/pincel-wasm/src/layer_ops.rs › tests::locked_layer_refuses_paint_until_unlocked` | built | |
| AC-25 | `ui/e2e/shortcuts.spec.ts › pixel mode: the shared keymap selects tools and cycles variants` | built | |

## Decisions

- 2026-10-29: Extracted from the code as it stands; differences to `pincel.md` §5.2 are not ACs — see the extraction report.
