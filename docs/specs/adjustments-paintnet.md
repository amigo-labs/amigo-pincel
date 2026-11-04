---
status: implemented
created: 2026-11-04
size: M
mode: both
design: [pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 paint.net catalog)]
---

# paint.net adjustments: Auto-Level, Black and White, Exposure, Highlights / Shadows, Invert Alpha, Sepia, Temperature and Tint

## Context

paint.net's Adjustments menu has thirteen entries (paint.net/doc/latest/AdjustmentsMenu.html). Pincel has seven of them (Brightness / Contrast, Curves, Hue / Saturation, Invert Colors, Levels, Posterize, and Grayscale as paint.net's Black and White) plus two of its own (Color Balance, Threshold). This spec adds the missing six and renames Grayscale to Black and White, completing the owner's "every paint.net adjustment".

All of them are per-pixel colour operations, so they appear in both modes (Decision Log 2026-11-04, `effect-catalog-v2`). The algorithms are Pincel's own, written from paint.net's documented behaviour; paint.net's docs give no formulas or ranges, so the parameters below are this spec's.

**Terms.** *Target*: the pixels an adjustment works on — the active layer (Image mode) or the active cel (Pixel mode), limited to the selection when one exists, as for every effect (`effects-adjustments`). *Luma*: 0.2126 R + 0.7152 G + 0.0722 B on 0–255 sRGB values. *Opaque pixel*: alpha > 0. *Neutral setting*: the parameter values listed as neutral in the table. *Exact values*: ACs that state exact pixel values or reversibility hold for pixels that are fully selected, or for a target without a selection; partly selected pixels of an Image-mode soft selection blend the result by their coverage, as every effect does.

## Adjustments

| Wire id | Name (en / de) | Parameters: range (default) | Neutral setting | What it does |
| ------- | -------------- | --------------------------- | --------------- | ------------ |
| `auto_level` | Auto-Level / Auto-Tonwert | none | — | Finds the lowest and highest luma L0 and L1 among opaque target pixels and maps every channel c to (c − L0) · 255 / (L1 − L0), clamped to 0–255; when L1 = L0 nothing changes |
| `grayscale` (renamed) | Black and White / Schwarzweiß | unchanged (Method, weights) | — | As before; only the name changes |
| `exposure` | Exposure / Belichtung | Exposure −5 to +5 EV, step 0.01 (0) | 0 | Multiplies each channel in linear light by 2^EV, converted back to sRGB, clamped |
| `highlights_shadows` | Highlights / Shadows / Tiefen / Lichter | Shadows −100 to 100 (0), Highlights −100 to 100 (0) | 0, 0 | With s = Shadows / 100, h = Highlights / 100, l = luma / 255 and each channel c in 0–1: c += s · 0.5 · (1 − l)² · (1 − c) for s > 0 or · c for s < 0; then c −= h · 0.5 · l² · c for h > 0 or · (1 − c) for h < 0; clamped |
| `invert_alpha` | Invert Alpha / Alpha umkehren | none | — | Alpha becomes 255 − alpha; colour channels unchanged |
| `sepia` | Sepia / Sepia | Intensity 0–100 (100) | 0 | Blends each pixel toward its sepia tone by Intensity %; the sepia tone of a pixel with luma l is (min(255, 1.0 · l), 0.86 · l, 0.67 · l) |
| `temperature_tint` | Temperature and Tint / Temperatur und Farbton | Temperature −100 to 100 (0), Tint −100 to 100 (0) | 0, 0 | Temperature > 0 warms (red up, blue down), < 0 cools; Tint > 0 shifts toward magenta (green down), < 0 toward green |

## Scope

### In scope

- The six new adjustments in both modes' Adjustments menu, through the shared effect pipeline (preview, OK / Cancel, selection, one undo step)
- Grayscale renamed to Black and White
- en + de names and parameter labels

### Out of scope

- Removing Color Balance or Threshold — because they are useful extras paint.net lacks, and removing them would break existing agent scripts
- Auto-Level options (clip percentage, per-channel mode) — because paint.net's Auto-Level has no dialog; one predictable stretch is the documented behaviour
- Matching paint.net's exact curves — because its algorithms are closed source (Decision Log 2026-11-04)

## Acceptance criteria

- [x] AC-01 (always): In both modes the Adjustments menu contains Auto-Level, Black and White…, Exposure…, Highlights / Shadows…, Invert Alpha, Sepia… and Temperature and Tint… alongside the existing entries, sorted as `effect-catalog-v2` AC-04 specifies, and no entry named Grayscale remains.
- [x] AC-02 (event): When the user opens Exposure, Highlights / Shadows, Sepia or Temperature and Tint, then the effect dialog shows exactly the parameters of the table with their ranges and defaults; Auto-Level and Invert Alpha show "This effect has no parameters."
- [x] AC-03 (always): Exposure, Highlights / Shadows, Sepia and Temperature and Tint at their neutral settings leave every pixel unchanged.
- [x] AC-04 (always): Every adjustment of this spec except Invert Alpha leaves every pixel's alpha unchanged; Invert Alpha leaves every pixel's colour channels unchanged and sets alpha to 255 − alpha, so applying it twice without a selection restores the target.
- [x] AC-05 (always): After Auto-Level on a gray target (R = G = B) with at least two different values, its darkest pixel is 0 ± 1 and its brightest 255 ± 1; a target whose opaque pixels all have the same luma (including one colour), or with no opaque pixel, is unchanged.
- [x] AC-06 (always): Exposure +1 EV on mid-gray (128, 128, 128) gives a lighter gray (all channels equal, > 128); −1 EV gives a darker gray; +5 EV on mid-gray gives (255, 255, 255).
- [x] AC-07 (always): Highlights / Shadows with Shadows 100 raises the luma of a dark pixel (luma 40) by more than it raises a bright pixel (luma 215); Highlights 100 lowers the bright pixel's luma by more than the dark pixel's.
- [x] AC-08 (always): Sepia at Intensity 100 maps every pixel to a colour with R ≥ G ≥ B, and any two pixels with the same luma to the same colour.
- [x] AC-09 (always): Temperature 100 on mid-gray gives R > 128 > B; Temperature −100 gives B > 128 > R; Tint 100 gives G below both R and B; Tint −100 gives G above both.
- [x] AC-10 (always): Each adjustment changes only pixels of the target: pixels outside the selection, other layers and other cels are unchanged, and applying it is one undo step (as `effects-adjustments` specifies for every effect).
- [x] AC-11 (error): If a wire call names one of the new adjustments with the wrong number of parameters, then it is rejected as a wrong parameter count and changes nothing; non-finite numbers read as 0 and every number is then clamped to its range, as for every effect (`effects-adjustments`).
- [x] AC-12 (option): Where the UI language is German, the names and parameter labels are the German ones of the table.
- [x] AC-13 (error): If the Pixel-mode document is not RGBA (indexed or grayscale), then the new adjustments are refused with the same message as every other effect and the document is unchanged.

## Technical notes

Hints, not contracts — paths drift (verified 2026-11-04).

- One module per adjustment under `crates/pincel-effects/src/adjust/`, a variant in `EffectSpec` (`spec.rs`: `from_params`, `run`, `label`, `EFFECT_NAMES`), an entry in `ui/src/lib/effects/catalog.ts` (modes both, submenu `adjust`), i18n keys `effect.<id>` / `fx.<label>` in `en.ts` and `de.ts`. The parity checks (`every_effect_name_is_accepted`, `assertCatalogCoversAllEffects`) catch drift.
- sRGB ↔ linear: the standard sRGB transfer function; `pincel-effects` works on f32 internally.
- Agent tools (MCP) pick up new `EFFECT_NAMES` automatically; their docs list names from that constant.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Auto-Level on a selection with only transparent pixels | Unchanged (AC-05) |
| Invert Alpha on a fully transparent pixel | Becomes fully opaque, showing whatever colour the layer stored for it (the cores keep colour channels of transparent pixels; a never-painted pixel is black) |
| Exposure on pure black | Stays black (2^EV × 0 = 0) |
| Auto-Level on several colours with one luma | Unchanged (L1 = L0) |
| Sepia on a pure white pixel | Becomes (255, 219, 171) |
| Temperature and Tint both at ±100 | Both shifts apply; channels clamp to 0–255 |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Grayscale's method choice stays under the new name | Removing it would change existing agent calls; paint.net's Black and White is the Luminosity method | Low |
| Ranges and formulas are this spec's | paint.net documents no formulas; the owner wants the behaviour, not pixel parity (Decision Log 2026-11-04) | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Summary

> Completed: 2026-11-04

- Six paint.net adjustments in `pincel-effects` (Auto-Level, Exposure, Highlights / Shadows, Invert Alpha, Sepia, Temperature and Tint), wired through `EffectSpec` / `EFFECT_NAMES` with parameter-count checks and clamping
- `EffectSpec::run_in` passes the host's selection as the target, so Auto-Level reads its luma range from the selection in both cores and the agent tools
- Both modes' Adjustments menu lists the new entries with en / de names and parameter labels; Grayscale is shown as Black and White (wire id `grayscale` unchanged)
- Agent (MCP) catalog exposes the new adjustments with the same keys and ranges

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `both modes list the paint.net adjustments, sorted, without Grayscale (AC-01)` (ui/e2e/adjustments-paintnet.spec.ts); `effect-menus.spec.ts` (Black and White in both modes) | verified | d4ea8a9 |
| AC-02 | `the dialogs show exactly the parameters of the table (AC-02)` (ui/e2e/adjustments-paintnet.spec.ts); pinned wire vectors in `ui/e2e/effect-catalog.spec.ts` | verified | d4ea8a9 |
| AC-03 | `neutral_settings_leave_every_pixel_unchanged` (crates/pincel-effects/tests/adjustments_paintnet.rs); `exposure_zero_is_the_identity_lut` (crates/pincel-effects/src/adjust/exposure.rs) | verified | 868d4d6 |
| AC-04 | `every_adjustment_of_the_spec_except_invert_alpha_keeps_alpha` (includes Black and White), `invert_alpha_flips_alpha_keeps_colour_and_twice_restores` (crates/pincel-effects/tests/adjustments_paintnet.rs) | verified | 868d4d6 |
| AC-05 | `auto_level_stretches_a_gray_target_to_the_full_range`, `auto_level_ignores_transparent_pixels_for_its_range`, `auto_level_without_a_luma_range_changes_nothing`, `auto_level_takes_its_range_from_the_target_pixels` (crates/pincel-effects/tests/adjustments_paintnet.rs); `apply_effect_auto_level_takes_its_range_from_the_selection` (crates/pincel-wasm/src/effects.rs); `apply_effect_auto_level_stretches_the_selection_only` (crates/pincel-image-wasm/src/lib.rs) | verified | 868d4d6 |
| AC-06 | `exposure_scales_mid_gray_in_linear_light` (crates/pincel-effects/tests/adjustments_paintnet.rs) | verified | 868d4d6 |
| AC-07 | `highlights_shadows_act_on_their_tonal_range` (crates/pincel-effects/tests/adjustments_paintnet.rs) | verified | 868d4d6 |
| AC-08 | `sepia_at_full_intensity_is_a_function_of_luma` (crates/pincel-effects/tests/adjustments_paintnet.rs) | verified | 868d4d6 |
| AC-09 | `temperature_and_tint_shift_mid_gray` (crates/pincel-effects/tests/adjustments_paintnet.rs) | verified | 868d4d6 |
| AC-10 | `apply_effect_paintnet_adjustments_change_only_the_target_in_one_undo_step` (crates/pincel-wasm/src/effects.rs); `paintnet_adjustments_match_the_editor_inside_the_selection` (crates/pincel-agent/src/tests/image_draw.rs); `apply_effect_auto_level_stretches_the_selection_only` (crates/pincel-image-wasm/src/lib.rs); `image mode: every new adjustment applies as one undo step (AC-10)`, `pixel mode: every new adjustment applies (AC-10)` (ui/e2e/adjustments-paintnet.spec.ts) | verified | 9c5b9b9 |
| AC-11 | `wrong_parameter_count_is_rejected`, `non_finite_numbers_read_as_zero_then_clamp_to_the_range` (crates/pincel-effects/tests/adjustments_paintnet.rs); `apply_effect_paintnet_wrong_parameter_count_changes_nothing` (crates/pincel-wasm/src/effects.rs) | verified | 868d4d6 |
| AC-12 | `German names and parameter labels (AC-12)` (ui/e2e/adjustments-paintnet.spec.ts) | verified | d4ea8a9 |
| AC-13 | `apply_effect_paintnet_on_an_indexed_document_is_refused` (crates/pincel-wasm/src/effects.rs) | verified | 9c5b9b9 |

## Decisions

- 2026-11-04: Written without a dialogue while the owner's goal "all planned specs implemented" was active, following the approved plan (#3). Parameters and formulas are Pincel's own because paint.net's docs give none.
- 2026-11-04: Clarification pass settled without a dialogue: Auto-Level, Highlights / Shadows and Sepia formulas pinned; Exposure AC on mid-gray; non-finite numbers read as 0 (the existing convention); exact-value ACs hold for fully selected pixels.
- 2026-11-04 (spec-build): The spec reached this branch by cherry-picking `5b0ca4c` from `spec/paintnet-effects-tabs`; its companion `5012906` was already contained here, so the cherry-pick was empty and skipped.
- 2026-11-04 (spec-build): Auto-Level reads L0 / L1 from the target, not from the whole layer — because the Terms define the target as the selection. New `EffectSpec::run_in(src, in_target)` passes the host's selection (Pixel: `Sprite::selection_contains`, Image: mask coverage > 0); every other effect ignores it. Target pixels that are transparent are mapped too (the formula covers every channel), they only do not count for the range.
- 2026-11-04 (spec-build): Luma is computed as the integer `2126 R + 7152 G + 722 B` — so "the same luma" (AC-05, AC-08) is an exact comparison without float noise.
- 2026-11-04 (spec-build): Temperature and Tint are multiplicative sRGB channel gains of ±30 % at ±100 (red ×(1 + 0.3 t), blue ×(1 − 0.3 t), green ×(1 − 0.3 n)) — because the spec gives directions only; gains keep black black.
- 2026-11-04 (spec-build): Exposure runs through a 256-entry LUT built in f64 (exactly the identity at 0 EV, AC-03); Sepia blends as `c · (1 − k) + tone · k`, exact at 0 % and 100 %.
- 2026-11-04 (spec-build): Parameter keys and German labels the table leaves open: `exposure` (Belichtung), `shadows` / `highlights` (Tiefen / Lichter), `intensity` (Intensität), `temperature` / `tint` (Temperatur / Farbton); the agent catalog uses the same keys and ranges. The new wire names follow `threshold` at the end of `EFFECT_NAMES`.
- 2026-11-04 (spec-build): The Rust label of `grayscale` (undo label) also becomes "Black and White"; the wire id stays `grayscale` (Assumptions).
- 2026-11-04 (spec-build): `effect-catalog.spec.ts` keeps requiring a pin for every catalog entry; the six new entries got pins of their own (table order and defaults) instead of exempting them.
- 2026-11-04 (spec-build): AC-13 is proven in the Pixel core (`pincel-wasm`), where the refusal lives — the UI cannot create a non-RGBA document to test it end to end.
