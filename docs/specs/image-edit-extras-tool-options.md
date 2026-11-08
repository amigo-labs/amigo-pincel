---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §9.2, pincel.md §15 (2026-10-12 effects honour the selection), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image mode: Antialiasing and Blending tool options

## Context

paint.net's toolbar carries two switches that most of its tools share: **Antialiasing** (smooth or hard edges) and **Blending** (Normal composites the paint over the layer; Overwrite replaces the pixels, alpha included). Image mode has only part of the first and none of the second:

- Shapes / Line and Text each have their own Anti-alias checkbox (`shapeAntiAlias`, `textAntiAlias` in the Image tool store), backed by `anti_alias` on `draw_shape` / `draw_text`.
- The Brush's hard round tip paints a binary edge (`BrushShape::HardRound`: a pixel center inside the circle is painted, else not); soft round and flat tips fall off by hardness. There is no antialiasing switch.
- Ellipse and lasso selections are hard-edged; `SelectionMask::ellipse` / `polygon` say "anti-aliasing is deferred".
- Every painting tool composites with straight-alpha source-over (`tools::src_over`): a semi-transparent color can never lower a pixel's alpha, so painting "with transparency" (paint.net's Overwrite with a transparent color) is impossible.

This is the second of two specs for paint.net's Edit and options-bar extras; the first is `image-edit-extras.md` (Fill Selection). Overwrite becomes most useful once `color-picker.md` gives Image mode color alpha, but it is observable without it through the tool opacity, which already scales the paint's alpha.

Decision Log 2026-11-05 "MCP tool parity" requires the agent tools to reach both options (AC-14 to AC-16).

**Terms.** *Coverage* of a pixel is how much of it a tool paints, 0–1: the tip's or shape's edge, the brush hardness falloff, and the selection's coverage multiplied together. *Paint alpha* is the color's alpha times the tool opacity. A *fully covered* pixel has coverage 1.

## Scope

### In scope

- One Antialiasing switch in the options bar, shared by the tools that draw edges: Brush, Eraser, Recolor and Smudge (tip edge), Line and Shapes, Text, Ellipse Select, Lasso and Polygon Lasso (selection edge)
- One Blending switch (Normal / Overwrite) in the options bar, shared by Brush, Paint Bucket, Gradient (Color channel), Line and Shapes, and Text
- Overwrite semantics in `pincel-image-core` for those tools
- Anti-aliased tip edges for the Brush family and anti-aliased ellipse / polygon selection masks
- The matching MCP arguments

### Out of scope

- Pixel mode — because pixel art wants hard edges (Pixel mode has no antialiasing by design, `canvas.md`) and its "replace vs composite" choice is the ink system (`shading-ink.md`, Aseprite's Simple ink), not a paint.net switch
- Antialiasing for the Paint Bucket and Magic Wand — because their regions are decided per pixel by color tolerance and have no sub-pixel edge to smooth
- Blending modes beyond Normal and Overwrite (paint.net 5's Multiply, Additive, … as a tool option) — because layer blend modes already offer them, and the topic asks for Normal vs Overwrite only
- Blending for Eraser, Recolor and Smudge — because they do not composite a paint color (the eraser removes alpha, recolor replaces a matched color, smudge carries existing colors), and paint.net shows no Blending switch for them
- Blending for the Gradient's Transparency channel — because that channel already writes alpha directly and has nothing to composite
- Persisting the two switches across sessions — because no other Image tool option is persisted yet; it belongs with tool-option persistence as a whole

## Acceptance criteria

### Antialiasing

- [ ] AC-01 (always): While the active tool is Brush, Eraser, Recolor, Smudge, Line, a shape tool, Text, Ellipse Select, Lasso or Polygon Lasso, the options bar shows one Antialiasing checkbox, labelled in every UI language; it is the same setting for all of them and for every Image tab, so turning it off in one tool shows it off in every other, and it is on after each app start.
- [ ] AC-02 (state): While Antialiasing is off, a Brush, Eraser, Recolor or Smudge dab with the hard round tip changes each pixel either fully (as at coverage 1) or not at all, and a pixel is changed exactly when its center lies inside the tip.
- [ ] AC-03 (state): While Antialiasing is on, a single hard round Brush dab of diameter 9 or more at 100 % opacity in an opaque color on a transparent layer leaves every pixel lying fully inside the circle at alpha 255, every pixel lying fully outside at alpha 0, and at least one pixel the circle's edge crosses at an alpha strictly between 0 and 255.
- [ ] AC-04 (state): Whatever the Antialiasing setting, a soft round or flat tip paints every pixel lying fully inside its outline exactly as with the other setting; while Antialiasing is off, no pixel whose center lies outside the tip outline changes.
- [ ] AC-05 (option): Where Antialiasing is on or off, Line, the shape tools and Text draw with anti-aliased or hard edges as their own Anti-alias checkboxes did before this spec, which no longer exist separately.
- [ ] AC-06 (state): While Antialiasing is on, an Ellipse Select, Lasso or Polygon Lasso outline gives every pixel lying fully inside it coverage 255, every pixel lying fully outside coverage 0, and at least one pixel its non-axis-aligned edge crosses a coverage between 1 and 254; while it is off, every pixel has coverage 0 or 255, decided by its center. Either outline is combined with the existing selection by the selection mode's coverage rule (replace, add = maximum, subtract, intersect = minimum).
- [ ] AC-07 (always): Rectangle Select and Magic Wand show no Antialiasing checkbox and give every pixel coverage 0 or 255, and a feather radius above 0 is applied on top of the (anti-aliased or hard) outline of every selection tool that has one.

### Blending

- [ ] AC-08 (always): While the active tool is Brush, Paint Bucket, Gradient with the Color channel, Line, a shape tool or Text, the options bar shows one Blending control with the choices Normal and Overwrite, labelled in every UI language; it is the same setting for all of them and for every Image tab, is Normal after each app start, and is hidden for every other tool and for the Gradient's Transparency channel.
- [ ] AC-09 (state): While Blending is Normal, every tool listed in AC-08 paints exactly as before this spec (source-over of the paint at its coverage × paint alpha).
- [ ] AC-10 (state): While Blending is Overwrite, every fully covered pixel a tool of AC-08 paints becomes exactly the paint color with the paint alpha, whatever the pixel held before: a Brush dab at 40 % opacity over an opaque pixel leaves that pixel at alpha 102 ± 1, where Normal would leave it at 255. For the Gradient, the paint color and alpha at a pixel are the ramp's color and alpha there (foreground to background), times the tool opacity.
- [ ] AC-11 (state): While Blending is Overwrite, a pixel with coverage c between 0 and 1 in one tool action moves from its value before that action toward the paint by c: its alpha equals previous alpha + (paint alpha − previous alpha) × c within ±1, its color lies between the previous and the paint color, and a pixel with coverage 0 is unchanged. For a Brush stroke, c is the highest coverage any dab of the stroke gives the pixel.
- [ ] AC-12 (state): While Blending is Overwrite, one Brush stroke whose dabs overlap leaves each pixel as a single dab with its highest coverage would: overlapping fully covered pixels end at the paint alpha, not above it, and a soft-tip stroke does not build up where its dabs overlap.
- [ ] AC-13 (always): A tool action under either Blending choice is the same single undo step it was before this spec, refuses a locked layer or a group in the same way, and changes only pixels with selection coverage above 0.

### MCP

- [ ] AC-14 (option): Where `draw.stroke` (both `tool` values) gets `anti_alias` (default `true`) and `draw.stroke` with `tool: "brush"`, `draw.shape`, `draw.text`, `draw.gradient` (Color channel) and `draw.fill` on an Image document get `blend` (`normal` or `overwrite`, default `normal`), then the document's pixels equal the editor's for the same tool with those settings.
- [ ] AC-15 (option): Where `select.ellipse` or `select.polygon` on an Image document gets `anti_alias` (default `true`), then the selection's coverage equals the editor's Ellipse Select or Lasso with that setting.
- [ ] AC-16 (error): If `blend` is not `normal` or `overwrite`, or `anti_alias` or `blend` is passed where this spec does not give it (for example `blend` on `draw.stroke` with `tool: "eraser"`, on `draw.gradient` with the Transparency channel, or on `draw.fill` for a Pixel document; `anti_alias` on `select.rect`, on `select.wand`, or on `select.ellipse` / `select.polygon` for a Pixel document), then the call fails with `isError: true` naming the argument (and the document's mode where the mode is the reason), and the document is unchanged.
- [ ] AC-17 (always): The `pincel://guide` resource describes `anti_alias` and `blend`, which tools take them, and that Overwrite replaces pixels including alpha.

## Technical notes

- Wire format: `CommandSpec` variants gain optional fields with serde defaults (the pattern `pencil_stroke` already uses for `shape`, `hardness`, …): `anti_alias` on the stroke variants and on `select_ellipse` / `select_polygon`, `blend` on `pencil_stroke`, `fill_bucket`, `draw_gradient`, `draw_shape`, `draw_text`. The defaults keep today's output for a command that omits them (`blend: "normal"`; `anti_alias: false` for strokes and selections), so the UI sends the setting explicitly while MCP tools apply their own documented defaults. Regenerate the ts-rs mirror. The UI sends a stroke as several merged segments (`stroke_id`); the highest-coverage rule of AC-11 spans the whole stroke, so the per-stroke `StrokeScratch` in `command/spec.rs` is the natural place for the stroke's pre-stroke pixels and coverage.
- One place decides Normal vs Overwrite: today every tool calls `tools::src_over`; an Overwrite counterpart interpolates toward the paint by coverage. Interpolating with premultiplied color avoids dark fringes when the previous pixel is transparent.
- The selection blend of Decision Log 2026-10-12 (`masked_region`) already interpolates by coverage; AC-11 states the same rule for tools.
- The two existing store fields `shapeAntiAlias` / `textAntiAlias` merge into one; the options bar is `ui/src/modes/image/components/toolbar/ToolOptions.svelte`.
- A suggested build order is the three AC blocks in turn (Antialiasing, Blending, MCP), one commit series each.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Overwrite Brush at opacity 100 % with an opaque color | Same result as Normal for fully covered pixels; differs only at partially covered edges |
| Overwrite with a fully transparent paint (alpha 0, after `color-picker.md`, or the bucket on a selection) | Fully covered pixels become fully transparent: Overwrite acts as an eraser, as in paint.net |
| Overwrite Paint Bucket with tolerance 0 on a semi-transparent region | The flood region takes exactly the paint alpha, lowering alpha where it was higher |
| Overwrite Shapes in Fill + Outline mode | The outline overwrites the fill where they overlap |
| Overwrite Text with Antialiasing on | Glyph interiors take the paint exactly; glyph edges interpolate by coverage (AC-11) |
| Antialiasing off with a 1 px Brush | Only the pixel whose center the tip covers changes, at full strength |
| Antialiased ellipse selection, then Delete Selection | Edge pixels lose alpha in proportion to their coverage, as for a feathered selection |
| Antialiasing toggled between two pointer-downs of one shape drag | Not reachable: the options bar does not take input during a drag; the setting at pointer-down applies to the whole gesture |
| Antialiased ellipse in subtract mode | Edge pixels of the hole keep partial coverage: the existing coverage minus the ellipse's |
| Overwrite with a feathered selection | The coverage includes the selection's, so the feather band interpolates (AC-11) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| One shared Antialiasing and one shared Blending setting instead of one per tool | paint.net keeps them shared across its tools; parity is the goal, and one switch avoids a shape tool and the text tool disagreeing silently | Low: a user who wants text aliased and shapes smooth toggles once |
| Antialiasing starts on, Blending starts at Normal | paint.net's defaults; shapes and text already default to anti-aliased | Medium: the Brush's hard round tip and ellipse / lasso selections change their default output from hard to smooth edges |
| The wire defaults keep the old output (`anti_alias: false` for strokes and selections) | A command recorded or sent without the new field must not change its result; the UI always sends the field | Low |
| Antialiasing applies to the Eraser, Recolor and Smudge too | They share the Brush's tip rasterizer, and paint.net's eraser and recolor have the switch | Low |
| `select.ellipse` and `select.polygon` default to `anti_alias: true`, changing the edge of an ellipse an agent selected before this spec | MCP tools take the editor's defaults (as `select.wand` does in `mcp-parity-selection-transform` AC-05), and the editor's default becomes on; `tools/list` is read on every connect | Low: an agent wanting the old edge passes `anti_alias: false` |
| `draw.stroke` defaults to `anti_alias: true`, changing the edge of a hard round stroke an agent drew before this spec | The tool describes itself as "like the editor's Brush", and the editor's default becomes on; `tools/list` is read on every connect (Decision Log 2026-11-05) | Low: an agent wanting the old edge passes `anti_alias: false` |
| Blending for the Gradient applies only to the Color channel | The Transparency channel already sets alpha; there is no paint color to overwrite with | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `one Antialiasing checkbox, shared by the edge tools and all tabs, on at start` (ui/e2e/image-antialias.spec.ts) | built |  |
| AC-02 | `hard_round_dab_is_all_or_nothing_off_and_blends_its_edge_on` (crates/pincel-image-core/tests/antialias.rs); `a hard round Brush dab has a smooth edge …` (ui/e2e/image-antialias.spec.ts) | built |  |
| AC-03 | `hard_round_dab_is_all_or_nothing_off_and_blends_its_edge_on` (crates/pincel-image-core/tests/antialias.rs); `a hard round Brush dab has a smooth edge …` (ui/e2e/image-antialias.spec.ts) | built |  |
| AC-04 | `soft_and_flat_tips_ignore_the_setting` (crates/pincel-image-core/tests/antialias.rs) | built |  |
| AC-05 | `Line and shapes follow the shared Antialiasing setting` (ui/e2e/image-antialias.spec.ts); `the colour, alignment and anti-alias restyle the live text without committing` (ui/e2e/image-live-text.spec.ts) | built |  |
| AC-06 | `ellipse_and_lasso_outlines_blend_their_edge_only_when_on`, `select_ellipse_and_polygon_commands_take_the_setting_and_combine` (crates/pincel-image-core/tests/antialias.rs); `an Ellipse Select and a Lasso edge are anti-aliased only with Antialiasing` (ui/e2e/image-antialias.spec.ts) | built |  |
| AC-07 | `one Antialiasing checkbox, …` (ui/e2e/image-antialias.spec.ts); `feather_applies_on_top_of_an_anti_aliased_outline` (crates/pincel-image-core/tests/antialias.rs) | built |  |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |

## Decisions

- 2026-11-05 (clarification pass): Rectangle Select gets no Antialiasing checkbox, because its corners are whole pixels and the switch would have no effect. `select.polygon` (now defined by `mcp-parity-selection-transform`) takes `anti_alias` here. Overwrite measures coverage per stroke (highest dab coverage against the pixels before the stroke), because per-dab interpolation would let overlapping soft dabs build up, which is what Overwrite exists to avoid. Both switches are per app start and shared by all Image tabs, because the Image tool store is shared and not persisted.
- 2026-11-08 (build): The Clone Stamp gets no Antialiasing checkbox and keeps sending `anti_alias: false`, because AC-01 does not list it. The checkbox is labelled "Antialiasing" (new key `opt.antialiasing`); Pixel mode's text dialog keeps its own "Anti-alias" label, because Pixel mode is out of scope.
