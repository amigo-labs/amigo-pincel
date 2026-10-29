---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §9.4, pincel.md §15, pincel.md §17.4, pincel.md §18.2, pincel.md §18.5]
depends_on: [pixel-pencil]
---

# Pen pressure in Pixel mode

## Context

Pincel reads pen pressure only in Image mode. §18.5 lists "Pen pressure in Pixel mode" as a gap: Aseprite (since 1.2.18), LibreSprite (1.2), Pixelorama and Resprite all map pressure to the brush size of their pixel tools.

Pixel mode was left out on purpose. Decision Log 2026-10-12 ("Pen pressure in Image mode") ends with "Pixel mode is excluded on purpose", and §17.4 says "Pixel-mode tools stay pressure-free — a 1-px pencil has nothing to scale". The `pixel-pencil` spec introduces brush sizes 1–8, so there is now something to scale. This feature reverses the exclusion, which needs a new Decision Log entry (see Open questions).

With a pen, pressure scales the Pixel-mode Pencil and Eraser size between 1 and the chosen size. It is opt-in through the tool options, the same as in Image mode.

## Scope

### In scope

- Pen pressure scales the size of the Pixel-mode Pencil and Eraser stamps
- A pressure "Size" checkbox in the Pencil / Eraser options, reusing Image mode's pressure labels
- Interaction of pressure with pixel-perfect strokes
- Every pointer sample of a stroke contributes its own pressure value

### Out of scope

- Pressure-driven opacity — because pixel art has no partial alpha, so only size is scaled
- Pen tilt in Pixel mode — because tilt has no meaning for square or round stamps
- Pressure for the shape tools and the bucket — because the bucket has no size and shapes are drawn from press and release points, not from a per-sample brush
- Brush sizes, shapes and the stroke itself — because the `pixel-pencil` spec delivers them

## Acceptance criteria

- [ ] AC-01 (option): Where the pressure "Size" option is on, each stamp of a pen stroke at pressure p (0–1) with chosen size N has the size max(1, round(N × p)).
- [ ] AC-02 (event): When a stroke segment runs between two samples with different pressures, the stamp size along the segment follows the pressure interpolated linearly between the two samples.
- [ ] AC-03 (always): A mouse or touch stroke, and a pen stroke while the pressure "Size" option is off, paint exactly what the same stroke at full pressure paints.
- [ ] AC-04 (always): Pressure never changes a painted pixel's color or alpha: every pixel a Pencil stroke paints has exactly the foreground color, every pixel an Eraser stroke touches becomes fully transparent.
- [ ] AC-05 (event): When the same pen stroke is drawn once at light pressure and once at firm pressure with size 8, the light stroke covers fewer pixels than the firm one.
- [ ] AC-06 (state): While pixel-perfect is on and the effective size varies along a stroke, L-corner removal applies only within runs of consecutive path points whose effective size is 1.
- [ ] AC-07 (event): When the Pencil or the Eraser becomes the active tool, the options bar shows a "Pen pressure" group with a "Size" checkbox (the same labels and hint as Image mode) and no opacity checkbox.
- [ ] AC-08 (always): The pressure "Size" checkbox starts checked in a fresh editor session, as in Image mode.
- [ ] AC-09 (always): A pressure stroke is one undo entry; undo restores every touched pixel to its value from before the stroke and redo reproduces the identical result.
- [ ] AC-10 (error): If a pen reports a pressure outside 0–1, then it is clamped to 0–1, and a pressure of 0 still paints a size-1 stamp.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Image-mode pattern to copy**

- Core: `crates/pincel-image-core/src/tools/mod.rs` — `PressureDynamics { size, opacity }` (≈L57), `PenSamples` (≈L77, pressure per point; a missing entry means full pressure), `dab_radius(p)` (≈L179); pressure is clamped (≈L252) and interpolated linearly along a segment (≈L349).
- wasm: `crates/pincel-image-wasm/src/lib.rs` — `CommandSpec::PencilStroke` carries `pressure: Vec<f32>` plus `pressure_size` / `pressure_opacity` (≈L470–500); `PressureDynamics` is built at ≈L856–880.
- UI: `ui/src/modes/image/tools/pointer.ts` — `pressureOf(e)` is `e.pointerType === 'pen' ? clamp(e.pressure) : 1` (≈L54); on move it loops over `e.getCoalescedEvents()` building `points[]` and `pressure[]` (≈L385–405). Store fields `pressureSize: true` / `pressureOpacity: false` in `ui/src/modes/image/stores/editor.svelte.ts` (≈L172). Checkboxes in `ui/src/modes/image/components/toolbar/ToolOptions.svelte` (≈L75–84) with i18n keys `pressure.label`, `pressure.hint`, `pressure.size`, `pressure.opacity` — reuse them.

**Pixel mode today**

- `ui/src/modes/pixel/pointer.ts` reads neither `e.pressure` nor `getCoalescedEvents()`.

**Design (from the legacy plan)**

- `PencilStroke` (from `pixel-pencil`) gets `pressure: Vec<f32>` parallel to `path` (empty = full pressure); each Bresenham step stamps the size interpolated from its neighbouring samples.
- wasm: `strokeTo(x, y, pressure: f32)`; the UI passes `1.0` for mouse and touch.
- `pointer.ts` uses `getCoalescedEvents()` for pens and mice (smoother strokes too) and passes `pressureOf(e)` when the "Size" checkbox is on.
- Suggested tests: `pencil_stroke_light_pressure_paints_smaller_stamp`, `pencil_stroke_full_pressure_matches_plain_stroke`, `pixel_perfect_applies_only_to_size_1_runs`, a wasm test for the pressure argument, and e2e `pixel mode: a light pen stroke is thinner than a firm one` comparing element screenshots like the Image-mode test. Move the CDP pen helper `penStroke(page, target, force, tilt)` from `ui/e2e/pressure.spec.ts` (≈L8–33) into `ui/e2e/helpers.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Size 1 with pressure on | Every stamp stays size 1 whatever the pressure (AC-01) |
| Pen reports pressure 0 on a sample (e.g. at first contact) | A size-1 stamp is painted there (AC-10) |
| Pen stroke with the "Size" option off | Full chosen size along the whole stroke (AC-03) |
| Mouse stroke with the "Size" option on | Full chosen size along the whole stroke (AC-03) |
| Firm-to-light stroke at size 8 with pixel-perfect on | Corner removal acts only on the tail where the effective size is 1 (AC-06) |
| Right-button pen stroke with the Pencil | Paints the background color with pressure-scaled stamps |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The pressure "Size" setting is shared or per tool exactly as `pixel-pencil` settles for size and shape | The plan adds it next to the size control and doesn't say | Low |
| `round` rounds halves up (2.5 → 3) | The plan writes `round(size × p)` without a tie rule | Low |

## Open questions

- The Decision Log entry reversing "Pixel mode is excluded on purpose" (2026-10-12) is not written yet; it also has to update §17.4 ("Pixel-mode tools stay pressure-free"), the §18.2 table row (Pincel "Pen pressure" → ✓) and §18.5, and record the extra `pressure` argument on the `pixel-pencil` stroke API of the public `pincel-wasm` package.

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

- Only size is driven by pressure, no opacity dynamics — because pixel art has no partial alpha (legacy plan).
- Pressure follows the Image-mode model (per-sample values, linear interpolation along a segment, mouse and touch at full pressure, checkbox on by default) and reuses its i18n keys — because both modes should behave the same and the pattern is proven (legacy plan).
