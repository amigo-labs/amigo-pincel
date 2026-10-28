---
id: todo-symmetry-tiled-mode
title: Symmetry drawing and tiled mode in Pixel mode
status: todo
size: L (two M sessions)
mode: Pixel
depends_on: [todo-pixel-pencil]
created: 2026-10-28
---

# Symmetry drawing and tiled mode in Pixel mode

## Goal

1. **Symmetry.** Pencil and Eraser strokes are mirrored across a horizontal axis, a vertical axis, or both. The axes are shown on the canvas and are movable.
2. **Tiled mode.** View ▸ Tiled Mode (None / X / Y / Both):
   - draws the canvas repeated around itself;
   - **painting wraps** across the edges, so seamless textures and tiles can be authored directly.

## Why

Aseprite and Pixelorama have both features. Krita has wrap-around mode. Tiled mode matters for game assets (floors, backgrounds and tiles), which are Pincel's focus. It is already a known request: §17.3, row #402, "Tiling / seamless mode | Phase 2; Aseprite-style canvas wrap-around for seamless authoring".

## Prerequisite

`01-pixel-pencil.md`: the core `PencilStroke` command and the `beginStroke` / `strokeTo` / `endStroke` wasm API. Both features hook into the stamping inside that command. With today's one-`applyTool`-per-pixel path, they would have to be faked in TS and would cost undo steps.

## Current state (read these first)

**Stroke path today.** `ui/src/modes/pixel/pointer.ts`
- `paintAt()` (≈L130–187) calls `applyTool` per pixel and **swallows "pixel out of bounds"**. Tiled mode needs those points wrapped instead.
- pointerdown ≈L203, move ≈L481, up ≈L546.

**Rendering.** `ui/src/modes/pixel/render.ts`
- `recompose()` (≈L71) runs `ed.doc.compose(frame, 1)` and then `renderer.draw`.
- `overlayState()` (≈L84), `paintOverlays` (≈L120), dirty recompose (≈L130–141).
- Renderers are in `ui/src/lib/render/`: WebGPU and Canvas2D.

**Overlays.** `ui/src/modes/pixel/overlay-painter.ts`: `OverlayState` (≈L29–50) and `paintOverlays` (≈L53). Axis lines go here.

**View state.** `viewInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L62–80) holds zoom, pan and the overlay canvas. It has no symmetry or tiled fields yet.

**View menu.** `ui/src/modes/pixel/menus.ts` (≈L224–235). The **Probe** checkbox item (`checked: c.probeOn`) is the pattern for a toggle.

**Absence confirmed.** No symmetry, mirror or wrap code exists. `render/onion.rs` ≈L17 says "no wrap-around".

## Design

### Symmetry

**State.** It lives with the tool options, not the view:
- `symmetry: 'none' | 'x' | 'y' | 'xy'`
- `symmetryX` / `symmetryY` in **half-pixel units**, so the axis can sit on a pixel edge or a pixel centre.
- Default: the canvas centre, which is a pixel edge for even sizes.

**Core.** `PencilStroke` gets `mirror: Option<Mirror { axis_x2: Option<i32>, axis_y2: Option<i32> }>`.
- Every stamped pixel is also stamped at its mirror image or images: `x' = axis_x2 − 1 − x` for an edge axis, generalized through the half-pixel coordinate.
- Pixel-perfect corner removal runs on the primary path, and the mirrored pixels follow it.
- One stroke, one undo step.

**UI.**
- A symmetry toggle group in the Pencil / Eraser options bar (none / horizontal / vertical / both).
- Axis lines in the overlay. Dragging an axis moves it (snaps to half pixels); double-click resets it to the centre.
- i18n keys.

**Scope.** Pencil and Eraser only, like Aseprite's default. Shapes and the bucket are out of scope.

### Tiled mode

**State.** `view.tiled: 'none' | 'x' | 'y' | 'xy'`. Optionally persisted per document; recommendation: keep it session-only.

**Rendering.**
- Draw the composited frame again at the neighbouring offsets: ±W on X, ±H on Y, and the diagonals for `xy`.
- Draw the copies at reduced opacity, or with a subtle outline around the real canvas, so it's clear which one is editable.
- Implement this in **both** renderer adapters (`ui/src/lib/render/`) as "draw the same texture N times". Composition stays one `compose()` call.
- Never read canvas pixels back (CLAUDE.md §9).

**Painting.**
- `PencilStroke` gets `wrap: { x: bool, y: bool }`: stamped coordinates are taken modulo the cel or canvas size instead of being skipped.
- In the UI, `spriteCoord()` accepts points outside the canvas when tiled mode is on and hands them over unwrapped. Wrapping happens in the core so Bresenham segments stay continuous across the seam.

**Cels smaller than the canvas** (Aseprite cels have their own bounds): wrap against the **canvas**, writing into the cel only after the core's `ensure_paint_cel` has grown it to the canvas. Check how `ensure_paint_cel` (`crates/pincel-wasm/src/lib.rs`) handles this, and record the choice.

## Tasks (one M session each)

1. **`docs(spec)` + symmetry.**
   - §5.2 tool table (Pencil / Eraser symmetry option), plus a Decision Log entry for half-pixel axes and Pencil / Eraser only.
   - Core mirror stamping. Tests:
     - `mirror_x_even_width_axis_on_edge`
     - `mirror_xy_stamps_four_points`
     - `mirror_with_pixel_perfect_follows_primary_path`
     - `mirror_stroke_is_one_undo_step`
   - wasm option and UI options bar, overlay axes, axis drag and i18n.
   - e2e: a symmetry-X stroke on the left half also paints the mirrored pixel (Eyedropper read-back as in `ui/e2e/pencil.spec.ts`).
2. **Tiled mode.**
   - Wrap in the core. Tests:
     - `wrap_x_stroke_across_right_edge_continues_on_left`
     - `wrap_off_skips_out_of_bounds`
   - Renderer repeat in both adapters, the View ▸ Tiled Mode submenu and i18n.
   - e2e: tiled X is on, a drag from inside the canvas past the right edge paints a pixel at the left edge; screenshot the tiled view (no pixel readback in app code; tests may screenshot, §7.4).

## Out of scope

- Radial or rotational symmetry (Pixelorama has it; a later option).
- Symmetry for shapes and fill.
- Isometric tiled preview.
- Tiled mode in Image mode.
