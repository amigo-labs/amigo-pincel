---
id: todo-pixel-pencil
title: Pixel-mode pencil — brush sizes, Shift constrain, pixel-perfect stroke
status: todo
size: L (three M sessions)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Pixel-mode pencil: brush sizes, Shift constrain, pixel-perfect stroke

## Goal

Make the Pixel-mode Pencil and Eraser what spec §5.2 promises and every competitor has:
- brush sizes 1–8;
- Shift to constrain the stroke to 0°, 45° or 90°;
- a **pixel-perfect** option that removes the L-shaped corner pixels from 1-px freehand strokes.

## Why

The competitive landscape (§18.2, §18.5) lists pixel-perfect as a gap. Aseprite, LibreSprite, Pixelorama, Pixaki, Resprite and Pro Motion all have it.

§5.2 (`docs/specs/pincel.md`, Pixel tool table) also says: "Pencil | Single-pixel line, brush-size variants 1–8, hold Shift = constrain to 0/45/90°". **Neither sizes nor Shift constrain exist today.**

## Current state (read these first)

**Core command.** `crates/pincel-core/src/command/set_pixel.rs`
- `SetPixel { layer, frame, writes: Vec<PixelWrite> }`: each write overwrites one pixel, with no size and no blending.
- `merge` (≈L161) appends the writes of the same layer and frame, so one drag is one undo entry.
- `revert` restores in reverse order, so a pixel painted twice gets its first prior value back.
- Out-of-bounds pixels are an **error**, and the UI swallows it. `DrawLine` skips them silently.

**Undo bus.** `crates/pincel-core/src/command/bus.rs`
- `seal()` ends a gesture.
- `execute` merges into the undo top unless the bus is sealed.
- `AnyCommand` dispatch is in `command/mod.rs` (≈L300).

**Bresenham.**
- In Rust: `BresenhamIter` / `bresenham()` in `crates/pincel-core/src/command/draw_line.rs` (≈L160–200, tests ≈L259).
- The pencil does **not** use it. The UI has its own TS copy, `linePoints()` in `ui/src/modes/pixel/tools.ts` (≈L175).

**wasm.** `crates/pincel-wasm/src/lib.rs`
- `applyTool(tool_id, x, y, color)` (≈L553) does one call per pixel. It handles `"pencil"` and `"eraser"`.
- Its doc comment says spec §9.3 wants a richer options struct.
- `endStroke()` (≈L801) calls `bus.seal()`.

**UI.**
- `ui/src/modes/pixel/pointer.ts` `paintAt()` (≈L130–186): rasterizes the segment between pointer samples with `linePoints` and calls `applyTool` once per pixel.
- pointerdown (≈L311) and pointerup / `endStroke` (≈L639) drive the stroke.
- `cancelGesture` (≈L50) undoes back to `live.gestureUndoDepth`.

**Options.**
- `toolInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L83) has no pencil size and no pixel-perfect flag.
- The options bar is the `{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L786–850). It has branches for the wand and the shapes, but none for pencil or eraser.
- `brushSmaller` / `brushLarger` (`[` / `]`) already exist in `ui/src/lib/shell/shortcuts.ts`. Pixel mode doesn't bind them yet.

**Tests to keep green.**
- pincel-core: the `SetPixel` tests (set_pixel.rs ≈L199–395).
- pincel-wasm:
  - `apply_tool_*` tests (lib.rs ≈L2218–2490)
  - `pencil_paints_into_active_layer_cel` (≈L3901)
- e2e: `ui/e2e/pencil.spec.ts` ("a fast pencil stroke stays connected").

## Design

### 1. A stroke command in the core

Add `PencilStroke` in `crates/pincel-core/src/command/pencil_stroke.rs`, with a new `AnyCommand` variant. It replaces the per-pixel `SetPixel` path for both pencil and eraser.

Fields:
- `layer`, `frame`
- `color: Rgba` (`TRANSPARENT` for the eraser)
- `brush: PixelBrush { size: u8 /* 1–8 */, shape: Square | Round }`
- `pixel_perfect: bool`
- `path: Vec<Point>`
- `prior: HashMap<(i32, i32), Rgba>`, the first-touch value of every pixel, used for revert

Behavior:
- `extend(to: Point)` rasterizes the segment from the last point with `BresenhamIter` and stamps the brush at each step.
- Pixels outside the cel are **skipped, not errors**, the same as `DrawLine`.
- **Pixel-perfect (size 1 only)**, following Aseprite: on each new path point `c`, look at the last three points `a`, `b`, `c`. If `a`→`c` is a diagonal step (|dx| = |dy| = 1) and `b` is orthogonally adjacent to both, then `b` is an L-corner: restore `b` from `prior` (unless the stroke painted it earlier) and drop it from the path.
  - This needs the whole stroke in one command, which is why `SetPixel` merging can't do it.
- Undo and redo:
  - `revert` writes `prior` back.
  - Redo replays the path, so `apply` must be deterministic.
- Merging: each wasm call is a `PencilStroke` segment, and `merge` folds the segments into the gesture's command until `seal()`. Alternatively, keep one live command on the bus (see 2).

### 2. A wasm API that sends points, not pixels

Recommendation: a gesture API on `Document`.
- `beginStroke(tool: &str, color: u32, size: u8, shape: &str, pixelPerfect: bool)`
- `strokeTo(x: i32, y: i32)`
- `endStroke()`, which already exists.

This removes the TS `linePoints` loop and the "pixel out of bounds" swallowing. Keep `applyTool` working for the single-pixel tests until callers have moved, then drop it.

This is a **new public API** (CLAUDE.md §3.3 stopping point) and needs a **Decision Log entry** that also updates §9.3, because spec §17.5 makes `pincel-wasm` a public npm API.

### 3. Shift constrain

Handle it in the UI (`pointer.ts`):
- While Shift is held, the stroke target snaps to the nearest of 0°, 45° and 90° from the **stroke's anchor point** (the pointerdown position, or the last point before Shift went down).
- Then call `strokeTo` with the snapped point.
- The latching Shift toggle for touch (`shiftHeld`) works the same way.

### 4. Options bar and keys

- Pencil and Eraser get **Size** (1–8, number + slider), **Shape** (square / round) and **Pixel-perfect** (checkbox, only enabled at size 1).
- `[` / `]` change the size.
- The options live in `toolState` (`editor.svelte.ts`).
- Optional: persist them through the IndexedDB `prefs` store, following the `lastColor` pattern in `PixelEditor.svelte` (≈L470–540). Never use localStorage (CLAUDE.md §9).
- Add i18n keys to `en.ts` and `de.ts` together.

## Tasks (one session each)

1. **M: core `PencilStroke`.**
   - Command plus `AnyCommand` variant: size and shape stamps, skipping out-of-bounds pixels, pixel-perfect corner removal, revert / redo / merge.
   - Test-first (CLAUDE.md §2.3).
   - Unit tests:
     - `pencil_stroke_size_1_matches_bresenham`
     - `pencil_stroke_size_3_square_stamps_3x3`
     - `pencil_stroke_round_shape_skips_corners`
     - `pixel_perfect_removes_l_corner`
     - `pixel_perfect_keeps_corner_painted_earlier_in_stroke`
     - `pencil_stroke_revert_restores_first_touch_values`
     - `pencil_stroke_out_of_bounds_points_are_skipped`
   - Add a proptest: a pixel-perfect stroke never contains three path pixels forming an L, and revert always restores the original cel (CLAUDE.md §7.3).
2. **M: wasm gesture API plus spec.**
   - `docs(spec)` commit first: Decision Log entry and an update to §9.3 and §5.2.
   - Then `beginStroke` / `strokeTo` / `endStroke`, with wasm tests mirroring the existing `apply_tool_*` ones (undo entry per gesture, two gestures make two entries, eraser, locked layer, group layer refused).
3. **M: UI.**
   - `pointer.ts` moves to the gesture API. Delete `linePoints` if nothing else uses it; check `tools.ts` callers first.
   - Shift constrain, options-bar branch, `[` / `]` bindings, i18n.
   - e2e in `ui/e2e/pencil.spec.ts`:
     - a size-3 click paints a 3×3 block (read back with the Eyedropper, as the existing test does);
     - a pixel-perfect diagonal staircase drag leaves the corner pixel empty;
     - a Shift-drag stays on a horizontal line.

## Out of scope

- Custom brushes (§13), pressure (`02-pixel-pressure.md`, which builds on this), shading ink and dither (`08-pixel-tools.md`) and symmetry (`07-symmetry-tiled-mode.md`).
- Do keep `PencilStroke`'s stamping in one function so symmetry can later stamp mirrored points through the same path.

## Open questions for the session to settle (stop and ask if unclear)

- Should pixel-perfect be on by default? Aseprite defaults to off.
- Should the eraser get pixel-perfect too? Recommendation: yes, it's the same command.
