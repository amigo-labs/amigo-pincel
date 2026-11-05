---
status: todo
created: 2026-11-04
size: M
mode: both
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-10-29 colour slot, 2026-11-04 paint.net catalog)]
---

# paint.net Object, Stylize, Color and Noise effects: Drop Shadow, Outline, Quantize, Reduce Noise

## Context

After `effect-catalog-v2` and `outline-effect`, four paint.net entries are still missing outside Artistic, Blurs, Distort, Photo and Render: **Object ▸ Drop Shadow**, **Stylize ▸ Outline** (paint.net's edge effect — different from `outline-effect`'s Object ▸ Outline Object for sprites), **Color ▸ Quantize** and paint.net's **Noise ▸ Reduce Noise** (Pincel's Reduce Noise is a plain median today).

Per-mode rule (`effect-catalog-v2`): Drop Shadow with Blur 0 and Quantize keep pixels hard-edged and work on colours, so they appear in both modes; Stylize ▸ Outline and Reduce Noise mix neighbours and stay Image-only.

Drop Shadow's colour uses the colour parameter kind that `outline-effect` introduces (one wire slot holding `0xRRGGBBAA`, Decision Log 2026-10-29); this spec depends on it.

**Terms.** *Target*, *luma*, *opaque pixel*: as in `adjustments-paintnet`. *Shape*: the target's alpha channel. *Distinct colours*: different RGB values among the opaque pixels. *Defaults* are the dialog's starting values; the wire always carries every value, so a per-mode default never reaches a core. *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Effects

| Wire id | Name (en / de) | Modes | Parameters: range (default) | What it does |
| ------- | -------------- | ----- | --------------------------- | ------------ |
| `drop_shadow` | Drop Shadow / Schlagschatten | both | Offset X −100 to 100 (4), Offset Y −100 to 100 (4), Blur radius 0–50 (Image 4, Pixel 0), Colour colour (black, alpha 255), Opacity 0–100 (50) | Draws a copy of the shape, filled with Colour at alpha = Colour's alpha × Opacity %, shifted by the offset (px) and blurred by the radius, **behind** the existing pixels: an opaque pixel keeps its colour; a partly transparent or transparent pixel shows the shadow beneath it |
| `stylize_outline` | Outline / Kontur | Image | Thickness 1–200 (3), Intensity 0–100 (50) | Darkens pixels near colour edges: each pixel is blended toward the darkest-luma pixel in the square of Thickness px around it, by Intensity % times the local luma range / 255 — flat areas (range 0) stay as they are |
| `quantize` | Quantize / Quantisieren | both | Colours 2–256 (16), Algorithm Median Cut / Octree (Median Cut), Dithering 0–100 (0) | Reduces the target's opaque colours to at most Colours distinct colours chosen by the algorithm; Dithering > 0 diffuses the error (Floyd–Steinberg, scaled by the percentage); alpha is kept |
| `reduce_noise` (rebuilt) | Reduce Noise / Rauschen entfernen | Image | Radius 1–200 (6), Strength 0–100 (40) | Blends each pixel toward its neighbourhood median by Strength %; the median ignores transparent pixels' colour, as Blurs ▸ Median does |

## Scope

### In scope

- The four effects in their submenus, per-mode as in the table, with a per-mode default for Drop Shadow's Blur radius
- Reduce Noise's new Strength parameter, keeping old wire calls working
- en + de names and labels

### Out of scope

- The colour parameter kind itself — because `outline-effect` builds it (Decision Log 2026-10-29); this spec reuses it
- Growing a layer or cel so a shadow fits beyond its edge — because effects never resize layers or cels (`outline-effect` AC-02 applies the same rule); the shadow is clipped at the edge
- Quantize to a given palette (the Pixel palette) — because colour-mode conversion and palette mapping are a separate backlog item ("Color mode conversion")

## Acceptance criteria

- [ ] AC-01 (always): Object ▸ Drop Shadow… and Color ▸ Quantize… appear in both modes; Stylize ▸ Outline… and Noise ▸ Reduce Noise… appear in Image mode only; entries are sorted as `effect-catalog-v2` AC-04 specifies.
- [ ] AC-02 (event): When the user opens one of the four, then the effect dialog shows exactly its parameters from the table with their ranges and defaults; Drop Shadow's Blur radius defaults to 4 in Image mode and 0 in Pixel mode, and its Colour defaults to opaque black.
- [ ] AC-03 (always): Drop Shadow never changes a fully opaque pixel, and with Opacity 0 or a fully transparent Colour it changes nothing.
- [ ] AC-04 (always): Drop Shadow with Blur 0 on a single opaque pixel at (x, y) on transparency, Offset (4, 4), Opacity 100 and an opaque Colour sets the pixel at (x+4, y+4) to exactly that colour, and no other pixel changes; with Blur 0 on a target whose pixels are all fully opaque or fully transparent, it adds at most one new colour (the shadow colour at its alpha).
- [ ] AC-05 (always): Stylize ▸ Outline leaves a solid single-colour layer unchanged and with Intensity 0 leaves every layer unchanged; at the boundary between a white and a black area it makes white pixels within Thickness of the boundary darker and leaves white pixels farther away unchanged.
- [ ] AC-06 (always): After Quantize the target's opaque pixels hold at most Colours distinct colours and every pixel's alpha is unchanged; a target that already has at most Colours distinct colours and Dithering 0 is unchanged, because both algorithms must return the input's own colours when there are no more of them than Colours.
- [ ] AC-07 (option): Where Quantize's Dithering is 0, two input pixels of the same colour get the same output colour; where Dithering is above 0, the result is still deterministic (same input, same output).
- [ ] AC-08 (always): Reduce Noise with Strength 0 leaves the layer unchanged, and with Strength 100 replaces each pixel by its neighbourhood median; a wire call with only the radius (the old form) behaves as Strength 100 (old calls with a radius above 10 now use that radius instead of 10).
- [ ] AC-09 (always): Stylize ▸ Outline and Reduce Noise never let a fully transparent pixel's stored colour tint an opaque neighbour (premultiplied, as `effects-adjustments` AC-18).
- [ ] AC-10 (error): If a wire call names one of the four with the wrong number of parameters, or Drop Shadow's colour slot is not an integer in 0…2³²−1 (a non-finite colour slot included), then it is rejected and changes nothing; in the number slots non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [ ] AC-11 (error): If the Pixel-mode document is not RGBA, then Drop Shadow and Quantize are refused with the same message as every other effect and the document is unchanged.
- [ ] AC-12 (option): Where the UI language is German, the names and parameter labels are German.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- Per-mode parameter defaults: `EffectDef` needs an optional per-mode default override (Drop Shadow Blur radius).
- Existing median: `crates/pincel-effects/src/noise/reduce_noise.rs`; `from_params` accepts 1 or 2 slots.
- Quantize: Median Cut and Octree are textbook algorithms; ties resolved deterministically.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Drop Shadow offset pushes the shadow fully outside the layer | Nothing visible changes |
| Drop Shadow on a trimmed Pixel cel | The shadow is clipped at the cel's edge, as every effect (never grows the cel) |
| Drop Shadow on a fully opaque layer | Unchanged (AC-03) |
| Quantize with Colours 256 on a 10-colour sprite | Unchanged |
| Quantize on a fully transparent target | Unchanged |
| Stylize ▸ Outline Thickness larger than the layer | Works; edge pixels clamp |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Drop Shadow paints behind, not as a new layer | Effects work on one layer; paint.net's Drop Shadow does the same within the layer | Low |
| Pixel mode's Blur radius default 0 | A blurred shadow creates anti-aliased colours; 0 keeps the sprite hard-edged, the user can still raise it | Low |
| Reduce Noise keeps its wire id with an optional Strength | Agent calls with the old one-slot form keep their meaning (AC-08) | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/effects-object-stylize-color-noise.spec.ts` › the four entries sit in their submenus, Outline and Reduce Noise in Image mode only (AC-01); `ui/e2e/effect-menus.spec.ts` › pixel mode: only per-pixel effects, no neighbourhood effect | built |  |
| AC-02 | `ui/e2e/effects-object-stylize-color-noise.spec.ts` › the dialogs show exactly the parameters of the table with their defaults (AC-02); › pixel mode: Drop Shadow with its defaults casts a hard half-opaque black shadow; `ui/e2e/effect-catalog.spec.ts` › drop_shadow: each mode starts at its own blur default, › drop_shadow / stylize_outline / quantize / reduce_noise: wire vectors equal the previous catalog's | built |  |
| AC-03 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` drop_shadow_never_changes_an_opaque_pixel, drop_shadow_opacity_zero_or_clear_colour_changes_nothing | built |  |
| AC-04 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` drop_shadow_blur_zero_single_pixel_lands_at_the_offset, drop_shadow_blur_zero_on_hard_alpha_adds_at_most_one_colour, drop_shadow_offset_outside_the_layer_changes_nothing; `crates/pincel-wasm/src/effects.rs` effects::tests::apply_effect_drop_shadow_casts_at_the_offset_in_one_undo_step, effects::tests::apply_effect_drop_shadow_clips_at_a_trimmed_cel | built |  |
| AC-05 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` stylize_outline_solid_layer_is_unchanged, stylize_outline_intensity_zero_changes_nothing, stylize_outline_darkens_white_within_thickness_only, stylize_outline_thickness_beyond_the_layer_works | built |  |
| AC-06 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` quantize_leaves_at_most_colours_and_keeps_alpha, quantize_few_colours_stay_unchanged, quantize_reads_its_colours_from_the_target; `crates/pincel-wasm/src/effects.rs` effects::tests::apply_effect_quantize_takes_its_palette_from_the_selection | built |  |
| AC-07 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` quantize_is_a_colour_map_without_dither_and_deterministic_with | built |  |
| AC-08 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` reduce_noise_strength_zero_changes_nothing, reduce_noise_strength_100_and_the_old_form_are_the_median | built |  |
| AC-09 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` neighbourhood_effects_ignore_transparent_colour | built |  |
| AC-10 | `crates/pincel-effects/tests/effects_object_stylize_color_noise.rs` from_params_wrong_parameter_count_is_rejected, drop_shadow_bad_colour_slot_is_rejected, from_params_non_finite_reads_as_zero_and_numbers_clamp; `crates/pincel-wasm/src/effects.rs` effects::tests::apply_effect_object_stylize_color_noise_errors_change_nothing | built |  |
| AC-11 | `crates/pincel-wasm/src/effects.rs` effects::tests::apply_effect_object_stylize_color_noise_on_an_indexed_document_are_refused | built |  |
| AC-12 | `ui/e2e/effects-object-stylize-color-noise.spec.ts` › German names and parameter labels (AC-12) | built |  |

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#6). Depends on `outline-effect` for the colour parameter kind.
- 2026-11-04: Clarification pass settled without a dialogue: distinct colours count RGB; shadow alpha = colour alpha × opacity; Outline's neighbourhood is a square; Reduce Noise's median ignores transparent colour; per-mode defaults are dialog starting values only; non-finite numbers read as 0, a non-finite colour slot is rejected.
- 2026-11-05 (spec-build): Red run before the wire existed: every test in `effects_object_stylize_color_noise.rs` failed with "unknown effect" for the three new ids, and the Reduce Noise tests with "effect `reduce_noise` expects 1 parameter(s), got 2".
- 2026-11-05 (spec-build): Drop Shadow's blur is a Gaussian (σ = radius / 3, as Gaussian Blur) of the shifted alpha, reading pixels beyond the layer as transparent; its alpha is rounded to a byte before compositing, so a shadow too faint for one alpha step leaves a pixel untouched — the reason AC-03 holds exactly for Opacity 0.
- 2026-11-05 (spec-build): Stylize ▸ Outline uses BT.709 luma (the Terms' definition, via `adjustments-paintnet`); Outline's square is `(2·Thickness + 1)²`, cut at the layer's edge, and fully transparent pixels take no part in it (neither colour nor luma) and stay as they are; alpha is kept. This is how it satisfies AC-09 — ignoring transparent pixels as Blurs ▸ Median does, rather than mixing premultiplied values, which would darken every shape boundary.
- 2026-11-05 (spec-build): Quantize reads its palette from the target (`EffectSpec::run_in`), because AC-06 requires a target with few colours to stay unchanged even when the rest of the layer has many. Pixels map to the nearest palette colour (RGB distance, earlier entry on a tie) for both algorithms; dithering diffuses only among the target's opaque pixels, left to right, top to bottom. Octree merges the deepest internal nodes with the fewest pixels first and may end below Colours (a merge removes up to seven leaves).
- 2026-11-05 (spec-build): Reduce Noise is Blurs ▸ Median at percentile 50 blended premultiplied by Strength; Strength 0 and 100 return the layer and the median exactly. New code lives in the submenu's module where one exists: `object::DropShadow`, `distort::StylizeOutline` (beside Emboss, Edge Detect and Relief, the other Stylize effects), a new `color` module for `Quantize`, `noise::ReduceNoise`.
- 2026-11-05 (spec-build): The Pixel-core tests for AC-10, AC-11 and the trimmed cel were green at once: the refusal of non-RGBA cels, the wire errors and the clipping at the cel are generic paths every effect shares. The selection test of AC-06 is not: with `run_in` bypassed for Quantize (a whole-layer palette) it and `quantize_reads_its_colours_from_the_target` fail.
- 2026-11-05 (spec-build): German labels: Versatz X / Y, Weichzeichnungsradius, Deckkraft (as the layer opacity), Farben, Median-Schnitt, Octree, Dithering, Stärke; Reduce Noise's German name changes from "Rauschen reduzieren" to the table's "Rauschen entfernen". The colour label reuses `fx.Color` (Farbe), as Outline Object does.
- 2026-11-05 (spec-build): Red run of the UI block, built with the catalog and i18n changes reverted: 10 of the 54 tests in `effects-object-stylize-color-noise.spec.ts` and `effect-catalog.spec.ts` failed — the new entries missing from the catalog and the menus ("Drop Shadow…" not in ["Outline Object…"], "Schlagschatten…" not in ["Objekt umranden…"]), Reduce Noise's pin and dialog on the old one-slot vector.
- 2026-11-05 (spec-build): The UI red run was repeated on a preview server verified with `netstat` as this checkout's own (another worktree's stale `vite preview` held port 4293, so this run and the full gates used the free port 4295): the same 10 of 54 tests failed for the same reasons; after restoring, the full gates were green (e2e 376 passed).
