---
status: todo
created: 2026-11-05
size: L
mode: both
design: [mcp.md, pincel.md §5.2, pincel.md §5.4, pincel.md §6, pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-structure]
---

# MCP parity: drawing tools

## Context

`mcp-headless-pixel` gave Pixel documents only 1-px drawing: `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse`, `draw.fill` and `draw.clear`. The Pixel editor draws much more: Pencil and Eraser strokes with a brush of size 1–8, square or round, with pixel-perfect; the Shading ink; symmetry and tiled-mode wrapping; styled shapes with stroke width and outline / fill modes; a dithered Gradient; and the Text tool. `mcp-headless-image` added `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient`, but only for Image documents.

Decision Log 2026-11-05 "MCP tool parity" gives one tool name per capability: a tool works on every mode whose editor offers the capability and fails with the mode error of `mcp-headless-image` AC-05 elsewhere. This spec makes `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` work on Pixel documents with what the Pixel editor offers. It also settles which drawing tools honour the selection. The Image-only tools and options (`draw.recolor`, `draw.smudge`, erase to background, nib angle, bucket sampling) and the Image effect defaults are in the sibling spec `mcp-parity-drawing-image`; `draw.clone` is specified with the Clone Stamp itself (`clone-stamp` AC-19, AC-20).

**"The editor"** means what `mcp-parity-structure` defines: the result of the core command the UI issues for the same action, with the same options, in a document of that mode. A test compares the tool's result with that command applied directly to an equal document.

**Conventions** inherited from `mcp-headless-pixel` and `mcp-headless-image`: coordinates are document pixels with the origin top-left; colors follow `mcp-headless-pixel` AC-21 (`#RRGGBB`, `#RRGGBBAA`, and `{ "index": n }` on Pixel documents); `layer` / `frame` default to the paint target (`mcp-headless-pixel` AC-43); a locked target fails as in `mcp-headless-pixel` AC-28; a group target fails as in `mcp-parity-structure` AC-13.

## Scope

### In scope

- `draw.stroke` on Pixel documents: brush and eraser, size 1–8, `square` / `round`, pixel-perfect, the Shading ink with a palette ramp and step ±1, symmetry (`mirror` with axes in half pixels) and tiled wrapping (`wrap`), all given as arguments of the call
- `draw.shape` on Pixel documents: rectangle, rounded rectangle, ellipse and polygon with stroke width, outline / fill / fill + outline and separate stroke and fill colors
- `draw.gradient` on Pixel documents: linear or radial, dither none / Bayer 2, 4, 8, clipped to the selection
- `draw.text` on Pixel documents: size, bold, italic, anti-alias and align, clipped to the selection
- Per-mode arguments and defaults of these four tools, and the error for an argument the document's mode does not offer
- Which drawing tools honour the selection in each mode, matching each editor (Pixel `draw.fill` keeps clipping to it)
- The mode error of `draw.line`, `draw.rect` and `draw.ellipse` on Image documents pointing to `draw.shape`
- `pincel://guide` describing the per-mode arguments, defaults and selection behavior

### Out of scope

- `draw.recolor`, `draw.smudge`, erase to background, the flat-nib angle, `draw.fill` `sample`, and the Image-mode effect defaults — because `mcp-parity-drawing-image` covers them
- `draw.clone` — because `clone-stamp` specifies it with the Clone Stamp tool (AC-19, AC-20)
- Pen pressure and tilt — because they describe input devices; an agent has no pen, and a call at full pressure is what a mouse produces
- Symmetry and wrapping for lines, shapes and the bucket — because the Pixel editor applies them only to Pencil and Eraser strokes (`symmetry-tiled-mode` AC-11, AC-22)
- Symmetry and tiled mode as persistent document or session state — because the editor keeps them as session-only tool and view options (`symmetry-tiled-mode`, Assumptions); per-call arguments keep each call self-contained
- Polygon rotation, dashed outlines and anti-aliased shapes in Pixel documents — because the Pixel editor's shape tools have none of them
- The selection tools themselves and the removal of `selection.set_rect` / `selection.clear` — because `mcp-parity-selection-transform` replaces them with `select.*`
- Clipping Pencil, Eraser, Line, shape and Bucket edits to the selection in the Pixel editor — because that changes editor behavior (`pixel-selection-move`, Out of scope); it is a backlog item of its own (owner, 2026-11-05)
- A font choice for `draw.text` — because font choice comes with `system-fonts-mcp`, which extends this tool; this spec uses the bundled Liberation Sans, as both editors do today

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains exactly one `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient`, each meeting `mcp-headless-pixel` AC-02, each with a description naming the arguments and defaults of both modes; this spec adds no other tool name.
- [ ] AC-02 (event): When the client calls `draw.stroke` on a Pixel document with `points` (`[[x, y], …]`, integers), `color` (default `#000000`), `size` 1–8 (default 1) and `shape` `square` or `round` (default `square`), then the target cel has the same pixels as after the editor's Pencil pressed at the first point, dragged through the others in order, and released, with that color and brush; one point gives one stamp.
- [ ] AC-03 (option): Where `draw.stroke` on a Pixel document gets `pixel_perfect: true`, the target cel equals the editor's Pencil stroke with Pixel-perfect on; at a size above 1 the result equals the stroke without it, as in the editor.
- [ ] AC-04 (event): When `draw.stroke` on a Pixel document gets `tool: "eraser"`, then the target cel equals the editor's Eraser stroke along the same points with the same size and shape: every touched pixel becomes transparent.
- [ ] AC-05 (option): Where `draw.stroke` on a Pixel document gets `ink: "shading"` with optional `ramp_from` and `ramp_to` (inclusive palette indexes, default the whole palette) and `step` `1` or `-1` (default `1`), the target cel equals the editor's Pencil stroke with the Shading ink, that palette range as the ramp, and the left (`1`) or right (`-1`) button.
- [ ] AC-06 (error): If a shading stroke gets a ramp index outside the palette, `ramp_from` greater than `ramp_to`, only one of the two, a `step` other than `1` or `-1`, `tool: "eraser"`, or targets a document with an empty palette, then the call fails with `isError: true` naming the argument (or saying that the palette is empty), and the document is unchanged.
- [ ] AC-07 (option): Where `draw.stroke` on a Pixel document gets `mirror` `x`, `y` or `xy` with optional `axis_x2` / `axis_y2` (axis position in half pixels from the left / top canvas edge, default the canvas width / height, i.e. the centre), the target cel equals the editor's stroke with symmetry left–right, top–bottom or both at those axes; mirror images outside the canvas are skipped unless `wrap` covers that axis (AC-08).
- [ ] AC-08 (option): Where `draw.stroke` on a Pixel document gets `wrap` `x`, `y` or `xy`, the target cel equals the editor's stroke in tiled mode X, Y or Both: every pixel the stroke paints beyond the canvas along a wrapped axis, mirror images included, appears at the opposite edge instead of being skipped — on a 64-px-wide canvas a stroke from (60, 0) to (70, 0) with `wrap: "x"` paints (60, 0)–(63, 0) and (0, 0)–(6, 0). Together with `mirror`, the result equals the editor's stroke with both aids on.
- [ ] AC-09 (error): If `axis_x2` or `axis_y2` lies outside 0 to twice the canvas width or height, or is given while `mirror` does not cover that axis, or a coordinate of a Pixel `draw.stroke` point, of Pixel `draw.shape` / `draw.gradient` corners (`x0`, `y0`, `x1`, `y1`) or of the Pixel `draw.text` anchor (`x`, `y`) is fractional, then the call fails with `isError: true` naming the argument (for a point, its index in `points`), and the document is unchanged. On Image documents the same arguments accept fractional values, as today.
- [ ] AC-10 (event): When the client calls `draw.shape` on a Pixel document with `shape` `rect`, `rounded`, `ellipse` or `polygon`, the inclusive corners (x0, y0) and (x1, y1) as integers, `stroke` and / or `fill` colors, `stroke_width` 1–64 (default 1), `corner_radius` 0–256 (default 4) for `rounded` and `sides` 3–64 (default 6) for `polygon`, then the target cel equals the editor's matching shape tool dragged over that box: outline with only `stroke`, fill with only `fill`, fill + outline with both, the polygon inscribed in the box with its first vertex up.
- [ ] AC-11 (error): If `draw.shape` on a Pixel document gets `shape: "line"`, or polygon geometry by `cx` / `cy` / `radius`, then the call fails with `isError: true` naming the argument and pointing to `draw.line` or to the corners (x0, y0)–(x1, y1), and the document is unchanged.
- [ ] AC-12 (event): When the client calls `draw.gradient` on a Pixel document with integer (x0, y0), (x1, y1), `from` and `to` colors, `kind` `linear` or `radial` (default `linear`) and `dither` `0`, `2`, `4` or `8` (default `4`), then the target cel equals the editor's Gradient tool dragged from (x0, y0) to (x1, y1) with `from` as foreground, `to` as background, that kind and that Bayer matrix (`0`: two hard bands).
- [ ] AC-13 (error): If `draw.gradient` targets an indexed or grayscale Pixel document, then the call fails with `isError: true` saying the gradient paints RGBA documents only, and the document is unchanged.
- [ ] AC-14 (event): When the client calls `draw.text` on a Pixel document with `text`, integer `x` and `y`, `size` 1–2000 (default 12), `color` (default `#000000`), `bold`, `italic`, `anti_alias` (default `false`) and `align` (default `left`), then the target cel equals the editor's Text tool applied at (x, y) with these options and the bundled font.
- [ ] AC-15 (event): When a Pixel `draw.text` places no glyph pixel on the canvas, or a Pixel `draw.gradient` has (x0, y0) equal to (x1, y1), then the call succeeds with `changed: false`, the document is unchanged and no undo step is added — as the editor's Text tool (which drops a text that lands nowhere) and Gradient tool (`dithered-gradient` AC-12) do.
- [ ] AC-16 (state): While a Pixel document has a selection, `draw.gradient`, `draw.text` and `draw.fill` (as in `mcp-headless-pixel` AC-25) change no pixel outside it, while `draw.stroke`, `draw.shape`, `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse` and `draw.clear` change exactly the pixels they change without a selection, as the editor's tools do.
- [ ] AC-17 (state): While an Image document has a selection, `draw.stroke`, `draw.shape`, `draw.text`, `draw.fill` and `draw.gradient` change no pixel outside it and blend feathered edges by coverage, as the editor's tools do (current behavior, pinned here; see Decisions).
- [ ] AC-18 (error): If a call passes an argument, or an argument value, that only the other mode's editor offers, then the call fails with `isError: true` naming the argument and the document's mode, and the document is unchanged. On a Pixel document these are: `draw.stroke` `opacity`, `hardness`, `shape` `hard_round` / `soft_round` / `flat`; `draw.shape` `cx`, `cy`, `radius`, `rotation`, `dash`, `anti_alias`, `shape` `line` (AC-11); `draw.gradient` `opacity`, `channel`, `kind` `reflected` / `conical` / `diamond`. On an Image document: `draw.stroke` `pixel_perfect`, `ink`, `ramp_from`, `ramp_to`, `step`, `mirror`, `axis_x2`, `axis_y2`, `wrap`, `shape` `square` / `round`; `draw.gradient` `dither`. This list is complete for the four tools; `mcp-parity-drawing-image` AC-08 lists the arguments that spec adds.
- [ ] AC-24 (error): If a Pixel document gets a value outside the Pixel editor's range — `size` outside 1–8, `stroke_width` outside 1–64, `corner_radius` outside 0–256, `sides` outside 3–64, text `size` outside 1–2000, `dither` other than 0, 2, 4 or 8 — or an Image document a value outside the Image editor's range (`stroke_width` 1–500, `sides` 3–100, text `size` 1–999), then the call fails with `isError: true` and the message "`{argument}` must be {range} on {mode} documents", and the document is unchanged; an unknown `kind`, `align`, `ink`, `mirror`, `wrap` or `tool` value fails naming the rejected value, as in `mcp-headless-pixel` AC-27.
- [ ] AC-19 (always): On Image documents, `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` called with the arguments of `mcp-headless-image` produce the same pixels, defaults included (brush 8 px `hard_round`, text 48 px with anti-aliasing), as before this spec.
- [ ] AC-20 (error): If a tool of this spec targets a locked layer or a layer inside a locked group ("is locked", `mcp-headless-pixel` AC-28), a group ("is a group", `mcp-parity-structure` AC-13), or a tilemap layer ("is not an image layer", as Pixel `draw.pixels` says today; the editor's paint target is never a tilemap layer), then the call fails with `isError: true` naming the layer and that reason, and the document is unchanged.
- [ ] AC-21 (always): Every successful call of a tool of this spec that changes the document is exactly one undo step, mirrored and wrapped pixels included, as in `mcp-headless-pixel` AC-09; a successful call that changes nothing adds no undo step; two `draw.stroke` calls are always two undo steps.
- [ ] AC-22 (always): The `pincel://guide` resource lists, for `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient`, the arguments and defaults of each mode, names for each mode which drawing tools honour the selection, and says that Pixel `draw.fill` clips to the selection although the Pixel editor's Bucket does not.
- [ ] AC-23 (error): If `draw.line`, `draw.rect` or `draw.ellipse` is called on an Image document, then the call fails with `isError: true` with the mode error of `mcp-headless-image` AC-05, the message points to `draw.shape` (`shape` `line`, `rect` or `ellipse`), and the document is unchanged; on Pixel documents the three tools behave as in `mcp-headless-pixel` AC-22.

## Technical notes

Pixel capabilities, as the Pixel editor calls them (grep for the names, lines drift):

| Tool on a Pixel document | Editor call (`crates/pincel-wasm`) | Core |
| ------------------------ | ---------------------------------- | ---- |
| `draw.stroke` brush / eraser | `beginStroke(tool, color, size, shape, pixel_perfect)` + `strokeTo` per point (`stroke.rs`) | `PencilStroke` (`with_ink`, `with_mirror`, `with_wrap`), `continue_stroke` |
| `draw.stroke` `ink: "shading"` | `beginShadingStroke(ramp, step, size, shape, pixel_perfect)` | `Ink::Shading { ramp, step }` |
| `mirror`, `axis_*` | `setStrokeMirror(axis_x2, axis_y2)` (`ui/src/modes/pixel/symmetry.ts` `armStroke`) | `Mirror { axis_x2, axis_y2 }` |
| `wrap` | `setStrokeWrap(x, y)` | `Wrap { x, y }` |
| `draw.shape` | `applyShape(kind, x0, y0, x1, y1, mode, stroke_width, stroke, fill, param)` (`shapes.rs`) | `DrawShape`, `ShapeKind`, `ShapeMode`, `ShapeStyle` |
| `draw.gradient` | `applyGradient(line, kind, dither, fg, bg)` (`gradient.rs`) | `DrawDitherGradient` (clips to the selection) |
| `draw.text` | `drawText(font_id, text, x, y, size, color, bold, italic, anti_alias, align)` (`text.rs`) | `DrawText` (clips to the selection) |

The Pixel editor's option ranges and defaults: `toolInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (brush 1, `square`, stroke width 1, corner radius 4, polygon sides 6, gradient linear / Bayer 4), the shape option inputs in `PixelEditor.svelte` (stroke width 1–64, radius 0–256, sides 3–64), `TextDialog.svelte` (size 12, 1–2000, anti-alias off). The shading ramp is the Shift-selected palette range or the whole palette (`shading-ink` AC-07, AC-08).

Pixel tools in `pincel-agent` run the editor's core command on a scratch copy of the cel and commit the before / after diff as one `PatchCel` (`mcp-headless-pixel`, Decisions 2026-10-29 build). A stroke drives one `PencilStroke` through all points, so pixel-perfect and the Shading ink's "each pixel changes once" (`shading-ink` AC-05) hold across the whole call.

`draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` sit in the Image-only `TOOLS` list today (`crates/pincel-agent/src/image/mod.rs`) and call `ctx.ws.image_mut`; they become shared tools that branch on `ctx.ws.is_image`, like `draw.fill` and `effect.apply`. `draw.text` on Pixel documents uses the font `pincel-agent` already embeds.

Per-mode bounds: the input schema allows the union of both modes' ranges (text `size` 1–2000, `sides` 3–100, `stroke_width` 1–500, coordinates as numbers), and the tool checks the range of the document's mode and integer coordinates itself, with the AC-24 / AC-09 message. Arguments of a variant the call does not use are in the same closed schema and are ignored (Decisions).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pixel `draw.stroke` with one point | One stamp of the brush at that point (AC-02) |
| Pixel `draw.stroke` with an empty `points` list | `isError: true` naming `points`, as on Image documents |
| Pixel `draw.stroke` entirely outside the canvas without `wrap` | Succeeds, `changed: false`, no undo step (AC-21) |
| Pixel `draw.stroke` with `size: 9` | Fails: `size` must be 1–8 on Pixel documents; document unchanged |
| Pixel `draw.stroke` with `pixel_perfect: true` at size 4 | Same pixels as without pixel-perfect (AC-03) |
| Shading stroke over pixels whose colors are not in the ramp | Those pixels keep their value; `changed: false` if none matched |
| Shading stroke passing twice over the same pixel | The pixel steps once (`shading-ink` AC-05) |
| `mirror: "x"` on an odd width without `axis_x2` | Axis through the centre column, which mirrors onto itself |
| `mirror` and `wrap` both set, a mirror image beyond the canvas | The mirror image wraps like any other stamped pixel (AC-08) |
| `axis_x2: 0` | Allowed: the axis is the left edge, so every mirror image falls outside the canvas and is skipped (unless `wrap` covers x) |
| Pixel `draw.shape` with x0 > x1 | Same result as the corners swapped, like a drag in the other direction |
| Pixel `draw.shape` with neither `stroke` nor `fill` | Fails as on Image documents, asking for a color |
| Pixel `draw.text` with an empty string | Succeeds with `changed: false`, no undo step (AC-15) |
| Pixel `draw.gradient` with a selection the drag line does not touch | The selection is still filled by the ramp (`dithered-gradient` AC-07); outside it nothing changes |
| `draw.stroke` with `color: { "index": 3 }` on an Image document | Fails naming `color` and the accepted forms, as today |
| Pixel `draw.stroke` with `ramp_from` but `ink` left at `simple` | `ramp_from` is ignored (Decisions) |
| Pixel `draw.shape` with a `#RRGGBB` color on an indexed document | Same as `draw.pixels` with that color (`mcp-headless-pixel` AC-19, AC-21) |
| Pixel `draw.gradient` with `dither: 3` | Fails as in AC-24 |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The Pixel `draw.stroke` points are a press at the first point and a drag through the rest; the core joins consecutive points with straight lines | That is what `strokeTo` does with pointer samples | Low |
| `axis_x2` / `axis_y2` range 0 to twice the canvas side, the positions an axis handle can reach | The handle moves in half-pixel steps along the canvas edge (`symmetry-tiled-mode` AC-09) | Low |
| The Pixel default stroke color is `#000000`, as on Image documents | An agent has no foreground color; one default for both modes | Low |
| Color arguments of the Pixel tools accept and convert colors exactly as `draw.pixels` does, in every color mode | One color rule for all Pixel tools (`mcp-headless-pixel` AC-19, AC-21) | Low |
| The Pixel `draw.gradient` takes the editor's foreground as `from` and background as `to` | The editor paints foreground at the drag start (`dithered-gradient` AC-01) | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`), `drawing_tools_describe_both_modes` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-02 | `pixel_stroke_equals_the_editors_pencil` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-03 | `pixel_perfect_strokes_equal_the_editors` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-04 | `pixel_eraser_stroke_equals_the_editors_eraser` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-05 | `pixel_shading_stroke_equals_the_editors_shading_ink` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-06 | `bad_shading_arguments_fail_naming_the_argument` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-07 | `pixel_mirror_and_wrap_equal_the_editors_symmetry_and_tiled_mode` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-08 | `pixel_mirror_and_wrap_equal_the_editors_symmetry_and_tiled_mode` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-09 | `bad_axes_and_fractional_pixel_coordinates_fail_naming_the_argument` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-10 | `pixel_shapes_equal_the_editors_shape_tools` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-11 | `pixel_shape_line_and_polygon_center_point_elsewhere` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-12 | `pixel_gradient_equals_the_editors_dithered_gradient` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-13 | `pixel_gradient_refuses_indexed_and_grayscale_documents` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-14 | `pixel_text_equals_the_editors_text_tool` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-15 | `text_that_lands_nowhere_and_zero_length_gradients_change_nothing` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-16 | `pixel_selection_clips_gradient_text_and_fill_only` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-17 | `image_selection_clips_every_drawing_tool` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-18 | `arguments_of_the_other_modes_editor_fail_naming_argument_and_mode` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-19 | `draw_stroke_matches_brush_and_eraser`, `draw_shape_matches_the_shape_tools`, `draw_text_matches_the_text_tool`, `draw_fill_and_gradient_match_bucket_and_gradient_tools` (`crates/pincel-agent/src/tests/image_draw.rs`) | built  |        |
| AC-20 | `drawing_on_locked_group_or_tilemap_layers_fails_with_the_reason` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-21 | `every_pixel_drawing_call_is_one_undo_step` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-22 | `read_resource_guide_covers_workflow_and_both_modes` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-23 | `line_rect_and_ellipse_on_an_image_document_point_to_draw_shape` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |
| AC-24 | `values_outside_the_modes_range_fail_with_the_range` (`crates/pincel-agent/src/tests/pixel_paint.rs`) | built  |        |

## Decisions

- 2026-11-05: One tool per capability across both modes (Decision Log 2026-11-05): `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` gain Pixel documents instead of new Pixel-only names — because the parity rule forbids a second name for the same capability.
- 2026-11-05: Symmetry and tiled wrapping are per-call arguments of `draw.stroke`, not session state — because a call then describes its whole result, and in the editor both are session-only options that change no document.
- 2026-11-05: Each mode keeps its editor's defaults (Pixel brush 1 `square`, text 12 px without anti-aliasing; Image brush 8 `hard_round`, text 48 px with anti-aliasing) — because "same pixels as the editor" includes the editor's starting options, and Image defaults must not change (AC-19).
- 2026-11-05: An argument only the other mode offers fails with the mode error (AC-18); an argument of the same mode that the chosen variant does not use (`ramp_from` with the simple ink, `corner_radius` on `ellipse`) is ignored — because a wrong-mode argument signals a misunderstanding the model must correct, as with `draw.fill` `tolerance` on Pixel documents (`mcp-headless-image`, Decisions 2026-10-29 build), while unused options of a variant are what the editor's option bar keeps too.
- 2026-11-05: Pixel polygons take the bounding box (x0, y0)–(x1, y1), Image polygons keep center, radius and rotation — because each follows its editor's drag geometry, and converting between them would not reproduce either editor's pixels.
- 2026-11-05: Pixel stroke, shape and 1-px tools ignore the selection, gradient and text honour it (AC-16), matching the Pixel editor; every Image drawing tool honours it (AC-17), matching the Image editor — because parity means the editor's result, and changing the Pixel editor is out of scope.
- 2026-11-05: Fractional coordinates on Pixel `draw.stroke` fail instead of being rounded (AC-09) — because Pixel tools address whole pixels and a silent rounding would hide a coordinate mix-up between the modes.
- 2026-11-05: Pixel `draw.fill` keeps clipping to the selection (`mcp-headless-pixel` AC-25 stays, AC-16) although the Pixel editor's Bucket ignores it; clipping the editor's tools becomes a backlog item — owner's choice, because a verified agent behavior must not regress, and clipped fills are what Aseprite and paint.net do.
- 2026-11-05: `draw.line`, `draw.rect` and `draw.ellipse` stay the Pixel editor's 1-px tools; on Image documents they fail with the mode error and point to `draw.shape` (AC-23) — owner's choice, because the Pixel editor issues separate commands for them (`applyLine`, `applyRectangle`, `applyEllipse`), so they are distinct capabilities from the styled shapes.
- 2026-11-05 (clarification): AC-17 states current behavior: the Image core's strokes, shapes, text, fill and gradient already clip to the selection by coverage; `mcp-headless-image` AC-11 named only fill / adjust / effect, so AC-17 pins the rest without changing pixels and does not conflict with AC-19.
- 2026-11-05 (clarification): Wrapping is described by its result (AC-08), not by a mechanism: the editor traces the stroke unwrapped and wraps every stamped pixel, mirror images included, which is what the edge cases assume.
- 2026-11-05 (clarification): Schemas allow the union of both modes' ranges and the tool enforces the range of the document's mode with a message naming the mode (AC-24) — one schema per tool cannot express per-mode bounds, and the message tells the model which limit applies.
- 2026-11-05 (clarification): Pixel coordinates of `draw.shape`, `draw.gradient` and `draw.text` must be integers like `draw.stroke`'s (AC-09), while Image keeps fractional values — same argument names with a per-mode type, because each editor addresses its canvas that way.
- 2026-11-05 (clarification): AC-18 owns the complete list of other-mode arguments of `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient`; `mcp-parity-drawing-image` AC-08 owns only the arguments it adds — one owner per list, so neither spec drifts from the other.
- 2026-11-05 (spec-build): A Pixel stroke that reaches beyond a cel smaller than the canvas grows the cel, as the editor's `PencilStroke` does; the tool then commits the whole new cel (`PutCel`) instead of a `PatchCel` diff, still one undo step.
- 2026-11-05 (spec-build): `frame` joined the schemas of `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` for Pixel documents (`mcp-headless-pixel` AC-43); Image documents ignore it, as their other tools do.
