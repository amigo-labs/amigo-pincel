---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §5.1, pincel.md §5.2, pincel.md §6.1, pincel.md §9.3, pincel.md §9.4]
---

# Pixel drawing tools

## Context

Pixel mode ships a set of drawing tools on the tool rail: Pencil, Eraser, Bucket, Eyedropper, Line, the Shapes group (Rectangle, Rectangle filled, Ellipse, Ellipse filled, Rounded Rectangle, Polygon) and Text. This spec records how they behave today, reverse-engineered from the code on 2026-10-29. The tool table in `pincel.md` §5.2 is the design reference; where the code differs (no Alt eyedropper, no all-matching Bucket), this spec documents the code, and the differences are tracked outside it. Brush size and shape, pixel-perfect and the Pencil / Eraser Shift lock are specified in `pixel-pencil`; the Pencil / Eraser ACs here hold at its default brush (size 1, square, pixel-perfect off).

Already decided: a press-drag-release Pencil / Eraser stroke is one undo entry — one `PencilStroke` command opened by wasm `beginStroke()` and closed by `endStroke()` (Decision Log 2026-07-09, `pixel-pencil`). Pixel-mode tools stay pressure-free (§17.4). Selections, the Move tool and selection-bounded edits are in `pixel-selection-move`; pen, touch and viewport input in `pen-input`.

## Scope

### In scope

- Pencil, Eraser, Bucket, Eyedropper, Line, the six Shapes variants and Text in Pixel mode
- Foreground / background color use, including the right mouse button
- Shift constraint for Line and the Shapes, from the key and the latched on-screen toggle
- Shape options: stroke width, outline / fill / fill + outline mode, corner radius, polygon sides
- Paint target resolution, automatic cel creation and the locked-layer refusal
- Tool selection by single keys and variant cycling

### Out of scope

- Brush sizes and shapes, the Pencil / Eraser Shift lock, pixel-perfect — because `pixel-pencil` covers them
- Pen pressure in Pixel mode — because §17.4 keeps Pixel tools pressure-free; the planned `pixel-pressure` spec covers it
- Selection tools, Move tool, Delete — because `pixel-selection-move` covers them
- Tilemap Stamp and Slice tools — because the `tilemaps` and `slices` specs cover them
- Undo / redo mechanics beyond "one gesture is one entry" — because `pixel-undo-history` covers them
- Color pickers, palette and the D / X color keys — because `pixel-palette` covers them
- Indexed / grayscale painting — because indexed painting is Phase 2 (§3.7); all tools write RGBA

## Acceptance criteria

- [ ] AC-01 (event): When the Pencil is dragged at size 1, every pixel on the straight line between consecutive pointer samples is painted in the foreground color, so a stroke whose pointer jumps between two samples still has no gaps.
- [ ] AC-02 (always): One press-drag-release Pencil or Eraser stroke is exactly one undo entry: undo removes the whole stroke, and a stroke made before it stays intact.
- [ ] AC-03 (event): When the Eraser passes over a pixel of the paint target layer, that pixel becomes fully transparent (RGBA 0, 0, 0, 0), whatever the foreground and background colors are.
- [ ] AC-04 (event): When a Pencil or Eraser stroke crosses the canvas edge, the on-canvas pixels are painted, the off-canvas part is skipped, and no error message appears in the status bar.
- [ ] AC-05 (event): When the right mouse button is pressed with the Pencil, Bucket, Line or a Shapes tool, the tool paints with the background color instead of the foreground color; with the Eyedropper it sets the background color; every other tool ignores the right button.
- [ ] AC-06 (event): When the Bucket is clicked on a pixel, every pixel 4-connected to it whose RGBA equals the clicked pixel's RGBA exactly is set to the color, pixels of any other value (and pixels connected only diagonally) stay unchanged, and the fill is one undo entry.
- [ ] AC-07 (event): When the Line tool is dragged from A to B and released, a 1-pixel line from A to B with both end pixels included is drawn in the color as one undo entry.
- [ ] AC-08 (state): While Shift is held when a Line drag is released, the committed line snaps from the start point by the offset (dx, dy) to the pointer: horizontal when |dx| > 2·|dy|, vertical when |dy| > 2·|dx|, otherwise at 45° with length max(|dx|, |dy|); Shift let go before the release leaves the line on the pointer. (The Pencil / Eraser Shift lock is a different rule: `pixel-pencil` AC-07.)
- [ ] AC-09 (state): While Shift is held when a Rectangle, Rectangle filled, Ellipse, Ellipse filled, Rounded Rectangle or Polygon drag is released, the shape's bounding box is a square whose side is the larger of the drag's width and height.
- [ ] AC-10 (always): A latched on-screen Shift toggle changes a Pixel-mode Line exactly as the held Shift key does.
- [ ] AC-11 (option): Where a Rectangle, Ellipse, Rounded Rectangle or Polygon is drawn in Outline mode with stroke width N, only a band N pixels wide inside the shape's edge is painted, in the stroke color, and the interior stays untouched.
- [ ] AC-12 (option): Where the shape mode is Fill + Outline, the edge band is painted in the stroke color and the interior in the fill color; where it is Fill, the whole shape is painted in the fill color.
- [ ] AC-13 (event): When a Shapes tool is drawn with the left button, the stroke color is the foreground color and the fill color is the background color; with the right button the two are swapped.
- [ ] AC-14 (option): Where the shape is a Rounded Rectangle (radius ≥ 2), an Ellipse or a Polygon (3–64 sides), the four corner pixels of the bounding box stay unpainted while the centre is painted.
- [ ] AC-15 (event): When Rectangle filled or Ellipse filled is dragged between two corners, the full rectangle or the full disk inscribed in it is painted in the drag color as one undo entry, and swapping the two corners gives the identical shape.
- [ ] AC-16 (event): When a Line or a shape extends beyond the canvas, the on-canvas pixels are painted, the rest is skipped, and no error is raised.
- [ ] AC-17 (event): When the Eyedropper is clicked on the canvas, the foreground color becomes the composited color of the visible layers at that pixel; a fully transparent pixel yields alpha 0.
- [ ] AC-18 (event): When the Text tool is clicked on the canvas, a text dialog opens anchored at the clicked pixel.
- [ ] AC-19 (event): When text is typed in the dialog, a live preview shows it on the canvas without changing the document or the undo history; when it is applied, the text is drawn in the foreground color with the chosen size, bold, italic, anti-alias and alignment as one undo entry.
- [ ] AC-20 (error): If applied text lands entirely outside the canvas, then no pixel changes and no undo entry is added.
- [ ] AC-21 (state): While a selection is active, the Text tool paints only pixels inside the selection.
- [ ] AC-22 (event): When a tool paints into a layer / frame that has no cel yet, a canvas-sized cel is created and the paint lands in it.
- [ ] AC-23 (always): Drawing tools paint into the active layer when it is an image layer; when the active layer is a group or tilemap layer, they paint into the lowest image layer instead.
- [ ] AC-24 (error): If the paint target layer or a group it is nested in is locked, then a drawing-tool press changes no pixel and the status bar shows "“{layer name}” is locked"; the notice stays until the next canvas press that is not refused (after unlocking, or with a tool that does not edit, such as the Eyedropper).
- [ ] AC-25 (event): When a tool-group key is pressed without Ctrl / Cmd / Alt and focus is not in a text field, that group's tool becomes active, and pressing the same key again cycles through the group's variants.

## Technical notes

> Paths and line numbers are hints from 2026-10-29 and drift — grep for the names.

**Tool ids** (`ui/src/modes/pixel/tools.ts`, `type Tool`): `'pencil' | 'eraser' | 'eyedropper' | 'bucket' | 'line' | 'rectangle' | 'rectangle-fill' | 'ellipse' | 'ellipse-fill' | 'rounded-rect' | 'polygon-shape' | … | 'text' | 'move' | 'tilemap-stamp' | 'slice'`. Rail groups and keys come from `toolGroups()` and `TOOL_GROUP_KEYS` in `ui/src/lib/shell/shortcuts.ts`: `B` pencil, `E` eraser, `G` fill, `I` eyedropper, `L` line, `U` shapes (cycles Rectangle → Rectangle filled → Ellipse → Ellipse filled → Rounded Rectangle → Polygon), `T` text. Default tool: `pencil`; default colors `#f87171` / `#ffffff` (`toolInit()` / `colorsInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts`).

**Pointer pipeline** (`ui/src/modes/pixel/pointer.ts`):

- `onPointerDown` → Pencil / Eraser set `live.painting` and call `paintAt`; Bucket calls `commitBucket` once per click; drag-shape tools (`isDragShapeTool`) record `live.dragStart` and commit on `onPointerUp`; Text calls `openTextAt`.
- `paintAt` opens the gesture with wasm `beginStroke(tool, color, size, shape, pixelPerfect)` on the first sample and sends every sample through `strokeTo(x, y)` (Shift: `directionTo` / `alongDirection`, `pixel-pencil` AC-07); the core stamps the line between samples and skips off-canvas pixels, other errors end the gesture and go to the status bar.
- `BG_TOOLS` = pencil, bucket, eyedropper, line and the six shapes; `strokeColor()` / `shapeFillColor()` pick fg / bg by `live.useBg` (right button).
- `constrainEndpoint()` (`tools.ts`) implements the Line / Shapes Shift constraint; `shiftHeld(e)` (`ui/src/lib/shell/modifiers.svelte.ts`) merges key and latched toggle.
- `lockedEditTarget()` refuses the press and sets `t('layers.locked', { name })` as `live.lockNotice`; the next press that is not refused clears it.
- `onPointerUp` calls `endStroke()` for paint strokes; `pointercancel` is bound to `onPointerUp`.

**wasm** (`crates/pincel-wasm/src/lib.rs`, `stroke.rs`, `shapes.rs`, `text.rs`), colors packed `0xRRGGBBAA`:

- `beginStroke` / `strokeTo` / `endStroke` (`stroke.rs`) drive one `PencilStroke` per gesture; `"eraser"` paints `Rgba::TRANSPARENT` whatever the color argument. `applyTool(tool_id, x, y, color)` (single pixel, `SetPixel`) stays for non-gesture callers.
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
| AC-01 | `ui/e2e/pencil.spec.ts › pixel mode: a fast pencil stroke stays connected`; `crates/pincel-wasm/src/stroke.rs › tests::stroke_to_jump_paints_connected_line`; `crates/pincel-core/src/command/pencil_stroke.rs › tests::pencil_stroke_size_1_matches_bresenham` | built | |
| AC-02 | `ui/e2e/pixel-tools.spec.ts › pixel mode: each pencil or eraser stroke is one undo step`; `crates/pincel-wasm/src/lib.rs › tests::eraser_drag_is_one_undo_entry_and_keeps_the_stroke_before_it`; `crates/pincel-wasm/src/stroke.rs › tests::stroke_gesture_is_one_undo_entry_and_redo_reproduces_it` | built | |
| AC-03 | `crates/pincel-wasm/src/lib.rs › tests::eraser_drag_is_one_undo_entry_and_keeps_the_stroke_before_it`, `tests::every_paint_tool_creates_a_canvas_sized_cel_on_a_fresh_layer_or_frame`; `crates/pincel-wasm/src/stroke.rs › tests::eraser_stroke_clears_the_pencil_footprint`; `ui/e2e/pixel-tools.spec.ts › pixel mode: each pencil or eraser stroke is one undo step` | built | |
| AC-04 | `ui/e2e/pixel-tools.spec.ts › pixel mode: a stroke across the canvas edge paints only the on-canvas part`; `crates/pincel-wasm/src/stroke.rs › tests::stroke_beyond_canvas_paints_in_bounds_part_without_error`; `crates/pincel-core/src/command/pencil_stroke.rs › tests::pencil_stroke_out_of_bounds_points_are_skipped` | built | |
| AC-05 | `ui/e2e/pixel-tools.spec.ts › pixel mode: the right button paints the background color` | built | |
| AC-06 | `crates/pincel-core/src/command/fill_region.rs › tests::fill_matches_the_seed_rgba_exactly`, `tests::fill_stops_at_a_color_boundary`, `tests::fill_is_four_connected_not_eight`; `crates/pincel-wasm/src/lib.rs › tests::apply_bucket_joins_the_undo_bus` | built | |
| AC-07 | `crates/pincel-core/src/command/draw_line.rs › tests::apply_paints_a_one_pixel_line_with_both_end_pixels`, `tests::apply_writes_horizontal_pixels`; `crates/pincel-wasm/src/lib.rs › tests::apply_line_joins_the_undo_bus` | built | |
| AC-08 | `ui/e2e/pixel-tools.spec.ts › pixel mode: Shift at release snaps a line by the 2:1 rule` | built | |
| AC-09 | `ui/e2e/pixel-tools.spec.ts › pixel mode: Shift at release squares every shape to the larger side` | built | |
| AC-10 | `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key` | built | |
| AC-11 | `crates/pincel-core/src/command/draw_shape.rs › tests::outline_and_fill_outline_paint_an_n_wide_edge_band_for_every_kind`, `tests::outline_rectangle_with_width_two_leaves_the_centre` | built | |
| AC-12 | `crates/pincel-core/src/command/draw_shape.rs › tests::outline_and_fill_outline_paint_an_n_wide_edge_band_for_every_kind`, `tests::fill_mode_paints_the_whole_shape_in_the_fill_colour`; `crates/pincel-wasm/src/shapes.rs › tests::apply_shape_fill_outline_rectangle` | built | |
| AC-13 | `ui/e2e/pixel-tools.spec.ts › pixel mode: shapes stroke with one color and fill with the other` | built | |
| AC-14 | `crates/pincel-core/src/command/draw_shape.rs › tests::rounded_ellipse_and_polygon_leave_all_four_bbox_corners_unpainted`; `crates/pincel-wasm/src/shapes.rs › tests::apply_shape_polygon_and_rounded_rectangle_and_errors` | built | |
| AC-15 | `crates/pincel-core/src/command/draw_rectangle.rs › tests::filled_rectangle_is_exactly_the_bbox_whichever_corner_comes_first`; `crates/pincel-core/src/command/draw_ellipse.rs › tests::filled_ellipse_is_the_full_disk_whichever_corner_comes_first`; `crates/pincel-wasm/src/lib.rs › tests::apply_rectangle_joins_the_undo_bus`, `tests::apply_ellipse_joins_the_undo_bus`; `ui/e2e/pixel-tools.spec.ts › pixel mode: the right button paints the background color` | built | |
| AC-16 | `crates/pincel-wasm/src/lib.rs › tests::lines_and_shapes_off_canvas_paint_the_on_canvas_part_without_error`; `crates/pincel-core/src/command/draw_line.rs › tests::apply_skips_pixels_outside_cel_silently`; `crates/pincel-core/src/command/draw_shape.rs › tests::shape_partially_off_cel_is_clipped_not_an_error` | built | |
| AC-17 | `crates/pincel-wasm/src/lib.rs › tests::pick_color_composites_the_visible_layers_only`, `tests::pick_color_returns_zero_on_transparent_pixel`; `ui/e2e/pencil.spec.ts › pixel mode: a fast pencil stroke stays connected`; `ui/e2e/pixel-tools.spec.ts › pixel mode: a stroke across the canvas edge paints only the on-canvas part` | built | |
| AC-18 | `ui/e2e/pixel-tools.spec.ts › pixel mode: the Text tool opens its dialog at the clicked pixel` | built | |
| AC-19 | `ui/e2e/pixel-tools.spec.ts › pixel mode: text previews live and places with its options as one undo step`; `crates/pincel-wasm/src/text.rs › tests::draw_text_applies_colour_size_bold_italic_anti_alias_and_alignment`, `tests::draw_text_paints_and_undoes_and_preview_leaves_state` | built | |
| AC-20 | `crates/pincel-wasm/src/text.rs › tests::draw_text_off_canvas_returns_false_and_leaves_no_undo_entry` | built | |
| AC-21 | `crates/pincel-core/src/command/draw_text.rs › tests::selection_keeps_the_glyph_pixels_inside_it`, `tests::selection_clips_and_off_canvas_is_a_noop` | built | |
| AC-22 | `crates/pincel-wasm/src/lib.rs › tests::every_paint_tool_creates_a_canvas_sized_cel_on_a_fresh_layer_or_frame`, `tests::paint_auto_creates_cel_on_a_fresh_layer`, `tests::paint_auto_creates_cel_on_a_fresh_frame` | built | |
| AC-23 | `crates/pincel-wasm/src/lib.rs › tests::group_or_tilemap_active_paints_into_the_lowest_image_layer`, `tests::pencil_paints_into_active_layer_cel`; `crates/pincel-wasm/src/stroke.rs › tests::stroke_skips_group_layer_to_paint_the_image_layer` | built | |
| AC-24 | `ui/e2e/pixel-tools.spec.ts › pixel mode: a locked layer refuses paint and says so until a press is accepted`; `crates/pincel-wasm/src/layer_ops.rs › tests::layer_in_a_locked_group_refuses_every_paint_tool_and_keeps_its_pixels`, `tests::locked_layer_refuses_paint_until_unlocked`; `crates/pincel-wasm/src/stroke.rs › tests::begin_stroke_on_locked_layer_errs_and_adds_no_undo_entry`; `ui/e2e/layers.spec.ts › pixel: a locked layer refuses paint` | built | |
| AC-25 | `ui/e2e/pixel-tools.spec.ts › pixel mode: single keys select the drawing tools and cycle the shapes`; `ui/e2e/shortcuts.spec.ts › pixel mode: the shared keymap selects tools and cycles variants` | built | |

## Decisions

- 2026-10-29: Extracted from the code as it stands; differences to `pincel.md` §5.2 are not ACs — see the extraction report.
- 2026-10-29: AC-04 was red against the code: `paintAt` swallowed only errors containing "pixel out of bounds", which the core's `PixelOutOfBounds` message ("pixel (x, y) is out of bounds for cel …") never contains — a stroke leaving the canvas reported "Paint failed", and one re-entering it dropped its whole segment. Fixed in `pointer.ts` (`PIXEL_OUT_OF_BOUNDS` matches the real message); the e2e test was red before the fix. Superseded by the merge of `pixel-pencil`: Pencil / Eraser drags now go through `beginStroke` / `strokeTo` and the core's `PencilStroke` skips out-of-bounds points (`pixel-pencil` AC-05), so the swallow is gone with the old per-pixel path; the same e2e test proves AC-04 on the new path.
- 2026-10-29: Merged with `pixel-pencil`. AC-01 now names size 1 (larger brushes: `pixel-pencil` AC-03); the Pencil / Eraser ACs (01–04, 22–24) are checked on the `beginStroke` / `strokeTo` / `endStroke` path the UI uses. Shift differs by tool: the Line (AC-08 here) snaps its endpoint at release by the 2:1 rule, the Pencil / Eraser (`pixel-pencil` AC-07) lock the nearest of eight directions (tan 22.5° boundary) while Shift is held; AC-08 says so, so the two specs don't contradict each other.
- 2026-10-29: AC-08 revised (same ID): the code snaps by a 2:1 rule (`|dx| > 2·|dy|` horizontal, `|dy| > 2·|dx|` vertical, else 45°), not by the nearest direction — between 22.5° and 26.6° the two differ. The AC now states the rule the code implements, and that Shift counts at the release.
- 2026-10-29: AC-14 revised (same ID): radius 1 does not cut the corner pixel — its centre lies within 1 px of the rounded rectangle's inner box (`SelectionMask::rounded_rect` covers by pixel centre); from radius 2 (> 1.71) the corners stay clear. The AC now starts at radius 2.
- 2026-10-29: AC-24 revised (same ID): unlocking does not clear the notice; the next canvas press that is not refused does (`live.lockNotice`), which also happens with the Eyedropper while the layer is still locked.
- 2026-10-29: Tests that were green at once (they pin existing behavior) were each shown to fail by a temporary break of the code they cover, then restored: fill tolerance ±1 (`fill_matches_the_seed_rgba_exactly`), a dropped line end pixel (`apply_paints_a_one_pixel_line_with_both_end_pixels`), stroke contraction by N+1 / Fill mode in the stroke colour / an ellipse covering its bbox (the three `draw_shape` tests), an exclusive fill end column (`filled_rectangle_…`), a narrowed disk row (`filled_ellipse_…`), text dropped under any selection (`selection_keeps_the_glyph_pixels_inside_it`), eraser writing its colour and sealing per pixel (`eraser_drag_…`, `every_paint_tool_…`), `pickColor` with hidden layers (`pick_color_composites_…`), a cel one pixel wider than the canvas (`every_paint_tool_…`), the topmost instead of the lowest image layer (`group_or_tilemap_…`), line / shape erroring off canvas (`lines_and_shapes_off_canvas_…`), a lock check ignoring parent groups (`layer_in_a_locked_group_…`), bold ignored (`draw_text_applies_…`); in the UI: no `endStroke()` on release (AC-02), Bucket dropped from `BG_TOOLS` (AC-05), the line threshold `>=` and the square side `min` (AC-08, AC-09), the fill color equal to the stroke color (AC-13), the text anchor shifted (AC-18), bold ignored, the background color used and the preview skipped (AC-19), no lock notice (AC-24), Ctrl / Alt not ignored and number fields not treated as text fields (AC-25), only the sample point painted (AC-01).
