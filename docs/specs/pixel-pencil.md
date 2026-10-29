---
status: implemented
created: 2026-10-28
size: L
mode: Pixel
design: [pincel.md §5.2, pincel.md §6.1, pincel.md §9.3, pincel.md §9.4, pincel.md §15, pincel.md §17.5, pincel.md §18.2, pincel.md §18.5]
---

# Pixel pencil: brush sizes, Shift constrain, pixel-perfect

## Context

The Pixel-mode tool table (`pincel.md` §5.2) promises "Pencil | Single-pixel line, brush-size variants 1–8, hold Shift = constrain to 0/45/90°". Neither brush sizes nor Shift constrain exist today: Pencil and Eraser paint exactly one pixel per sample.

The competitive landscape (§18.2, §18.5) lists **pixel-perfect** strokes as a gap. Aseprite, LibreSprite, Pixelorama, Pixaki, Resprite and Pro Motion all have it: on a 1-px freehand stroke the L-shaped corner pixels are removed so diagonals stay clean.

Already decided: a press-drag-release Pencil / Eraser stroke is one undo entry, delimited by `Bus::seal()` / wasm `endStroke()` (Decision Log 2026-07-09). `pincel-wasm` is a public npm API (§17.5), so any new stroke API on it needs a Decision Log entry and a §9.3 update.

This spec is the base for `pixel-pressure`, `symmetry-tiled-mode` and `shading-ink`, which extend the same stroke.

## Scope

### In scope

- Brush size 1–8 and brush shape (square / round) for the Pixel-mode Pencil and Eraser
- Shift constrain to 0°, 45° and 90° for Pencil and Eraser strokes, from the keyboard Shift key and from the latching on-screen Shift toggle
- A pixel-perfect option (off by default) that removes L-shaped corner pixels from size-1 freehand Pencil and Eraser strokes
- One size / shape / pixel-perfect setting shared by Pencil and Eraser, persisted across sessions
- Options-bar controls for size, shape and pixel-perfect, and the `[` / `]` keys to change the size
- Strokes that run partly outside the canvas paint their in-bounds part without an error
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- Custom brushes — because they are Phase 2 (§13.3)
- Pen pressure — because the `pixel-pressure` spec builds it on top of this one
- Shading ink and dithering — because the `shading-ink` and `dithered-gradient` specs cover them
- Symmetry and tiled-mode wrapping — because the `symmetry-tiled-mode` spec covers them; it reuses this spec's stamping
- Painting into indexed or grayscale cels — because indexed painting is Phase 2 (§3.7); Pencil and Eraser stay RGBA-only as today

## Acceptance criteria

- [x] AC-01 (option): Where the Pencil size is N (1–8) with the square shape, a single click paints a filled N×N block of the foreground color that contains the clicked pixel; for odd N the block is centred on the clicked pixel, and for even N the block spans `x − N/2 … x + N/2 − 1` and `y − N/2 … y + N/2 − 1` (integer division).
- [x] AC-02 (option): Where the Pencil shape is round, a single click at size 3 paints exactly the 5-pixel plus (the clicked pixel and its four orthogonal neighbours), and for every size N ≥ 3 the four corner pixels of the N×N bounding square stay unpainted.
- [x] AC-03 (event): When the pointer jumps between two samples of one Pencil drag with no samples in between, the painted result is the stamp applied at every pixel of the straight line between the two samples, with no gaps, at every size.
- [x] AC-04 (always): One press-drag-release Pencil or Eraser stroke adds exactly one undo entry; undo restores every touched pixel to its value from before the stroke, redo reproduces the identical result, and two strokes add two entries.
- [x] AC-05 (event): When a stroke or a stamp reaches beyond the canvas, the in-bounds pixels are painted, the out-of-bounds part is skipped, and no error or notice is shown.
- [x] AC-06 (option): Where the Eraser has size N and shape S, an Eraser click or drag clears to transparent exactly the pixels a Pencil click or drag with the same size, shape and path would paint.
- [x] AC-07 (state): While Shift is held (keyboard) or the on-screen Shift toggle is latched during a Pencil or Eraser drag, the segment painted from the anchor runs along whichever of the eight directions 0°, 45°, 90°, 135°, 180°, 225°, 270°, 315° is nearest to the pointer, where the anchor is the press position, or the last painted point before Shift went down.
- [x] AC-08 (option): Where pixel-perfect is on and the size is 1, a freehand stroke of the Pencil or the Eraser (which clears the path pixels) contains no L-corner — no painted path pixel b whose path neighbours a and c are diagonal to each other while b is orthogonally adjacent to both — and a removed corner pixel shows its value from before the stroke.
- [x] AC-09 (event): When pixel-perfect would remove a corner pixel that an earlier part of the same stroke had already painted, that pixel stays painted.
- [x] AC-10 (state): While the size is greater than 1, the pixel-perfect option has no effect on the stroke and its checkbox is disabled.
- [x] AC-11 (event): When the Pencil or the Eraser becomes the active tool, the options bar shows a Size control (number field plus slider, range 1–8), a Shape control (square / round) and a Pixel-perfect checkbox.
- [x] AC-12 (error): If a size outside 1–8 is typed into the Size field, then the size becomes the nearest bound (1 or 8).
- [x] AC-13 (event): When `[` or `]` is pressed while the Pencil or the Eraser is active, the size goes down or up by 1 and stays within 1–8.
- [x] AC-14 (error): If the stroke targets a locked layer or the document has no paintable image layer, then no pixel changes and no undo entry is added.
- [x] AC-15 (always): In a fresh session the Pixel-perfect checkbox is unchecked.
- [x] AC-16 (event): When the user changes size, shape or pixel-perfect with the Pencil active, then the Eraser shows the same values when it becomes active, and vice versa.
- [x] AC-17 (event): When the user changes size, shape or pixel-perfect and reloads the app, then the values survive the reload.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- `crates/pincel-core/src/command/set_pixel.rs` — `SetPixel { layer, frame, writes: Vec<PixelWrite> }`: one write per pixel, no size, no blending. `merge` (≈L161) appends writes of the same layer/frame so one drag is one undo entry; `revert` restores in reverse order. Out-of-bounds pixels are an error, which the UI swallows; `DrawLine` skips them silently.
- `crates/pincel-core/src/command/bus.rs` — `seal()` (≈L58) ends a gesture; `execute` (≈L64) merges into the undo top unless sealed. `AnyCommand` is in `command/mod.rs` (≈L138).
- Bresenham: `BresenhamIter` / `bresenham()` in `crates/pincel-core/src/command/draw_line.rs` (≈L160–210, tests ≈L259). The pencil doesn't use it; the UI has its own TS copy `linePoints()` in `ui/src/modes/pixel/tools.ts` (≈L180).
- `crates/pincel-wasm/src/lib.rs` — `applyTool(tool_id, x, y, color)` (≈L552) is one call per pixel for `"pencil"` / `"eraser"`; `endStroke()` (≈L801) seals the bus; `edit_target_layer` (≈L1174) refuses locked layers; `ensure_paint_cel` (≈L1195) seeds a canvas-sized empty cel.
- `ui/src/modes/pixel/pointer.ts` — `paintAt()` (≈L130) rasterizes between pointer samples with `linePoints` and calls `applyTool` per pixel; pointerdown ≈L203; `cancelGesture` (≈L49) undoes back to `live.gestureUndoDepth`. Right button with the Pencil paints the background color (`BG_TOOLS`, ≈L21).
- `toolInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L83) has no pencil size or pixel-perfect flag. The options bar is `{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L786); it has branches for wand and shapes, none for Pencil / Eraser.
- `brushSmaller` / `brushLarger` (`[` / `]`) exist in `ui/src/lib/shell/shortcuts.ts` (≈L68); Pixel mode doesn't bind them yet.
- Tests to keep green: `SetPixel` tests in `set_pixel.rs`; `apply_tool_*` tests (lib.rs ≈L2218–2500) and `pencil_paints_into_active_layer_cel`; e2e `ui/e2e/pencil.spec.ts` ("a fast pencil stroke stays connected", Eyedropper read-back pattern).

**Design (from the legacy plan)**

- A `PencilStroke` command in `crates/pincel-core/src/command/pencil_stroke.rs` with an `AnyCommand` variant replaces the per-pixel `SetPixel` path for Pencil and Eraser. Fields: `layer`, `frame`, `color: Rgba` (`TRANSPARENT` for the Eraser), `brush: PixelBrush { size: u8, shape: Square | Round }`, `pixel_perfect: bool`, `path: Vec<Point>`, `prior: HashMap<(i32, i32), Rgba>` (first-touch value per pixel, for revert).
- `extend(to)` rasterizes from the last point with `BresenhamIter` and stamps the brush per step; out-of-cel pixels are skipped like `DrawLine`. Keep the stamping in one function so symmetry and wrapping can reuse it.
- Pixel-perfect follows Aseprite: on each new path point c, look at a, b, c; if a→c is a diagonal step (|dx| = |dy| = 1) and b is orthogonally adjacent to both, restore b from `prior` (unless the stroke painted it earlier) and drop it from the path. This needs the whole stroke in one command, which `SetPixel` merging can't give.
- `revert` writes `prior` back; redo replays the path, so `apply` must be deterministic. Merging: each wasm call is a segment that `merge` folds into the gesture's command until `seal()`, or one live command stays on the bus.
- Decided wasm gesture API on `Document` (2026-10-29): `beginStroke(tool, color, size, shape, pixelPerfect)`, `strokeTo(x, y)`, `endStroke()` (exists). `pixel-pressure` later adds an optional third argument `pressure?: number` (`Option<f32>`) to `strokeTo`, which is non-breaking. It removes the TS `linePoints` loop and the out-of-bounds swallowing. `applyTool` stays for single pixels.
- Shift constrain lives in the UI (`pointer.ts`): snap the target to the nearest direction from the anchor, then send the snapped point.
- Options live in `toolState`, one setting shared by Pencil and Eraser, persisted as one IndexedDB pref `pixelBrush` `{ size, shape, pixelPerfect }` the way `lastColor` is stored, with defaults when the pref is missing; i18n keys go into `en.ts` and `de.ts` together.
- Suggested Rust tests: `pencil_stroke_size_1_matches_bresenham`, `pencil_stroke_size_3_square_stamps_3x3`, `pencil_stroke_round_shape_skips_corners`, `pixel_perfect_removes_l_corner`, `pixel_perfect_keeps_corner_painted_earlier_in_stroke`, `pencil_stroke_revert_restores_first_touch_values`, `pencil_stroke_out_of_bounds_points_are_skipped`, plus a proptest (no L-corner in a pixel-perfect stroke; revert always restores the original cel). wasm tests mirror `apply_tool_*` (one undo entry per gesture, two gestures two entries, eraser, locked layer, group layer skipped). e2e in `ui/e2e/pencil.spec.ts`: size-3 click paints 3×3, pixel-perfect staircase leaves the corner empty, Shift-drag stays horizontal.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Size-8 click on the corner pixel (0, 0) | Only the in-bounds part of the stamp is painted; no error (AC-05) |
| Pixel-perfect stroke that doubles back over itself | Pixels painted earlier in the stroke stay painted (AC-09) |
| Undo after a size-5 drag that crossed the same pixels several times | Every pixel returns to its value from before the stroke, not an intermediate one (AC-04) |
| Shift pressed in the middle of a drag | The constrained segment starts at the last painted point before Shift went down (AC-07) |
| Shift released in the middle of a drag | The stroke continues freehand from where it is |
| Pointer swings towards another direction while Shift stays down | The stroke keeps the 45° direction its first movement after the anchor chose; the pointer is projected onto that ray (AC-07). Releasing and pressing Shift again starts a new anchor and a new direction |
| Right-button drag with the Pencil at size 3 | Paints the background color with the same 3×3 stamp, as the right button does today |
| A second finger lands during a stroke (pinch starts) | The stroke is cancelled: the cel returns to its pre-stroke state and no undo entry remains |
| Stroke on a newly added layer that has no cel yet | A cel is created and the stroke paints into it |
| Pixel-perfect checked, then size raised to 3 | Checkbox is disabled, stroke is a plain size-3 stroke (AC-10) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Round masks for sizes 4–8 follow Aseprite's round brush | The plan models the feature on Aseprite and only fixes size 3 by a test | Low — masks can be adjusted without changing other behavior |
| Typed sizes outside 1–8 are clamped to the nearest bound | The control is a 1–8 slider plus number; the plan gives no error behavior | Low |
| A stroke whose pointer direction lies exactly between two Shift directions snaps to the horizontal or vertical one | The plan does not define ties | Low |

## Summary

> Completed: 2026-10-29

- `PencilStroke` command in `pincel-core`: square / round brush of size 1–8, Bresenham stamping between samples, out-of-canvas pixels skipped, pixel-perfect L-corner pruning at size 1, one undo entry per stroke with first-touch revert
- wasm gesture API `beginStroke` / `strokeTo` / `endStroke` drives one `PencilStroke` per Pencil / Eraser drag; `applyTool` stays for single pixels
- Pixel-mode options bar for Pencil and Eraser: Size (field + slider, clamped 1–8, `[` / `]`), Shape, Pixel-perfect (off by default, disabled above size 1), shared by both tools and persisted as the IndexedDB pref `pixelBrush`
- Shift constrain (key or latched touch toggle) to the eight 45° directions, anchored at the press or at the last point before Shift went down, direction locked until Shift is released

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pencil_stroke_square_click_paints_n_by_n_block`, `pencil_stroke_size_3_square_stamps_3x3`, `pencil_stroke_even_size_extends_top_left` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_size_3_click_paints_centred_block` (`crates/pincel-wasm/src/stroke.rs`); "a size-3 pencil click paints a 3x3 block" (`ui/e2e/pencil.spec.ts`) | verified | 6a7e87e |
| AC-02 | `pencil_stroke_round_shape_skips_corners` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_round_size_3_click_paints_plus` (`crates/pincel-wasm/src/stroke.rs`) | verified | 6a7e87e |
| AC-03 | `pencil_stroke_jump_stamps_every_line_pixel_at_every_size`, `pencil_stroke_size_1_matches_bresenham` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_to_jump_paints_connected_line` (`crates/pincel-wasm/src/stroke.rs`); "pixel mode: a fast pencil stroke stays connected" (`ui/e2e/pencil.spec.ts`) | verified | 6a7e87e |
| AC-04 | `pencil_stroke_revert_restores_first_touch_values`, `pencil_stroke_two_strokes_add_two_undo_entries`, `pencil_stroke_random_strokes_hold_invariants` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_gesture_is_one_undo_entry_and_redo_reproduces_it` (`crates/pincel-wasm/src/stroke.rs`) | verified | 6a7e87e |
| AC-05 | `pencil_stroke_out_of_bounds_points_are_skipped` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_beyond_canvas_paints_in_bounds_part_without_error` (`crates/pincel-wasm/src/stroke.rs`) | verified | 6a7e87e |
| AC-06 | `pencil_stroke_transparent_clears_pencil_footprint` (`crates/pincel-core/src/command/pencil_stroke.rs`); `eraser_stroke_clears_the_pencil_footprint` (`crates/pincel-wasm/src/stroke.rs`) | verified | 6a7e87e |
| AC-07 | "a Shift pencil drag snaps to the nearest 45 degree direction", "a Shift drag keeps the direction its first movement chose", "Shift pressed mid-drag anchors at the last painted point", "touch device › the latched Shift toggle constrains a pencil drag" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-08 | `pixel_perfect_removes_l_corner`, `pencil_stroke_random_strokes_hold_invariants`, `pencil_stroke_transparent_clears_pencil_footprint` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_pixel_perfect_drops_staircase_corners` (`crates/pincel-wasm/src/stroke.rs`); "a pixel-perfect staircase leaves its corners empty" (`ui/e2e/pencil.spec.ts`) | verified | 6a7e87e |
| AC-09 | `pixel_perfect_keeps_corner_painted_earlier_in_stroke` (`crates/pincel-core/src/command/pencil_stroke.rs`) | verified | 6a7e87e |
| AC-10 | `pixel_perfect_has_no_effect_above_size_1` (`crates/pincel-core/src/command/pencil_stroke.rs`); "pixel-perfect is disabled above size 1" (`ui/e2e/pencil.spec.ts`) | verified | 6a7e87e |
| AC-11 | "pencil and eraser show size, shape and pixel-perfect options" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-12 | "a typed size outside 1-8 clamps to the nearest bound" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-13 | "[ and ] step the brush size within 1-8" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-14 | `begin_stroke_on_locked_layer_errs_and_adds_no_undo_entry`, `begin_stroke_without_image_layer_errs` (`crates/pincel-wasm/src/stroke.rs`) | verified | ec6b135 |
| AC-15 | "pencil and eraser show size, shape and pixel-perfect options" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-16 | "pencil and eraser share one size / shape / pixel-perfect setting" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |
| AC-17 | "brush settings survive a reload" (`ui/e2e/pencil.spec.ts`) | verified | f730687 |

## Decisions

- Pencil and Eraser share one stroke command (the Eraser paints transparent) — because the behavior is identical apart from the color, and pixel-perfect needs the whole stroke in one command, which per-pixel `SetPixel` merging can't provide (legacy plan).
- Shift constrain is resolved in the UI before points reach the core — because it depends on input state (Shift key, latching touch toggle) the core does not know about (legacy plan).
- Out-of-bounds stamp pixels are skipped, not errors — because that matches `DrawLine` and the natural clipping of drawing tools (legacy plan).
- 2026-10-29: The wasm gesture API is `beginStroke(tool, color, size, shape, pixelPerfect)`, `strokeTo(x, y)` and `endStroke()`, driving one `PencilStroke` command; `applyTool` stays for single pixels, `pixel-pressure` later adds an optional `pressure` (`Option<f32>`) to `strokeTo`, and a Decision Log entry plus `pincel.md` §9.3 / §5.2 updates (sizes 1–8 square / round, pixel-perfect) land in the same commit — pixel-perfect needs the whole stroke in one command, and an `Option` parameter lets pressure arrive later without a second breaking change to the npm API (owner accepted the recommendation).
- 2026-10-29: Pixel-perfect is off by default (AC-15) — it matches Aseprite, avoids surprising missing pixels for new users, and keeps existing e2e tests valid (owner accepted the recommendation).
- 2026-10-29: The Eraser gets pixel-perfect too, through the same command and checkbox (AC-08) — it costs nothing extra, and Aseprite applies pixel-perfect to the eraser as well (owner accepted the recommendation).
- 2026-10-29: Pencil and Eraser share one size / shape / pixel-perfect setting (AC-16) — Image mode already shares `tool.size` and `tool.shape` between Pencil and Eraser, and Aseprite shares its brush across tools (owner accepted the recommendation).
- 2026-10-29: An even-size square stamp gets its extra row / column on the top and left: it spans `x − N/2 … x + N/2 − 1` (AC-01) — this is Aseprite's brush origin (`−size/2`), so strokes line up with what Aseprite users expect (owner accepted the recommendation).
- 2026-10-29: Size, shape and pixel-perfect persist across sessions in one IndexedDB `pixelBrush` pref, with defaults when it is missing (AC-17) — brush size is the option users set once, it is a few lines, and the colour-picker spec persists colours the same way (owner accepted the recommendation).
- 2026-10-29 (spec-build): The live stroke stays on the undo top and `Bus::continue_stroke(to, cels)` extends it, instead of each `strokeTo` merging a segment command — the bus applies a command before merging it, so a standalone segment would paint pixel-perfect corners the merge could no longer take back.
- 2026-10-29 (spec-build): `PencilStroke` replays its input samples on every `apply` and records first-touch values in `prior`, so redo is deterministic and undo restores pre-stroke values however often the stroke crossed a pixel.
- 2026-10-29 (spec-build): Pixel-perfect keeps a per-pixel count of the kept path; a corner is restored only when its count drops to zero, which is how "painted earlier in the same stroke" (AC-09) is decided.
- 2026-10-29 (spec-build): Round masks keep cell (i, j) while (2i−(N−1))² + (2j−(N−1))² < (N−1)² + 2(N−1) (sizes 1–2 are full squares) — it gives the 5-pixel plus at size 3 and cuts the corners at every N ≥ 3, close to Aseprite's round brushes (assumption row 1).
- 2026-10-29 (spec-build): A Shift-snapped diagonal runs as far as the longer of the two pointer deltas, like the Line tool's Shift snap; the axis wins when |minor| ≤ tan 22.5° · |major|.
- 2026-10-29 (spec-build): The random-stroke invariant test is a seeded LCG loop inside `pencil_stroke.rs` instead of `proptest`, because `proptest` is not a `pincel-core` dev-dependency and adding one needs approval.
- 2026-10-29 (spec-build): `beginStroke` takes `size` as `u32` and clamps it to 1–8, so an out-of-range JS number cannot wrap; the Shape control is a select (Square / Round) like the shape tools' mode select.
- 2026-10-29 (spec-build): E2e tests read pixels back with the Eyedropper (`#000000` = transparent); a pick of a transparent pixel also sets the foreground alpha to 0, so each spec paints before it picks.
- 2026-10-29 (spec-build): New public API — `pincel-core`: `PencilStroke`, `PixelBrush`, `BrushShape`, `MIN_BRUSH_SIZE`, `MAX_BRUSH_SIZE`, `AnyCommand::PencilStroke`, `Bus::continue_stroke`; `pincel-wasm`: `Document.beginStroke`, `Document.strokeTo`.
- 2026-10-29 (spec-check rework): While Shift is down, the first movement away from the anchor locks the nearest 45° direction, and every later sample is projected onto that ray from the anchor until Shift is released; releasing and re-pressing Shift starts a new anchor. Snapping each sample afresh let a direction change mid-drag draw a segment between two rays that ran in none of the eight directions (orchestrator decision).
