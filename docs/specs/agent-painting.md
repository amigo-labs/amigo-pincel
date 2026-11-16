---
status: todo
created: 2026-11-16
size: M
mode: both
design: [mcp.md §Tools, pincel.md §15 (2026-11-05 MCP tool parity, 2026-11-16 Agent stroke batches), pincel.md §17.4]
depends_on: [mcp-parity-drawing, mcp-parity-drawing-image, pen-input]
---

# Agent painting: stroke batches and pen pressure

## Context

An agent can already paint over MCP: `draw.stroke` takes a color, a size, a tip and a list of points, `doc.preview` shows the result and `doc.undo` takes back a mistake. Painting a picture stroke by stroke needs hundreds to thousands of strokes (Anthropic's Claude Fable 5.2 painted pictures from 111 to several thousand strokes, each a color, a width and a few waypoints, the-decoder.de, 2026-11). Two things make that impractical today:

- **One stroke per call.** A thousand strokes are a thousand MCP round trips and a thousand undo steps. A painting pass is one unit for the agent, and in the desktop app's live mode (`mcp-live-desktop`) the user should be able to undo it as one step.
- **No pressure.** The Image editor's brushes swell and fade with pen pressure (`pen-input` AC-14 – AC-16), but the agent's strokes always paint at full pressure (`mcp-parity-drawing-image` left pressure out because "an agent's stroke is the full-pressure stroke a mouse makes"). Tapering strokes are what makes painted marks look painted.

This spec adds a batch tool `draw.strokes` (many strokes, one call, one undo step) in both modes and per-point `pressure` on Image strokes. "The editor" is defined as in `mcp-parity-structure`: the result of the core command the UI issues for the action with the same options; for a pen stroke that is the stroke command with one pressure value per point, interpolated between points by the core as for the editor's pointer samples. Item indexes in errors start at 0, as JSON array indexes do. Decision Log 2026-11-16 "Agent stroke batches" records why a batch tool does not break the one-tool-per-capability rule of 2026-11-05.

## Scope

### In scope

- New tool `draw.strokes` for Image and Pixel documents: a list of strokes, each with the arguments of `draw.stroke` for that document's mode, painted in order on one target layer as one undo step
- Per-point `pressure` with the switches `pressure_size` and `pressure_opacity` on Image `draw.stroke` and on each Image stroke of `draw.strokes`, for the brush and the eraser
- Limits: at most 1000 strokes and at most 100 000 points per `draw.strokes` call
- `pincel://guide` describing `draw.strokes` and `pressure`

### Out of scope

- A painterly brush (bristles, wet paint mixing, canvas texture) — because it is a new brush model in the Image core and the editor's tools, size L on its own; a later spec
- Pressure on `draw.recolor`, `draw.smudge` and `draw.clone` — because a painting is built from brush strokes; these tools can get `pressure` later with the same semantics
- Pen tilt (`azimuth`, "Tilt turns the nib") for agent strokes — because the fixed `angle` per stroke already turns a flat nib wherever the agent wants it
- Pressure on Pixel documents — because Pixel-mode tools stay pressure-free (`pincel.md` §17.4)
- Strokes on several layers in one `draw.strokes` call — because one target layer per call keeps the undo step and the lock checks as simple as `draw.stroke`'s; an agent paints each layer with its own call
- Recolor and smudge strokes inside `draw.strokes` — because `draw.stroke` itself accepts only `brush` and `eraser` (`mcp-parity-drawing-image` AC-15), and the batch mirrors it
- A reference-image comparison tool — because `doc.preview` already shows the agent its canvas; comparing against a reference is the agent's work

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains `draw.strokes`, meeting `mcp-headless-pixel` AC-02, with a required `strokes` array (1–1000 items) whose items take the arguments of `draw.stroke` except `layer`, `frame` and `doc`, plus top-level `doc`, `layer` and `frame` (Pixel) as `draw.stroke` has them.
- [ ] AC-02 (event): When the client calls `draw.strokes` on an Image document, then the target layer has the same pixels as after calling `draw.stroke` once per item, in list order, with that item's arguments on the same layer.
- [ ] AC-03 (event): When the client calls `draw.strokes` on a Pixel document, then the target cel has the same pixels as after calling `draw.stroke` once per item, in list order, with that item's arguments on the same layer and frame — including `ink: "shading"`, `mirror` and `wrap`.
- [ ] AC-04 (always): A successful `draw.strokes` call that changes the document is exactly one undo step, never merged with another call: one `doc.undo` restores the document as it was before the call, and one `doc.redo` repeats the whole call. A call whose strokes change no pixel adds no undo step and reports `changed: false`.
- [ ] AC-05 (event): When `draw.strokes` succeeds, then its result names the document, the target layer, `changed` as `draw.stroke` does, and `strokes`, the number of strokes painted.
- [ ] AC-06 (error): If `strokes` is empty or has more than 1000 items, or its items hold more than 100 000 points together, then the call fails with `isError: true` naming the limit that was broken, and the document is unchanged.
- [ ] AC-07 (error): If any item of `strokes` has an argument `draw.stroke` would refuse (an empty `points` list, a bad color, a value out of range, an argument of the other mode, an unknown `tool`, an unknown argument name) or carries `doc`, `layer` or `frame`, then the call fails with `isError: true` naming the item's index and the argument with the same reason `draw.stroke` gives, no stroke is painted and no undo step is added.
- [ ] AC-08 (error): If the target layer of `draw.strokes` is locked, lies inside a locked group or is itself a group, then the call fails as `draw.stroke` fails for that target, and the document is unchanged.
- [ ] AC-09 (event): When an Image `draw.stroke` (brush or eraser) gets `pressure`, a list of numbers 0–1 with one entry per point, then the target layer has the same pixels as after the Image editor's Brush or Eraser stroke along the same points with those pen pressures and the options Pressure → Size set to `pressure_size` (default `true`) and Pressure → Opacity set to `pressure_opacity` (default `false`), the tip at its fixed `angle`.
- [ ] AC-10 (always): An Image `draw.stroke` without `pressure`, with every entry 1, or with `pressure_size` and `pressure_opacity` both `false`, paints exactly the pixels the same stroke paints without any pressure argument.
- [ ] AC-11 (event): When an Image item of `draw.strokes` gets `pressure`, `pressure_size` or `pressure_opacity`, then it paints as `draw.stroke` with the same arguments (AC-09).
- [ ] AC-12 (error): If `pressure` has a different number of entries than `points`, or holds an entry that is not a finite number in 0–1, then the call fails with `isError: true` naming `pressure` (and in `draw.strokes` the item's index), and the document is unchanged.
- [ ] AC-13 (error): If a `draw.stroke` or an item of `draw.strokes` on a Pixel document gets `pressure`, `pressure_size` or `pressure_opacity`, then the call fails with `isError: true` naming the argument and the document's mode, and the document is unchanged.
- [ ] AC-14 (event): When an agent calls `draw.strokes` on the document open in the desktop app (live mode), then the whole call is one step in the editor's undo history: the editor's Undo takes back all of its strokes at once.
- [ ] AC-15 (always): The `pincel://guide` resource names `draw.strokes` with its limits and that it is one undo step, and `draw.stroke`'s `pressure`, `pressure_size` and `pressure_opacity` with their defaults and that they work on Image documents only.

## Technical notes

- `draw.strokes` arguments: `{ doc?, layer?, frame?, strokes: [ { points, tool?, color?, size?, hardness?, opacity?, shape?, angle?, anti_alias?, blend?, eraser_mode?, background?, pressure?, pressure_size?, pressure_opacity?, pixel_perfect?, ink?, ramp_from?, ramp_to?, step?, mirror?, axis_x2?, axis_y2?, wrap? } ] }`. Each item parses with `draw.stroke`'s argument code; validate every item before painting the first.
- Image: the per-point pressure reaches `CommandSpec::PencilStroke` / `EraserStroke` `pressure`, `pressure_size`, `pressure_opacity` (today hard-coded empty / `false` in `crates/pincel-agent/src/image/draw.rs`). One undo step for many strokes: the Image core's batch command (`command/batch.rs`) or a shared `stroke_id`; Pixel: the workspace's existing single-step edit wrapper. Both are implementation choices.
- Pressure dynamics, minimum dab radius and the opacity factor are the core's (`PressureDynamics`, `Brush::dab_radius`, `Brush::dab_strength`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `strokes` with exactly 1000 items, or exactly 100 000 points | Accepted |
| Item 412 of 800 has `size: 900` on an Image document | Fails naming item 412 and `size`; nothing painted (AC-07) |
| One item with one point | One stamp, as `draw.stroke` with one point |
| `pressure: [0, 0, 0]` with `pressure_size: true` | Dabs at the minimum radius, never vanish (`pen-input` edge case) |
| `pressure: [0, 0, 0]` with `pressure_opacity: true` | The dabs paint nothing; `changed: false`, no undo step (AC-04) |
| A call breaks a limit and also has an invalid item | The limit error is reported (AC-06 is checked first) |
| Several items are invalid, or one item breaks several rules | Only the first invalid item by index is reported, with the first argument `draw.stroke`'s checks refuse |
| Both limits are broken | The stroke-count limit is named |
| Points limit with Pixel `mirror` / `wrap` | Counts the entries of the `points` lists only, not the mirrored or wrapped stamps |
| `pressure_size` / `pressure_opacity` without `pressure` on an Image stroke | Accepted; paints as without them (AC-10) |
| `strokes` is not an array, or an item is not an object | Fails as a schema error naming `strokes` (or the item), as for any malformed argument |
| Image document with top-level `frame` | Fails naming `frame` and the document's mode, as `draw.stroke` does |
| `pressure` given, `pressure_size` and `pressure_opacity` both `false` | Same pixels as without `pressure` (AC-10) |
| `pressure` on an eraser stroke | Erases with smaller / fainter dabs as the editor's Eraser (AC-09) |
| Every stroke lies outside the canvas | `changed: false`, no undo step (AC-04) |
| `draw.strokes` with an active selection | Each stroke is clipped to it, as `draw.stroke` is (AC-02) |
| Live mode: the user presses Undo while a long `draw.strokes` call runs | The call runs to the end first, as every live call does; the Undo then takes back the whole call |
| An item carries `layer`, `frame` or `doc` | Fails naming the item and the argument: the target is set once per call (AC-07) |
| Live mode: the user paints, locks or deletes layers, or a second client calls, while `draw.strokes` runs | As for every tool call in `mcp-live-desktop`: calls run one at a time, a user stroke queues the call until pointerup, and the call checks its target when it runs |
| Live mode: `draw.strokes` takes longer than the 30 s call timeout | As `mcp-live-desktop` AC-19 and its edge case: the client gets the timeout, the work completes and stays one undo step |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Limits 1000 strokes / 100 000 points per call | One painting pass fits; keeps a request at a few MB of JSON and a call well inside `mcp-live-desktop`'s 30 s timeout (not measured: 100 000 points of a 40 px brush is about the work of a few hundred editor strokes) | An agent needs more calls for a dense layer, or a heavy call times out in live mode; then change the limits in a revision |
| No speed AC | The batch saves round trips, the painting itself is the same core code; the gain depends on the client | A slow batch would show in use; then add a benchmark AC |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- 2026-11-16 (owner): both modes get `draw.strokes`; at most 1000 strokes per call; one invalid item fails the whole call, because a half-painted pass is harder for the agent to reason about and undo than a refusal it can fix and resend; pressure follows the Image editor's Pressure → Size / Opacity options with their defaults.
- 2026-11-16: a `draw.strokes` call never merges with another call's undo step (AC-04), as every MCP tool call (`mcp-headless-pixel` AC-09), so `doc.undo` always takes back exactly one call.
- 2026-11-16: `pressure` reverses the out-of-scope line of `mcp-parity-drawing-image` ("an agent's stroke is the full-pressure stroke a mouse makes"): painting needs tapering strokes, and pressure is a property of the stroke, not only of an input device.
