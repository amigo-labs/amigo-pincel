---
status: implemented
created: 2026-11-05
size: M
mode: Image
design: [pincel.md §5.4, pincel.md §6.2, pincel.md §15 (2026-10-11 One effect wire format), pincel.md §15 (2026-11-04 effect panel), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image Layer ▸ Rotate / Zoom

## Context

paint.net's Layers ▸ Rotate / Zoom turns the active layer by any angle, pans it and zooms it, with a live preview, as one history step. Pincel's Image mode can only flip a layer and turn it by 90° / 180° (`TransformLayer`, `RotateLayer90`); the Photo ▸ Straighten effect rotates by at most ±45° around the center and cannot pan or zoom freely. The owner wants paint.net parity for Image mode.

This spec adds Layer ▸ Rotate / Zoom… with angle, pan, zoom and a resampling quality, previewed live and applied as one undo step. It is the first half of the `image-transform-handles` topic: it builds the arbitrary rotate-and-scale resampler that the on-canvas handles of `image-transform-handles` then reuse, and it needs no live-edit architecture because it runs through the existing effect preview / apply path (Decision Log 2026-10-11 "One effect wire format"), which already gives a preview that leaves the document alone, selection confinement, one undo step, a history name and the MCP `effect.apply` tool.

## Scope

### In scope

- Layer ▸ Rotate / Zoom… in Image mode, with Angle, Pan (X / Y), Zoom and Quality (Nearest / Bilinear / Bicubic)
- A live preview on the canvas while the dialog is open; Apply / Cancel
- Confinement to the selection, like every effect
- One undo step per Apply
- en + de strings for every new label and the history step

### Out of scope

- The Tiling and Preserve-background options of paint.net's dialog — because the topic asks for angle, pan and zoom; both can follow as extra parameters
- Pixel mode — because Pixel mode's arbitrary rotation is the `rotsprite` spec (Layer ▸ Rotate… with Nearest / RotSprite)
- Growing the canvas to fit the rotated layer — because Image layers are canvas-sized and paint.net's Rotate / Zoom keeps the canvas too; Image ▸ Canvas Size exists
- Applying to every layer of a group — because effects act on one raster layer and this command runs as one; a group as active layer is refused (AC-08)
- On-canvas handles — because `image-transform-handles` covers them
- A dedicated MCP tool — because the command runs as an effect, which `effect.apply` already reaches (Decision Log 2026-11-05 "MCP tool parity": one name per capability)

## Acceptance criteria

- [x] AC-01 (event): When the user chooses Layer ▸ Rotate / Zoom… with a document open, then the effect dialog opens with Angle (−180° to 180°, step 0.1°, default 0°), Pan X / Pan Y (the shift of the layer's center from the canvas center, in percent of the canvas width / height, −100 % to 100 %, step 0.1 %, default 0 %), Zoom (0.1× to 10×, default 1×) and Quality (Nearest / Bilinear / Bicubic, default Bilinear); without a document the item is disabled.
- [x] AC-02 (state): While the dialog is open, the canvas shows the active layer rotated, panned and zoomed with the current values, and the document and its undo history are unchanged; Cancel or Escape closes the dialog with no change and no undo step.
- [x] AC-03 (event): When the user clicks Apply, then the active layer shows the previewed result, the history gains exactly one step labelled `Rotate / Zoom` (kind `rotate_zoom`), and one Undo restores every pixel of the layer; its visible English / German name in the History panel comes with `history-panel` AC-04.
- [x] AC-04 (always): The layer's content turns about the canvas center by the angle, positive clockwise on screen, is scaled by the zoom about that center and then shifted by the pan (Pan X × canvas width / 100 px to the right, Pan Y × canvas height / 100 px down, sub-pixel shifts allowed); the canvas center is the point (width / 2, height / 2) between pixel edges, so on an odd size it is the center of the middle pixel; pixels that land outside the canvas are dropped and canvas pixels no source pixel reaches become transparent.
- [x] AC-05 (always): Angle 0°, pan 0 % / 0 % and zoom 1× leave every pixel unchanged, on even and odd canvas sizes alike, with every quality, and Apply with these values adds no undo step.
- [x] AC-06 (option): Where the quality is Nearest, every result pixel has a color (alpha included) that occurs in the source layer or is fully transparent, and an angle of 90° on a square canvas equals Layer ▸ Rotate 90° CW pixel for pixel.
- [x] AC-07 (state): While a selection exists, only selected pixels change, feathered edges blend the result with the original by their coverage, as every effect does, and pixels outside the selection keep their values; the source of the rotation is the whole layer, not only the selected part.
- [x] AC-08 (error): If the active layer is locked (itself or through its group) or is a group, then the item shows the locked-layer notice layer edits show (`layers.locked`) and opens no dialog, and nothing changes.
- [x] AC-09 (error): If an angle (−180° to 180°), pan (−100 % to 100 %) or zoom (0.1× to 10×) outside its range is typed, then the value becomes the nearest bound; a non-numeric entry keeps the previous value.
- [x] AC-10 (always): The MCP tool `effect.apply` with the Rotate / Zoom effect and the same parameters produces the same pixels as Apply in the editor.
- [x] AC-11 (always): Where the quality is Nearest, an angle of 180° equals Image ▸ Rotate 180° pixel for pixel on canvases of odd and even width and height, and a pan of exactly one pixel (100 / width %) at angle 0° and zoom 1× shifts every pixel by one to the right, with the vacated column transparent.

## Technical notes

> Paths are hints from 2026-11-05 and drift — grep for the names.

- Precedent: `photo::straighten` in `crates/pincel-effects/src/photo/straighten.rs` rotates via `distort::edge::{remap, sample}` (inverse mapping, bilinear on premultiplied pixels, `EdgeBehavior::Transparent`). Rotate / Zoom is the general affine case plus Nearest and Bicubic taps; the same resampler serves `image-transform-handles` (`pincel-image-core` already depends on `pincel-effects`).
- `Interpolation` (`crates/pincel-image-core/src/transform/mod.rs`) names the three qualities; the effect's Quality parameter mirrors it as a choice.
- An Image-only entry in `ui/src/lib/effects/catalog.ts` (`modes: IMAGE`) with two number parameters Pan X / Pan Y in percent (the catalog's `point` parameter is a position in [0, 1] of the image area and cannot express a shift to or beyond the edge; percent keeps the fixed ranges the catalog needs while the canvas size varies), opened from the Layer menu instead of the Effects menu; `preview_effect` / `apply_effect` in `crates/pincel-image-wasm/src/lib.rs` do preview, selection masking and the single `SetPixels` step.
- The history kind is the effect id, as `history-panel` specifies for effects.
- **Resampler contract for `image-transform-handles`**: a function from a source buffer of any size (not only the canvas size), read as transparent outside its pixels, through an inverse affine map with sub-pixel center and offset, into a target buffer of any size, with the three qualities. Rotate / Zoom uses it with source = target = the layer; the handles spec uses it with the lifted patch as source (its AC-11, AC-13 test that use).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Angle 180° vs −180° | Same result |
| Zoom 0.1× on a 1×1 canvas | The single pixel is sampled or the canvas becomes transparent; no error |
| Pan 100 % / 0 % at zoom 1× | The content moves a full canvas width right; the canvas becomes transparent inside the selection; Apply is one undo step |
| Odd canvas (5×5), angle 90°, Nearest | Equals Layer ▸ Rotate 90° CW (AC-06; the center is the middle pixel's center) |
| Fully transparent layer | Stays transparent; Apply adds no undo step, because no pixel changed (as in AC-05) |
| Selection near the edge, angle 45° | Only selected pixels change (AC-07) |
| Cancel after moving every control | Document and history unchanged (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Rotate / Zoom runs as an Image-only effect of `pincel-effects` | The effect path already gives preview, selection confinement, one step, history name and MCP; a new command channel would duplicate it | Low |
| Pan is a shift in percent of the canvas size, −100 % to 100 % | The catalog needs fixed ranges independent of the canvas; ±100 % reaches every position from fully off to fully off | Low |
| Zoom range 0.1× – 10× | Covers shrinking to a thumbnail and a 10× blow-up without huge sampling cost | Low |
| Angle step 0.1° | Matches Straighten's angle control | Low |
| Positive angles turn clockwise | Straighten and `rotsprite` use the same convention | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `Layer ▸ Rotate / Zoom… opens the dialog with its controls and defaults (AC-01)`, `without a document the item cannot be chosen (AC-01)`, `German › the item, the dialog and its controls are German (AC-01, AC-03)` — `ui/e2e/image-rotate-zoom.spec.ts`; `rotate_zoom: wire vectors equal the previous catalog's` — `ui/e2e/effect-catalog.spec.ts` | verified | ea45578d |
| AC-02 | `the preview turns the layer; Cancel and Escape leave document and history (AC-02)` — `ui/e2e/image-rotate-zoom.spec.ts` | verified | ea45578d |
| AC-03 | `Apply keeps the preview as one undo step; Nearest 90° equals Rotate 90° CW (AC-03, AC-06)`, `German › …(AC-01, AC-03)` — `ui/e2e/image-rotate-zoom.spec.ts`; `apply_rotate_zoom_is_one_undo_step_that_undo_reverts`, `rotate_zoom_step_is_of_the_rotate_zoom_kind` (the pushed step's history kind) — `crates/pincel-image-wasm/src/rotate_zoom_tests.rs`; `from_params_rotate_zoom_reads_angle_pans_zoom_and_quality` (history label) — `crates/pincel-effects/src/spec/tests.rs` | verified | f11480f2 |
| AC-04 | `rotate_zoom_quarter_turn_is_clockwise_about_the_centre`, `rotate_zoom_arbitrary_angle_turns_clockwise_about_the_centre` (30°, 45°, 7×7 and 8×8), `rotate_zoom_zoom_scales_about_the_canvas_centre`, `rotate_zoom_pan_shifts_after_rotation_and_zoom`, `rotate_zoom_pan_by_a_full_side_empties_the_canvas`, `rotate_zoom_sub_pixel_pan_interpolates_and_reads_transparent_outside`, `rotate_zoom_half_turn_either_way_is_the_same` — `crates/pincel-effects/src/transform/tests.rs` | verified | d9c949d0 |
| AC-05 | `rotate_zoom_identity_leaves_every_pixel_unchanged`, `resample_identity_map_copies_every_pixel_exactly` — `crates/pincel-effects/src/transform/tests.rs`; `apply_rotate_zoom_identity_adds_no_undo_step`, `apply_rotate_zoom_on_a_transparent_layer_adds_no_undo_step` — `crates/pincel-image-wasm/src/rotate_zoom_tests.rs`; `the identity changes no pixel and adds no undo step (AC-05)` — `ui/e2e/image-rotate-zoom.spec.ts` | verified | d9c949d0 |
| AC-06 | `rotate_zoom_nearest_keeps_source_colours` — `crates/pincel-effects/src/transform/tests.rs`; `rotate_zoom_nearest_quarter_turn_equals_rotate_layer_90_cw` — `crates/pincel-image-core/tests/rotate_zoom.rs`; `Apply keeps the preview … (AC-03, AC-06)` — `ui/e2e/image-rotate-zoom.spec.ts` | verified | d9c949d0 |
| AC-07 | `rotate_zoom_in_a_feathered_selection_blends_by_coverage_and_reads_the_whole_layer` — `crates/pincel-image-core/tests/rotate_zoom.rs` | verified | 48155c01 |
| AC-08 | `a locked layer or a group refuses with the locked-layer notice (AC-08)` — `ui/e2e/image-rotate-zoom.spec.ts` | verified | ea45578d |
| AC-09 | `typed values outside a range become the nearest bound; text keeps the value (AC-09)` — `ui/e2e/image-rotate-zoom.spec.ts`; `rotate_zoom_new_clamps_and_reads_non_finite_as_zero` — `crates/pincel-effects/src/transform/tests.rs` | verified | ea45578d |
| AC-10 | `effect_apply_rotate_zoom_matches_the_editor` — `crates/pincel-agent/src/tests/rotate_zoom.rs`; `rotate_zoom: wire vectors equal the previous catalog's` (UI wire order) — `ui/e2e/effect-catalog.spec.ts` | verified | 2230ca27 |
| AC-11 | `rotate_zoom_nearest_half_turn_equals_rotate_canvas_180`, `rotate_zoom_nearest_one_pixel_pan_shifts_every_pixel_right` — `crates/pincel-image-core/tests/rotate_zoom.rs`; `typed values … (AC-09)` (180° equals Image ▸ Rotate 180°) — `ui/e2e/image-rotate-zoom.spec.ts` | verified | 48155c01 |

## Summary

> Completed: 2026-11-06

- `pincel-effects::transform`: an affine resampler (`Affine`, `Quality`, `resample`) for any source and target size, transparent outside the source, with Nearest, Bilinear and Bicubic (Catmull-Rom) taps, ready for `image-transform-handles`
- `rotate_zoom` effect (`EffectSpec::RotateZoom`, wire `[angle, pan_x, pan_y, zoom, quality]`): turn clockwise about the canvas centre, zoom about it, then pan in percent of the canvas; exact at multiples of 90°
- Image mode Layer ▸ Rotate / Zoom… through the effect panel: live preview, selection confinement, one undo step labelled "Rotate / Zoom", no step when no pixel changes, the `layers.locked` refusal for locked layers and groups, en + de strings
- The shared effect panel clamps typed numbers to their range on commit and restores the value on a non-numeric entry
- MCP `effect.apply` with `rotate_zoom` gives the editor's pixels

## Decisions

- 2026-11-05: Split from `image-transform-handles` — because together they exceed size L; this half needs no live-edit decision and builds the shared resampler, so it goes first.
- 2026-11-05: Clarification pass — Pan is a shift from the canvas center in percent of the canvas size (AC-01, AC-04, AC-09), because the catalog's point parameter is a position in [0, 1] and cannot reach beyond the edge, and fixed percent ranges fit a catalog that does not know the canvas size. The canvas center is (width / 2, height / 2), so odd sizes turn about the middle pixel's center (AC-04, AC-11). The resampler's contract for `image-transform-handles` (any source size, transparent outside, sub-pixel center) is in Technical notes and is proved by that spec's AC-11 / AC-13.
- 2026-11-06 (spec-build): The resampler is new public API of `pincel-effects`: `transform::{Affine, Quality, resample, RotateZoom}` (pixel-edge coordinates, an inverse map, taps outside the source transparent), and `EffectSpec::RotateZoom` with the wire id `rotate_zoom` `[angle, pan_x, pan_y, zoom, quality]` — because the effect path needs a wire id and `image-transform-handles` needs the resampler from a crate `pincel-image-core` already uses.
- 2026-11-06 (spec-build): Nearest copies source bytes; Bilinear and Bicubic (Catmull-Rom) interpolate premultiplied pixels, and a sample exactly on a source pixel centre copies that pixel's bytes — because premultiplying loses the colour of transparent pixels, which AC-05's "every pixel unchanged" forbids; angles at multiples of 90° use exact sines so AC-06 / AC-11 hold bit for bit.
- 2026-11-06 (spec-build): No undo step for an unchanged layer is limited to Rotate / Zoom (`commit_region` in `pincel-image-wasm`) — because effect-panel AC-04 and adjustments-paintnet AC-10 pin one step per Apply for the other effects even where the pixels stay (a generic rule turned four e2e specs red); the UI shows the empty-selection notice only for an empty region.
- 2026-11-06 (spec-build): The catalog entry has the group `layer`, which no effect menu iterates; the Layer menu item opens it through the same refusal as other pixel edits (`layerEdit`), so a locked layer, a layer in a locked group and a group show the `layers.locked` notice ("Layer “…” is locked") and the dialog does not open (AC-08) — because the item is a layer command and that is the notice pixel edits show before anything runs.
- 2026-11-06 (spec-build): AC-09's clamping lives in the shared effect panel's number field (every effect's typed numbers now clamp on commit, a non-numeric entry restores the value) — because the panel is the one place values are typed, and an unclamped field would show a value the core then clamps silently.
- 2026-11-06 (spec-build): "Apply" is the effect panel's OK button; the history name "Rotate / Zoom" is the step's core label (`EffectSpec::label`) and the effect id `rotate_zoom`, whose English and German names (`effect.rotate_zoom`: "Rotate / Zoom" / "Drehen / Zoomen") the effect catalog carries — `history-panel` (AC-04) names effect steps by that id; this branch has no history panel to show the row yet.
- 2026-11-06 (spec-build): Pan X / Pan Y are plain number controls labelled as in AC-01 (German "Verschieben X / Y"), Zoom steps by 0.01 — the AC fixes neither.
- 2026-11-06 (spec-build) red runs: transform tests red against a stub resampler (8 of 14); the identity tests red with the exact-tap path and the identity shortcut removed; Nearest-colour and clamp tests red against Nearest-as-bilinear and an unclamped zoom; `pincel-image-core` rotate_zoom tests red against a counter-clockwise angle, an unscaled pan, a non-lerping feather and a selection-only source; wasm no-step tests red before the rule, the one-step test red against a doubled `SetPixels`; agent AC-10 test red against swapped pan / quality slots; e2e red against an unclamped field (AC-09), no `layerEdit` (AC-08), the old notice rule (AC-05), a Cancel that keeps the preview (AC-02), quality default Nearest and zoom max 20 (AC-01), an English German name, a doubled step (AC-03) and a negated angle (AC-02, AC-06). Green at once and not breakable here: "without a document the item cannot be chosen" (the shell keeps the editor and its menu bar hidden until the document is loaded; the item also carries `disabled: !hasDoc`) and `rotate_zoom_tiny_canvas_and_extreme_zoom_do_not_fail` (an edge-case guard).
- 2026-11-06 (spec-check rework): AC-03 revised — the step is labelled `Rotate / Zoom` with the kind `rotate_zoom`, and its visible English / German name in the History panel comes with `history-panel` AC-04 — because this branch has no History panel and nothing outside `pincel-image-core` can read a step's label without new public API; the kind is the effect id the step comes from (`EffectSpec` `rotate_zoom`), and the core command carries no kind field yet, so `history-panel` adds it (or maps the label) when it names steps. Decided by the lead session under the owner's ask-at-the-end instruction; listed for review.
- 2026-11-06 (spec-check rework): AC-08 revised to the built behaviour — the item shows the locked-layer notice layer edits show (`layers.locked`, "Layer “…” is locked") and opens no dialog, where effects open the dialog and fail with "<effect> failed: layer is locked" — because the item is a layer command (Decision of 2026-11-06 above). Decided by the lead session under the owner's ask-at-the-end instruction; listed for review.
- 2026-11-06 (spec-check rework): `rotate_zoom_arbitrary_angle_turns_clockwise_about_the_centre` checks AC-04 off the multiples of 90°, which run through the general `sin_cos` branch: on a coordinate-coded source (every pixel names itself) it checks 4 target pixels per case at 30° and 45° on 8×8 and 7×7 against source pixels worked by hand, each at least 0.17 px from a pixel edge, plus a transparent corner. Green at once; red, each reverted, against a negated angle in the general branch, the angle read as radians, a centre at ((w − 1) / 2, (h − 1) / 2) (also red in five other tests) and that centre only off the multiples of 90° (red in this test alone). `rotate_zoom_step_is_labelled_rotate_zoom` (AC-03) builds the step through `effect_step`, the helper `commit_region` pushes, and reads its label; green at once, red ("Edit Pixels") against `effect_step` without `with_label`. Since history-panel replaced labels with history kinds the test is `rotate_zoom_step_is_of_the_rotate_zoom_kind` and reads the kind `effect.rotate_zoom`.
- 2026-11-06 (spec-check rework): Pixel `effect.apply` still runs `rotate_zoom` on the target cel (about the cel's centre, not the canvas centre). Refusing it there with the mode error contradicts `effect-catalog-v2` AC-08 (implemented: the Pixel agent tools accept every effect name, pinned by `effect_apply_accepts_every_effect_on_a_pixel_document`), so it is left for the owner: either `effect-catalog-v2` AC-08 gains an exception for Image-only layer commands and Pixel `effect.apply` refuses `rotate_zoom`, or Pixel keeps it. Listed for review.
