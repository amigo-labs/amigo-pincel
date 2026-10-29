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

Pixel mode was left out on purpose. Decision Log 2026-10-12 ("Pen pressure in Image mode") ends with "Pixel mode is excluded on purpose", and §17.4 says "Pixel-mode tools stay pressure-free — a 1-px pencil has nothing to scale". The `pixel-pencil` spec introduces brush sizes 1–8, so there is now something to scale. This feature reverses the exclusion through a 2026-10-29 Decision Log entry (see Decisions).

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
- wasm: `strokeTo(x, y, pressure?: number)` — an optional `Option<f32>`, absent = 1, so the public npm API stays non-breaking; the UI passes `1.0` (or nothing) for mouse and touch.
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

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pencil_stroke_pressure_scales_stamp_size` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_to_pressure_scales_the_stamp` (`crates/pincel-wasm/src/stroke.rs`) | built |        |
| AC-02 | `pencil_stroke_pressure_interpolates_along_segment` (`crates/pincel-core/src/command/pencil_stroke.rs`) | built |        |
| AC-03 |      | open   |        |
| AC-04 | `pencil_stroke_random_pressure_strokes_hold_invariants` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_to_pressure_scales_the_stamp` (`crates/pincel-wasm/src/stroke.rs`) | built |        |
| AC-05 | `pencil_stroke_light_pressure_paints_fewer_pixels` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_to_light_pressure_covers_fewer_pixels` (`crates/pincel-wasm/src/stroke.rs`) | built |        |
| AC-06 | `pixel_perfect_applies_only_to_size_1_runs`, `pixel_perfect_keeps_corner_covered_by_a_larger_stamp`, `pencil_stroke_random_pressure_strokes_hold_invariants` (`crates/pincel-core/src/command/pencil_stroke.rs`) | built |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 | `pencil_stroke_random_pressure_strokes_hold_invariants` (`crates/pincel-core/src/command/pencil_stroke.rs`); `pressure_stroke_is_one_undo_entry_and_redo_reproduces_it` (`crates/pincel-wasm/src/stroke.rs`) | built |        |
| AC-10 | `pencil_stroke_pressure_is_clamped_to_unit_range` (`crates/pincel-core/src/command/pencil_stroke.rs`); `stroke_to_clamps_pressure_outside_unit_range` (`crates/pincel-wasm/src/stroke.rs`) | built |        |

## Decisions

- Only size is driven by pressure, no opacity dynamics — because pixel art has no partial alpha (legacy plan).
- Pressure follows the Image-mode model (per-sample values, linear interpolation along a segment, mouse and touch at full pressure, checkbox on by default) and reuses its i18n keys — because both modes should behave the same and the pattern is proven (legacy plan).
- 2026-10-29: A Decision Log entry supersedes "Pixel mode is excluded on purpose" (2026-10-12): Pencil / Eraser stamp size = max(1, round(N · p)), interpolated per Bresenham step, opt-in "Size" checkbox on by default like Image mode, and wasm `strokeTo(x, y, pressure?)` with an optional `Option<f32>` (absent = 1); `pincel.md` §17.4 ("Pixel-mode tools scale stamp size only, never opacity"), §18.2 (Pen pressure ✓) and §18.5 are updated — an optional argument keeps the npm API non-breaking and matches the `pixel-pencil` stroke API shape (owner accepted the recommendation).
- 2026-10-29 (spec-build): NaN pressure counts as no reading (full pressure), the way Image mode treats a missing entry — AC-10 only covers values outside 0–1, and NaN has no position on that range.
- 2026-10-29 (spec-build): Pixel-perfect acts wherever the effective stamp size is 1, so a light tail of a size-8 pen stroke is pruned while the chosen size is 8 (AC-06, edge-case table); a stroke at full pressure keeps `pixel-pencil` AC-10 unchanged. The Pixel-perfect checkbox stays disabled above size 1 as `pixel-pencil` AC-10 requires — its stored value still applies.
- 2026-10-29 (spec-build): A pruned corner is restored only when no other kept stamp of the stroke covers it (a per-pixel coverage count over all stamps, generalising `pixel-pencil`'s path count), so pixels a larger stamp painted earlier stay painted (`pixel-pencil` AC-09).
- 2026-10-29 (spec-build): `PencilStroke::new`, `PencilStroke::extend_to` and `Bus::continue_stroke` take the pressure as a plain `f32` (the wasm layer maps an absent value to 1.0), and `PencilStroke` drops its `Eq` derive because it now holds `f32` pressures.
