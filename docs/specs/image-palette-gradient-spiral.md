---
status: todo
created: 2026-11-05
size: M
mode: Image
design: [pincel.md §5.4, pincel.md §15 (2026-10-27 Image mode gains Gradient, Recolor and Smudge), pincel.md §15 (2026-10-29 gradient math in pincel-effects), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image mode: spiral gradient kinds

## Context

paint.net's Gradient tool offers seven kinds: linear, linear reflected, linear diamond, radial, conical, spiral clockwise and spiral counter-clockwise. Pincel's Image-mode Gradient (§5.4, `image-editing` AC-14) has five: linear, reflected, radial, conical and diamond. The owner wants Image mode at paint.net parity (2026-11-05); the two spiral kinds are the gap.

A spiral gradient winds the ramp around the drag start: the color repeats once per drag length outward and once per full turn around the start, so bands of one color form a spiral arm. "Clockwise" and "counter-clockwise" name the way an arm turns as it runs outward.

The MCP tool `draw.gradient` already paints Image-mode gradients with the editor's five kinds. The MCP tool parity decision (Decision Log 2026-11-05) requires the tool to reach every kind the editor offers, so it gains the two spiral kinds in the same change.

This spec is the second half of the topic "Image palette and gradient"; the palette half is `image-palette-gradient.md`.

## Scope

### In scope

- Two new Gradient kinds in Image mode, Spiral (clockwise) and Spiral (counter-clockwise), in both channels (Color and Transparency)
- The kinds in the Gradient tool options, with English and German labels
- `draw.gradient` accepts both kinds and paints what the editor paints

### Out of scope

- Spiral kinds for Pixel mode's Gradient — because Pixel mode's Gradient is a dithered two-color ramp with linear and radial only (`dithered-gradient`); a spiral there needs its own dithering decisions and spec
- A setting for the number of turns or the arm count — because paint.net has none; the drag length already sets the spacing of the arms
- Smoothing the seam where the ramp wraps from the end color back to the start color — because the seam is what makes the spiral visible, and paint.net draws it sharp as well (see Assumptions)
- Gradient presets or multi-stop gradients — because every kind keeps the two-color ramp from foreground to background; multi-stop is a separate feature for all kinds

## Acceptance criteria

Terms used below: the drag runs from the start point `a` to the end point `b`; `L` is its length. For a pixel, `r` is the distance from `a` to the pixel's centre and `θ` is the angle from the drag direction to the line from `a` to the pixel's centre, measured clockwise on screen (y grows downward), in [0°, 360°); for a pixel whose centre is exactly `a`, `θ` is 0°. `frac(x)` is `x − floor(x)`, always in [0, 1), also for negative `x` (`frac(−0.25)` = 0.75). `t` is the ramp position: 0 gives the start color, values towards 1 the end color, exactly as for the existing kinds.

- [ ] AC-01 (always): The Gradient tool options in Image mode offer, after the five existing kinds, Spiral (clockwise) and Spiral (counter-clockwise).
- [ ] AC-02 (event): When the user drags with the Gradient tool and the kind Spiral (clockwise), then every pixel takes `t` = `frac(r / L − θ / 360°)`; with Spiral (counter-clockwise), `t` = `frac(r / L + θ / 360°)`.
- [ ] AC-03 (event): When the user drags on a 32 × 32 transparent layer from the centre of pixel (8, 8) to the centre of pixel (24, 8) with an opaque black foreground and an opaque white background, then with Spiral (clockwise) pixel (16, 8) has the grey value 128, pixel (8, 16) 64 and pixel (8, 0) 191, and with Spiral (counter-clockwise) pixel (16, 8) has 128, pixel (8, 16) 191 and pixel (8, 0) 64 (each channel ±1).
- [ ] AC-04 (always): For a drag along a horizontal line that lies on a pixel boundary, the Spiral (counter-clockwise) result equals the Spiral (clockwise) result mirrored across that line (each channel ±1).
- [ ] AC-05 (always): Unlike Linear and Radial, a spiral kind never holds the end color beyond the drag: pixels on the ray from `a` through `b` (and beyond `b`) at distance `L` to `2L` from `a` repeat the ramp that ray shows at distance 0 to `L`; on the opposite ray behind `a` (`θ` = 180°) the ramp also repeats every `L`, shifted by half a period.
- [ ] AC-06 (option): Where the Transparency channel is chosen, a spiral kind keeps each pixel's color and sets its alpha along the same `t` as in the Color channel, as the other kinds do in that channel.
- [ ] AC-07 (state): While a selection exists, a spiral gradient changes only pixels inside it, blending feathered edges by coverage, as the other kinds do (`image-editing` AC-11).
- [ ] AC-08 (event): When a spiral gradient is drawn, then it is one undo step, and undo restores the layer exactly.
- [ ] AC-09 (error): If the drag has zero length, then a spiral kind changes no pixel and adds no undo step, as the other kinds do.
- [ ] AC-10 (always): `tools/list` shows `draw.gradient` with a `kind` argument whose accepted values include `spiral_cw` and `spiral_ccw`, and the tool's description names both.
- [ ] AC-11 (event): When the client calls `draw.gradient` on an Image document with `kind` `spiral_cw` or `spiral_ccw`, then the layer's pixels equal those the editor's Gradient tool produces for the same drag, colors, channel, opacity and selection with Spiral (clockwise) or Spiral (counter-clockwise), in one undo step.
- [ ] AC-12 (error): If `draw.gradient` is called with a `kind` that is none of the accepted values (e.g. `spiral`), then the call fails with `isError: true`, the message lists the accepted kinds, and the document is unchanged.
- [ ] AC-13 (error): If `draw.gradient` with a spiral kind is called on a Pixel document, then the call fails with `isError: true` and the usual mode error naming the document's mode (Pixel mode's Gradient has no spiral kinds), and the document is unchanged — also if another spec gives `draw.gradient` a Pixel route for its linear and radial kinds.
- [ ] AC-14 (option): Where the UI language is German, the two kinds read "Spirale (im Uhrzeigersinn)" and "Spirale (gegen den Uhrzeigersinn)".

## Technical notes

- Core: `GradientKind` and `Gradient::parameter` in `crates/pincel-image-core/src/tools/gradient.rs`, next to Conical, which already computes `θ` from `base_angle` with `rem_euclid`. The wire strings go through `parse_gradient_kind` in `crates/pincel-image-core/src/command/spec.rs`; `CommandSpec::DrawGradient.kind` is a `String`, so the generated TypeScript mirror does not change, but the hand-written `GradientKind` union in `ui/src/modes/image/core/wasm.ts` does.
- `parse_gradient_kind` maps an unknown string to Linear. The editor never sends one, and `draw.gradient` rejects it before the command is built (the schema's `enum` check in `crates/pincel-agent/src/schema.rs`), which AC-12 relies on.
- MCP: `GRADIENT` in `crates/pincel-agent/src/image/draw.rs` (schema `choice` list and description); the existing test `draw_fill_and_gradient_match_bucket_and_gradient_tools` in `crates/pincel-agent/src/tests/image_draw.rs` is the pattern for AC-11.
- UI: `ui/src/modes/image/components/toolbar/ToolOptions.svelte` (`<option>` list), keys `opt.gradientSpiralCw` / `opt.gradientSpiralCcw` in `en.ts` and `de.ts`.
- Spec text to update with the change: the Gradient row of `pincel.md` §5.4 and the `draw_gradient` kind list in `image-editing.md` Technical notes.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pixel whose centre is exactly the drag start (`r` = 0) | `r` = 0 and `θ` = 0° by definition, so `t` = 0: the start color; no error (AC-02) |
| Pixels just inside and just outside distance `L` on the drag line | A sharp seam: the end color on the inside, the start color on the outside (AC-05) |
| Drag shorter than one pixel but not zero | Valid; the arms are packed tightly and the result is noisy, as paint.net's is |
| Spiral with the selection a single feathered ellipse | Only selected pixels change, feathered edges blend (AC-07) |
| Spiral drag pointing up-left instead of right | The pattern rotates with the drag: `θ` is measured from the drag direction (AC-02) |
| `draw.gradient` with `kind` `spiral` (no direction) | Refused, lists the accepted kinds (AC-12) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| One turn of the arm per drag length `L`, a sawtooth ramp with a sharp seam | Matches the shape of paint.net's spiral gradients and the drag-defined spacing of the other kinds | paint.net's exact formula is not published; a user comparing side by side may see a different arm spacing |
| Wire names `spiral_cw` / `spiral_ccw` | The existing command vocabulary abbreviates directions the same way (`rotate_canvas` `cw90` / `ccw90`) | Agents might guess `spiral_clockwise`; AC-12's error message lists the accepted names |
| The spiral `t` math stays in `pincel-image-core` beside Conical and Diamond | Only linear / radial math is shared with Pixel mode (Decision Log 2026-10-29), and Pixel mode gets no spiral (out of scope) | Moves to `pincel-effects` if Pixel mode ever gains spirals |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 | `spiral_every_pixel_follows_the_spiral_formula`, `spiral_start_pixel_takes_the_start_color_for_any_direction` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-03 | `spiral_pins_the_spec_example_values` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-04 | `spiral_ccw_mirrors_spiral_cw_across_a_horizontal_drag_on_a_boundary` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-05 | `spiral_repeats_the_ramp_every_drag_length_on_the_drag_line` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-06 | `spiral_transparency_fades_alpha_along_t_and_keeps_the_color` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-07 | `spiral_inside_a_feathered_selection_changes_only_selected_pixels` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-08 | `spiral_is_one_undo_step_and_undo_restores_the_layer_exactly` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-09 | `spiral_zero_length_drag_changes_nothing_and_adds_no_step` (`crates/pincel-image-core/tests/gradient_spiral.rs`) | built |  |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- 2026-11-05: Split from `image-palette-gradient.md`, because palette and spiral together exceed size L and share no code.
- 2026-11-05: `draw.gradient` gains the spiral kinds in the same change as the editor, because the MCP tool parity decision (2026-11-05) requires the tool to reach every kind the editor offers.
- 2026-11-05: `frac` is defined with `floor` (AC terms), because a truncating fractional part keeps the sign and would paint the start color where the spiral should continue.
