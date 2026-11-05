---
status: implemented
created: 2026-11-04
size: M
mode: both
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-10-29 colour slot, 2026-11-04 paint.net catalog)]
---

# paint.net Render effects: Clouds, Turbulence, Julia Fractal, Mandelbrot Fractal

## Context

paint.net's Render menu (paint.net/doc/latest/EffectsRenderMenu.html) generates images instead of filtering them: Clouds, Julia Fractal, Mandelbrot Fractal and Turbulence. Pincel has none of them. This spec adds all four.

Per-mode rule (`effect-catalog-v2`): Clouds and Turbulence are texture generators artists use for backgrounds and noise maps, also in pixel art (followed by Quantize or Posterize), so they appear in both modes; the fractals paint a fixed rainbow palette of hundreds of colours unrelated to a sprite's palette, so they stay Image-only. Clouds and Turbulence paint with two colours, using the colour parameter kind from `outline-effect` (Decision Log 2026-10-29).

**Terms.** *Target*, *opaque pixel*: as in `adjustments-paintnet`. *Rendered value*: the generator's value in 0–1 at a pixel. *Blend mode*: how the rendered colour combines with the existing pixel — Normal replaces each target pixel by the rendered colour including its alpha (a transparent Colour 2 leaves transparent pixels); Multiply, Screen and Overlay combine the rendered colour with the existing pixel's colour and keep its alpha. *Defaults* are the dialog's starting values; the wire always carries every value. *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Effects

| Wire id | Name (en / de) | Modes | Parameters: range (default) | What it does |
| ------- | -------------- | ----- | --------------------------- | ------------ |
| `clouds` | Clouds / Wolken | both | Scale 2–1000 (250), Roughness 0–100 (50), Seed random seed 0–9999 (1), Colour 1 colour (foreground), Colour 2 colour (background), Blend mode (Normal) | Seeded fractal value noise: octave k has feature size Scale / 2^k px (k = 0 … while the size is ≥ 1 px) and weight (Roughness / 100)^k, summed and normalised to 0–1, mapped from Colour 1 (0) to Colour 2 (1) |
| `turbulence` | Turbulence / Turbulenz | both | Scale 2–1000 (200), Octaves 1–8 (5), Seed random seed 0–9999 (1), Colour 1 (foreground), Colour 2 (background), Blend mode (Normal) | The absolute value of each noise octave summed (sharp ridges, marble-like), mapped between the two colours |
| `julia_fractal` | Julia Fractal / Julia-Fraktal | Image | Factor 1–10 (4), Zoom 1–50 (1), Angle angle (0), Quality 1–5 (2) | Renders the Julia set z ↦ z² + c (fixed c = −0.4 + 0.6i) over the view centred on 0 with half-height 2 / Zoom (rotated by Angle about the centre); a point that escapes after n of 256 iterations gets the hue (n · Factor · 7) mod 360 at full saturation and value, a point that never escapes is black; Quality q samples q × q points per pixel and averages them |
| `mandelbrot_fractal` | Mandelbrot Fractal / Mandelbrot-Fraktal | Image | Factor 1–10 (1), Zoom 1–100 (10), Angle angle (0), Quality 1–5 (2), Invert colours switch (off) | Renders the Mandelbrot set over the view centred on −0.5 + 0i with half-height 15 / Zoom, coloured as Julia Fractal; Invert colours inverts every result pixel |

The fractals are opaque everywhere they render. Clouds and Turbulence take the colours' alpha into account (a transparent Colour 2 gives see-through clouds).

## Scope

### In scope

- The four render effects in the Render submenu, per-mode as in the table
- Colour parameters defaulting to the current foreground / background colours
- en + de names and labels

### Out of scope

- Animated or tileable (seamless) noise — because a seamless option needs wrap-around noise and is better as a follow-up with Pixel mode's tiled mode (`symmetry-tiled-mode`)
- More blend modes than the four — because layer blend modes already offer the rest by rendering on a new layer
- Free choice of the Julia constant — because paint.net offers none either; Zoom and Angle explore the set

## Acceptance criteria

- [x] AC-01 (always): The Render submenu lists Clouds… and Turbulence… in both modes, and Julia Fractal… and Mandelbrot Fractal… in Image mode only, sorted as `effect-catalog-v2` AC-04 specifies.
- [x] AC-02 (event): When the user opens one of the four, then the effect dialog shows exactly its parameters from the table with their ranges and defaults; Colour 1 and Colour 2 start at the current foreground and background colours.
- [x] AC-03 (always): Each render effect gives the same pixels on every run for the same parameters and target size; Clouds and Turbulence give different pixels for a different seed.
- [x] AC-04 (option): Where Blend mode is Normal, every Clouds / Turbulence output pixel lies between Colour 1 and Colour 2 on each channel (inclusive), independent of the previous content; where Colour 1 equals Colour 2, the target is filled with that colour.
- [x] AC-05 (option): Where Blend mode is Multiply, no output channel is brighter than the existing pixel's; where Screen, none is darker.
- [x] AC-06 (always): Clouds with Roughness 0 has only one octave: it is smoother (a lower sum of absolute differences between neighbouring pixels) than Roughness 100 for the same seed.
- [x] AC-07 (always): The fractals fill every target pixel opaquely; Mandelbrot with Invert colours on gives each pixel the inverted colour of the result with it off.
- [x] AC-08 (always): Increasing Quality never changes the image size, and a pixel whose whole area lies inside the set (the Mandelbrot pixel at the view centre −0.5 + 0i at Zoom 10) stays black at every Quality.
- [x] AC-09 (always): Each render effect changes only the target: pixels outside the selection, other layers and cels stay unchanged; one undo step restores the target.
- [x] AC-10 (error): If a wire call names one of the four with the wrong number of parameters or an invalid colour slot, then it is rejected and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [x] AC-11 (error): If the Pixel-mode document is not RGBA, or the current frame has no cel on the active layer, then Clouds and Turbulence are refused with the same message as every other effect and the document is unchanged.
- [x] AC-12 (option): Where the UI language is German, the names, parameter labels and blend-mode options are German.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- New module `crates/pincel-effects/src/render/`; seeded value noise from the crate's seeded generator; fractal iteration count fixed at 256.
- Colour defaults from the editor's foreground / background: the catalog's colour kind (`outline-effect`) takes a default source. Escape-time colouring is plain (integer n), not smoothed, so interior pixels stay exactly black.
- The noise is positioned in layer coordinates, so the same seed gives the same texture regardless of the selection.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Render with a selection | Only selected pixels change; the texture is the same as without selection there |
| Clouds on a fully transparent layer, Blend Multiply | Multiply over transparent stays transparent |
| Scale larger than the layer | Large soft shapes; works |
| Mandelbrot Zoom 100 | Works; detail at the default centre |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Clouds and Turbulence in Pixel mode | They are texture generators; the owner's rule keeps "what makes sense", and generating a noise map before Posterize is a common pixel-art step | Medium: they create gradients |
| Fixed Julia constant | paint.net offers no constant either | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `image mode lists the four render effects, sorted; pixel mode Clouds and Turbulence (AC-01)` (ui/e2e/effects-render.spec.ts); `effect-menus.spec.ts` (Pixel mode exactly the catalog's Pixel entries) | verified | def5de7 |
| AC-02 | `the render dialogs show exactly the parameters of the table (AC-02)`, `pixel mode: Clouds previews and applies as one undo step`, `image mode: …` (ui/e2e/effects-render.spec.ts); pinned wire vectors `clouds` / `turbulence` / `julia_fractal` / `mandelbrot_fractal` (ui/e2e/effect-catalog.spec.ts) | verified | def5de7 |
| AC-03 | `render_effects_are_reproducible_and_clouds_and_turbulence_follow_the_seed`, `noise_follows_the_canvas_position_of_the_buffer` (crates/pincel-effects/tests/effects_render.rs) | verified | 1122d87 |
| AC-04 | `normal_blend_stays_between_the_two_colours_whatever_the_content`, `normal_blend_takes_the_colours_alpha` (crates/pincel-effects/tests/effects_render.rs) | verified | 1122d87 |
| AC-05 | `multiply_never_brightens_and_screen_never_darkens`, `multiply_over_transparency_stays_transparent` (crates/pincel-effects/tests/effects_render.rs); `combine_multiply_and_screen_with_black_and_white` (crates/pincel-effects/src/render/mod.rs) | verified | 1122d87 |
| AC-06 | `clouds_roughness_zero_is_smoother_than_roughness_hundred` (crates/pincel-effects/tests/effects_render.rs); `clouds_roughness_zero_uses_one_octave` (crates/pincel-effects/src/render/noise.rs) | verified | 1122d87 |
| AC-07 | `fractals_fill_every_pixel_opaquely_and_invert_inverts`, `fractals_paint_hues_outside_the_set_and_black_inside` (crates/pincel-effects/tests/effects_render.rs) | verified | 1122d87 |
| AC-08 | `quality_keeps_the_size_and_the_set_interior_stays_black`, `quality_supersamples_the_edge_of_the_set` (crates/pincel-effects/tests/effects_render.rs); `main_bulbs_shortcut_agrees_with_iterating` (crates/pincel-effects/src/render/fractal.rs) | verified | 1122d87 |
| AC-09 | `apply_effect_render_changes_only_the_target_in_one_undo_step`, `apply_effect_clouds_on_a_trimmed_cel_matches_the_canvas_texture` (crates/pincel-wasm/src/effects.rs); `apply_effect_render_changes_only_the_selection_in_one_undo_step` (crates/pincel-image-wasm/src/lib.rs) | verified | 31c1020 |
| AC-10 | `wrong_parameter_count_is_rejected`, `an_invalid_colour_slot_is_rejected`, `non_finite_numbers_read_as_zero_then_clamp_to_the_range` (crates/pincel-effects/tests/effects_render.rs); `apply_effect_render_wrong_parameters_change_nothing` (crates/pincel-wasm/src/effects.rs) | verified | 31c1020 |
| AC-11 | `apply_effect_render_on_an_indexed_document_or_without_a_cel_is_refused` (crates/pincel-wasm/src/effects.rs) | verified | 31c1020 |
| AC-12 | `German names, parameter labels and blend-mode options (AC-12)` (ui/e2e/effects-render.spec.ts) | verified | def5de7 |

## Summary

> Completed: 2026-11-05

- `pincel_effects::render`: Clouds (seeded fractal value noise) and Turbulence (gradient-noise ridges) between two colours with Normal / Multiply / Screen / Overlay blending, positioned in canvas coordinates (`EffectSpec::run_at` for trimmed cels)
- Julia Fractal and Mandelbrot Fractal: escape-time rainbow over 256 iterations, opaque, with Quality supersampling, Angle rotation and Mandelbrot's Invert colours
- Four new wire names (`clouds`, `turbulence`, `julia_fractal`, `mandelbrot_fractal`) in both cores and the agent catalog, rejected on a wrong parameter count or colour slot
- UI: Effects > Render lists Clouds and Turbulence in both modes and the fractals in Image mode only; colours default to the editor's foreground / background; en + de strings

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#8). Depends on `outline-effect` for the colour parameter kind.
- 2026-11-04: Clarification pass settled without a dialogue: Normal blend replaces including alpha; octave count and weights, fractal views and colouring pinned (no smooth colouring); fractals Image-only because of their fixed rainbow palette; Pixel mode refuses a render without a cel; non-finite numbers read as 0.

- 2026-11-05 (spec-build): Wire order: `clouds` [scale, roughness, seed, color1, color2, blend], `turbulence` [scale, octaves, seed, color1, color2, blend], `julia_fractal` [factor, zoom, angle, quality], `mandelbrot_fractal` [factor, zoom, angle, quality, invert]; blend as its option index (0 = Normal, 1 = Multiply, 2 = Screen, 3 = Overlay), rounded and clamped like every choice; colours as one `0xRRGGBBAA` slot each (`EffectError::ColorSlot` otherwise, also for non-finite values, as Outline Object does); Scale, Octaves, Quality and seeds rounded to whole numbers, Factor and Zoom kept fractional, the angle wrapped into 0–360 like the dial. The new names follow `twist` at the end of `EFFECT_NAMES`.
- 2026-11-05 (spec-build): Formulas the table leaves open: value noise is bilinear over a lattice of SplitMix64 values (per seed, octave and lattice point, quintic fade), sampled at pixel centres in canvas coordinates (`EffectSpec::run_at` passes a trimmed cel's position, like Pixelate); "normalised to 0–1" divides the weighted sum by the sum of the weights, not by the image's min / max, so the texture never depends on the target's size or selection. Turbulence uses gradient (Perlin) noise instead, because the absolute value of value noise shows its lattice as straight ridges (seen in the first build's rendering); its octave k has feature size Scale / 2^k (at least 1 px) and weight 1 / 2^k, each octave contributing |n| · √2, capped at 1 (gradient noise peaks near ±0.7). The rendered colour interpolates every straight-alpha channel from Colour 1 to Colour 2.
- 2026-11-05 (spec-build): Multiply, Screen and Overlay blend each RGB channel (Overlay: Multiply ×2 where the existing channel is below 0.5, Screen ×2 above), then mix the result into the existing pixel by the rendered colour's alpha and keep the existing alpha — because the Context says the colours' alpha counts ("see-through clouds") and the Terms say these modes keep the pixel's alpha; a fully transparent rendered colour leaves the pixel unchanged.
- 2026-11-05 (spec-build): Fractal view: the view's half-height maps to half the image height (square pixels), its centre to the image centre, the imaginary axis pointing up; Angle rotates the sampled point about the view centre. Escape means |z| > 2 within 256 iterations; Quality q averages q × q evenly spaced sub-samples in straight RGB. Mandelbrot skips iterating inside the main cardioid and the period-2 bulb (same result, faster). The Julia constant −0.4 + 0.6i lies just outside the Mandelbrot set, so the Julia rendering has no black interior at any zoom the table allows — AC-07 / AC-08's black interior is proven on Mandelbrot, as AC-08 names it.
- 2026-11-05 (spec-build): AC-09 – AC-11 needed no host code: the cores already keep effects to the selection (blending a soft selection by coverage), commit one undo step and refuse indexed documents and a missing cel ("no cel on layer … at frame …", the message of every effect) before running any effect. Their host tests were green at once; the red run came from the wire tests in `pincel-effects` (unknown effect) and, for the trimmed-cel texture, from `run_at` without the Clouds / Turbulence origin. The fractals run in the Pixel core too (the wire accepts every effect in both cores, `effect-catalog-v2` AC-08).
- 2026-11-05 (spec-build): The agent catalog names the colours `color1` / `color2` (`#RRGGBB[AA]`, defaults black / white — an agent has no foreground / background colour) and the blend mode by lower-case name (`normal` / `multiply` / `screen` / `overlay`); Invert colours is the boolean `invert`.
- 2026-11-05 (spec-build): Labels the table leaves open: Color 1 / Farbe 1, Color 2 / Farbe 2, Blend mode / Mischmodus with Normal / Normal, Multiply / Multiplizieren, Screen / Negativ multiplizieren, Overlay / Ineinanderkopieren (the layer blend modes' German names), Octaves / Oktaven, Factor / Faktor, Zoom / Zoom, Quality / Qualität, Invert colors / Farben umkehren; Scale, Roughness, Seed and Angle reuse the existing strings. Every number steps by 1; Angle uses the angle dial, Seed the random-seed control with Reseed.
- 2026-11-05 (spec-build): New public API: `pincel_effects::render::{Clouds, Turbulence, JuliaFractal, MandelbrotFractal, RenderBlend}` and `EffectSpec::{Clouds, Turbulence, JuliaFractal, MandelbrotFractal}`; `Clouds::with_origin` / `Turbulence::with_origin` as `Pixelate::with_origin`.
