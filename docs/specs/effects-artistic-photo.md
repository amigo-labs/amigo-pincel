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
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 | `pencil_sketch_is_gray_and_whitens_a_flat_area` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-06 | `ink_sketch_draws_dark_lines_on_edges` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-07 | `oil_painting_keeps_a_solid_layer_and_merges_a_ramp`, `oil_painting_leaves_a_transparent_layer_unchanged` (crates/pincel-effects/tests/effects_artistic_photo.rs) | built |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#7).
- 2026-11-04: Clarification pass settled without a dialogue: Red Eye Removal and Vignette stay Image-only (photo-specific, though per pixel); Pencil Sketch, red dominance and Warmth formulas pinned; Straighten rotates the whole layer and masks by the selection; Oil Painting and Vignette ACs use fixed fixtures; non-finite numbers read as 0.
