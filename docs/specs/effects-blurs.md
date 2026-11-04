---
status: implemented
created: 2026-11-04
size: L
mode: Image
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 paint.net catalog)]
---

# paint.net blurs: Bokeh, Fragment, Median, Sketch, Surface, Zoom

## Context

paint.net's Blurs menu has ten entries (paint.net/doc/latest/EffectsBlursMenu.html): Bokeh, Fragment, Gaussian, Median, Motion, Radial, Sketch, Square, Surface and Zoom. After `effect-catalog-v2` Pincel's Blurs submenu has Gaussian, Motion, Radial and Square Blur. This spec adds the other six and completes the existing ones where paint.net offers an option Pincel lacks (Motion Blur "Centered").

Blurs mix neighbouring pixels, so they appear in Image mode only (Decision Log 2026-11-04, `effect-catalog-v2` AC-03); the Pixel-mode core still accepts them on the wire (`effect-catalog-v2` AC-08). The algorithms are Pincel's own, written from paint.net's documented behaviour.

**Terms.** *Target*, *luma*, *opaque pixel* and *neutral setting*: as in `adjustments-paintnet`. *Premultiplied averaging*: averaging colours weighted by their alpha, so a fully transparent pixel contributes no colour (`effects-adjustments` AC-18). *Edge pixels*: neighbours outside the layer read as the nearest pixel on the layer's edge (clamp), as the existing blurs do. *Point*: percent of the image area (`effect-catalog-v2` AC-10). *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Effects

| Wire id | Name (en / de) | Parameters: range (default) | Neutral setting | What it does |
| ------- | -------------- | --------------------------- | --------------- | ------------ |
| `bokeh` | Bokeh / Bokeh | Radius 1–100 (10), Highlights 0–100 (25) | — | Averages each pixel over a filled disc of the radius (a camera-lens blur); Highlights > 0 weights bright pixels more, so bright spots spread into discs |
| `fragment` | Fragment / Fragmente | Fragments 2–50 (4), Distance 0–100 (8), Rotation angle (0) | Distance 0 | Averages n copies of the target, each shifted by Distance px in directions spaced 360°/n apart, starting at Rotation |
| `median` | Median / Median | Radius 1–200 (10), Percentile 0–100 (50) | — | Replaces each colour channel by the given percentile of that channel over the opaque pixels of the square neighbourhood of the radius (transparent pixels are ignored, so they never tint), and alpha by the percentile of all alphas there |
| `sketch_blur` | Sketch Blur / Skizzenhaftes Weichzeichnen | Radius 1–100 (10), Seed random seed 0–9999 (1) | — | Smears each pixel along a short line of random direction (seeded per pixel position) and length up to Radius, like hand-drawn hatching |
| `surface_blur` | Surface Blur / Oberflächen-Weichzeichner | Radius 1–100 (6), Threshold 1–100 (25) | — | Averages each pixel only with neighbours whose largest channel difference from it is at most Threshold % of 255, so edges stay sharp while flat areas smooth |
| `zoom_blur` | Zoom Blur / Zoom-Weichzeichnen | Amount 1–100 (10), Center point (50 / 50) | — | Averages along the line from each pixel toward the centre, longer farther out — today's Radial Blur "Zoom" kind as its own entry |
| `motion_blur` (extended) | Motion Blur | adds Centered switch (on) | — | On: smears symmetrically around each pixel (today's behaviour); off: smears only backwards along the angle, like a trail |
| `radial_blur` (changed entry) | Radial Blur | Amount, Center; the Kind choice is no longer shown | — | Always spins; zooming moved to Zoom Blur |

All averaging blurs use premultiplied averaging; all use clamped edge pixels. Blurs ▸ Median is a new entry; Noise ▸ Reduce Noise stays in the Noise submenu and is rebuilt by `effects-object-stylize-color-noise`. Motion Blur's appended Centered slot changes that entry's parameter vector; `effect-catalog-v2`'s pin test (its AC-07) is updated for it.

## Scope

### In scope

- The six new blurs in Image mode's Blurs submenu; Motion Blur's Centered switch; Radial Blur without the Kind choice
- New wire names and parameter lists in both cores' shared crate; existing wire calls keep working
- en + de names and labels

### Out of scope

- A Quality / sample-count option on Radial, Zoom and Motion Blur — because the existing implementations sample exactly; a speed-for-quality option only matters once previews are slow, which `performance-background-jobs` addresses
- GPU or background computation — because `performance-background-jobs` owns moving effect work off the main thread
- Pixel-mode menu entries for any blur — because blurs create anti-aliased colours (`effect-catalog-v2`)

## Acceptance criteria

- [x] AC-01 (always): In Image mode the Blurs submenu lists Bokeh…, Fragment…, Gaussian Blur…, Median…, Motion Blur…, Radial Blur…, Sketch Blur…, Square Blur…, Surface Blur… and Zoom Blur…, sorted as `effect-catalog-v2` AC-04 specifies; in Pixel mode none of them appears.
- [x] AC-02 (event): When the user opens one of the new blurs, then the effect dialog shows exactly its parameters from the table with their ranges and defaults, using the angle, point and random-seed controls of `effect-catalog-v2` where the table says so.
- [x] AC-03 (always): Each new blur returns an image of the same size as its input, and every output alpha lies between the lowest and the highest alpha of the input layer.
- [x] AC-04 (always): A solid single-colour opaque layer stays unchanged under Bokeh, Fragment, Median, Sketch Blur, Surface Blur and Zoom Blur at any parameters.
- [x] AC-05 (always): No new blur lets the colour of a fully transparent pixel tint an opaque neighbour: a red opaque square on transparent pixels whose stored colour is green yields no opaque output pixel with more green than red.
- [x] AC-06 (always): Fragment with Distance 0 leaves the layer unchanged.
- [x] AC-07 (always): Median with Percentile 0 makes no channel brighter than the darkest value in its neighbourhood, Percentile 100 none darker than the brightest; a single bright pixel on a dark area disappears under Median radius 1, percentile 50.
- [x] AC-08 (always): Surface Blur at Threshold 25 leaves the edge between a black and a white area sharp (every pixel keeps its colour) and, on an area of two alternating grays that differ by 10, makes the largest difference between neighbouring pixels smaller than 10.
- [x] AC-09 (always): Sketch Blur with the same seed and parameters gives the same pixels on every run; a different seed gives a different result on a non-uniform layer.
- [x] AC-10 (always): On an odd-width image of 1-px vertical stripes with the centre on its middle column, Zoom Blur leaves the middle column unchanged and the mean change per column grows with the column's distance from the middle; its output equals Radial Blur's zoom kind for the same amount and the same centre position on the image.
- [x] AC-11 (option): Where Motion Blur's Centered is off, a single opaque pixel on transparency spreads only to one side along the angle; where it is on, it spreads to both sides symmetrically, as today.
- [x] AC-12 (event): When the user opens Radial Blur, then it shows Amount and Center but no Kind choice, and it always spins.
- [x] AC-13 (always): Existing wire calls still work unchanged: `motion_blur` with its two parameters behaves as Centered on, and `radial_blur` with kind Zoom still zooms.
- [x] AC-14 (error): If a wire call names a new blur with the wrong number of parameters, then it is rejected as a wrong parameter count and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [x] AC-16 (error): If a wire call runs a blur on a Pixel-mode document that is not RGBA, then it is refused with the same message as every other effect and the document is unchanged.
- [x] AC-15 (option): Where the UI language is German, the names and parameter labels are the German ones of the table.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- Modules under `crates/pincel-effects/src/blur/` (existing: `gaussian.rs`, `box_blur.rs`, `motion.rs`, `radial.rs`); `EffectSpec` in `spec.rs`. `motion_blur` gains an optional third slot (`from_params` accepts 2 or 3, as `grayscale` accepts 1 or 4). Radial Blur's hidden Kind needs a catalog way to send a constant slot (a fixed parameter not shown in the dialog).
- Median over large radii: a sliding histogram per channel keeps it O(r) per pixel instead of O(r²).
- Catalog entries: submenu `blurs`, modes `image`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Radius larger than the layer | Works; edge pixels clamp |
| Bokeh on a layer with a few bright pixels and Highlights 100 | Bright pixels grow into visible discs |
| Fragment with 50 fragments and Distance 100 | Works; result is a ring-shaped smear |
| Median on a fully transparent layer | Unchanged |
| Zoom Blur centre at 0 / 0 (a corner) | Pixels streak toward that corner |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Median moves to Blurs as its own wire id, Reduce Noise keeps its id | paint.net has both; keeping `reduce_noise` avoids breaking agent calls | Low |
| Radial Blur's Kind is hidden rather than removed from the wire | Agent calls with kind Zoom keep working (AC-13) | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `image mode lists the ten blurs, sorted; pixel mode none (AC-01)` (ui/e2e/effects-blurs.spec.ts); `effect-menus.spec.ts` (Pixel mode exactly the catalog's Pixel entries) | verified | ec36c78 |
| AC-02 | `the new blur dialogs show exactly the parameters of the table (AC-02)` (ui/e2e/effects-blurs.spec.ts); pinned wire vectors in `ui/e2e/effect-catalog.spec.ts` | verified | ec36c78 |
| AC-03 | `new_blurs_keep_size_and_the_input_alpha_range` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-04 | `a_solid_opaque_layer_stays_unchanged` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-05 | `transparent_colour_never_tints_an_opaque_neighbour` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-06 | `fragment_with_distance_zero_is_the_identity` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-07 | `median_percentile_zero_and_hundred_bound_every_channel`, `median_removes_a_single_bright_pixel` (crates/pincel-effects/tests/effects_blurs.rs); `median_sliding_histogram_matches_sorting` (crates/pincel-effects/src/blur/median.rs) | verified | 6bb9973 |
| AC-08 | `surface_blur_keeps_a_black_white_edge_sharp`, `surface_blur_smooths_two_close_grays` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-09 | `sketch_blur_is_reproducible_per_seed` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-10 | `zoom_blur_grows_with_the_distance_from_the_centre`, `zoom_blur_equals_radial_blur_zoom` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-11 | `motion_blur_uncentered_spreads_to_one_side_only`, `motion_blur_centered_spreads_symmetrically` (crates/pincel-effects/tests/effects_blurs.rs); `effect_apply_new_blurs_match_the_editor` (Centered off, crates/pincel-agent/src/tests/draw.rs) | verified | 6bb9973 |
| AC-12 | `Radial Blur shows Amount and Center but no Kind choice (AC-12)` (ui/e2e/effects-blurs.spec.ts); `radial_blur: wire vectors equal the previous catalog's` (kind slot always 0, ui/e2e/effect-catalog.spec.ts) | verified | ec36c78 |
| AC-13 | `existing_wire_calls_keep_working` (crates/pincel-effects/tests/effects_blurs.rs) | verified | 6bb9973 |
| AC-14 | `wrong_parameter_count_is_rejected`, `non_finite_numbers_read_as_zero_then_clamp_to_the_range` (crates/pincel-effects/tests/effects_blurs.rs); `apply_effect_blurs_wrong_parameter_count_changes_nothing` (crates/pincel-wasm/src/effects.rs) | verified | 6bb9973 |
| AC-15 | `German names and parameter labels (AC-15)` (ui/e2e/effects-blurs.spec.ts) | verified | ec36c78 |
| AC-16 | `apply_effect_blurs_on_an_indexed_document_are_refused` (crates/pincel-wasm/src/effects.rs) | verified | 8266a00 |

## Summary

> Completed: 2026-11-04

- Six new blurs in `pincel-effects` (Bokeh, Fragment, Median with a sliding histogram, Sketch Blur, Surface Blur, Zoom Blur), premultiplied and edge-clamped, with wire names, clamping and labels in `EffectSpec`
- Motion Blur gains the Centered switch (optional third wire slot, two-slot calls stay centred); Radial Blur's zoom kind now averages toward the centre and equals Zoom Blur
- Image mode's Blurs submenu lists all ten paint.net blurs; Radial Blur's dialog drops the Kind choice and always sends spin; en + de names and labels
- Agent catalog and the Pixel core accept the new blurs on the wire (parameter-count and non-RGBA refusals tested)

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#4). Parameters are Pincel's own; paint.net's docs list the effects, not formulas.
- 2026-11-04: Clarification pass settled without a dialogue: Bokeh and Median have no neutral setting; Median ignores transparent pixels' colour; Surface Blur threshold is % of 255; Zoom Blur's AC uses an odd-width stripe image; non-finite numbers read as 0.
- 2026-11-04 (spec-build): Radial Blur's zoom kind now averages from each pixel toward the centre (scale 1 down to 1 − 0.4 · amount / 100) instead of symmetrically around the pixel — because the table describes Zoom Blur that way, and the symmetric version sampled past the layer edge, so the outermost columns changed less than their neighbours (AC-10 was red on it). Zoom Blur and Radial Blur's wire kind 1 run the same code (AC-10, AC-13).
- 2026-11-04 (spec-build): Motion Blur with Centered off samples from the pixel forward along the angle, so the smear trails behind (opposite the angle), as behind an object moving along it.
- 2026-11-04 (spec-build): Formulas the table leaves open: Bokeh weights a pixel by `1 + 100 · (h / 100) · l⁴` (l = premultiplied luma 0–1) over the disc `dx² + dy² ≤ r²`, using row prefix sums (O(r) per pixel); Fragment shifts copies with bilinear sampling, Distance 0 returns the layer as is; Median takes the value at rank `round(p / 100 · (n − 1))` and keeps the pixel's colour when its neighbourhood has no opaque pixel; Sketch Blur draws direction and length (0..Radius) per pixel from SplitMix64 of (seed, x, y) and averages a line centred on the pixel; Surface Blur compares premultiplied RGBA (so a transparent neighbour never counts as close to an opaque pixel) over the square window, O(r²) per pixel — speed is `performance-background-jobs`' concern.
- 2026-11-04 (spec-build): Clamping per AC-14: radii, Fragments, Seed and Threshold are rounded to whole numbers; Fragment's Rotation wraps into 0–360 like the angle dial instead of clamping (370 → 10); Zoom Blur's centre clamps to 0–1. Motion Blur's two existing slots keep their old unclamped reading (AC-13); its Centered slot is a boolean like every other switch.
- 2026-11-04 (spec-build): New wire names follow `temperature_tint` at the end of `EFFECT_NAMES` (bokeh, fragment, median, sketch_blur, surface_blur, zoom_blur); `EffectSpec::MotionBlur` gains `centered`, `MotionBlur` gains `centered` / `with_centered`, and `blur::{Bokeh, Fragment, Median, SketchBlur, SurfaceBlur}` and `EffectSpec::{Bokeh, Fragment, Median, SketchBlur, SurfaceBlur, ZoomBlur}` are new public API.
- 2026-11-04 (spec-build): AC-14 ("changes nothing") and AC-16 are proven in the Pixel core (`pincel-wasm`), where the document and the RGBA refusal live; both tests were green at once, because the refusal and the parameter-count error run before any effect and every new name already parsed in `pincel-effects` (whose tests were red first). The agent catalog gained the new blurs because its tests require it to mirror `EFFECT_NAMES`.
- 2026-11-04 (spec-build): Radial Blur's hidden Kind uses the catalog's existing `toParams` hook (amount, centre fractions, then a constant 0) instead of a new hidden parameter kind — because `EffectDialog.svelte` renders every unknown kind as a choice and counts `params` for "no parameters", so a new kind would need dialog changes while `effect-panel` reworks that dialog in parallel. The unused `fx.Kind` / `fx.Spin` / `fx.Zoom` strings were removed.
- 2026-11-04 (spec-build): Labels the table leaves open: Fragments / Fragmente, Rotation / Drehung (an angle dial), Percentile / Perzentil, Centered / Zentriert; Bokeh's Highlights, Radius, Distance, Threshold, Amount, Seed and Center reuse the existing strings (Lichter, Radius, Distanz, Schwellenwert, Stärke, Startwert, Mitte). All six blurs and the Radial / Zoom amount keep step 1.
