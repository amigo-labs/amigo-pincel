---
status: todo
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

- [ ] AC-01 (always): The Render submenu lists Clouds… and Turbulence… in both modes, and Julia Fractal… and Mandelbrot Fractal… in Image mode only, sorted as `effect-catalog-v2` AC-04 specifies.
- [ ] AC-02 (event): When the user opens one of the four, then the effect dialog shows exactly its parameters from the table with their ranges and defaults; Colour 1 and Colour 2 start at the current foreground and background colours.
- [ ] AC-03 (always): Each render effect gives the same pixels on every run for the same parameters and target size; Clouds and Turbulence give different pixels for a different seed.
- [ ] AC-04 (option): Where Blend mode is Normal, every Clouds / Turbulence output pixel lies between Colour 1 and Colour 2 on each channel (inclusive), independent of the previous content; where Colour 1 equals Colour 2, the target is filled with that colour.
- [ ] AC-05 (option): Where Blend mode is Multiply, no output channel is brighter than the existing pixel's; where Screen, none is darker.
- [ ] AC-06 (always): Clouds with Roughness 0 has only one octave: it is smoother (a lower sum of absolute differences between neighbouring pixels) than Roughness 100 for the same seed.
- [ ] AC-07 (always): The fractals fill every target pixel opaquely; Mandelbrot with Invert colours on gives each pixel the inverted colour of the result with it off.
- [ ] AC-08 (always): Increasing Quality never changes the image size, and a pixel whose whole area lies inside the set (the Mandelbrot pixel at the view centre −0.5 + 0i at Zoom 10) stays black at every Quality.
- [ ] AC-09 (always): Each render effect changes only the target: pixels outside the selection, other layers and cels stay unchanged; one undo step restores the target.
- [ ] AC-10 (error): If a wire call names one of the four with the wrong number of parameters or an invalid colour slot, then it is rejected and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [ ] AC-11 (error): If the Pixel-mode document is not RGBA, or the current frame has no cel on the active layer, then Clouds and Turbulence are refused with the same message as every other effect and the document is unchanged.
- [ ] AC-12 (option): Where the UI language is German, the names, parameter labels and blend-mode options are German.

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

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#8). Depends on `outline-effect` for the colour parameter kind.
- 2026-11-04: Clarification pass settled without a dialogue: Normal blend replaces including alpha; octave count and weights, fractal views and colouring pinned (no smooth colouring); fractals Image-only because of their fixed rainbow palette; Pixel mode refuses a render without a cel; non-finite numbers read as 0.
