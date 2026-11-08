---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6, pincel.md §9.2, pincel.md §9.4, pincel.md §15 (2026-10-27 Image mode gains Gradient, Recolor and Smudge), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [image-live-objects]
---

# Image mode: Line / Curve tool with arrow heads and more dash patterns

## Context

Image mode's Line tool (`L`, pincel.md §5.4) draws one straight segment with the shape style: stroke width, dash (solid / dashed / dotted), anti-alias. It has round ends and nothing else. paint.net's Line / Curve tool lets the user bend a line after drawing it by dragging control points onto it, put arrow heads on either end and pick from more dash styles; annotating a screenshot ("this button", a curved arrow) is the most common use. pincel.md §5.4 lists "bézier curves" as not implemented.

`image-live-objects` (core) and `image-live-objects-ui` (editor) keep a drawn line, shape or gradient a *live object* until it is committed: the core draws it into its layer at every change, each edit is an ordinary undo step, the commit folds the object's steps into one, Escape cancels it, Enter commits it, and any other document change or a change of the active layer commits it first. This spec builds on that: while a line is live, the user adds, moves and removes control points and changes its caps; each such edit is one undo step of the live object.

This is part 1 of two. Part 2, `image-shapes-curves-shapes.md`, adds the new area shapes (triangle, diamond, star, block arrow, heart, callout). The split keeps each part at size L (AGENTS.md §3.2).

Per Decision Log 2026-11-05 "MCP tool parity", every new capability is reachable through the existing MCP tool `draw.shape` (one tool name per capability).

## Scope

### In scope

- Line becomes Line / Curve: control points added by dragging on the live line's stroke, moved by dragging, removed by double-click / double-tap; the curve passes through both endpoints and every control point in order (centripetal Catmull-Rom spline, see Technical notes)
- A move grip on the live line that moves the whole line, since a drag on the stroke adds a control point
- End caps per end: Round (today's behavior, default), Flat, Arrow (a filled arrow head in the stroke color)
- Two new dash patterns for every shape and the line: Dash-Dot and Dash-Dot-Dot
- Anti-aliased rasterization of curves and arrow heads, bounded by the selection like every pixel tool
- `draw.shape` with `shape: "line"` gains control points, caps and the new dash patterns
- A criterion benchmark scenario for a long curve

### Out of scope

- A Bézier curve type with off-curve handles (paint.net's second curve mode) — because a spline through the points does what a single cubic can do with one or two extra points, and a second handle model doubles the gesture and handle UI for little gain
- Closed curves and freeform filled curve shapes — because Lasso + Fill already make a filled freeform area, and a closed spline needs its own fill rules
- Open (chevron) arrow heads, custom head sizes and custom dash patterns — because each needs its own options UI; the filled head and five fixed patterns cover annotation, the main use
- Caps on the inner ends of dashes — because caps describe the line's two ends; dashes keep today's round ends
- Curves in Pixel mode — because Pixel-mode lines are pixel-perfect Bresenham strokes, which need a separate design (`pixel-drawing-tools`)
- Editing a curve after commit — because a committed line is pixels; keeping it editable longer is `image-live-objects`' concern
- The new area shapes — because they are part 2 (`image-shapes-curves-shapes.md`)

## Acceptance criteria

- [ ] AC-01 (event): When the user drags with the Line / Curve tool and commits without adding a control point, with Round caps (the default), then the active layer gets exactly the pixels today's Line tool paints for the same endpoints, stroke width, colors, dash and anti-alias setting.
- [ ] AC-02 (event): When, while a line is live, the user presses on its *stroke* — the points within half the stroke width (converted to CSS px at the current zoom) or 8 CSS px of the curve, whichever is larger (16 CSS px for touch and pen), and not on a handle or the move grip — and drags, then a control point is inserted between the two points it lies between, follows the pointer and the curve passes through it; releasing keeps it, the line stays live, and the whole drag is one undo step. A press on the stroke without movement adds no point and no step, and neither commits the line nor starts a new one.
- [ ] AC-03 (event): When, while a line is live, the user drags an endpoint or a control point, then that point follows the pointer and the curve still passes through the endpoints and every control point, in order; the whole drag is one undo step. With Shift held, an endpoint snaps the segment from its neighbouring point (the adjacent control point, or the other endpoint when there is none) to the nearest multiple of 45°; a control point ignores Shift.
- [ ] AC-04 (event): When, while a line is live, the user double-clicks or double-taps a control point, then that point is removed and the curve passes through the remaining points, as one undo step; the first click of the double-click, a press without movement, adds no step of its own; a double-click on an endpoint changes nothing and adds no step.
- [ ] AC-05 (always): A committed curve with anti-alias on leaves every pixel unchanged whose center lies farther than (stroke width / 2 + 0.5) px from the curve and outside every arrow head inflated by 0.5 px, for every dash pattern and cap. A solid curve with Round caps also gives full stroke coverage to every pixel whose center lies within (stroke width / 2 − 0.5) px of the curve; with Flat or Arrow caps this holds except within (stroke width / 2 + 0.5) px of an end, which AC-08 and AC-09 cover.
- [ ] AC-06 (always): A curve whose control points lie on the straight segment between its endpoints, in order, paints exactly the pixels of the straight line with the same style — for every dash pattern and every cap combination.
- [ ] AC-07 (state): While anti-alias is off, every pixel of a committed curve is either unchanged or written with the full stroke color, and a solid curve with stroke width 1 and Round caps forms an 8-connected path from one endpoint to the other.
- [ ] AC-08 (option): Where a cap is Arrow, a filled arrow head in the stroke color sits at that end, its tip on the endpoint and its axis along the curve's direction at that end; the head is solid whatever the dash pattern, its length is max(8, 4 × stroke width) px and its base as wide as it is long, and no pixel lies more than 1 px beyond the tip along that direction.
- [ ] AC-09 (option): Where a cap is Flat, no pixel lies more than 1 px beyond the endpoint along the curve's direction at that end; where it is Round, the stroke ends in a half disc of radius stroke width / 2 around the endpoint.
- [ ] AC-10 (event): When the arrow heads are together longer than the curve — one Arrow cap longer than the curve, or two whose lengths add up to more — then each head shrinks by the factor (curve length) / (sum of the head lengths), so a single head spans the whole curve and two heads meet without overlapping along it; every tip stays on its endpoint.
- [ ] AC-11 (option): Where the dash pattern is Dash-Dot or Dash-Dot-Dot, the outline of the line and of every shape repeats dash, gap, dot, gap (Dash-Dot-Dot: two dots) with the run lengths in Technical notes; the pattern's phase is measured along the whole curve from the start point, without restarting at a control point and unchanged by the caps — an arrow head covers the part of the pattern under it.
- [ ] AC-12 (event): When the user changes the start cap, end cap, dash pattern, stroke width, color or anti-alias while a line is live, then the live line shows the change at once and the commit uses it.
- [ ] AC-13 (event): When a live line is committed (Enter or any other commit trigger of `image-live-objects-ui`), then its steps fold into one undo step: one Undo restores the layer as it was before the line, Redo paints the same curve again, and no object is live after either. When Undo runs while the line is still live, then the last control-point, cap or style edit is undone and the line stays live with its handles; when Escape cancels it, then the layer shows the pixels from before the line and none of its steps remain.
- [ ] AC-14 (state): While a selection exists, a committed curve and its arrow heads change only selected pixels, scaled by the selection coverage.
- [ ] AC-15 (error): If the live line already has 64 points (endpoints included), then a press on its stroke adds no point and the curve stays as it is; moving and removing points still work.
- [ ] AC-16 (error): If two consecutive points of a curve coincide, then they act as one point and no stray pixel is painted. If all points coincide — a live line whose points were all dragged onto one spot (`image-live-objects` refuses to begin one), or an MCP call — then with Round caps on both ends the layer shows the dot today's Line paints for a zero-length segment, and with a Flat or Arrow cap on either end nothing; a live line drawn as nothing commits without a step (`image-live-objects` AC-12), an MCP call reports `changed: false`.
- [ ] AC-17 (always): The options bar of the Line / Curve tool offers Start cap and End cap (Round, Flat, Arrow) and the five dash patterns, the tool's name reads "Line / Curve" in the rail, its tooltip and the shortcut overview, and every new label exists in English and German.
- [ ] AC-18 (event): When an MCP client calls `draw.shape` with `shape: "line"`, endpoints, `control` points, `start_cap`, `end_cap` and a `dash` pattern (any of the five, which every other `draw.shape` kind accepts too), then the layer has exactly the pixels the editor commits for the same points and style, in one undo step.
- [ ] AC-19 (error): If an MCP `draw.shape` line call has more than 62 `control` points, a non-finite coordinate, a cap or dash value outside the listed choices, or `control`, `start_cap` or `end_cap` on a kind other than `line`, then the call fails with `isError: true` naming the bad argument, and the document is unchanged.
- [ ] AC-20 (always): `tools/list` describes `draw.shape`'s `control`, `start_cap` and `end_cap` parameters and lists all five dash patterns.
- [ ] AC-21 (event): When `cargo bench -p pincel-image-core --bench compose` runs, then it reports a median for drawing a 64-point curve with stroke width 20, Dash-Dot pattern, anti-alias and Arrow caps across a 3840×2160 layer.
- [ ] AC-22 (event): When, while a line with control points is live, the user drags its move grip (`image-live-objects-ui` AC-04), then the whole line, control points and caps included, moves by the drag offset, as one undo step; the grip sits beside the curve's midpoint by arc length, perpendicular to the curve there, instead of beside the straight segment's midpoint.

## Technical notes

**Curve model.** The points are P0 (start), the control points in order, and Pn (end). The curve is a centripetal Catmull-Rom spline (α = 0.5) through them: it passes through every point, has no cusps or self-loops within a segment, and with collinear in-order points degenerates to the straight segment (AC-06). The end segments use phantom points reflected through the ends (P−1 = 2·P0 − P1, Pn+1 = 2·Pn − Pn−1). The end direction for caps and arrow heads is the curve's tangent at that end. The curve is flattened to a polyline fine enough that its distance to the exact curve stays below 0.1 px; coverage is then the distance-to-polyline band the line uses today, so joins are round and dashes walk the polyline's arc length as `dash_segments` already does.

**Caps.** Round: today's capsule end. Flat: the band is cut by the line perpendicular to the end direction through the endpoint. Arrow: the stroke is cut flat at the head's base, the head is a filled isosceles triangle (tip on the endpoint, length L = max(8, 4w), base width L), rasterized with the polygon SDF so it anti-aliases like the shapes. AC-10's shrink factor is (curve length) / (sum of head lengths).

**Dash run lengths** (w = stroke width, at least 1): Dashed 3w on / 2w off and Dotted w on / 1.5w off stay as today; Dash-Dot 3w, 1.5w, w, 1.5w; Dash-Dot-Dot 3w, 1.5w, w, 1.5w, w, 1.5w. `DashPattern::runs` becomes a slice of on/off lengths.

**Core and wasm.** `draw_shape` `line` takes `points` with 2–64 entries (first and last are the endpoints; today's two-point form stays valid), and new serde-defaulted fields `start_cap` / `end_cap` (`round` default, `flat`, `arrow`); `dash` gains `dash_dot` / `dash_dot_dot`. The ts-rs mirror under `ui/src/modes/image/core/generated/` is regenerated. `shapes.rs` is already over the 800-line limit, so the curve, caps and arrow heads go into their own module. The committed step's undo label is set at commit: "Draw Line" for two points, "Draw Curve" with control points; its history kind stays the tool's, Line (`image-live-objects` "History kinds").

**MCP.** `draw.shape` `line` keeps `x0, y0, x1, y1` and adds `control: [[x, y], …]` (0–62 interior points, in order), `start_cap` / `end_cap` (`round` / `flat` / `arrow`, default `round`) and the two dash values; the agent builds the same `CommandSpec` the editor sends.

**Budget.** AC-21's scenario gets a budget in `performance-benchmarks`' budget table, proposed 40 ms median on the reference machine (`pending` until first met).

**Live object.** The line is a live object of `image-live-objects`: its record holds the points and both caps besides the shared style, the core draws it into its layer at every edit, and each control-point, cap or grip edit is one gesture (one undo step, `image-live-objects` AC-03). The record lives on `CommandBus`, beside the document, never in `.ora` (`image-live-objects` AC-13).

**UI.** Hit-testing the stroke uses the same flattened polyline the core draws, mapped to screen space. AC-02's stroke replaces the Line *body* of `image-live-objects-ui` ("within 8 CSS px of the segment"), which defers to this spec once it lands. Control-point handles look like the endpoint handles there and follow its handle size and reach (AC-05 there). The move grip, its cursor and its 16 CSS px distance are `image-live-objects-ui`'s; only its anchor moves to the curve's arc-length midpoint (AC-22). Handles win over the grip, the grip over the stroke.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Press on the stroke exactly on a handle | The handle is dragged (AC-03); no point is added |
| Move grip lying over the stroke (a tight curve) | The grip is dragged (AC-22); no point is added |
| Escape during a control-point drag | That drag is undone, the line stays live (`image-live-objects-ui` AC-11); a second Escape cancels the line |
| Undo right after adding a control point | The point disappears, the line stays live (AC-13) |
| Active layer changed while the curve is live | The curve is committed first as one step (`image-live-objects` AC-08) |
| A control point dragged onto its neighbour | The two act as one point (AC-16); dragging it away separates them again |
| A sharp zig-zag of control points | The curve still passes through every point without a loop between two of them |
| Arrow caps on a dashed line | The heads are solid; the dashes stop at the heads' bases |
| Arrow head larger than the canvas or partly off-canvas | Only the on-canvas part is painted; off-canvas parts are skipped without error |
| A line with both Arrow caps shorter than one head | Both heads shrink to meet in the middle (AC-10) |
| One Arrow cap on a line shorter than the head | The head shrinks to the curve's length; its base sits on the other endpoint (AC-10) |
| Two arrow heads that overlap on a strongly bent curve (their lengths fit along it) | Both are painted; where they overlap the pixel takes the stroke color once, not darker |
| Press on the stroke and release without moving | No point, no step; the line stays live (AC-02) |
| Stroke width 1, anti-alias off, steep curve | An 8-connected path without gaps (AC-07) |
| Zoomed far out (curve a few screen pixels long) | The 8 CSS px stroke reach (16 for touch and pen) still lets the user grab the stroke; handles take precedence |
| Zoomed far in on a wide stroke | The stroke's own half width, converted to CSS px, is the reach (AC-02) |
| A 64th point present, user drags on the stroke | No point is added (AC-15) |
| A two-point line saved through MCP by an older client without `control` / caps | Same pixels as today (AC-01) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `image-live-objects` and `image-live-objects-ui` land first and give the line endpoint handles, option edits, Escape / Enter and the commit triggers | They are this spec's `depends_on`; this spec adds control points, caps and the grip on top | Low |
| A cap of 64 points | Enough for any hand-drawn curve, keeps hit-testing and the command payload bounded | Low |
| Arrow head length max(8, 4w), base as wide as long | Readable at stroke width 1 (8 px) and proportional at large widths; paint.net's heads scale with the width the same way | Low: a size option can be added later |
| Round stays the default cap | Keeps every existing line, MCP call and test byte-identical (AC-01) | Low |
| Double-click / double-tap removes a control point | Needs no extra key or menu and works on touch | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `a_two_point_round_line_is_todays_line` (crates/pincel-image-core/tests/curves.rs); the live line draws through `CurveLine`, which hands a two-point Round line to `Shape::Line` — `control_points_and_caps_edit_the_live_line_and_the_commit_uses_them` / `draw_shape_draws_a_line_through_its_control_points_with_caps` (crates/pincel-image-core/tests/live_curves.rs) and the unchanged live-line e2e of `ui/e2e/image-live-objects.spec.ts` | built |        |
| AC-02 | `a drag on the stroke adds a control point the curve passes through; a press alone adds nothing (AC-02)` (ui/e2e/image-curves.spec.ts); `the body moves a box, the grip moves a line, the stroke adds a control point` (ui/e2e/image-live-objects.spec.ts); `the stroke reaches 8 CSS px, or half the stroke width when wider (AC-02)`, `a drag on the stroke between two points inserts the new point between them (AC-02)`, `touch › a touch on the stroke reaches 16 CSS px (AC-02)` (ui/e2e/image-curves.spec.ts) | built |        |
| AC-03 | `control points and endpoints drag; Shift snaps an end against its neighbour (AC-03)` (ui/e2e/image-curves.spec.ts); `Shift does not snap a control point (AC-03)` (ui/e2e/image-curves.spec.ts) | built |        |
| AC-04 | `a double-click removes a control point as one step; on an endpoint it does nothing (AC-04)` (ui/e2e/image-curves.spec.ts) | built |        |
| AC-05 | `a_curve_paints_only_its_band_and_heads_and_fills_its_core` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-06 | `collinear_control_points_draw_the_straight_line` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-07 | `without_anti_alias_pixels_are_full_and_a_thin_curve_is_8_connected` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-08 | `an_arrow_head_sits_on_the_end_solid_and_sized_by_the_width`, `the_stroke_and_its_heads_join_without_a_seam` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-09 | `flat_ends_stop_at_the_end_and_round_ends_are_half_discs` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-10 | `heads_longer_than_the_curve_shrink_to_fit`, `the_stroke_and_its_heads_join_without_a_seam` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-11 | `dash_dot_patterns_repeat_dash_gap_dot_gap` (crates/pincel-image-core/tests/curves.rs; line and rectangle outline); `the_dash_phase_does_not_depend_on_the_caps` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-12 | `caps and dash changes show on the live line at once; the commit folds into one step (AC-12, AC-13)` (ui/e2e/image-curves.spec.ts); `control_points_and_caps_edit_the_live_line_and_the_commit_uses_them` (crates/pincel-image-core/tests/live_curves.rs; width, color and anti-alias too) | built |        |
| AC-13 | `caps and dash changes show on the live line at once; the commit folds into one step (AC-12, AC-13)` (ui/e2e/image-curves.spec.ts); `control_points_and_caps_edit_the_live_line_and_the_commit_uses_them`, `undo_while_live_takes_back_the_last_edit_and_escape_cancels` (crates/pincel-image-core/tests/live_curves.rs) | built |        |
| AC-14 | `a_curve_changes_only_selected_pixels` (crates/pincel-image-core/tests/curves.rs); `a_curve_is_scaled_by_the_selection_coverage` (crates/pincel-image-core/tests/curves.rs) | built |        |
| AC-15 | `a line with 64 points takes no more; moving points still works (AC-15)` (ui/e2e/image-curves.spec.ts); `a_line_holds_at_most_64_points` (crates/pincel-image-core/tests/live_curves.rs) | built |        |
| AC-16 | `coincident_points_act_as_one` (crates/pincel-image-core/tests/curves.rs); `a_live_line_on_one_spot_with_a_flat_cap_commits_without_a_step` (crates/pincel-image-core/tests/live_curves.rs); `edit_live_to_degenerate_geometry_stays_live_and_draws_nothing` (crates/pincel-image-core/tests/live_objects.rs, Round dot / Flat nothing); `draw_shape_curves_match_the_editors_live_line` (`changed: false`, crates/pincel-agent/src/tests/image_draw.rs) | built |        |
| AC-17 | `the options bar offers caps and five dash patterns; the tool reads Line / Curve (AC-17)` (ui/e2e/image-curves.spec.ts; English and German, rail and shortcut overview) | built |        |
| AC-18 | `draw_shape_curves_match_the_editors_live_line` (crates/pincel-agent/src/tests/image_draw.rs); `draw_shape_draws_a_line_through_its_control_points_with_caps` (crates/pincel-image-core/tests/live_curves.rs) | built |        |
| AC-19 | `draw_shape_refuses_bad_curve_arguments` (crates/pincel-agent/src/tests/image_draw.rs) | built |        |
| AC-20 | `tools_list_describes_curves` (crates/pincel-agent/src/tests/image_draw.rs) | built |        |
| AC-21 | `image_shapes/curve_64pts_w20_dashdot_arrows_4k` (crates/pincel-image-core/benches/compose.rs); budget ≤ 40 ms `pending` in `ui/e2e-perf/budgets.json` and the `performance-benchmarks` budget table | built |        |
| AC-22 | `the grip sits beside the curve’s midpoint and moves the whole line as one step (AC-22)` (ui/e2e/image-curves.spec.ts) | built |        |

## Decisions

- 2026-11-05: Spline through the points (centripetal Catmull-Rom) is the only curve model — because the curve goes where the user drags, which also works on touch, and centripetal parameterization avoids the cusps and loops a uniform Catmull-Rom makes with uneven point spacing.
- 2026-11-05: Line and curve stay one tool and one `draw.shape` kind (`line`) — because the editor has one Line / Curve tool, and Decision Log 2026-11-05 asks for one tool name per capability.
- 2026-11-05: The dash phase runs from the curve's start and ignores caps (AC-11) — because toggling an Arrow cap should not shift every dash along the line.
- 2026-11-05: Shift on an endpoint snaps against its neighbouring point (AC-03) — because snapping against the far endpoint means nothing once the line bends; control points stay free, since a 45° rule has no natural anchor for them.
- 2026-11-05: MCP `control`, `start_cap` and `end_cap` on a non-line kind fail (AC-19) — because an agent that sends them expects an effect, and ignoring them would hide its mistake.
- 2026-11-05: Caps apply to the line's two ends only, dashes keep round ends — because that changes no existing shape outline.
- 2026-11-05, owner's choice: a plain drag on a live line's stroke inserts a control point, and the whole line moves by a dedicated move grip (AC-02, AC-22); area shapes keep the body drag that moves them (`image-live-objects-ui` AC-04) — because it is paint.net's gesture and makes curves discoverable without a modifier.
- 2026-11-05, owner's choice: the line follows the shared live-object architecture — the core draws it into its layer at every change, its record sits beside the document on `CommandBus` (not in `.ora`), each edit is an ordinary undo step, the commit folds them into one, Escape cancels, Enter commits, and any other command or active-layer change commits it first — because one model for every live object keeps undo, autosave and MCP behavior identical across kinds.
- 2026-11-08 (spec-build): The curve lives in its own module, `tools/curve.rs` (`Curve`, `CurveLine`, `LineCap`, `MAX_LINE_POINTS`), since `shapes.rs` is over the size limit; `DashPattern::runs` became a list of on / off runs, and the dash walker gives today's two-run patterns the same floats as before (the existing shape tests and snapshots are unchanged). A two-point line with Round caps, and a curve whose points are collinear in order, are handed to `Shapes`' `Shape::Line`, so AC-01 and AC-06 hold by construction; the flattening tolerance is 0.02 px (finer than the 0.1 px in Technical notes, so the band stays within AC-05's half pixel of the exact curve).
- 2026-11-08 (spec-build): Where the stroke meets an arrow head, and where two shrunk heads meet, coverage is the clamped *sum* of the stroke's cut ramp and the heads — the two ramps are complementary, so the sum joins them without a lighter seam, and the clamp keeps overlapping heads from painting darker (Edge cases). Red run: with `max`, `the_stroke_and_its_heads_join_without_a_seam` failed with a 50 % column at the head's base.
- 2026-11-08 (spec-build): The rasterizer fills a distance buffer only around each segment (walked in short pieces) and shades only pixels within reach of the band or a head; measuring every pixel of the region against every segment took 35 s for AC-21's scenario, now ≈ 184 ms in the dev container under load — the 40 ms budget stays `pending`.
- 2026-11-08 (spec-build): The live line's record (`LiveShape`) holds `control` (the interior points) and `start_cap` / `end_cap`; `edit_live` takes `control` (the whole list, so add, move and remove are one field) as a geometry edit and the caps as a parameter group of their own, and refuses either on a box shape (`LiveEditMismatch`), as it refuses more than 62 control points (AC-15's core half). `live_object_spec` reports the control points, the caps, the flattened polyline (for the editor's stroke hit-test) and the grip anchor and normal (AC-22). A curve's undo label is "Draw Curve" while its history kind stays the Line's (`draw_line`), during the live edits and after the commit.
- 2026-11-08 (spec-build): AC-16 changes one rule of `image-live-objects`: a live line whose points were all edited onto one spot now shows today's zero-length dot with Round caps instead of drawing nothing (`edit_live_to_degenerate_geometry_stays_live_and_draws_nothing` was revised to say so, and checks that a Flat cap draws nothing); beginning such a line is still refused, and a line drawn as nothing still commits without a step (that spec's AC-12).
- 2026-11-08 (spec-build): MCP `draw.shape` validates the curve arguments in its schema (`control` ≤ 62 points of two numbers, the cap and dash enums) and in the tool (line-only arguments on another kind — checked before the Pixel / Image split, so a Pixel call fails the same way —, non-finite coordinates); it builds the same `draw_shape` the editor's commit draws, with `points` = start, control points, end.
- 2026-11-08 (spec-build): The editor hit-tests a live line on the polyline the core reports (`live-handles.ts`: handles first, then the grip, then the stroke within max(half the stroke width, 8 CSS px — 16 for touch and pen)); a press on the stroke becomes a control point only once the pointer has moved 2 CSS px, inserted between the two points of the line whose stretch of the polyline it hits. A second press on the same control point within 400 ms removes it (the first press, without movement, sends nothing). Image mode's tool reads "Line / Curve" through a new key `tool.lineCurve`; Pixel mode keeps "Line" (its lines are Bresenham strokes, Out of scope). The e2e specs that picked the Image tool as "Line" now name it "Line / Curve".
- 2026-11-08 (spec-build, verifier round 1): The four clauses the verifier found without a failing test now have one — the stroke's reach (8 CSS px for a thin mouse line, half the stroke width when that is wider, 16 CSS px for touch), insertion between the two points a press lies between (a point added left of an existing one becomes `ctrl0`), Shift ignored on a control point, the dash phase unchanged by the caps away from the ends, and a selection at coverage 128 halving the curve's coverage. All passed at once: the behaviour was built, the assertions were missing.
