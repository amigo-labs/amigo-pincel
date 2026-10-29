---
status: todo
created: 2026-10-28
size: L
mode: Pixel
design: [pincel.md §4.4, pincel.md §5.2, pincel.md §17.1, pincel.md §17.3]
depends_on: [pixel-pencil]
---

# Symmetry drawing and tiled mode

## Context

Two drawing aids Aseprite and Pixelorama have (Krita has a wrap-around mode) and Pincel lacks:

1. **Symmetry.** Pencil and Eraser strokes are mirrored across a vertical axis, a horizontal axis, or both. The axes are shown on the canvas and can be moved.
2. **Tiled mode.** View ▸ Tiled Mode (None / X / Y / Both) draws the canvas repeated around itself, and **painting wraps** across the edges, so seamless textures and tiles can be authored directly.

Tiled mode matters for game assets (floors, backgrounds, tiles), Pincel's focus. It is a known request: §17.3 row #402, "Tiling / seamless mode | Phase 2; Aseprite-style canvas wrap-around for seamless authoring".

Both features hook into the stamping of the Pencil / Eraser stroke from the `pixel-pencil` spec. With today's one-call-per-pixel path they would have to be faked in the UI and would cost undo steps.

## Scope

### In scope

- Symmetry for the Pixel-mode Pencil and Eraser: off, left–right (vertical axis), top–bottom (horizontal axis), or both
- Axis positions in half-pixel steps (on a pixel edge or a pixel centre), shown on the canvas, movable, resettable to the centre
- A symmetry control in the Pencil / Eraser options
- Tiled mode None / X / Y / Both in the Pixel-mode View menu: repeated canvas display and Pencil / Eraser strokes that wrap across the canvas edges
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- Radial or rotational symmetry — because the legacy plan defers it to a later option (Pixelorama has it)
- Symmetry for shapes, Line and Bucket — because symmetry follows Aseprite's default of Pencil and Eraser only
- Isometric tiled preview — because it needs a diamond repeat layout, not the rectangular repeat this spec builds
- Tiled mode in Image mode — because stroke wrapping lives in the Pixel-mode core; Image mode has its own core and tools
- Wrapping for Line, shapes and Bucket — follow-up, because the design wraps only the stroke command, matching the symmetry scope (AC-11)

## Acceptance criteria

- [ ] AC-01 (option): Where left–right symmetry is on with the vertical axis at position A (in half pixels from the left canvas edge), every pixel (x, y) a Pencil or Eraser stroke paints is also painted at (A − 1 − x, y).
- [ ] AC-02 (option): Where top–bottom symmetry is on with the horizontal axis at position B (in half pixels from the top canvas edge), every pixel (x, y) a Pencil or Eraser stroke paints is also painted at (x, B − 1 − y).
- [ ] AC-03 (option): Where both symmetries are on, every painted pixel (x, y) is also painted at (A − 1 − x, y), (x, B − 1 − y) and (A − 1 − x, B − 1 − y).
- [ ] AC-04 (always): An axis that has not been moved sits at the canvas centre: A equals the canvas width and B equals the canvas height in half pixels, so it lies on the middle pixel edge for even sizes and through the middle pixel's centre for odd sizes.
- [ ] AC-05 (always): A symmetric stroke, mirrored pixels included, is one undo entry; undo restores every touched pixel to its value from before the stroke.
- [ ] AC-06 (option): Where symmetry and pixel-perfect are both on, the mirrored pixels are exactly the mirror images of the primary stroke's pixels after its L-corner removal.
- [ ] AC-07 (event): When a mirror image falls outside the canvas (an off-centre axis) and tiled mode is None, that mirrored pixel is skipped without an error.
- [ ] AC-08 (state): While symmetry is on and the Pencil or the Eraser is active, the active axis lines are drawn over the canvas; while symmetry is on, the axis handles (AC-09) are shown with any tool active; while symmetry is off, neither lines nor handles are drawn.
- [ ] AC-09 (event): When the user drags an axis handle — a small triangle in the canvas margin outside the sprite, above the canvas for the vertical axis and left of it for the horizontal axis — the axis moves in half-pixel steps with any tool active and later strokes mirror across the new position; when the user double-clicks a handle, its axis returns to the canvas centre. A press inside the canvas never moves an axis.
- [ ] AC-10 (event): When the Pencil or the Eraser becomes the active tool, the options bar shows a symmetry control with the choices none, left–right, top–bottom and both.
- [ ] AC-11 (state): While symmetry is on, the Line, shape and Bucket tools paint without mirroring.
- [ ] AC-12 (event): When the user opens View ▸ Tiled Mode, it offers None, X, Y and Both with the current choice checked, and the default is None.
- [ ] AC-13 (state): While tiled mode is X, Y or Both, the composited frame is also shown at the neighbouring offsets (left and right for X, above and below for Y, all eight neighbours for Both), and with None no copy is shown.
- [ ] AC-14 (state, manual: visual design): While tiled mode is on, the repeated copies are visually distinguishable from the editable canvas.
- [ ] AC-15 (state): While tiled mode is X or Both, a Pencil or Eraser stroke that crosses the right or left canvas edge continues on the same row at the opposite edge, without a gap at the seam; the same holds vertically for Y or Both.
- [ ] AC-16 (state): While tiled mode is X or Both, pressing on a repeated copy paints the corresponding canvas pixel (its column taken modulo the canvas width); the same holds vertically for Y or Both.
- [ ] AC-17 (state): While tiled mode does not cover an axis (None, or the other axis only), stroke parts beyond the canvas along that axis are skipped, as without tiled mode.
- [ ] AC-18 (always): Tiled mode never changes the document: it is not written into the saved file, the composited frame and exports are identical with and without it, and a reload starts with None.
- [ ] AC-19 (always): A wrapping stroke is one undo entry; undo restores every touched pixel on both sides of the seam.
- [ ] AC-20 (event): When the canvas is resized, an axis that has not been moved stays at the new canvas centre, and a moved axis keeps its position, clamped to 0 … 2 × the new size in half pixels; when another document is opened or created, both axes return to the canvas centre.
- [ ] AC-21 (event): When a Pencil or Eraser stroke (mirrored or wrapped pixels included) stamps pixels outside the bounds of the target cel, then the cel grows to cover them — along a wrapped axis to the full canvas extent — and one undo restores both the cel's previous bounds and its pixels.
- [ ] AC-22 (state): While tiled mode is X, Y or Both, the Line, shape and Bucket tools clip at the canvas edges as without tiled mode and do not wrap.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- Stroke path: `ui/src/modes/pixel/pointer.ts` — `paintAt()` (≈L130) calls `applyTool` per pixel and swallows "pixel out of bounds"; tiled mode needs those points wrapped instead. `spriteCoord()` (≈L71) maps client coordinates through the canvas size; pointerdown ≈L203.
- Rendering: `ui/src/modes/pixel/render.ts` — `recompose()` (≈L71) runs `ed.doc.compose(frame, 1)` then `renderer.draw`; `overlayState()` (≈L84), `paintOverlays` (≈L120), dirty recompose (≈L130). Renderers in `ui/src/lib/render/` (`webgpu-renderer.ts`, `canvas2d-renderer.ts`); the `CanvasRenderer` interface (§4.4) draws one buffer into a sprite-sized canvas and has no repeat yet.
- Overlays: `ui/src/modes/pixel/overlay-painter.ts` — `OverlayState` (≈L29), `paintOverlays` (≈L53). Axis lines and the margin handles (AC-09) go here; the handles' hit-test runs before the tool's pointerdown.
- View state: `viewInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L62) — zoom, pan, overlay canvas; no symmetry or tiled fields.
- View menu: `ui/src/modes/pixel/menus.ts` (≈L223); the Probe checkbox item (`checked: c.probeOn`) is the toggle pattern.
- `crates/pincel-wasm/src/lib.rs` `ensure_paint_cel` (≈L1195) only creates a missing cel (canvas-sized, at the origin); it does not grow an existing smaller cel.
- No symmetry, mirror or wrap code exists; `crates/pincel-core/src/render/onion.rs` (≈L17) says "there is no wrap-around".

**Design (from the legacy plan)**

- Symmetry state lives with the tool options, not the view: `symmetry: 'none' | 'x' | 'y' | 'xy'`, `symmetryX` / `symmetryY` in half-pixel units, `null` while unmoved (= centre, so it follows resizes; AC-20). With an axis at half-pixel position A, the mirror of column x is A − 1 − x for both edge and centre axes.
- Core: `PencilStroke` gets `mirror: Option<Mirror { axis_x2: Option<i32>, axis_y2: Option<i32> }>`; every stamped pixel is also stamped at its mirror image(s); pixel-perfect runs on the primary path and the mirrored pixels follow it.
- Tiled state: `view.tiled: 'none' | 'x' | 'y' | 'xy'`.
- Tiled rendering: draw the composited frame again at ±W / ±H (and the diagonals for `xy`), in both renderer adapters, as "draw the same texture N times"; composition stays one `compose()` call. Copies at reduced opacity or with an outline around the real canvas. Never read canvas pixels back (CLAUDE.md §9).
- Tiled painting: `PencilStroke` gets `wrap: { x: bool, y: bool }`; stamped coordinates are taken modulo the canvas size instead of skipped. `spriteCoord()` accepts points outside the canvas while tiled and hands them over unwrapped; wrapping happens in the core so Bresenham segments stay continuous across the seam.
- Cel growth (AC-21): the stroke command itself grows the target cel to cover the stamped pixels before writing and keeps the prior cel bounds and pixels in its undo record; `ensure_paint_cel` is unchanged.
- Suggested tests: `mirror_x_even_width_axis_on_edge`, `mirror_xy_stamps_four_points`, `mirror_with_pixel_perfect_follows_primary_path`, `mirror_stroke_is_one_undo_step`, `wrap_x_stroke_across_right_edge_continues_on_left`, `wrap_off_skips_out_of_bounds`. e2e: a symmetry stroke on the left half also paints the mirrored pixel (Eyedropper read-back as in `ui/e2e/pencil.spec.ts`); with tiled X, a drag past the right edge paints a pixel at the left edge; an element screenshot of the tiled view (no pixel readback in app code, §7.4).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Odd canvas width 15, left–right symmetry, default axis | Axis runs through the centre of column 7; a pixel painted in column 7 mirrors onto itself (AC-01, AC-04) |
| Stroke painted exactly on the axis column with an edge axis | Mirror lands on the neighbouring column across the edge (AC-01) |
| Axis dragged near the right edge, stroke on the left half | Mirror images beyond the canvas are skipped (AC-07) |
| Symmetry and tiled X both on, mirror image beyond the canvas | The mirror image wraps like any other stamped pixel (see Assumptions) |
| Tiled X, stroke from inside past the right edge in one jump with no samples between | The line continues seamlessly at the left edge (AC-15) |
| Tiled Y only, stroke past the right edge | The part beyond the right edge is skipped (AC-17) |
| Size-8 stamp straddling the corner in tiled Both | Its parts wrap to all three other corners (AC-15) |
| Undo of a symmetric or wrapping stroke | One undo restores all pixels, mirrored and wrapped ones included (AC-05, AC-19) |
| Axis moved to the right edge, then the canvas is shrunk | The axis is clamped to the new right edge (AC-20) |
| Axis never moved, canvas resized from 16 to 15 wide | The axis follows to the new centre, through column 7 (AC-20) |
| Wrapping stroke into a 4×4 cel on a 32×32 canvas | The cel grows; one undo restores its 4×4 bounds and pixels (AC-21) |
| Line dragged past the right edge in tiled X | The part beyond the edge is clipped (AC-22) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| With symmetry and tiled mode both on, mirror images are computed first and then wrapped like any other stamped pixel | Wrapping is applied to stamped coordinates in the core, and mirroring produces stamped coordinates | Low |
| Symmetry settings (mode, axis positions) are session-only like tiled mode | The plan stores them with the tool options and mentions no persistence | Low |

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
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |

## Decisions

- 2026-10-29: Axis handles show with any tool while symmetry is on; axis lines only with Pencil / Eraser — because AC-09 lets any tool move an axis, so its handle must be visible (orchestrator, resolving the AC-08 / AC-09 gap).

- Symmetry applies to Pencil and Eraser only — because that is Aseprite's default and the stroke command is the single place where mirroring can stay one undo step (legacy plan).
- Axis positions use half-pixel units — because an axis must be able to sit on a pixel edge (even sizes) or a pixel centre (odd sizes) (legacy plan).
- Wrapping happens in the core, not in the UI — because only there do Bresenham segments stay continuous across the seam and the stroke stays one undo entry (legacy plan).
- Tiled mode is session-only view state, not stored in the document — because the plan recommends it and it is a viewing aid, not document content (legacy plan).
- The repeated copies are drawn from the one composited frame by both renderer adapters, never by reading canvas pixels back — because document state lives in Rust and the canvas is render-only (§17.1, CLAUDE.md §9).
- 2026-10-29: The Decision Log records that Pencil / Eraser strokes take optional `mirror { axis_x2, axis_y2 }` (half-pixel axes) and `wrap { x, y }`, applied in the core so a stroke stays one undo entry, that Line, shapes and Bucket neither mirror nor wrap, and that tiled mode is session-only view state; §5.2 and the §17.5 API listing are updated — these are new fields on the npm-facing stroke API (owner accepted the recommendation).
- 2026-10-29: Axes are grabbed by small handles in the canvas margin outside the sprite (a triangle above for the vertical axis, left for the horizontal one), draggable with any tool and reset by double-click (AC-09) — this is Aseprite's approach and never conflicts with a press inside the canvas (owner accepted the recommendation).
- 2026-10-29: An unmoved axis is stored as `null` (= centre) and follows resizes; a moved axis keeps its position, clamped to 0 … 2 × size; opening or creating another document resets both (AC-20) — the default stays centred, and an explicit choice survives where it still makes sense (owner accepted the recommendation).
- 2026-10-29: The stroke command grows a target cel smaller than the stamped extent and keeps the prior bounds and pixels in its undo record; `ensure_paint_cel` stays unchanged (AC-21) — Aseprite grows cels on paint, and keeping the growth in the stroke command keeps the one-undo-entry rule (owner accepted the recommendation).
- 2026-10-29: Only the Pencil and Eraser wrap in tiled mode; Line, shapes and Bucket clip as before (AC-22), and wrapping for them is a follow-up — the design wraps only the stroke command, matching the symmetry scope of AC-11 (owner accepted the recommendation).
