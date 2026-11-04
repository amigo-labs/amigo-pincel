---
status: implemented
created: 2026-10-28
size: M
mode: both
design: [pincel.md §13.4, pincel.md §9.2, pincel.md §15 (2026-10-11 one effect wire format), pincel.md §15 (2026-10-12 Image-mode effects honour the selection)]
---

# Outline effect

## Context

Game sprites almost always need an outline. Aseprite has Edit ▸ FX ▸ Outline with these options, and it is a standard pixel-art workflow. Pincel's effects cover adjustments, blur, distort, noise and sharpen, but no outline. `pincel.md` §13.4 lists "Outline (configurable thickness, inside/outside/both)" among the Phase 2 filters; a 2026-10-29 Decision Log entry moves it into Phase 1.

Effects ▸ **Outline…** draws a hard outline in a chosen colour around the non-transparent pixels of the active layer, within the selection if there is one. Options: place (outside, inside or both), shape (circle = 4-neighbour, square = 8-neighbour), thickness 1–8 px, colour (foreground by default). It has a live preview like every effect and is one undo step.

Effects are stateless and shared by both cores through `pincel-effects`; both modes build their Effects menus and dialogs from one catalog and one positional wire format (Decision Log 2026-10-11). Pixel mode ships first; Image mode gets the same entry from the shared catalog.

## Scope

### In scope

- The Outline effect in `pincel-effects`, with the options place (outside, inside, both), shape, thickness and colour
- A colour parameter kind for the effect catalog and the effect dialog (native colour input plus hex text)
- Effects ▸ Outline… in Pixel mode and in Image mode, with live preview and one undo step
- en + de strings

### Out of scope

- Anti-aliased or soft outlines — because the effect is a hard pixel-art outline; a soft "stroke" layer effect for Image mode belongs to the non-destructive layer-effects backlog item.
- A hue-shifted outline colour (the `mcp.md` vision) — because it is a possible later option on top of a plain colour.
- A dedicated colour picker in the dialog — because `color-picker.md` builds it; until then the native colour input plus hex text is used.
- An alpha threshold — because pixel art has hard alpha; opaque means alpha > 0 (Decisions).

## Acceptance criteria

- [x] AC-01 (event): When the user chooses Effects ▸ Outline… in Pixel mode or in Image mode, then the effect dialog opens with Place (Outside, Inside, Both; default Outside), Shape (Circle, Square; default Square), Thickness (1–8 px; default 1) and Color (default: the current foreground colour).
- [x] AC-02 (always): A pixel counts as part of the sprite ("opaque") when its alpha is above 0; pixels beyond the cel's edge (Pixel mode) or the layer's edge (Image mode) count as transparent, and no pixel outside the cel is written.
- [x] AC-03 (option): Where Place is Outside and Shape is Square with thickness t, every transparent pixel whose Chebyshev distance to an opaque pixel is at most t becomes the outline colour, and no other pixel changes (a single opaque pixel with t = 1 gets its 8 neighbours coloured).
- [x] AC-04 (option): Where Place is Outside and Shape is Circle with thickness t, every transparent pixel whose Manhattan distance to an opaque pixel is at most t becomes the outline colour, and no other pixel changes (a single opaque pixel with t = 1 gets its 4 edge neighbours coloured).
- [x] AC-05 (option): Where Place is Inside with thickness t, every opaque pixel that has a transparent pixel within distance t (by the chosen shape's metric) becomes the outline colour, and opaque pixels farther inside and all transparent pixels stay unchanged.
- [x] AC-06 (always): An outline pixel that is fully selected (or any outline pixel when there is no selection) takes the chosen colour exactly, including its alpha.
- [x] AC-07 (state): While a selection exists, only pixels inside the selection change, and opaque pixels outside the selection still produce outline pixels inside it; in Image mode a feathered selection blends the result by coverage, as every Image-mode effect does.
- [x] AC-08 (state): While the Outline dialog is open, the canvas shows the result live for the current options; when the user cancels, the layer is unchanged.
- [x] AC-09 (event): When the user applies the Outline, then one undo restores the layer exactly as before, and redo applies the same outline again.
- [x] AC-10 (error): If the Pixel-mode document is indexed, then the Outline is refused with the same message as every other effect and the document is unchanged.
- [x] AC-11 (error): If the Color field's hex text is not `#RRGGBB` or `#RRGGBBAA`, then the dialog marks the field invalid and does not use that value.
- [x] AC-12 (always): A thickness outside 1–8 arriving through the wire format is clamped to 1–8, like every other effect parameter, and a wrong number of parameters is an error that changes nothing; a colour slot that is not an integer in 0…2^32−1 is an error that changes nothing.
- [x] AC-13 (option): Where Place is Both, the changed pixels are exactly the union of AC-03 / AC-04 and AC-05 for the same shape and thickness.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**Effects crate.** `crates/pincel-effects/src/`: `lib.rs`, `spec.rs` (`EffectSpec`, `from_params`; constructors clamp params), `effect.rs`, `image.rs` (`EffectImage`), `kernel.rs`, `error.rs` (`EffectError`: unknown effect, wrong parameter count, buffer length). Groups `adjust/`, `blur/`, `distort/`, `noise/`, `sharpen/`. The outline can live in `distort/` or a new `outline/`. For t ≤ 8, t passes of 3×3 dilation (cross for circle, full for square) are enough; a distance transform also works.

**Colour on the wire.** The wire format is a positional `f64` vector (Decision Log 2026-10-11). Decided (2026-10-29): the 2026-10-11 format is extended without a version bump — a `color` parameter kind travels as one `f64` slot holding `0xRRGGBBAA` as an integer (exact below 2^53); `EffectSpec::from_params` rejects non-integer or out-of-range values.

**UI catalog.** `ui/src/lib/effects/catalog.ts`: `ParamDef` is `number | bool | choice` (≈L14–25) — no `color` kind yet; `menu.ts` and `labels.ts` beside it. `ui/src/lib/components/EffectDialog.svelte` renders params and the live preview; `ui/src/lib/components/ColorPair.svelte` has swatch styling to reuse. The catalog is guarded by `every_effect_name_is_accepted` (Rust) and `assertCatalogCoversAllEffects` (TS).

**Pixel mode.** `applyEffectParams` / `cancelEffect` in `ui/src/modes/pixel/image-ops.ts` (≈L46), wired in `PixelEditor.svelte`; wasm `run_effect` in `crates/pincel-wasm/src/effects.rs` (≈L45) works on the active cel's buffer, refuses non-RGBA documents ("effects require an RGBA document", ≈L56) and applies `sprite.selection` / `selection_mask`. Note: the effect only rewrites the cel's own rectangle; an Outside outline stays clipped at the cel's edge and never grows the cel (AC-02).

**Image mode.** `applyEffect` / `previewEffect` in `ui/src/modes/image/core/controller.ts` (≈L353–369); wasm `apply_effect` / `preview_effect` and `layer_effect_image` in `crates/pincel-image-wasm/src/lib.rs` (≈L1439–1495); selection through `masked_region` (Decision Log 2026-10-12).

**e2e.** Proposed: Pixel mode, 5×5 sprite with one pixel in the centre, Outline with defaults, then Eyedropper read-back at one neighbour (outline colour) and one non-neighbour (transparent), reading the foreground swatch from the DOM; Eyedropper pattern in `ui/e2e/pencil.spec.ts`. Effects pattern: `ui/e2e/effects.spec.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Opaque pixel on the layer's edge, Inside | It is recoloured — the edge counts as transparent |
| Opaque pixel on the layer's edge, Outside | Outline pixels are written only inside the layer; nothing wraps around |
| Layer with no opaque pixel | Nothing changes |
| Fully opaque layer, Outside | Nothing changes — no transparent pixel to colour |
| Fully opaque layer, Inside, t = 1 | Only the border ring of the layer is recoloured |
| Thickness 3, Square, single opaque pixel | A 7×7 block around it is coloured, the centre keeps its colour |
| Selection that contains no opaque pixel but borders one | Outline pixels appear inside the selection next to the opaque pixel |
| Semi-transparent pixel (alpha 1) | Counts as opaque: Outside does not cover it, Inside may recolour it |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Defaults Shape = Square and Thickness = 1 (AC-01) | The legacy plan's e2e test says Outline "with defaults colors the 8 neighbours" of a single pixel | Low |
| Invalid hex text is rejected in the dialog (AC-11) | The colour param accepts `#RRGGBB[AA]` only; any other value has no colour to apply | Low |

## Summary

> Completed: 2026-11-03

- `pincel-effects` gains Outline: outside, inside or both, square or circle, thickness 1 – 8 by a two-pass distance transform, any colour; out-of-range parameters are errors
- The effect catalog and dialog gain a colour parameter (native colour input plus `#RRGGBB` / `#RRGGBBAA` text); Effects ▸ Outline… previews live and applies as one undo step in both modes
- The agent tools list the outline effect with a hex colour parameter

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pixel: Effects ▸ Outline… opens with Place, Shape, Thickness and Color at their defaults (AC-01)`, `image: …` (same title) (`ui/e2e/outline.spec.ts`); `pixel: Effects and Adjustments list every entry, disabled without a document (AC-01)`, `image: …` (`ui/e2e/effects.spec.ts`) | verified | ba53299 |
| AC-02 | `apply_outside_at_the_edge_writes_only_inside_the_buffer`, `apply_alpha_1_counts_as_opaque`, `apply_matches_the_distance_definition_for_every_option` (`crates/pincel-effects/src/outline.rs`); `apply_effect_outline_never_writes_outside_a_trimmed_cel` (`crates/pincel-wasm/src/effects.rs`) | verified | ba53299 |
| AC-03 | `apply_outside_square_single_pixel_colours_its_8_neighbours`, `apply_outside_square_thickness_3_colours_a_7x7_block`, `apply_matches_the_distance_definition_for_every_option` (`crates/pincel-effects/src/outline.rs`) | verified | 6aed618 |
| AC-04 | `apply_outside_circle_single_pixel_colours_its_4_edge_neighbours`, `apply_matches_the_distance_definition_for_every_option` (`crates/pincel-effects/src/outline.rs`) | verified | ba53299 |
| AC-05 | `apply_matches_the_distance_definition_for_every_option`, `apply_inside_fully_opaque_thickness_1_recolours_the_border_ring`, `apply_alpha_1_counts_as_opaque` (`crates/pincel-effects/src/outline.rs`) | verified | 35f631c |
| AC-06 | `apply_outside_square_single_pixel_colours_its_8_neighbours` (`crates/pincel-effects/src/outline.rs`); `apply_effect_outline_is_one_undo_step_and_redo_reapplies_it` (`crates/pincel-wasm/src/effects.rs`); `apply_effect_outline_blends_a_feathered_selection_by_coverage` (`crates/pincel-image-wasm/src/lib.rs`) | verified | 6aed618 |
| AC-07 | `apply_effect_outline_fills_the_selection_from_an_opaque_pixel_outside_it` (`crates/pincel-wasm/src/effects.rs`); `apply_effect_outline_blends_a_feathered_selection_by_coverage` (`crates/pincel-image-wasm/src/lib.rs`) | verified | 35f631c |
| AC-08 | `pixel: the canvas previews the outline live; Cancel leaves the layer unchanged (AC-08)`, `image: …` (same title) (`ui/e2e/outline.spec.ts`) | verified | 9af6b69 |
| AC-09 | `apply_effect_outline_is_one_undo_step_and_redo_reapplies_it` (`crates/pincel-wasm/src/effects.rs`, `crates/pincel-image-wasm/src/lib.rs`); `pixel: the outline applies as one undo step and redo brings it back (AC-09)`, `image: …` (same title) (`ui/e2e/outline.spec.ts`) | verified | 9a7f4f0 |
| AC-10 | `apply_effect_outline_on_an_indexed_sprite_errors_with_the_rgba_message` (`crates/pincel-wasm/src/effects.rs`) | verified | c43a845 |
| AC-11 | `pixel: a hex that is not #RRGGBB or #RRGGBBAA is marked invalid and not used (AC-11)`, `image: …` (same title) (`ui/e2e/outline.spec.ts`) | verified | 5dd2afe |
| AC-12 | `from_params_outline_clamps_thickness_to_1_through_8`, `from_params_outline_wrong_arity_is_an_error`, `from_params_outline_color_not_a_u32_integer_is_an_error` (`crates/pincel-effects/src/spec.rs`); `apply_effect_outline_bad_color_or_arity_errors_and_changes_nothing` (`crates/pincel-wasm/src/effects.rs`) | verified | 22f5a52 |
| AC-13 | `apply_both_is_the_union_of_outside_and_inside`, `apply_matches_the_distance_definition_for_every_option` (`crates/pincel-effects/src/outline.rs`) | verified | 3678aba |

## Decisions

- 2026-10-28 (legacy plan): Opaque means alpha > 0 — Aseprite treats any non-zero alpha as part of the sprite.
- 2026-10-28 (legacy plan): Indexed Pixel documents are handled like every other effect (refused today) — the plan says to follow the existing effects' treatment of indexed sprites, and `pincel-wasm` refuses non-RGBA documents for all effects.
- 2026-10-28 (legacy plan): Until the own colour picker exists, the Color parameter uses the native colour input plus hex text — no new dependency and no wait on `14-color-picker.md`.
- 2026-10-29: A Decision Log entry extends the 2026-10-11 effect wire format, without a version bump, with a `color` parameter kind carried as one `f64` slot holding `0xRRGGBBAA` as an integer; `EffectSpec::from_params` rejects non-integer or out-of-range values (AC-12) — a u32 fits exactly in an f64, and existing effects keep their slot layout (owner accepted the recommendation).
- 2026-10-29: Place Both (= Inside ∪ Outside, same thickness and metric) is in scope (AC-13), and a Decision Log entry moves Outline from §13.4 Phase 2 into Phase 1, removing it from §13.4 — §13.4 names "both", a feature spec shouldn't contradict the design spec, and Both is a union of two paths that already exist (owner accepted the recommendation).
- 2026-10-29: In Pixel mode the outline clips at the cel's edge and never grows a trimmed cel (AC-02) — Phase 1 never auto-grows cels (`MoveSelectionContent`), and trimmed cels only come from imported files (owner accepted the recommendation).
- 2026-10-29: No alpha threshold parameter; opaque stays alpha > 0 — pixel art has hard alpha and the parameter would only add UI; it can be added later as an extra slot (owner accepted the recommendation).
- 2026-11-03 (build): The wire order is `outline: [place, shape, thickness, color]` with choice indices in the dialog's option order (place 0 = Outside, 1 = Inside, 2 = Both; shape 0 = Circle, 1 = Square) — it follows AC-01's field order like every other catalog entry.
- 2026-11-03 (build): Distances come from a two-pass chamfer transform (exact for the city-block and chessboard metrics) instead of t dilation passes — it is O(w·h) whatever the thickness, which matters for large Image-mode layers.
- 2026-11-03 (build): A colour slot that is not an integer in `0..=u32::MAX` is a new `EffectError::InvalidParam { effect, index, reason }` — `ParamCount` and `UnknownEffect` do not describe a bad value, and AC-12 needs an error, not a clamp.
- 2026-11-03 (build): `pincel-wasm` and `pincel-image-wasm` needed no change: both already run an effect over the whole cel / layer and keep the result inside the selection, so an opaque pixel outside the selection still outlines into it (AC-07); their outline tests were red only against the effects crate without the outline (`unknown effect`).
- 2026-11-03 (build): Outline sits directly in the Effects menu after a separator below the Blur / Sharpen / Distort / Noise submenus (catalog group `effects`) — AC-01 names it Effects ▸ Outline…, not a submenu entry; `effects.spec.ts`'s AC-01 test of `effects-adjustments.md` now expects that row after the submenus.
- 2026-11-03 (build): The hex field shows `#rrggbb` for an opaque colour and `#rrggbbaa` otherwise, accepts at most 9 characters, and the native colour input changes only RGB and keeps the alpha — the native input has no alpha, and the shorter form matches the foreground hex field.
- 2026-11-03 (build): The colour default is Pixel mode's foreground with its alpha slider value and Image mode's foreground as opaque — Image mode has no foreground alpha yet (`color-picker.md` adds it).
- 2026-11-04 (build): The agent's effect catalog (`pincel-agent`, tools `effect.apply` and the `pincel://effects` resource) gains `outline` with a `color` parameter kind taking `"#RRGGBB"` / `"#RRGGBBAA"` and defaulting to opaque black — its tests require it to list every `EFFECT_NAMES` id, and an agent has no foreground colour to default to.
