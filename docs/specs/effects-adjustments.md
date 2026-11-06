---
status: implemented
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

- ~~AC-01 (always): In both modes the Effects menu holds the submenus Blur, Sharpen, Distort and Noise, and the Adjustments menu lists nine adjustments …~~ dropped: superseded by [effect-catalog-v2](effect-catalog-v2.md) AC-01 – AC-06 (paint.net submenus, per-mode filter).
- [x] AC-02 (event): When the user picks an effect, then its dialog opens titled with the effect's name, showing each parameter at its default from the table in Technical notes, as amended by the later effect specs (`effects-blurs`, `effects-object-stylize-color-noise`, …) for the parameters they changed — a number as a slider with a number field, a switch as a checkbox, a choice as a dropdown, an angle, point, random seed or linked pair as [effect-catalog-v2](effect-catalog-v2.md) AC-09 – AC-13 specify — or "This effect has no parameters."; Reset returns every parameter to its default.
- [x] AC-03 (state): While the effect dialog is open with Preview on, the canvas shows the active layer with the effect at the current parameters, and the document and its undo history stay unchanged; turning Preview off shows the image without the effect.
- [x] AC-04 (event): When the user confirms the dialog (OK or Enter), then the effect is applied to the active layer (Pixel mode: the active layer's cel on the current frame) as exactly one undo step that never merges with a neighbouring stroke, and the dialog closes; Pixel mode's status bar reads "<Effect> applied".
- [x] AC-05 (event): When the user cancels the dialog (Cancel or Escape), then it closes, the canvas shows the unchanged image and no undo step is added.
- [x] AC-06 (state): While a selection exists in Image mode, the effect changes only selected pixels, and a partially selected (feathered) pixel becomes the mix of its old and new value weighted by its selection coverage.
- [x] AC-07 (state): While a marquee selection exists in Pixel mode, the effect changes only the pixels inside it and the rest of the cel keeps its pixels.
- [x] AC-08 (state): While a selection exists, the dialog header names "selection" as the effect's scope; otherwise it names "whole layer".
- [x] AC-09 (error): If the selection is empty (Image mode) or lies entirely outside the active cel (Pixel mode), then confirming changes nothing, adds no undo step and shows "<Effect>: the selection is empty, nothing changed" (Image mode) or "<Effect>: selection is outside the layer, nothing changed" (Pixel mode).
- [x] AC-10 (error): If an effect is requested with an unknown name or the wrong number of parameters, then it is rejected with an error and the document and undo history are unchanged.
- [x] AC-11 (error): If the Pixel sprite is indexed or the active layer has no image cel on the current frame, then applying an effect fails with "<Effect> failed: …" in the status bar and nothing changes.
- [x] AC-12 (error): If the active layer is locked, then applying an effect leaves the layer unchanged and adds no undo step.
- [x] AC-13 (always): Every effect returns an image the size of its input, and at neutral settings leaves pixels unchanged: Brightness / Contrast 0 / 0 (plain and enhanced), Color Balance all 0, the identity Curve, identity Levels, Hue / Saturation on a gray pixel, Box Blur 1 × 1, Gaussian Blur radius 0, Motion Blur distance 1, Unsharp Mask amount 0, Add Noise amount 0.
- [x] AC-14 (always): A solid single-color area stays unchanged under Gaussian, Box, Motion and Radial (spin and zoom) Blur, Sharpen, Unsharp Mask, Relief and Reduce Noise; Edge Detect turns it black and Emboss flat gray.
- [x] AC-15 (always): Invert Colors flips each RGB channel (`255 − v`), keeps alpha, and applied twice restores the original.
- [x] AC-16 (always): Add Noise with the same parameters and seed produces identical pixels, a different seed produces different pixels, alpha is kept, and Monochromatic noise keeps the three channels of a gray pixel equal.
- [x] AC-17 (always): Grayscale makes the three channels of every pixel equal; Threshold turns every pixel black or white by its luminance; Posterize with 2 levels snaps channels to 0 or 255 and keeps 0 and 255 unchanged; a Curve on one channel leaves the other channels untouched.
- [x] AC-18 (always): A spatial effect (blur, sharpen, distort, reduce noise) never lets the color of a fully transparent pixel tint its opaque neighbours.

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

**Dialog** (`ui/src/lib/components/EffectDialog.svelte`): non-modal panel (`role="dialog"`, `aria-modal="false"`), preview re-run coalesced through `requestAnimationFrame`; `onPreview(null)` means "preview off" (an empty vector is the real parameter vector of Invert Colors and Sharpen). Menus: `ui/src/lib/effects/menu.ts` `effectMenus(pick, disabled)`; labels translated via `lib/effects/labels.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A parameter typed beyond its slider range | Clamped by the effect's constructor; non-finite values read as 0 |
| Invert Colors on a layer with transparent pixels | They stay transparent (alpha untouched) |
| An effect applied right after a brush stroke, then another stroke | Three separate undo steps |
| Preview toggled off and on again | The preview re-runs with the current parameters |
| Grayscale sent with one parameter instead of four | Accepted; the channel-mixer weights default to 0.3 / 0.59 / 0.11 |
| Curves with fewer than one control point or an odd coordinate count | Rejected as a wrong parameter count |

## Summary

> Completed: 2026-11-05

- The effect dialog shows each parameter at its default as the right control (slider and field, checkbox, dropdown, angle dial, point, seed, linked pair, as amended by the later effect specs), resets, previews live without touching the document or its history, applies on OK / Enter as one undo step, and closes on Cancel / Escape
- Effects apply to the active layer or cel inside the selection (Image mode blends feathered pixels by coverage); the dialog names its scope; an empty selection, a missing cel, an indexed sprite, a locked layer or a bad wire request change nothing and are reported
- The pixel contract of the shared effects is pinned in `pincel-effects`: neutral settings are identities, solid areas stay solid, Invert and Add Noise behave as specified, and spatial effects ignore the colour of fully transparent pixels

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | dropped |        |
| AC-02 | `ui/e2e/effects.spec.ts`: "{pixel,image}: every effect panel shows its parameters at their defaults; Reset restores them (AC-02)" (all 20 effects of the Technical notes table, per mode as the catalog lists them, with the defaults of the later effect specs) | verified | b6a1eb2 |
| AC-03 | `ui/e2e/effects.spec.ts › pixel: the preview shows the effect and leaves the document and its history alone (AC-03)`, `ui/e2e/effects.spec.ts › image: …` (same title); `pincel-image-wasm: effect_handle_tests::preview_effect_shows_the_effect_and_leaves_document_and_history_untouched`; `pincel-wasm: effects::tests::preview_effect_leaves_document_and_history_untouched` | verified | 6aed618 |
| AC-04 | `ui/e2e/effects.spec.ts › image mode: effects and adjustments come from the shared catalog`; `ui/e2e/effects.spec.ts › pixel mode: the Levels adjustment applies`; `ui/e2e/effect-panel.spec.ts › {pixel,image}: OK applies and Cancel, × and Escape leave the document alone (AC-04)` (Enter); `pincel-wasm: effects::tests::apply_effect_invert_changes_pixels_and_undo_restores`, `effects::tests::apply_effect_is_its_own_undo_entry_after_a_stroke` | verified | ba53299 |
| AC-05 | `ui/e2e/effects.spec.ts › pixel: Cancel and Escape close the dialog and change nothing (AC-05)`, `ui/e2e/effects.spec.ts › image: …` (same title) | verified | 35f631c |
| AC-06 | `ui/e2e/effects.spec.ts › image mode: an effect stays inside the selection`; `pincel-image-core: selection::tests::masked_region_with_hard_mask_crops_to_the_selection`, `selection::tests::masked_region_mixes_partial_coverage` | verified | 6aed618 |
| AC-07 | `pincel-wasm: effects::tests::apply_effect_respects_the_marquee_selection` | verified | 35f631c |
| AC-08 | `ui/e2e/effects.spec.ts › pixel: the dialog names its scope, selection or whole layer (AC-08)`, `ui/e2e/effects.spec.ts › image: …` (same title) | verified | 9af6b69 |
| AC-09 | `ui/e2e/effects.spec.ts › image: an empty selection changes nothing and says so (AC-09)`, `ui/e2e/effects.spec.ts › pixel: a selection outside the active cel changes nothing and says so (AC-09)`; `pincel-image-wasm: effect_handle_tests::apply_effect_with_an_empty_selection_changes_nothing`; `pincel-wasm: effects::tests::apply_effect_with_selection_off_canvas_is_a_noop` | verified | 9a7f4f0 |
| AC-10 | `pincel-effects: spec::tests::from_params_unknown_name_is_an_error`, `spec::tests::from_params_wrong_arity_is_an_error`; `pincel-wasm: effects::tests::apply_effect_unknown_name_or_bad_arity_errors` | verified | c43a845 |
| AC-11 | `pincel-wasm: effects::tests::apply_effect_on_an_indexed_sprite_errors_and_changes_nothing`, `effects::tests::apply_effect_without_a_cel_on_the_current_frame_errors_and_changes_nothing`; `ui/e2e/effects.spec.ts › pixel: a layer without a cel on the current frame refuses the effect (AC-11)` | verified | 5dd2afe |
| AC-12 | `pincel-wasm: effects::tests::apply_effect_on_a_locked_layer_errors_and_changes_nothing`; `ui/e2e/effects.spec.ts › pixel: a locked layer stays unchanged and gains no undo step (AC-12)`, `ui/e2e/effects.spec.ts › image: …` (same title) | verified | 22f5a52 |
| AC-13 | `pincel-effects: spec::tests::from_params_every_name_parses_with_its_documented_arity`, `adjust::brightness_contrast::tests::zero_is_identity_legacy`, `adjust::brightness_contrast::tests::zero_is_identity_enhanced`, `adjust::color_balance::tests::zero_balance_is_identity`, `adjust::curves::tests::curves_identity_is_a_no_op`, `adjust::levels::tests::levels_identity_is_a_no_op`, `adjust::hue_sat::tests::identity_on_gray_is_unchanged`, `blur::box_blur::tests::box_blur_1x1_returns_identity`, `blur::gaussian::tests::gaussian_blur_with_sigma_zero_returns_identity`, `blur::motion::tests::motion_blur_distance_one_is_identity`, `sharpen::unsharp_mask::tests::unsharp_amount_zero_is_identity`, `noise::add_noise::tests::add_noise_amount_zero_is_identity` | verified | 3678aba |
| AC-14 | `pincel-effects: blur::gaussian::tests::gaussian_blur_of_solid_color_is_unchanged`, `blur::box_blur::tests::box_blur_of_solid_color_is_unchanged`, `blur::motion::tests::motion_blur_of_solid_color_is_unchanged`, `blur::radial::tests::radial_spin_of_solid_color_is_unchanged`, `blur::radial::tests::radial_zoom_of_solid_color_is_unchanged`, `sharpen::convolution::tests::sharpen_of_solid_color_is_unchanged`, `sharpen::unsharp_mask::tests::unsharp_of_solid_color_is_unchanged`, `distort::relief::tests::relief_of_solid_color_is_unchanged`, `noise::reduce_noise::tests::reduce_noise_of_solid_is_unchanged`, `distort::edge_detect::tests::edge_detect_of_solid_is_black`, `distort::emboss::tests::emboss_of_solid_is_flat_gray` | verified | 48b5d53 |
| AC-15 | `pincel-effects: spec::tests::run_invert_flips_rgb_and_keeps_alpha`, `adjust::invert::tests::invert_of_invert_is_identity`, `adjust::invert::tests::invert_preserves_alpha` | verified | 6aed618 |
| AC-16 | `pincel-effects: noise::add_noise::tests::add_noise_is_reproducible_for_a_fixed_seed`, `noise::add_noise::tests::different_seeds_differ`, `noise::add_noise::tests::add_noise_changes_pixels_and_preserves_alpha`, `noise::add_noise::tests::monochromatic_keeps_channels_equal_on_gray` | verified | 35f631c |
| AC-17 | `pincel-effects: adjust::grayscale::tests::grayscale_output_channels_are_equal`, `adjust::threshold::tests::threshold_splits_by_luminance`, `adjust::posterize::tests::posterize_two_levels_snaps_to_black_or_white`, `adjust::posterize::tests::posterize_preserves_endpoints`, `adjust::curves::tests::red_channel_curve_leaves_others_untouched` | verified | b5e6482 |
| AC-18 | `pincel-effects: tests/transparent_neighbours.rs › spatial_effects_ignore_the_colour_of_transparent_pixels`, `spatial_effects_keep_gray_neighbours_of_transparent_red_pixels_gray` | verified | 6aed618 |

## Decisions

- 2026-11-06 — effect-panel AC-03 (owner option c) makes the Edit menu inert while an effect is open, so the AC-03 e2e can no longer read the document under the preview through Edit ▸ Copy. It now runs against the mocked Tauri bridge and compares a native-menu save taken under each preview with one taken before (byte-equal: the document is unchanged). Counter-checked: a save after applying Invert Colors with OK differs, so the comparison sees a real change.

- 2026-11-03 — The open question about Image mode's locked layer was settled as a bug, not a wording change: an effect refused there threw out of the dialog's Apply, left the dialog open and reported nothing. Apply now closes the dialog, clears the preview and shows "<Effect> failed: …" as Pixel mode does; AC-12's e2e test checks both modes and was red for Image mode before.

- 2026-11-03: Test gaps closed for behavior that already existed; each new test was shown to fail on a deliberate, reverted break: AC-01 (menu entries never disabled), AC-02 (Gaussian Radius default 3; Reset keeping the values), AC-05 (Escape applying instead of cancelling), AC-08 (scope line always "whole layer"), AC-09 (notices replaced by "<Effect> applied"; Image core ignoring the selection), AC-11 (Pixel failure status replaced; `applyEffect` swallowing errors), AC-12 (Pixel core targeting a locked layer; Image core without its lock check), AC-03 Image core (preview without the effect). AC-03's e2e and AC-18 were red on the unchanged code (the two bugs below).
- 2026-11-03: Bug (AC-03): Preview off kept showing Invert Colors and Sharpen, because "preview off" was the empty parameter vector — also their real one. The dialog now sends `null` for "preview off" (`fix(ui)`).
- 2026-11-03: Bug (AC-18): Emboss, Edge Detect and Relief read a straight-RGB luma field, and Reduce Noise took the median over every pixel of its window, so a transparent pixel's hidden colour shifted opaque neighbours. The distort height field is now premultiplied luma (luma · alpha), and the median skips fully transparent pixels (`fix(pincel-effects)`) — the smallest changes that make the result independent of invisible colour.
- 2026-11-03: AC-11's indexed case is proven in the core only: the Aseprite reader refuses indexed files and no command converts a sprite to indexed, so no UI path reaches an indexed sprite today. The "<Effect> failed: …" status is proven with the no-cel case.
- 2026-11-03: AC-09 in Pixel mode needs a cel smaller than the canvas; New and Paste create canvas-sized cels, so the e2e test opens a hand-built `.aseprite` with a 2 × 2 cel.
- 2026-11-04: AC-01 dropped (superseded by `effect-catalog-v2`: paint.net submenus, Pixel mode shows only hard-edged effects); AC-02 reworded to cover the new parameter kinds, back to `open`. The Technical notes table keeps the old group column as history; the catalog mapping lives in `effect-catalog-v2`.
- 2026-11-05: AC-02's defaults follow the later effect specs where they changed a parameter (Box Blur as a linked pair, Motion Blur's Centered, Radial Blur without its kind, Reduce Noise's Radius 6 / Strength 40); the Technical notes table stays as history. Found when PR #123's AC-02 test met the merged catalog.
- 2026-11-05 (spec-build): AC-02 needed no new test: the test adapted in `d364a53` after the 2026-11-04/05 rewording covers all 20 effects of the table with the amended defaults and the new parameter kinds, and passes in the full gate run of 2026-11-05; its red run against a deliberate break is recorded under 2026-11-03.
