---
status: todo
created: 2026-11-04
size: L
mode: Image
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 paint.net catalog)]
---

# paint.net Artistic and Photo effects

## Context

paint.net's Artistic menu (Ink Sketch, Oil Painting, Pencil Sketch) and Photo menu (Glow, Red Eye Removal, Sharpen, Soften Portrait, Straighten, Vignette) are photo filters (paint.net/doc/latest/EffectsArtisticMenu.html, EffectsPhotoMenu.html). Pincel has Sharpen (and Unsharp Mask) in Photo after `effect-catalog-v2`; this spec adds the other eight.

Six of them mix neighbouring pixels or resample; Red Eye Removal and Vignette work per pixel but target photographs (eyes in photos, lens falloff that paints a gradient), which pixel art has no use for, so all eight appear in Image mode only (`effect-catalog-v2`); both cores still accept them on the wire. The algorithms are Pincel's own, from paint.net's documented behaviour.

**Terms.** *Target*, *luma*, *opaque pixel*, *premultiplied averaging*: as in `adjustments-paintnet` and `effects-blurs`. *Point*: a position in percent of the image area (`effect-catalog-v2` AC-10). *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Effects

| Wire id | Name (en / de) | Parameters: range (default) | Neutral setting | What it does |
| ------- | -------------- | --------------------------- | --------------- | ------------ |
| `ink_sketch` | Ink Sketch / Tuschezeichnung | Ink outline 0–99 (50), Colouring 0–100 (50) | — | Dark ink lines along edges (from an edge-detect of the luma, stronger with Ink outline) over the image, whose colour saturation is scaled to Colouring % |
| `oil_painting` | Oil Painting / Ölgemälde | Brush size 1–8 (3), Coarseness 3–255 (50) | — | Each pixel takes the average colour of the most frequent luma bin (Coarseness bins) in its Brush-size neighbourhood — flat, brush-like patches |
| `pencil_sketch` | Pencil Sketch / Bleistiftskizze | Pencil tip size 1–20 (2), Colour range −20 to 20 (0) | — | A gray image of pencil lines (colour dodge): with l the luma and b the Gaussian blur (radius = tip size) of 255 − l, the value is 255 when b = 255, else min(255, l · 255 / (255 − b)); Colour range adds that many × 5 to the value, clamped |
| `glow` | Glow / Glühen | Radius 1–20 (6), Brightness −100 to 100 (10), Contrast −100 to 100 (10) | — | Screen-blends a blurred, brightness- and contrast-adjusted copy over the image |
| `red_eye_removal` | Red Eye Removal / Rote Augen entfernen | Tolerance 0–100 (70), Saturation 0–100 (90) | Saturation 0 | Lowers the red channel of strongly red pixels toward the green / blue average, by Saturation %; a pixel is strongly red when (R − (G + B) / 2) / 255 × 100 is above 100 − Tolerance |
| `soften_portrait` | Soften Portrait / Porträt weichzeichnen | Softness 0–10 (5), Lighting −20 to 20 (0), Warmth 0–20 (10) | all 0 | Blends a blurred copy (Softness) for smooth skin, adjusts brightness (Lighting) and warms (Warmth w acts as Temperature 5 · w in `adjustments-paintnet`) |
| `straighten` | Straighten / Gerade ausrichten | Angle −45 to 45, step 0.1 (0), Fill Zoom to fill / Transparent corners (Zoom to fill) | Angle 0 | Rotates the whole layer around its centre by Angle (bilinear); Zoom to fill scales it up just enough that no transparent corner appears; with a selection, only selected pixels take the result |
| `vignette` | Vignette / Vignette | Center point (50 / 50), Radius 10–400 % (100), Density 0–100 (100), Colour Black / White (Black) | Density 0 | Darkens (or lightens) toward the edges with a smooth falloff starting at Radius % of half the image diagonal from the centre |

## Scope

### In scope

- The eight effects in Image mode's Artistic and Photo submenus, through the shared effect pipeline
- en + de names and labels

### Out of scope

- Automatic red-eye detection (face / eye finding) — because paint.net works on the selection the user draws around the eyes, which Pincel's selection already supports
- Straighten with an on-canvas guide line — because the number field and preview are enough for a first version; a guide is a separate interaction
- Pixel-mode entries — because these effects create in-between colours (`effect-catalog-v2`)

## Acceptance criteria

- [ ] AC-01 (always): In Image mode the Artistic submenu lists Ink Sketch…, Oil Painting… and Pencil Sketch…, and the Photo submenu lists Glow…, Red Eye Removal…, Sharpen, Soften Portrait…, Straighten…, Unsharp Mask… and Vignette…, sorted as `effect-catalog-v2` AC-04 specifies; Pixel mode shows neither submenu.
- [ ] AC-02 (event): When the user opens one of the eight, then the effect dialog shows exactly its parameters from the table with their ranges and defaults, using the point control where the table says so.
- [ ] AC-03 (always): Each effect returns an image of the same size as its input and leaves alpha unchanged, except Straighten (which rotates the alpha with the image and, with Transparent corners, makes the uncovered corners transparent).
- [ ] AC-04 (always): Soften Portrait, Straighten and Vignette at their neutral settings leave the layer unchanged; Red Eye Removal with Saturation 0 leaves it unchanged.
- [ ] AC-05 (always): Pencil Sketch output is gray (R = G = B ± 1 for every pixel) and, with Colour range 0, a flat area of any colour becomes white (luma ≥ 250).
- [ ] AC-06 (always): Ink Sketch draws dark lines on edges: at the boundary between two flat areas the boundary pixels are darker than pixels deep inside either area; Colouring 0 gives a gray image.
- [ ] AC-07 (always): Oil Painting leaves a solid single-colour layer unchanged, and on a horizontal gray ramp 0 – 255 over 256 px with Brush size 3 and Coarseness 8 the output has fewer distinct colours than the input.
- [ ] AC-08 (always): Red Eye Removal lowers the red channel of a saturated red pixel (255, 30, 30) and leaves a pixel with red not dominant (100, 120, 110) unchanged.
- [ ] AC-09 (always): Vignette with Radius 50, Colour Black and Density 100 leaves the centre pixel unchanged and makes corner pixels darker; with White, lighter.
- [ ] AC-10 (option): Where Straighten's Fill is Zoom to fill, no pixel that was opaque in a fully opaque layer becomes transparent; where it is Transparent corners, the corners of a fully opaque layer become transparent for any Angle ≠ 0.
- [ ] AC-11 (always): Glow makes a mid-gray layer lighter or equal everywhere for Brightness ≥ 0 and Contrast 0 (screen blend never darkens).
- [ ] AC-12 (error): If a wire call names one of the eight with the wrong number of parameters, then it is rejected and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [ ] AC-13 (option): Where the UI language is German, the names and parameter labels are German.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- New modules `crates/pincel-effects/src/artistic/` and `src/photo/`; they can reuse `blur::gaussian`, the edge detector and the bilinear sampler from `effects-distort`.
- Catalog entries: submenus `artistic` / `photo`, modes `image`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Straighten by 45° with Zoom to fill on a very wide image | Scales up enough to fill; much of the image is cropped |
| Red Eye Removal without a selection | Works on the whole layer (every strongly red pixel) |
| Vignette centre in a corner | The darkening is centred on that corner |
| Oil Painting on a fully transparent layer | Unchanged |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Formulas are Pincel's own | paint.net's docs describe the look, not the math (Decision Log 2026-11-04) | Low |
| Red Eye Removal works without a selection | paint.net requires a selection; letting it work on the whole layer is simpler and the user can still select | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 | `every_effect_keeps_size_and_alpha`, `straighten_keeps_the_size_and_rotates_the_alpha` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-04 | `neutral_settings_leave_the_layer_unchanged` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-05 | `pencil_sketch_is_gray_and_whitens_a_flat_area` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-06 | `ink_sketch_draws_dark_lines_on_edges` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-07 | `oil_painting_keeps_a_solid_layer_and_merges_a_ramp`, `oil_painting_leaves_a_transparent_layer_unchanged` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-08 | `red_eye_removal_lowers_dominant_red_only` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-09 | `vignette_darkens_or_lightens_toward_the_edges`, `vignette_centred_on_a_corner_darkens_around_it` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-10 | `straighten_zoom_to_fill_keeps_every_pixel_opaque`, `straighten_transparent_corners_clears_the_corners` (crates/pincel-effects/tests/effects_artistic_photo.rs); `source_zoom_to_fill_scales_just_enough_to_cover_the_layer` (crates/pincel-effects/src/photo/straighten.rs) | built |        |
| AC-11 | `glow_never_darkens` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-12 | `wrong_parameter_count_is_rejected`, `non_finite_numbers_read_as_zero_then_clamp_to_the_range`, `photo_numbers_read_non_finite_as_zero_then_clamp` (crates/pincel-effects/tests/effects_artistic_photo.rs); `apply_effect_artistic_photo_wrong_parameter_count_changes_nothing` (crates/pincel-wasm/src/effects.rs) | built |        |
| AC-13 |      | open   |        |

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#7).
- 2026-11-04: Clarification pass settled without a dialogue: Red Eye Removal and Vignette stay Image-only (photo-specific, though per pixel); Pencil Sketch, red dominance and Warmth formulas pinned; Straighten rotates the whole layer and masks by the selection; Oil Painting and Vignette ACs use fixed fixtures; non-finite numbers read as 0.

- 2026-11-05 (spec-build): Wire order per effect: `ink_sketch` [ink_outline, coloring], `oil_painting` [brush_size, coarseness], `pencil_sketch` [tip_size, color_range], `glow` [radius, brightness, contrast], `red_eye_removal` [tolerance, saturation], `soften_portrait` [softness, lighting, warmth], `straighten` [angle, fill] (0 = Zoom to fill, 1 = Transparent corners), `vignette` [center_x, center_y, radius, density, color] (centre as fractions 0–1 read as Radial Blur reads them, color 0 = Black, 1 = White). Choices are rounded and clamped to their options; Brush size, Coarseness, Pencil tip size and Colour range are rounded to whole numbers, every other number stays continuous. The new wire names follow `twist` at the end of `EFFECT_NAMES`.
- 2026-11-05 (spec-build): Formulas the table leaves open: Ink Sketch inks with d = min(1, |Sobel| · Ink outline / 25), the normalised Sobel gradient (taps clamped at the edge) of the premultiplied luma — so a silhouette against transparency is inked too — and gives each channel (l + (c − l) · Colouring / 100) · (1 − d); Oil Painting sorts the opaque pixels of the clipped (2 · Brush size + 1)² square into Coarseness bins of the BT.709 luma, takes the alpha-weighted average colour of the fullest bin (ties: the darker bin) in integer arithmetic and keeps the pixel when it has no opaque neighbour; Pencil Sketch uses whole-number lumas and blurs the negative as an opaque gray image with the existing Gaussian blur, so a flat area dodges to exactly 255; Glow screen-blends a Gaussian blur (radius) passed through the existing legacy Brightness / Contrast; Soften Portrait mixes Softness · 5 % of a Gaussian blur with radius 2 · Softness into the straight colours, then adds 2.5 · Lighting to every channel, then applies Temperature 5 · Warmth; Vignette moves each channel toward black or white by Density % · smoothstep((d − r0) / r0), r0 = Radius % of half the diagonal, so the fade completes at 2 · r0.
- 2026-11-05 (spec-build): Every effect except Straighten keeps the source alpha, also where it blurs (the blurred copies contribute only colour) — AC-03 says so for all of them. Straighten rotates clockwise on screen for a positive angle, like the angle dial, around the layer centre ((w − 1) / 2, (h − 1) / 2), resampling premultiplied pixels bilinearly through the distortions' `remap`: Zoom to fill scales by cos|a| + max(w / h, h / w) · sin|a| with Clamp taps, Transparent corners keeps scale 1 with the Transparent edge behaviour (a position outside the pixel area reads transparent).
- 2026-11-05 (spec-build): AC-10's "for any Angle ≠ 0" is proven on an 800 × 800 layer at ±0.1°, ±1°, ±10° and ±45° — because a corner pixel's centre leaves the layer only once the rotation moves it by more than half a pixel, which on a small layer needs more than 0.1°; Transparent corners does not antialias (as the Transparent edge behaviour of `effects-distort`).
- 2026-11-05 (spec-build): The agent catalog names Straighten's Fill `zoom_to_fill` / `transparent_corners` and Vignette's Colour `black` / `white`; the other parameters keep the wire names (`ink_outline`, `coloring`, `brush_size`, `coarseness`, `tip_size`, `color_range`, `radius`, `brightness`, `contrast`, `tolerance`, `saturation`, `softness`, `lighting`, `warmth`, `angle`, `center_x`, `center_y`, `density`). The agent test that used `glow` as an unknown effect id now uses `no_such_effect`, because Glow is a real effect now.
- 2026-11-05 (spec-build): AC-12's "changes nothing" is proven in the Pixel core (`pincel-wasm`), as in `effects-blurs` and `effects-distort`: the Image core's `apply_effect` returns a `JsError`, which cannot be built in a native test, and it parses the spec before it touches the document just like the Pixel core. The test was green at once, because the parameter-count error runs before any effect and every new name already parsed in `pincel-effects` (whose tests were red first).
