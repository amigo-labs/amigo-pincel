---
status: todo
created: 2026-10-29
size: M
mode: both
design: [pincel.md §13.4, pincel.md §15 (2026-09-06 Fineliner feature parity), pincel.md §15 (2026-09-06 Two cores, one shell), pincel.md §15 (2026-10-11 One effect wire format for both modes), pincel.md §15 (2026-10-12 Image-mode effects honour the selection)]
---

# Effects and adjustments in both modes

## Context

Both editors offer the same twenty effects and adjustments — blurs, sharpening, distortions, noise and nine colour adjustments — inherited from the sister project Fineliner and vendored as the shared crate `pincel-effects` (Decision Log 2026-09-06). Every effect is a pure function from RGBA8 pixels to RGBA8 pixels; the two wasm layers call it with one wire format (effect name plus a flat number vector) and both UIs build their menus and the one parameter dialog from a single catalog (Decision Log 2026-10-11). Image mode blends the result into the layer by selection coverage (Decision Log 2026-10-12); Pixel mode confines it to the marquee. This spec records that behavior as it is today.

## Scope

### In scope

- The Effects menu (Blur ▸, Sharpen ▸, Distort ▸, Noise ▸) and the Adjustments menu, identical in both modes
- The effect dialog: parameters and defaults, live preview, Reset, OK / Enter, Cancel / Escape, the scope line
- Applying an effect to the active layer (Image mode) or the active cel on the current frame (Pixel mode), inside the selection, as one undo step
- The pixel behavior of the effects that users rely on (neutral parameters, solid areas, alpha, reproducible noise)
- The wire contract both wasm layers accept

### Out of scope

- An Outline effect and a colour parameter kind — because the planned `outline-effect` spec adds them
- Running effects off the main thread — because the planned `performance-background-jobs` spec covers it
- Non-destructive adjustment layers and layer effects — because they need compose changes in both cores and are not planned yet
- Effects on indexed Pixel sprites — because Pixel mode only runs effects on RGBA documents today (see AC-11); indexed compose is the planned `indexed-grayscale-compose` spec's area
- Drop Shadow and Color replace from §13.4 — because neither is implemented; §13.4's Hue / Saturation shift exists as the Hue / Saturation adjustment

## Acceptance criteria

- [ ] AC-01 (always): In both modes the Effects menu holds the submenus Blur (Gaussian Blur, Box Blur, Motion Blur, Radial Blur), Sharpen (Sharpen, Unsharp Mask), Distort (Emboss, Edge Detect, Relief) and Noise (Add Noise, Reduce Noise), and the Adjustments menu lists Brightness / Contrast, Hue / Saturation, Curves, Levels, Color Balance, Invert Colors, Grayscale, Posterize and Threshold; an entry with parameters ends in "…", and every entry is disabled while no document is open.
- [ ] AC-02 (event): When the user picks an effect, then its dialog opens titled with the effect's name, showing each parameter at its default from the table in Technical notes — a number as a slider with a number field, a switch as a checkbox, a choice as a dropdown — or "This effect has no parameters."; Reset returns every parameter to its default.
- [ ] AC-03 (state): While the effect dialog is open with Preview on, the canvas shows the active layer with the effect at the current parameters, and the document and its undo history stay unchanged; turning Preview off shows the image without the effect.
- [ ] AC-04 (event): When the user confirms the dialog (OK or Enter), then the effect is applied to the active layer (Pixel mode: the active layer's cel on the current frame) as exactly one undo step that never merges with a neighbouring stroke, and the dialog closes; Pixel mode's status bar reads "<Effect> applied".
- [ ] AC-05 (event): When the user cancels the dialog (Cancel or Escape), then it closes, the canvas shows the unchanged image and no undo step is added.
- [ ] AC-06 (state): While a selection exists in Image mode, the effect changes only selected pixels, and a partially selected (feathered) pixel becomes the mix of its old and new value weighted by its selection coverage.
- [ ] AC-07 (state): While a marquee selection exists in Pixel mode, the effect changes only the pixels inside it and the rest of the cel keeps its pixels.
- [ ] AC-08 (state): While a selection exists, the dialog header names "selection" as the effect's scope; otherwise it names "whole layer".
- [ ] AC-09 (error): If the selection is empty (Image mode) or lies entirely outside the active cel (Pixel mode), then confirming changes nothing, adds no undo step and shows "<Effect>: the selection is empty, nothing changed" (Image mode) or "<Effect>: selection is outside the layer, nothing changed" (Pixel mode).
- [ ] AC-10 (error): If an effect is requested with an unknown name or the wrong number of parameters, then it is rejected with an error and the document and undo history are unchanged.
- [ ] AC-11 (error): If the Pixel sprite is indexed or the active layer has no image cel on the current frame, then applying an effect fails with "<Effect> failed: …" in the status bar and nothing changes.
- [ ] AC-12 (error): If the active layer is locked, then applying an effect leaves the layer unchanged and adds no undo step.
- [ ] AC-13 (always): Every effect returns an image the size of its input, and at neutral settings leaves pixels unchanged: Brightness / Contrast 0 / 0 (plain and enhanced), Color Balance all 0, the identity Curve, identity Levels, Hue / Saturation on a gray pixel, Box Blur 1 × 1, Gaussian Blur radius 0, Motion Blur distance 1, Unsharp Mask amount 0, Add Noise amount 0.
- [ ] AC-14 (always): A solid single-color area stays unchanged under Gaussian, Box, Motion and Radial (spin and zoom) Blur, Sharpen, Unsharp Mask, Relief and Reduce Noise; Edge Detect turns it black and Emboss flat gray.
- [ ] AC-15 (always): Invert Colors flips each RGB channel (`255 − v`), keeps alpha, and applied twice restores the original.
- [ ] AC-16 (always): Add Noise with the same parameters and seed produces identical pixels, a different seed produces different pixels, alpha is kept, and Monochromatic noise keeps the three channels of a gray pixel equal.
- [ ] AC-17 (always): Grayscale makes the three channels of every pixel equal; Threshold turns every pixel black or white by its luminance; Posterize with 2 levels snaps channels to 0 or 255 and keeps 0 and 255 unchanged; a Curve on one channel leaves the other channels untouched.
- [ ] AC-18 (always): A spatial effect (blur, sharpen, distort, reduce noise) never lets the color of a fully transparent pixel tint its opaque neighbours.

## Technical notes

Hints, verified 2026-10-29; paths and line numbers drift.

**Crate** (`crates/pincel-effects/`, no `pincel-core` dependency, `thiserror` only): `pub trait Effect { fn apply(&self, src: &EffectImage) -> EffectImage; fn scaled(&self, factor: f32) -> Self; fn preview(&self, src, max_dim) -> EffectImage }`; `EffectImage::from_rgba8(w, h, Vec<u8>) -> Result<_, EffectError>`; sRGB (not linearised), f32 inside, premultiplied alpha for spatial effects. `EffectSpec::from_params(name: &str, p: &[f64]) -> Result<EffectSpec, EffectError>` (`EffectError::{BufferSizeMismatch, UnknownEffect, ParamCount}`), `EffectSpec::run(&src)`, `EffectSpec::label()`; `EFFECT_NAMES` in menu order. Non-finite numbers read as 0; constructors clamp. `Effect::preview` (downscaled preview) is not used by either app — both previews run at full resolution.

**Wire / catalog** (`ui/src/lib/effects/catalog.ts`, `EFFECTS`; choices travel as the option index, booleans as 0 / 1):

| id | Group | Parameters (default) |
| -- | ----- | -------------------- |
| `gaussian_blur` | Blur | Radius 0.1–250 (2) |
| `box_blur` | Blur | Width 1–251 (3), Height 1–251 (3) |
| `motion_blur` | Blur | Distance 1–100 (6), Angle 0–360 (0) |
| `radial_blur` | Blur | Amount 1–100 (10), Center X 0–1 (0.5), Center Y 0–1 (0.5), Kind Spin / Zoom (Spin) |
| `sharpen` | Sharpen | — |
| `unsharp_mask` | Sharpen | Amount % 1–500 (50), Radius 0.1–50 (1), Threshold 0–255 (0) |
| `emboss` | Distort | Angle 0–360 (45), Elevation 0–90 (45), Relief 0.1–10 (1) |
| `edge_detect` | Distort | Algorithm Sobel / Prewitt / Laplacian (Sobel), Amount % 1–400 (100) |
| `relief` | Distort | Angle 0–360 (45), Amount 0.1–10 (1) |
| `add_noise` | Noise | Amount 1–100 (20), Distribution Uniform / Gaussian (Uniform), Channels RGB / Monochromatic (RGB), Seed 0–9999 (1) |
| `reduce_noise` | Noise | Radius 1–10 (1) |
| `brightness_contrast` | Adjust | Brightness −150–150 (0), Contrast −150–150 (0), Enhanced (S-curve) (off) |
| `hue_saturation` | Adjust | Hue −180–180 (0), Saturation −100–100 (0), Lightness −100–100 (0), Colorize (off) |
| `curves` | Adjust | Channel Composite / Red / Green / Blue / Alpha (Composite), Out @ 0 / 64 / 128 / 192 / 255 (identity); sent as `[channel, x0, y0, …]` in [0, 1] |
| `levels` | Adjust | Channel Composite / Red / Green / Blue (Composite), Input black 0–255 (0), Input white 0–255 (255), Gamma 0.1–10 (1), Output black (0), Output white (255); points sent in [0, 1] |
| `color_balance` | Adjust | Shadows / Midtones / Highlights × cyan–red, magenta–green, yellow–blue, each −100–100 (0); Preserve luminosity (on) |
| `invert` | Adjust | — |
| `grayscale` | Adjust | Method Luminosity / Average / BT.709 / Channel mixer (Luminosity), Red 0–1 (0.3), Green 0–1 (0.59), Blue 0–1 (0.11) |
| `posterize` | Adjust | Levels 2–64 (4) |
| `threshold` | Adjust | Threshold 0–255 (128) |

**Image mode** (`crates/pincel-image-wasm/src/lib.rs`): `apply_effect(handle, layer, name, params: &[f64]) -> Result<bool, JsError>` — refuses a locked layer (`"layer is locked"`), runs the effect on the whole layer, then `masked_region(before, after, selection)` blends by coverage (`before + (after − before)·cov/255`) and crops the delta to the selection's bounding box; commits `SetPixels::new(layer, region, after).with_label(spec.label())`; `Ok(false)` when the selection is empty. `preview_effect(handle, layer, name, params) -> Clamped<Vec<u8>>` composites the whole canvas with that layer replaced. UI: `ImageEditor.svelte` opens `EffectDialog` with `hasSelection={editor.hasSelection}`; `controller.ts` `applyEffect` / `previewEffect` / `clearEffectPreview` (preview shown via `editor.previewComposite`).

**Pixel mode** (`crates/pincel-wasm/src/effects.rs`): `effectNames()`, `Document.applyEffect(name, params) -> Result<bool, String>` and `previewEffect(name, params, zoom) -> ComposeFrame`; target = `edit_target_layer()` (errors on a locked layer) and its cel on `current_frame`; errors on a non-image cel and on `ColorMode` other than `Rgba`; selection = `sprite.selection` rect, per pixel `selection_mask.contains` for shaped selections (binary, no coverage); commits `ReplaceCelPixels` between two `bus.seal()` calls and emits a `dirty-rect` event. UI: `ui/src/modes/pixel/image-ops.ts` (`openEffect`, `previewEffectParams`, `applyEffectParams`, `cancelEffect`), status strings `px.effectApplied` / `px.effectOutside` / `px.effectFailed`.

**Dialog** (`ui/src/lib/components/EffectDialog.svelte`): non-modal panel (`role="dialog"`, `aria-modal="false"`), preview re-run coalesced through `requestAnimationFrame`; an empty parameter vector means "preview off". Menus: `ui/src/lib/effects/menu.ts` `effectMenus(pick, disabled)`; labels translated via `lib/effects/labels.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A parameter typed beyond its slider range | Clamped by the effect's constructor; non-finite values read as 0 |
| Invert Colors on a layer with transparent pixels | They stay transparent (alpha untouched) |
| An effect applied right after a brush stroke, then another stroke | Three separate undo steps |
| Preview toggled off and on again | The preview re-runs with the current parameters |
| Grayscale sent with one parameter instead of four | Accepted; the channel-mixer weights default to 0.3 / 0.59 / 0.11 |
| Curves with fewer than one control point or an odd coordinate count | Rejected as a wrong parameter count |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 | `ui/e2e/effects.spec.ts › image mode: effects and adjustments come from the shared catalog`; `ui/e2e/effects.spec.ts › pixel mode: the Levels adjustment applies`; `pincel-wasm: effects::tests::apply_effect_invert_changes_pixels_and_undo_restores`, `effects::tests::apply_effect_is_its_own_undo_entry_after_a_stroke` | built | |
| AC-05 |      | open   |        |
| AC-06 | `ui/e2e/effects.spec.ts › image mode: an effect stays inside the selection`; `pincel-image-core: selection::tests::masked_region_with_hard_mask_crops_to_the_selection`, `selection::tests::masked_region_mixes_partial_coverage` | built | |
| AC-07 | `pincel-wasm: effects::tests::apply_effect_respects_the_marquee_selection` | built | |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 | `pincel-effects: spec::tests::from_params_unknown_name_is_an_error`, `spec::tests::from_params_wrong_arity_is_an_error`; `pincel-wasm: effects::tests::apply_effect_unknown_name_or_bad_arity_errors` | built | |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 | `pincel-effects: spec::tests::from_params_every_name_parses_with_its_documented_arity`, `adjust::brightness_contrast::tests::zero_is_identity_legacy`, `adjust::brightness_contrast::tests::zero_is_identity_enhanced`, `adjust::color_balance::tests::zero_balance_is_identity`, `adjust::curves::tests::curves_identity_is_a_no_op`, `adjust::levels::tests::levels_identity_is_a_no_op`, `adjust::hue_sat::tests::identity_on_gray_is_unchanged`, `blur::box_blur::tests::box_blur_1x1_returns_identity`, `blur::gaussian::tests::gaussian_blur_with_sigma_zero_returns_identity`, `blur::motion::tests::motion_blur_distance_one_is_identity`, `sharpen::unsharp_mask::tests::unsharp_amount_zero_is_identity`, `noise::add_noise::tests::add_noise_amount_zero_is_identity` | built | |
| AC-14 | `pincel-effects: blur::gaussian::tests::gaussian_blur_of_solid_color_is_unchanged`, `blur::box_blur::tests::box_blur_of_solid_color_is_unchanged`, `blur::motion::tests::motion_blur_of_solid_color_is_unchanged`, `blur::radial::tests::radial_spin_of_solid_color_is_unchanged`, `blur::radial::tests::radial_zoom_of_solid_color_is_unchanged`, `sharpen::convolution::tests::sharpen_of_solid_color_is_unchanged`, `sharpen::unsharp_mask::tests::unsharp_of_solid_color_is_unchanged`, `distort::relief::tests::relief_of_solid_color_is_unchanged`, `noise::reduce_noise::tests::median_of_solid_is_unchanged`, `distort::edge_detect::tests::edge_detect_of_solid_is_black`, `distort::emboss::tests::emboss_of_solid_is_flat_gray` | built | |
| AC-15 | `pincel-effects: spec::tests::run_invert_flips_rgb_and_keeps_alpha`, `adjust::invert::tests::invert_of_invert_is_identity`, `adjust::invert::tests::invert_preserves_alpha` | built | |
| AC-16 | `pincel-effects: noise::add_noise::tests::add_noise_is_reproducible_for_a_fixed_seed`, `noise::add_noise::tests::different_seeds_differ`, `noise::add_noise::tests::add_noise_changes_pixels_and_preserves_alpha`, `noise::add_noise::tests::monochromatic_keeps_channels_equal_on_gray` | built | |
| AC-17 | `pincel-effects: adjust::grayscale::tests::grayscale_output_channels_are_equal`, `adjust::threshold::tests::threshold_splits_by_luminance`, `adjust::posterize::tests::posterize_two_levels_snaps_to_black_or_white`, `adjust::posterize::tests::posterize_preserves_endpoints`, `adjust::curves::tests::red_channel_curve_leaves_others_untouched` | built | |
| AC-18 |      | open   |        |

Partial coverage (status stays `open`): AC-01 — `effects.spec.ts` opens Blur ▸ Gaussian Blur… and Levels… only; `pincel-wasm: effects::tests::effect_names_matches_the_effects_crate` checks the count of wire names, not the menus. AC-02 — `effects.spec.ts › image mode: effects and adjustments come from the shared catalog` checks the title and one default (Levels "Input white" 255); the other defaults and Reset are untested. AC-03 — `pincel-wasm: effects::tests::preview_effect_leaves_document_and_history_untouched` covers Pixel mode's core; Image mode's `preview_effect` and the Preview checkbox are untested. AC-08 — `effects.spec.ts › image mode: an effect stays inside the selection` checks "selection" in Image mode only. AC-09 — `pincel-wasm: effects::tests::apply_effect_with_selection_off_canvas_is_a_noop` covers Pixel mode's core (no undo step) without the notice; Image mode is untested. AC-18 — `pincel-effects: image::tests::premultiplied_transparent_pixel_has_zero_color_weight` covers the conversion helper, not an effect.

## Decisions

{Filled during revision and implementation — decision plus reason.}
