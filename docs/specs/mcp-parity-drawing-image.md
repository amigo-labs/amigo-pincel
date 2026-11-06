---
status: todo
created: 2026-11-05
size: M
mode: Image
design: [mcp.md, pincel.md §5.4, pincel.md §6, pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-structure]
---

# MCP parity: Image drawing options and effect defaults

## Context

Second half of the drawing parity gap (the first is `mcp-parity-drawing`, which brings `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` to Pixel documents). On Image documents the MCP tools reach only part of what the Image editor's tools offer:

- `draw.stroke` knows the Brush and the Eraser, but not the Recolor and Smudge tools, and its Eraser always erases to transparency, while the editor also erases to the background color.
- The flat brush tip always lies at 45°; the editor's Angle option (0–180°) turns it.
- `draw.fill` always samples the target layer; the editor's Bucket can sample all layers.
- **Bug:** `effect.apply` and `adjust.apply` use one parameter table for both modes, and it holds Pixel-mode defaults. On Image documents Pixelate then defaults to the centre pixel instead of the average, and Drop Shadow to blur 0 instead of 4, unlike the Image editor's effect dialog (`modeDefaults` in `ui/src/lib/effects/catalog.ts`). `pincel://effects` reports the same Pixel defaults for both modes.

Decision Log 2026-11-05 "MCP tool parity" applies: one tool name per capability. Recolor and Smudge get their own tools `draw.recolor` and `draw.smudge`, next to `draw.clone`, which `clone-stamp` specifies (AC-19, AC-20); the other options become arguments of the existing tools. A Pixel document fails with the mode error of `mcp-headless-image` AC-05. "The editor" is defined as in `mcp-parity-structure`: the result of the core command the UI issues for the action with the same options.

## Scope

### In scope

- New tools `draw.recolor` (source color, replacement color, tolerance) and `draw.smudge` (strength) for Image documents, with the brush arguments `points`, `size`, `hardness`, `opacity`, `shape` and `angle` of `draw.stroke`
- `draw.stroke` `tool: "eraser"` with `eraser_mode` `to_transparent` / `to_background` and a `background` color
- `angle` of the flat brush tip on `draw.stroke`, `draw.recolor` and `draw.smudge` on Image documents
- `draw.fill` `sample` `current_layer` / `all_layers` on Image documents
- Per-mode parameter defaults for `effect.apply`, `adjust.apply` and `pincel://effects`
- `pincel://guide` describing the new arguments

### Out of scope

- `draw.clone` — because `clone-stamp` specifies it together with the Clone Stamp tool (AC-19, AC-20); its brush arguments should match `draw.smudge`'s (see Decisions)
- Pen pressure and tilt (`pressure`, `azimuth`, `tilt_angle`) — because they describe an input device; an agent's stroke is the full-pressure stroke a mouse makes
- Colour parameters defaulting to an editor's foreground / background colour (Outline Object, Clouds, Turbulence) — because a headless session has no foreground or background colour; the black / white defaults equal the Image editor's colours at start-up (`#000000` / `#ffffff`), and live control of the open document is `mcp-live-desktop`'s concern
- Recolor, Smudge, erase to background, nib angle and bucket sampling on Pixel documents — because the Pixel editor has none of them
- New effects or changed effect ranges — because the effect catalog specs own them; this spec only aligns the defaults with the editor of each mode

## Acceptance criteria

- [ ] AC-15 (always): `tools/list` contains `draw.recolor` and `draw.smudge`, each meeting `mcp-headless-pixel` AC-02, and `draw.stroke` accepts only `brush` and `eraser` as `tool`.
- [ ] AC-01 (event): When the client calls `draw.recolor` on an Image document with `points`, `from` (the color to replace), `color` (the replacement, default `#000000`), `tolerance` 0–255 (default 32) and the brush arguments of `draw.stroke` (`size`, `hardness`, `opacity`, `shape`, `angle`), then the target layer has the same pixels as after the editor's Recolor tool along the same points with `from` as background, `color` as foreground, that tolerance and that brush.
- [ ] AC-02 (error): If `draw.recolor` has no `from`, or `from` or `color` is malformed or a palette index (`{ "index": n }`; Image documents have no palette), then the call fails with `isError: true` naming the argument and the accepted forms `#RRGGBB` / `#RRGGBBAA`, and the document is unchanged.
- [ ] AC-03 (event): When the client calls `draw.smudge` on an Image document with `points`, `strength` 1–100 % (default 60) and the brush arguments of `draw.stroke` (without `color`), then the target layer has the same pixels as after the editor's Smudge tool along the same points with that strength and brush.
- [ ] AC-04 (option): Where an Image `draw.stroke` with `tool: "eraser"` gets `eraser_mode: "to_background"` and `background` (default `#FFFFFF`), the target layer has the same pixels as after the editor's Eraser in To Background mode with that background color; with `eraser_mode: "to_transparent"` or without `eraser_mode`, the same pixels as the eraser of `mcp-headless-image` AC-06.
- [ ] AC-05 (option): Where an Image `draw.stroke`, `draw.recolor` or `draw.smudge` gets `angle` 0–180 (degrees, default 45), the target layer has the same pixels as after the editor's stroke of that tool with the Angle option at that value; with a round tip the angle changes nothing, as in the editor.
- [ ] AC-06 (option): Where `draw.fill` on an Image document gets `sample: "all_layers"`, the target layer has the same pixels as after the editor's Bucket with Sample set to all layers from the same seed and options; with `sample: "current_layer"` or without `sample`, the same pixels as before this spec.
- [ ] AC-07 (error): If `tolerance`, `strength` or `angle` lies outside its range, `eraser_mode` or `sample` has an unknown value, or `background` is malformed, then the call fails with `isError: true` naming the argument and its allowed range or values, and the document is unchanged.
- [ ] AC-08 (error): If `draw.recolor` or `draw.smudge` is called on a Pixel document, or `draw.stroke` with `eraser_mode`, `background` or `angle`, or `draw.fill` with `sample`, then the call fails with `isError: true` — for the two new tools with the mode error of `mcp-headless-image` AC-05, otherwise naming the argument and the document's mode — and the document is unchanged. This AC owns only the arguments this spec adds; the other-mode arguments of `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` are listed in `mcp-parity-drawing` AC-18.
- [ ] AC-17 (state): While an Image document has a selection, `draw.recolor` and `draw.smudge` change no pixel outside it and blend feathered edges by coverage, as the editor's Recolor and Smudge tools do.
- [ ] AC-16 (error): If `draw.recolor` or `draw.smudge` gets an empty `points` list, or a target that is locked, inside a locked group or a group, then the call fails with `isError: true` naming `points` or the layer, as `draw.stroke` does, and the document is unchanged.
- [ ] AC-09 (event): When `effect.apply` is called on an Image document with an effect id and without some parameters, then the omitted parameters take the defaults of the Image editor's effect dialog: `pixelate` samples the average color of each cell, `drop_shadow` has blur radius 4; the target layer equals the Image editor's result for these values.
- [ ] AC-10 (event): When `effect.apply` is called on a Pixel document with omitted parameters, then the Pixel editor's defaults apply as before this spec: `pixelate` samples the centre pixel, `drop_shadow` has blur radius 0.
- [ ] AC-11 (always): For every effect and adjustment of the catalog and every parameter that is not a colour parameter, the default `effect.apply` and `adjust.apply` use for a document equals the catalog default of that effect for the document's mode (the entry's default with that mode's `modeDefaults` applied), which is the value the editor's effect dialog of that mode opens with every time.
- [ ] AC-12 (always): In `pincel://effects`, a parameter whose default differs between the modes carries `"defaults": { "pixel": …, "image": … }` and no `"default"`, and every other parameter keeps its single `"default"` as today.
- [ ] AC-13 (always): A successful `draw.recolor`, `draw.smudge` or `draw.stroke` call that changes the document is exactly one undo step and never merges with another call, as in `mcp-headless-pixel` AC-09; a call that changes no pixel (a recolor that finds no pixel within tolerance) adds no undo step.
- [ ] AC-14 (always): The `pincel://guide` resource names `draw.recolor` and `draw.smudge` with their arguments, `draw.stroke`'s `eraser_mode` and `background`, `angle`, `draw.fill` `sample`, and that effect defaults follow the document's mode.

## Technical notes

All capabilities exist in `pincel_image_core::command::spec::CommandSpec` and the editor sends them (`ui/src/modes/image/core/controller.ts`, grep for the names):

| Argument | `CommandSpec` field | Editor source |
| -------- | ------------------- | ------------- |
| `draw.recolor`: `from`, `color`, `tolerance` | `RecolorStroke { from, to, tolerance }` | `recolorStroke()`: `from` = background, `to` = foreground, `tool.recolorTolerance` (default 32) |
| `draw.smudge`: `strength` | `SmudgeStroke { strength }` (0–1) | `smudgeStroke()`: `tool.smudgeStrength / 100` (default 60, slider 1–100) |
| `eraser_mode`, `background` | `EraserStroke { mode, background }` | `eraseStroke()`: `tool.eraserMode`, background swatch |
| `angle` | `angle` on every stroke variant | `penFields()`: `tool.angle` (default 45, slider 0–180) |
| `sample` | `FillBucket { sample }` | `fillAt()`: `tool.fillSample` |

`draw.recolor` and `draw.smudge` share `draw.stroke`'s brush schema and argument parsing (`points_arg`, the `percent` / `SHAPES` helpers); they are Image-only tools in the Image `TOOLS` list. `crates/pincel-agent/src/image/draw.rs` hard-codes `angle: DEFAULT_BRUSH_ANGLE`, `mode: "to_transparent"` and `sample: "current_layer"` today.

The effect parameter table is `EFFECTS` in `crates/pincel-agent/src/pixel/effects.rs`; `pixelate` (`choice_at("color", …, 1)`) and `drop_shadow` (`blur` default 0) carry the Pixel defaults, and `resources.rs` `effects_json` reads the same table. The editor's per-mode overrides are `modeDefaults` in `ui/src/lib/effects/catalog.ts`, applied by `forMode` / `entriesFor`. AC-11 needs a check that pins the Rust defaults to the catalog for both modes, so a later `modeDefaults` entry cannot drift unnoticed (e.g. a Node-side Playwright spec over `catalog.ts`, as `ui/e2e/effect-catalog.spec.ts` does, against the defaults the Rust table exports).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `draw.recolor` with `from` equal to `color` | Succeeds; no pixel changes, `changed: false`, no undo step |
| `draw.recolor` with `tolerance: 0` over antialiased edges | Only pixels exactly equal to `from` change, as in the editor |
| `draw.smudge` with a single point | Same result as a single click with the editor's Smudge tool |
| Eraser `to_background` with `background: "#00000000"` | Same pixels as the editor's To Background eraser with a transparent background swatch |
| `angle` with `shape: "hard_round"` | Accepted; pixels equal the stroke without `angle` (AC-05) |
| `draw.fill` `sample: "all_layers"` on a document with one layer | Same pixels as `current_layer` |
| `draw.fill` `sample: "all_layers"` with hidden layers | Same result as the editor's Bucket with Sample all layers (the editor's rule for hidden layers) |
| `effect.apply` `pixelate` on an Image document with `color: "centre"` given | The given value wins over the mode default |
| `effect.apply` with only some parameters on an Image document | Only the omitted parameters take Image defaults |
| `draw.stroke` with `tool: "recolor"` | Fails naming `tool` and its values `brush` / `eraser`, pointing to `draw.recolor` (AC-15) |
| `draw.smudge` with `color` | Fails as an unknown argument (closed schema, `mcp-headless-pixel` AC-37); smudge paints no color of its own |
| `draw.smudge` with `strength: 37.5` or `draw.stroke` with `angle: 22.5` | Accepted (Decisions) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `from` is required for `draw.recolor` while `color` defaults to `#000000` | An agent has no background swatch; a recolor without a source color has no meaningful default, while every other stroke color already defaults to black | Low |
| `background` defaults to `#FFFFFF` | The Image editor's background colour at start-up | Low |
| `draw.stroke` ignores `eraser_mode` and `background` with `tool: "brush"`; `draw.recolor` and `draw.smudge` have no arguments they ignore | `draw.stroke`'s closed schema holds both variants' arguments; the same rule as `mcp-parity-drawing` (Decisions): unused options of a variant are ignored, unknown arguments fail the schema, wrong-mode options fail with the mode | Low |
| Only `pixelate` and `drop_shadow` have mode-specific defaults today | The only `modeDefaults` entries in `catalog.ts` on 2026-11-05 | Low: AC-11's check catches later ones |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `draw_recolor_equals_the_editors_recolor_tool` (`crates/pincel-agent/src/tests/image_brushes.rs`), `draw_recolor_without_tolerance_replaces_colors_within_32` (`crates/pincel-agent/src/tests/image_brush_parity.rs`) | built  |        |
| AC-02 | `draw_recolor_with_a_bad_color_fails_naming_it` (`crates/pincel-agent/src/tests/image_brushes.rs`), `validate_missing_or_unlisted_value_appends_the_description` (`crates/pincel-agent/src/schema.rs`) | built  |        |
| AC-03 | `draw_smudge_equals_the_editors_smudge_tool` (`crates/pincel-agent/src/tests/image_brushes.rs`), `draw_smudge_passes_hardness_to_a_soft_tip` (`crates/pincel-agent/src/tests/image_brush_parity.rs`) | built  |        |
| AC-04 | `eraser_to_background_equals_the_editors_eraser` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-05 | `the_nib_angle_reaches_every_image_stroke` (`crates/pincel-agent/src/tests/image_brushes.rs`), `the_eraser_turns_a_flat_tip_by_angle` (`crates/pincel-agent/src/tests/image_brush_parity.rs`) | built  |        |
| AC-06 | `draw_fill_sample_all_layers_equals_the_editors_bucket` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-07 | `out_of_range_or_unknown_brush_values_fail_naming_the_argument` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-08 | `image_only_brushes_and_options_fail_on_pixel_documents` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-09 | `image_effect_defaults_follow_the_image_editor` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-10 | `pixel_effect_defaults_stay_the_pixel_editors` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-11 | `effect_defaults_file_is_current` (`crates/pincel-agent/src/pixel/effects.rs`) + `agent effect defaults equal the editor catalog in each mode` (`ui/e2e/effect-defaults.spec.ts`) | built  |        |
| AC-12 | `effects_resource_lists_per_mode_defaults` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-13 | `image_brush_calls_are_one_undo_step_and_none_without_a_change` (`crates/pincel-agent/src/tests/image_brushes.rs`), `image_drawing_calls_that_change_nothing_add_no_undo_step` (`crates/pincel-agent/src/tests/image_draw.rs`), `recolor_smudge_and_eraser_undo_and_redo_exactly_one_call` (`crates/pincel-agent/src/tests/image_brush_parity.rs`) | built  |        |
| AC-14 | `read_resource_guide_covers_workflow_and_both_modes` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-15 | `recolor_and_smudge_are_listed_and_stroke_takes_brush_or_eraser` (`crates/pincel-agent/src/tests/image_brushes.rs`), `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`), `validate_missing_or_unlisted_value_appends_the_description` (`crates/pincel-agent/src/schema.rs`) | built  |        |
| AC-16 | `recolor_and_smudge_refuse_empty_points_locked_and_group_targets` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |
| AC-17 | `recolor_and_smudge_stay_inside_the_selection`, `recolor_and_smudge_blend_feathered_selection_edges_by_coverage` (`crates/pincel-agent/src/tests/image_brushes.rs`) | built  |        |

## Decisions

- 2026-11-05: Recolor and Smudge get their own tools `draw.recolor` and `draw.smudge`, not `tool` values of `draw.stroke`, and clone stamping is `draw.clone` (`clone-stamp` AC-19) — owner's choice, because each editor tool is one capability with its own arguments, and small, separate schemas are unambiguous for models (as with the one-tool-per-verb choice in `mcp-parity-structure`).
- 2026-11-05: `draw.recolor`, `draw.smudge` and `draw.clone` share `draw.stroke`'s brush arguments (`points`, `size`, `hardness`, `opacity`, `shape`, `angle`), and a `sample` argument takes `current_layer` / `all_layers` as `draw.fill` and `select.wand` do — because one vocabulary across the stroke tools keeps them predictable; `clone-stamp` AC-19 uses the same values.
- 2026-11-05: Colour parameters of effects keep their black / white defaults on both modes — because a headless session has no foreground / background colour, and black / white are the Image editor's colours at start-up; `mcp-live-desktop` decides whether live mode takes the open editor's colours.
- 2026-11-05: The effect-default mismatch is fixed as part of this spec, with AC-09 red before the fix — because it is a bug in specified behavior (`mcp-headless-image` AC-12: "the same pixels as after the editor applies it"), found by the parity inventory.
- 2026-11-05 (clarification): `strength` and `angle` accept fractional values within their ranges — the core takes them as floating-point numbers, and the editor's integer slider values are a subset, so tests compare against integer values.
- 2026-11-05 (clarification): Per-mode defaults appear in `pincel://effects` as a `defaults` object instead of `default` (AC-12) — a client then cannot read one mode's default as the other's, and parameters without a difference keep today's shape.
- 2026-11-05 (clarification): The effect dialog always opens at the catalog defaults (`EffectSession` calls `defaultValues`; it remembers no earlier values), so "the editor's default" in AC-11 is the catalog default for that mode.
- 2026-11-05 (spec-build): The agent's effect table carries the catalog's base defaults (the Image editor's) plus `modeDefaults.pixel` overrides, like `catalog.ts`. AC-11 is pinned through a committed file: the Rust test `effect_defaults_file_is_current` writes every effect's default wire vector per mode to `crates/pincel-agent/src/effect-defaults.json` (and fails while it is stale), and the Node-side Playwright spec `ui/e2e/effect-defaults.spec.ts` compares the catalog's `forMode` defaults with that file; colour slots are `null` because colour defaults follow the editor's swatches. The full gate runs `cargo test` before e2e, so both sides meet on the current table.
- 2026-11-05 (spec-build): `draw.recolor` and `draw.smudge` decide "no change" (AC-13) by a dry run on a copy of the document, because the core records a Recolor / Smudge step even where no pixel matched.
- 2026-11-06 (spec-build): AC-17's "blend feathered edges by coverage" is checked against the same stroke under a hard mask (every pixel the feathered selection touches fully selected): some feathered pixel differs from both that result and the original, and for Recolor every fully selected pixel equals it (Smudge carries colour from neighbouring pixels, so its fully selected pixels may differ).
- 2026-11-06 (spec-build): The added checks were green at once, because the behavior existed; each went red against a temporary break that was then reverted: AC-07's allowed ranges and values in the message (the `tolerance` schema maximum set to 254: "misses 0–255 … tolerance must be 0–254"), AC-16's layer inside a locked group and the layer named in the message (the Image core's `effectively_locked` ignoring the parent group: "draw.recolor unexpectedly succeeded"), `recolor_and_smudge_blend_feathered_selection_edges_by_coverage` (AC-17; `SelectionMask::coverage` returning 0 or 255: "draw.recolor: no feathered pixel was blended by its coverage").
- 2026-11-06 (spec-build): A missing `from` failed in the schema check with only "missing required argument `from`", and `draw.stroke` `tool: "recolor"` with only "tool must be one of: brush, eraser"; the test accepted the first through `contains("#RRGGBB") || contains("missing")`. The schema check now appends the property's description to a missing-argument and an unlisted-value error, Image-only color arguments describe only `"#RRGGBB"` / `"#RRGGBBAA"`, and `draw.stroke`'s `tool` description names `draw.recolor` / `draw.smudge`. `draw_recolor_with_a_bad_color_fails_naming_it` (AC-02, now requiring both forms in every case) and `recolor_and_smudge_are_listed_and_stroke_takes_brush_or_eraser` (AC-15) were red before the fix ("missing required argument `from`"; "draw.recolor in … tool must be one of: brush, eraser (got \"recolor\")").
- 2026-11-06 (spec-build): `draw.stroke` on Image documents ran its command without a dry run, so an eraser stroke over transparent pixels reported `changed: true` and recorded an undo step (AC-13); every Image drawing tool now decides "changed" by the dry run `draw.recolor` / `draw.smudge` use. `image_drawing_calls_that_change_nothing_add_no_undo_step` was red before the fix ("draw.stroke … eraser: left: true, right: false").
- 2026-11-06 (spec-build): Added checks, each red against a temporary break that was then reverted: `draw.recolor` `hardness` / `opacity` / `shape` (AC-01; `hardness` fixed to 100 %), `draw.smudge` `size` / `hardness` / `opacity` with a fractional `strength` (AC-03; `size` fixed to 8 and `opacity` to 100 %), the eraser's transparent background swatch (AC-04; the background's alpha forced to 255), the flat tip's default angle 45 and a fractional `angle` for all three stroke tools (AC-05; default angle 30), a given effect parameter winning over the Image default (AC-09; given parameters ignored on Image documents: "pixelate {\"color\":\"centre\"}"), `pincel://effects` keeping `default` everywhere else (AC-12; an extra `default` beside `defaults`), and the guide's full argument wording (AC-14; Recolor's tolerance default written as 30). Green at once without a separate break: `draw.recolor` `color: { "index": 2 }` (AC-02, the existing malformed-color message), `draw.smudge` with one point and with `color` (AC-03 edge cases; the closed schema), and `sample: "all_layers"` with a hidden layer (AC-06 edge case; the core's sampling rule, which the agent forwards unchanged).
- 2026-11-06 (spec-build): Acceptance round. `draw_fill_sample_all_layers_equals_the_editors_bucket` now also calls `draw.fill` without `sample` on the two-layer document where both sample modes differ; shown red against a temporary default of `all_layers` in `image/draw.rs`, reverted. `the_nib_angle_reaches_every_image_stroke` compares a round tip at 10° and 170° with the stroke without `angle` for Brush, Eraser, Recolor and Smudge (the edge case's wording). No break was staged for that comparison: a round tip ignores the angle by the core's rule, whatever the agent sends.
- 2026-11-06 (correction): The acceptance-round entry above claimed that all builders pass `angle` through the same path. They do not: `draw.stroke`'s Brush and Eraser branches, `draw.recolor` and `draw.smudge` each read `angle` on their own line in `crates/pincel-agent/src/image/draw.rs`, and the Eraser's flat tip had no test. `the_eraser_turns_a_flat_tip_by_angle` now pins it (`angle: 120` and the default 45).
- 2026-11-06 (acceptance rework): Added checks, green at once because the behavior existed; each went red against a temporary break that was then reverted: `draw_recolor_without_tolerance_replaces_colors_within_32` (AC-01; bands at distance 32, 33 and 39 from `from`; red with the default `TOLERANCE` set to 31 and to 0), `draw_smudge_passes_hardness_to_a_soft_tip` (AC-03; `soft_round` and `flat` at hardness 30 %; red with `hardness` fixed to 100 % in `SMUDGE`), `the_eraser_turns_a_flat_tip_by_angle` (AC-05; red with the Eraser's angle fixed to 45 and, separately, with its default set to 30), `recolor_smudge_and_eraser_undo_and_redo_exactly_one_call` (AC-13; two calls each of `draw.recolor`, `draw.smudge` and the `to_background` eraser inside a selection, undo / redo compared with the exact layers and selection before and after each call; red with `next_stroke` always returning 1, which merges the second call into the first: "draw.recolor undo of the second call").
