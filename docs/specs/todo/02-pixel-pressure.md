---
id: todo-pixel-pressure
title: Pen pressure in Pixel mode
status: todo
size: M (one session)
mode: Pixel
depends_on: [todo-pixel-pencil]
created: 2026-10-28
---

# Pen pressure in Pixel mode

## Goal

With a pen, pressure scales the Pixel-mode Pencil and Eraser size between 1 and the chosen size. This is opt-in, the same as in Image mode.

## Why

The gap is listed in §18.5. Aseprite (since 1.2.18), LibreSprite (1.2), Pixelorama and Resprite all map pressure to brush size in their pixel tools. Pincel has pressure only in Image mode.

Pixel mode was left out deliberately. Decision Log, 2026-10-12, "Pen pressure in Image mode", rationale: "Spec §17.4 promised pressure from the start; scaling per dab inside the existing rasterizer needs no new command or undo shape, and old command JSON stays valid. **Pixel mode is excluded on purpose**".

That rationale doesn't say *why*. The likely reason is that the Pixel pencil had no size, so there was nothing to scale (see `01-pixel-pencil.md`). This task needs a **new Decision Log entry that reverses that exclusion**: say that 01 introduced sizes and that the competitors all offer it.

## Prerequisite

`01-pixel-pencil.md` has to be done first: brush sizes 1–8, the `PencilStroke` command, and the `beginStroke` / `strokeTo` / `endStroke` wasm API.

## Copy the Image-mode pattern

**Core.** `crates/pincel-image-core/src/tools/mod.rs`
- `PressureDynamics { size, opacity }` (≈L57)
- `PenSamples` (≈L77): pressure per point, where a missing entry means full pressure.
- `dab_radius(p)` (≈L178)
- Pressure is clamped (≈L252) and interpolated linearly along a segment (≈L349).

**wasm.** `crates/pincel-image-wasm/src/lib.rs`
- `CommandSpec::PencilStroke` carries `pressure: Vec<f32>` plus `pressure_size` / `pressure_opacity` (≈L470–500).
- `PressureDynamics` is built at ≈L856–880.

**UI.**
- `ui/src/modes/image/tools/pointer.ts`:
  - `pressureOf(e)` is `e.pointerType === 'pen' ? clamp(e.pressure) : 1` (≈L53).
  - On move it loops over `e.getCoalescedEvents()` and builds `points[]` and `pressure[]` (≈L385–405).
- Store fields `pressureSize: true` and `pressureOpacity: false` are in `ui/src/modes/image/stores/editor.svelte.ts` (≈L171).
- The checkboxes are in `ui/src/modes/image/components/toolbar/ToolOptions.svelte` (≈L75–84), with i18n keys `pressure.label`, `pressure.hint`, `pressure.size` and `pressure.opacity`. **Reuse these keys.**

**Pixel mode today.**
- `ui/src/modes/pixel/pointer.ts` reads neither `e.pressure` nor `getCoalescedEvents()`.

## Design

**Size from pressure.**
- Effective size per path point is `max(1, round(size × p))`.
- Pixel art has no partial alpha, so there is **no opacity dynamics**. Only offer "Size".

**Core.** `PencilStroke` gets a `pressure: Vec<f32>` that parallels `path` (empty means full pressure) and stamps each Bresenham step with the size interpolated from its neighbouring samples.

**Pixel-perfect.** It applies only where the effective size is 1. When pressure is on, decide per point: corner removal runs only on runs of size-1 points.

**wasm.** `strokeTo(x, y, pressure: f32)`. Pass `1.0` for a mouse.

**UI.**
- `pointer.ts` uses `getCoalescedEvents()` for both pens and mice. That also gives smoother strokes, which is good on its own.
- It passes `pressureOf(e)` when the "Size" checkbox in the Pencil / Eraser options is on.
- The checkbox defaults to **on** for pens, the same as Image mode.

## Tasks

1. **`docs(spec)`.**
   - Decision Log entry reversing the Pixel-mode exclusion.
   - Update the §18 table row (Pincel "Pen pressure" → ✓) and §18.5.
2. **`feat(pincel-core)`.**
   - Pressure in `PencilStroke`.
   - Tests:
     - `pencil_stroke_light_pressure_paints_smaller_stamp`
     - `pencil_stroke_full_pressure_matches_plain_stroke`
     - `pixel_perfect_applies_only_to_size_1_runs`
3. **`feat(pincel-wasm)`.** The `strokeTo` pressure argument, plus a wasm test.
4. **`feat(ui)`.** Coalesced events, the pressure checkbox and i18n reuse.
   - e2e: move the CDP pen helper `penStroke(page, target, force, tilt)` from `ui/e2e/pressure.spec.ts` (≈L8–33, `Input.dispatchMouseEvent` with `pointerType: 'pen'` and `force`) into `ui/e2e/helpers.ts`.
   - Add `pixel mode: a light pen stroke is thinner than a firm one`, which compares screenshots the way the Image-mode test does.

## Out of scope

- Tilt in Pixel mode: no meaning for square or round stamps.
- Pressure for shapes and the bucket.
