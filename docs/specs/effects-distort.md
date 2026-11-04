---
status: todo
created: 2026-11-04
size: L
mode: both
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 paint.net catalog)]
---

# paint.net distortions: Bulge, Crystalize, Dents, Frosted Glass, Morphology, Pixelate, Polar Inversion, Tile Reflection, Twist

## Context

paint.net's Distort menu (paint.net/doc/latest/EffectsDistortMenu.html) has nine entries; Pincel has none of them yet (its former "Distort" group held Emboss, Edge Detect and Relief, which `effect-catalog-v2` moved to Stylize). This spec adds all nine.

Following the per-mode rule of `effect-catalog-v2`, **Morphology** and **Pixelate** keep pixels hard-edged (Morphology picks existing pixels; Pixelate's Centre colour does too and is its Pixel-mode default), so they appear in Pixel mode too; the other seven resample the image (creating in-between colours) and appear in Image mode only. All nine stay accepted on the wire in both cores (`effect-catalog-v2` AC-08).

**Terms.** *Target*, *luma*, *opaque pixel*: as in `adjustments-paintnet`. *Resampling*: reading the source at a non-integer position by bilinear interpolation of premultiplied pixels. *Edge behaviour*: what a resampling distortion reads where the mapped position falls outside the layer — **Clamp** (nearest edge pixel), **Wrap** (tile the layer), **Reflect** (mirror at the edge) or **Transparent** (fully transparent); default Clamp. *Point*: a position in percent of the image area (`effect-catalog-v2` AC-10). *Outside the target*: for Morphology, pixels beyond the cel's edge (Pixel mode) or the layer's edge count as transparent, as in `outline-effect` AC-02; resampling distortions use the edge behaviour. *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Effects

| Wire id | Name (en / de) | Modes | Parameters: range (default) | Neutral setting | What it does |
| ------- | -------------- | ----- | --------------------------- | --------------- | ------------ |
| `bulge` | Bulge / Wölben | Image | Amount −200 to 100 (45), Center point (50 / 50), Edge behaviour (Clamp) | Amount 0 | Magnifies (Amount > 0) or pinches (< 0) a circular area of radius half the shorter image side around the centre |
| `crystalize` | Crystalize / Kristallisieren | Image | Cell size 2–250 (8), Seed random seed 0–9999 (1) | — | Splits the image into random convex cells (Voronoi cells of seeded points about Cell size apart) and fills each with the premultiplied average colour of its pixels |
| `dents` | Dents / Beulen | Image | Scale 1–200 (25), Refraction 0–200 (50), Roughness 0–100 (10), Seed random seed (1), Edge behaviour (Clamp) | Refraction 0 | Displaces each pixel by a seeded smooth noise field (feature size Scale px, strength Refraction, extra detail Roughness) |
| `frosted_glass` | Frosted Glass / Milchglas | Image | Amount 1–200 (10), Seed random seed (1) | — | Replaces each pixel by a seeded random pixel within Amount px (like looking through frosted glass), positions outside the layer clamped to its edge; no interpolation |
| `morphology` | Morphology / Morphologie | both | Operation Dilate / Erode / Open / Close (Dilate), Shape Square / Circle (Square), Radius 1–100 (1) | — | Dilate: each pixel becomes the highest-luma opaque pixel in its neighbourhood of the shape and radius, ties going to the first such pixel in row-major order (a transparent neighbourhood stays transparent); Erode: the lowest-luma pixel, and any transparent neighbour makes it transparent; Open = Erode then Dilate; Close = Dilate then Erode. It picks existing pixels, never mixes colours |
| `pixelate` | Pixelate / Verpixeln | both | Cell size 2–200 (8), Colour Average / Centre (Image: Average, Pixel: Centre) | — | Divides the image into Cell-size squares from the canvas origin (top-left of the document, also for a trimmed cel) and fills each square's target pixels with one colour: the premultiplied average of its pixels, or its centre pixel (the pixel at offset ⌊(w − 1) / 2⌋, ⌊(h − 1) / 2⌋ of the square, clipped to the image) |
| `polar_inversion` | Polar Inversion / Polarkoordinaten umkehren | Image | Amount −4 to 4, step 0.01 (1), Center point (50 / 50), Edge behaviour (Wrap) | — | Maps each pixel through the inversion z ↦ Amount / z̄, with z the pixel's offset from the centre in units of half the shorter image side (paint.net's "inside-out" warp) |
| `tile_reflection` | Tile Reflection / Kachelspiegelung | Image | Tile size 2–200 (40), Angle angle (30), Curvature −100 to 100 (8), Edge behaviour (Clamp) | — | Overlays a grid of rotated glass tiles, each refracting the image beneath with the given curvature |
| `twist` | Twist / Verdrehen | Image | Amount −100 to 100 (30), Size 1–200 % (100), Center point (50 / 50), Edge behaviour (Clamp) | Amount 0 | Rotates pixels around the centre by an angle that grows toward the centre, inside a circle of Size % of half the shorter image side |

## Scope

### In scope

- The nine distortions in the Distort submenu, per-mode as in the table
- The shared edge behaviour choice for the resampling distortions
- en + de names and labels

### Out of scope

- Antialiasing / quality options on the resampling distortions — because they always resample bilinearly; a quality switch belongs with `performance-background-jobs` if previews get slow
- Tile Reflection in Pixel mode — because its refraction resamples and creates in-between colours, against the per-mode rule (`effect-catalog-v2`)
- Interactive handles on the canvas for centre or tile grid — because the point control's Pick on canvas (`effect-catalog-v2` AC-11) covers placing the centre

## Acceptance criteria

- [ ] AC-01 (always): In Image mode the Distort submenu lists all nine entries of the table, sorted as `effect-catalog-v2` AC-04 specifies; in Pixel mode it lists only Morphology… and Pixelate….
- [ ] AC-02 (event): When the user opens a distortion, then the effect dialog shows exactly its parameters from the table with their ranges and defaults (Pixelate's Colour starts at the current mode's default), using the angle, point and random-seed controls of `effect-catalog-v2` where the table says so.
- [ ] AC-03 (always): Each distortion returns an image of the same size as its input, and Bulge, Dents and Twist at their neutral settings leave the layer unchanged.
- [ ] AC-04 (always): Morphology never produces a colour that is not already in the target (every output pixel equals some input pixel or is fully transparent), in every operation, shape and radius.
- [ ] AC-05 (always): Morphology Dilate makes the set of opaque pixels grow (every pixel opaque before stays opaque); Erode makes it shrink (every pixel transparent before stays transparent); a single opaque pixel on transparency becomes a 3 × 3 square under Dilate Square radius 1 and a plus shape under Dilate Circle radius 1, and disappears under Erode radius 1.
- [ ] AC-06 (always): After Pixelate, every Cell-size square of the grid (squares at the layer's right and bottom edge may be smaller) holds exactly one colour; with Colour Centre that colour is the square's centre pixel, so a Pixel-mode sprite pixelated with Centre gains no new colours.
- [ ] AC-07 (always): Bulge with Amount > 0 makes a centred checkerboard's centre squares larger than its edge squares; with Amount < 0 smaller.
- [ ] AC-08 (always): Twist leaves the pixels outside its circle unchanged and rotates those inside; the centre pixel is unchanged.
- [ ] AC-09 (always): Crystalize, Dents and Frosted Glass give the same pixels on every run for the same seed and parameters, and different pixels for a different seed on a non-uniform layer; on a noisy layer with Cell size 8, every Crystalize output pixel has at least one 4-neighbour of the same colour.
- [ ] AC-10 (always): Frosted Glass output pixels are all taken from the input (no colour mixing), each from within Amount px of its position.
- [ ] AC-11 (option): Where Edge behaviour is Transparent, positions mapped outside the layer give fully transparent pixels; Wrap reads the opposite edge; Reflect reads the mirrored position; Clamp reads the nearest edge pixel.
- [ ] AC-12 (always): A solid single-colour opaque layer stays unchanged under every distortion except with Edge behaviour Transparent (where mapped-out pixels become transparent).
- [ ] AC-13 (error): If a wire call names a distortion with the wrong number of parameters, then it is rejected as a wrong parameter count and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [ ] AC-14 (error): If the Pixel-mode document is not RGBA, then Morphology and Pixelate are refused with the same message as every other effect and the document is unchanged.
- [ ] AC-15 (option): Where the UI language is German, the names, parameter labels and edge-behaviour options are German.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- New module `crates/pincel-effects/src/distort/` (Emboss / Edge Detect / Relief may already live there from before the regrouping — the Rust module layout is free; only the catalog submenu changed). A shared `EdgeBehavior` enum and a bilinear `sample` helper serve the resampling distortions; choices travel as option index.
- Seeded randomness: the crates' existing seeded generator used by Add Noise.
- Catalog entries: submenu `distort`; modes per table.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pixelate cell size larger than the layer | One cell covers the whole layer: one colour |
| Morphology on a fully transparent layer | Unchanged |
| Morphology Erode on a layer that is fully opaque | Pixels within the radius of the layer's (or cel's) edge become transparent, because outside counts as transparent; Dilate never grows a cel |
| Polar Inversion with the centre on a corner | Works; Wrap tiles the result |
| Twist Size 200 % | The circle extends beyond the image; pixels inside it rotate |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Morphology ranks pixels by luma and picks whole pixels | Per-channel max/min would invent colours, breaking Pixel mode's palette; luma ranking keeps the result in the input's colours | Low: differs from per-channel morphology on colour images |
| Pixelate's grid starts at the canvas origin | Cells line up with the document's pixel grid in both modes, whatever a Pixel cel's offset | Low |
| Default edge behaviour Clamp (Wrap for Polar Inversion) | Clamp avoids transparent holes; Polar Inversion maps most positions outside the image, so Wrap is its useful default | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `image mode lists the nine distortions, sorted; pixel mode Morphology and Pixelate (AC-01)` (ui/e2e/effects-distort.spec.ts); `effect-menus.spec.ts` (Pixel mode exactly the catalog's Pixel entries) | built |        |
| AC-02 | `the distortion dialogs show exactly the parameters of the table (AC-02)` (ui/e2e/effects-distort.spec.ts); `pixelate: each mode starts at its own colour default` and pinned wire vectors (ui/e2e/effect-catalog.spec.ts) | built |        |
| AC-03 | `every_distortion_keeps_the_size`, `bulge_dents_and_twist_at_their_neutral_setting_change_nothing` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-04 | `morphology_only_picks_existing_colours` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-05 | `morphology_dilate_grows_and_erode_shrinks_the_opaque_pixels`, `morphology_on_a_single_pixel` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-06 | `pixelate_fills_every_square_with_one_colour`, `pixelate_centre_takes_the_square_centre_pixel_and_adds_no_colour`, `pixelate_grid_starts_at_the_canvas_origin` (crates/pincel-effects/tests/effects_distort.rs); `apply_effect_pixelate_aligns_a_trimmed_cel_to_the_canvas_grid` (crates/pincel-wasm/src/effects.rs) | built |        |
| AC-07 | `bulge_magnifies_or_pinches_the_centre` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-08 | `twist_rotates_inside_its_circle_only` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-09 | `seeded_distortions_are_reproducible_per_seed`, `crystalize_cells_have_no_single_pixels` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-10 | `frosted_glass_copies_input_pixels_from_within_amount` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-11 | `edge_behaviour_decides_what_a_position_outside_reads`, `edge_behaviour_transparent_clears_only_positions_outside` (crates/pincel-effects/tests/effects_distort.rs); `sample_edge_behaviours_read_outside_positions` (crates/pincel-effects/src/distort/edge.rs) | built |        |
| AC-12 | `a_solid_opaque_layer_stays_unchanged`, `morphology_on_a_solid_layer_changes_only_pixels_near_the_edge` (crates/pincel-effects/tests/effects_distort.rs) | built |        |
| AC-13 | `wrong_parameter_count_is_rejected`, `non_finite_numbers_read_as_zero_then_clamp_to_the_range` (crates/pincel-effects/tests/effects_distort.rs); `apply_effect_distortions_wrong_parameter_count_changes_nothing` (crates/pincel-wasm/src/effects.rs) | built |        |
| AC-14 | `apply_effect_distortions_on_an_indexed_document_are_refused` (crates/pincel-wasm/src/effects.rs) | built |        |
| AC-15 | `German names, parameter labels and edge-behaviour options (AC-15)` (ui/e2e/effects-distort.spec.ts) | built |        |

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#5). Tile Reflection moved to Image mode only, against the plan's proposal, because its refraction resamples.
- 2026-11-04: Clarification pass settled without a dialogue: Pixelate defaults to Centre in Pixel mode (no new colours); its grid starts at the canvas origin; Morphology treats outside the cel / layer as transparent (as `outline-effect`) and breaks luma ties in row-major order; Polar Inversion and Tile Reflection have no neutral setting; non-finite numbers read as 0.

- 2026-11-04 (spec-build): Wire order per distortion: `bulge` [amount, center_x, center_y, edge], `crystalize` [cell_size, seed], `dents` [scale, refraction, roughness, seed, edge], `frosted_glass` [amount, seed], `morphology` [operation, shape, radius], `pixelate` [cell_size, colour], `polar_inversion` [amount, center_x, center_y, edge], `tile_reflection` [tile_size, angle, curvature, edge], `twist` [amount, size, center_x, center_y, edge]; choices as their option index in the table's order (edge 0 = Clamp, 1 = Wrap, 2 = Reflect, 3 = Transparent; Morphology 0 = Dilate … 3 = Close, 0 = Square, 1 = Circle; Pixelate 0 = Average, 1 = Centre), centres as fractions 0–1 read as Radial Blur reads them (fraction · (side − 1)). A choice index is rounded and clamped to its options (AC-13: "every number is clamped to its range"), so 9 reads as the last option; cell sizes, tile size, radius, Amount of Frosted Glass and seeds are rounded to whole numbers; the angle wraps into 0–360 like the dial.
- 2026-11-04 (spec-build): Formulas the table leaves open: Bulge scales a pixel's offset from the centre by 1 − (Amount / 100) · (1 − r)² for r = distance / radius < 1 (paint.net's bulge); Twist rotates by (Amount / 100) · 2π · (1 − r / R)² inside R = Size % of half the shorter side; Polar Inversion reads cx + R · Amount · z / |z|² (the centre itself maps to infinity, i.e. outside); Tile Reflection follows Pinta's TileReflectionEffect (rotate into the grid frame, s += Curvature · tan(s · π / Tile size) per axis, rotate back) around the image centre; Dents displaces by (Refraction / 100) · Scale · n with n four octaves of seeded gradient noise at feature size Scale, octave k weighted (Roughness / 100)^k; Crystalize jitters one point per Cell-size grid square within the square's middle half; Frosted Glass draws dx uniform in −Amount..Amount and dy uniform within the disc's chord at dx; Morphology ranks by integer luma 2126 R + 7152 G + 722 B (the Terms' BT.709 weights), over the square or the disc dx² + dy² ≤ r².
- 2026-11-04 (spec-build): Edge behaviour Transparent clears positions outside the pixel area (x < −0.5 or x ≥ w − 0.5, likewise y); inside it, bilinear taps clamp, so the last half pixel does not fade out. Clamp, Wrap and Reflect map each bilinear tap (Reflect mirrors with the edge pixel repeated: w → w − 1).
- 2026-11-04 (spec-build): AC-12 and the edge-case table ("Morphology Erode on a fully opaque layer clears pixels within the radius of the edge") meet in Morphology's fixed edge behaviour: outside the layer always counts as transparent (Terms), so Morphology falls under AC-12's Transparent exception — Dilate leaves a solid layer unchanged; Erode, Open (the circle's corners) and Close change only pixels nearer the edge than the radius, and only to transparent.
- 2026-11-04 (spec-build): Crystalize gives a pixel that shares its cell with none of its 4-neighbours the cell of its first neighbour (left, up, right, down) — because the Voronoi diagram of jittered points can leave a lone pixel on a cell corner, which AC-09's "at least one 4-neighbour of the same colour" forbids; joining never leaves another pixel alone.
- 2026-11-04 (spec-build): Pixelate averages all pixels of a square (clipped to the buffer), not only the target's; the host still writes only target pixels. Its grid origin reaches the effect through the new `EffectSpec::run_at(src, origin, in_target)`; `run_in` is `run_at` at (0, 0). Sketch Blur's SplitMix64 position hash moved to `crate::random` and serves Crystalize, Dents and Frosted Glass (the table's "seeded generator"; per-position hashing keeps results independent of visiting order).
- 2026-11-04 (spec-build): New public API: `EffectSpec::run_at`; `EffectSpec::{Bulge, Crystalize, Dents, FrostedGlass, Morphology, Pixelate, PolarInversion, TileReflection, Twist}`; `distort::{Bulge, Crystalize, Dents, EdgeBehavior, FrostedGlass, Morphology, MorphologyOp, MorphologyShape, Pixelate, PixelateColor, PolarInversion, TileReflection, Twist}`. The new wire names follow `zoom_blur` at the end of `EFFECT_NAMES`.
- 2026-11-04 (spec-build): AC-13 ("changes nothing") and AC-14 are proven in the Pixel core (`pincel-wasm`), where the document and the RGBA refusal live; both tests were green at once, because the refusal and the parameter-count error run before any effect and every new name already parsed in `pincel-effects` (whose tests were red first). The Pixel core and the agent's Pixel tools now run effects through `run_at` with the cel position (the trimmed-cel test was red before).
- 2026-11-04 (spec-build): The agent catalog names choices as lower-case words (`edge`: clamp / wrap / reflect / transparent; `operation`: dilate / erode / open / close; `shape`: square / circle; `color`: average / centre). It serves both modes with one table, so Pixelate's agent default is Centre — Pixel mode's, the one that never adds a colour; an agent passes `"color": "average"` for Image mode's look.
- 2026-11-04 (spec-build): Pixelate's per-mode default is a new optional catalog field `EffectDef.modeDefaults` (parameter key → default per mode), applied by the new `forMode(def, mode)` that `entriesFor` now runs — because the menus hand the dialog the definition, so the dialog (and its Reset) picks up the mode's default without changes to `EffectDialog.svelte`, which `effect-panel` reworks in parallel. `effectById` and `defaultValues` keep the catalog's own defaults (Image mode's Average).
- 2026-11-04 (spec-build): Labels the table leaves open: Edge behavior / Randverhalten with Clamp / Rand wiederholen, Wrap / Kacheln, Reflect / Spiegeln, Transparent / Transparent; Cell size / Zellgröße, Scale / Skalierung, Refraction / Brechung, Roughness / Rauheit, Operation / Operation (Dilate / Ausdehnen, Erode / Abtragen, Open / Öffnen, Close / Schließen), Shape / Form (Square / Quadrat, Circle / Kreis), Color / Farbe (Average / Durchschnitt, Center / Mitte), Tile size / Kachelgröße, Curvature / Krümmung, Size % / Größe %; Amount, Radius, Seed, Angle and Center reuse the existing strings. English labels use US spelling like the rest of the UI. Polar Inversion's Amount steps by 0.01, every other number by 1.
