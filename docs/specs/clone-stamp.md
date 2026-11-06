---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6, pincel.md §9.2, pincel.md §9.4, pincel.md §15 (2026-10-27 Image mode tool set), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Clone Stamp

## Context

paint.net's Clone Stamp copies pixels from one part of an image to another with a brush: Ctrl+click sets a source point, and painting then lays down the pixels found at a fixed offset from the brush. It is the standard tool for retouching photos and screenshots (removing a speck, repeating a texture, covering a watermark). Image mode lacks it: `pincel.md` §5.4 lists it under "Not implemented", and the Decision Log entry of 2026-10-27 left it out on purpose, because "it needs a source anchor that survives the stroke and a second cursor, which is a UI problem of its own". This spec settles that UI problem and adds the tool.

The Smudge tool is the closest existing tool: it also reads pixels while painting, a drag arrives as several commands that share a `stroke_id` and merge into one undo step, and per-drag memory is kept across these commands. The Clone Stamp uses the same brush tip as the Brush: shape, size, hardness, opacity, and pen pressure for size and opacity.

Decision Log 2026-11-05 "MCP tool parity" says that every document-changing capability of the editor gets an MCP tool, so this spec adds one (AC-19).

**Terms used in this spec:**

- **Pixel under a point**: the canvas pixel whose area contains the pointer position, that is the pixel (floor(x), floor(y)) of a fractional canvas position (x, y).
- **Source point**: the pixel under the press the user made with Ctrl+click (⌘+click on macOS) or with the *Set source* toggle.
- **Offset**: the source point minus the pixel under the press of the first stroke after the source point was set, a whole-pixel vector. Later strokes keep the offset until a new source point is set ("aligned" cloning).
- **Source image**: the pixels the tool copies from. With *Sample: Current Layer* this is the active layer. With *Sample: All Layers* it is the composite of the visible layers. In both cases it is the state the source image had when the stroke's press happened.
- **Stroke**: everything from a press on the canvas to its release or cancel. The editor delivers it as a sequence of segments, each from the last delivered point to the newest pointer samples.
- **Dab**: one placement of the brush tip along the stroke. Dabs are placed exactly where the Brush places them for the same segments.
- **Effective coverage** of a pixel under one dab: the brush tip coverage at that pixel, times opacity, times pen pressure where pressure opacity is on, times the selection coverage at that pixel. This is the same value the Brush uses for that dab.

## Scope

### In scope

- A Clone Stamp tool in Image mode's tool rail, with the tool-group key `C` and the Brush's tip options plus *Sample: Current Layer / All Layers*
- Setting the source point with Ctrl+click (⌘+click on macOS) and with a *Set source* toggle in the options bar for touch and pen use without a keyboard
- Aligned cloning: the offset is fixed at the first stroke and kept for later strokes
- Painting that copies source pixels through the brush tip, bounded by the selection
- A crosshair at the source position, while hovering and while painting
- One undo step per stroke
- An MCP tool `draw.clone` for the capability
- English and German strings, and the tool in the shortcut overview

### Out of scope

- Non-aligned cloning (every stroke starts again from the source point) — because paint.net's Clone Stamp has only the aligned behavior, and a second mode would need an option paint.net does not have
- Cloning in Pixel mode — because Pixel mode has no soft brushes, and a pixel-art clone tool (palette indexes, tilemaps, frames) is a separate design
- Sampling from another document or from another tab — because the source image would have to stay alive across tabs, and paint.net samples only inside the image (it does allow other layers, which *All Layers* covers)
- Scaling, rotating or flipping the cloned pixels — because paint.net's Clone Stamp does not transform the source; such options belong with selection transforms (Phase 2, §13)
- Healing (blending the clone with the destination's texture and light) — because it is a separate tool (Healing Brush) with its own algorithm
- Saving the source point in the file or in the undo history — because it is a tool setting, like the brush size, and not part of the image

## Acceptance criteria

- [ ] AC-01 (always): Image mode's tool rail shows a Clone Stamp button in its own group, in the paint section directly after Smudge. Its tooltip names the tool and its key `C`. Pressing `C` selects the tool. Pixel mode has no Clone Stamp, and the key does nothing in Pixel mode. Help ▸ Keyboard Shortcuts lists the tool and its key in Image mode.
- [ ] AC-02 (state): While the Clone Stamp is the active tool, the options bar shows the Brush's tip controls (shape, size, hardness, opacity, angle, tilt angle, pressure size, pressure opacity) plus *Sample* (*Current Layer*, the default, or *All Layers*) and a *Set source* toggle. The tip controls hold the same values as the Brush's: a change made with either tool shows in the other. *Sample* is the Clone Stamp's own setting. `[` and `]` change the size as they do for the Brush.
- [ ] AC-03 (event): When the user presses on the canvas with the Clone Stamp while holding Ctrl (⌘ on macOS), then the pixel under the press becomes the source point, no pixel of the document changes, and no undo step is added.
- [ ] AC-04 (option): Where *Set source* is on, the next press on the canvas sets the source point as in AC-03 instead of painting, and *Set source* switches off.
- [ ] AC-05 (event): When the user makes the first stroke after setting a source point, then the offset is fixed, and every dab composites, at each pixel p it covers, the source image's pixel at p + offset source-over onto p's current value, with the source pixel's alpha scaled by p's effective coverage under that dab. Overlapping dabs build up as the Brush's do. The stroke writes only into the active layer.
- [ ] AC-06 (always): A Clone Stamp stroke whose source image is one opaque color C, at every pixel the stroke reads, gives the same pixels as a Brush stroke in color C with the same points delivered in the same segments, and the same tip settings and pen input. With a hard round tip at 100% opacity, no selection and an opaque source image, every pixel p the stroke covers whose source position p + offset lies inside the canvas ends up equal to the source image's pixel there.
- [ ] AC-07 (state): While the source point stays the same, every later stroke uses the offset fixed in AC-05: a stroke that starts at point q copies from q + offset, not from the source point. Setting a new source point makes the next stroke fix a new offset.
- [ ] AC-08 (always): A stroke reads the source image as it was at the stroke's press. Pixels written earlier in the same stroke are not read again. Delivering the same sequence of segments, each starting at the end point of the one before, as one command or as several commands of the same stroke gives the same pixels.
- [ ] AC-09 (option): Where *Sample* is *All Layers*, the stroke copies from the composite of the visible layers (as the Paint Bucket's *All Layers* sample does) and writes into the active layer. Hidden layers are not part of the composite.
- [ ] AC-10 (state): While a selection exists, the stroke changes only selected pixels. A feathered edge scales the effective coverage by the selection coverage, as it does for the Brush. The source position p + offset may lie outside the selection.
- [ ] AC-11 (always): A destination pixel whose source position p + offset lies outside the canvas stays unchanged.
- [ ] AC-12 (always): Each stroke that changes the document is exactly one undo step, whatever number of segments it has. Undo restores the layer byte for byte, and redo restores the stroke's result. Undo and redo do not change the source point or the offset.
- [ ] AC-13 (error): If the user presses on the canvas with the Clone Stamp, without Ctrl / ⌘ and without *Set source*, and no source point is set in this document, then no pixel changes, no undo step is added, and the status bar tells the user to Ctrl+click (⌘+click on macOS) to set a source. Where AC-14 also applies, the status bar shows AC-14's notice instead.
- [ ] AC-14 (error): If the active layer is locked, lies inside a locked group, or is a group, then a Clone Stamp stroke changes nothing and adds no undo step, and the status bar shows the notice the Brush shows in the same case, whether or not a source point is set. Setting a source point still works.
- [ ] AC-15 (state): While the Clone Stamp is the active tool and a source point is set, a crosshair over the canvas marks the source position: the centre of the source point before the offset is fixed; once the offset is fixed, the pointer position plus the offset, while hovering over the canvas and while painting. The crosshair keeps the same on-screen size at every zoom. Without a source point, or with another tool, no crosshair is shown.
- [ ] AC-16 (always): Each document tab has its own source point and offset. Switching tools, switching layers or switching tabs and back keeps them. Closing the document drops them. They are not saved with the file.
- [ ] AC-17 (event): When a press with Ctrl / ⌘ or with *Set source* lands outside the canvas (on the stage beside it), then the source point is unchanged and no pixel changes.
- [ ] AC-18 (always): Every user-visible string of the tool (tool name, tooltip, options, *Set source*, the status-bar notice of AC-13, and the history step's name where a history panel exists) exists in English and German.
- [ ] AC-19 (event): When an MCP client calls `draw.clone` on an Image document with `points`, `source` (`[x, y]`, the integer canvas pixel copied to the pixel under the first point), and optional `layer` (default: the active layer), the brush arguments of `draw.stroke` (`size`, `hardness`, `opacity`, `shape`, `angle`) and `sample` (`current_layer`, the default, or `all_layers`), then the target layer equals the editor's result after setting that source point and painting one full-pressure stroke through `points`, delivered as one segment, with these settings on that layer. A call that changes the document is exactly one undo step and never merges with another call; a call that changes no pixel (every source position outside the canvas, or every pixel copied onto an equal pixel) adds no undo step.
- [ ] AC-20 (error): If `draw.clone` is called on a Pixel document, without `source`, with a `source` that is not two integers or lies outside the canvas, with an empty `points` list, with `sample` or a brush argument outside its allowed values, or with a target layer that is locked, inside a locked group or a group, then the call fails with `isError: true` (the mode error of `mcp-headless-image` AC-05 for a Pixel document, otherwise a message naming the argument or the layer, as `draw.stroke` does), and the document is unchanged.

## Technical notes

Existing parts to reuse (grep for the names, line numbers drift):

- `Brush::rasterize` in `crates/pincel-image-core/src/tools/mod.rs` already applies the tip, pressure and the selection coverage (`DabPixel.eff`) and hands each pixel to a per-tool operation with its canvas coordinates (`DabPixel.x`, `.y`). A clone operation reads the source image at `(x, y) + offset`.
- `SmudgeStroke` / `RecolorStroke` in `crates/pincel-image-core/src/command/spec.rs` show the shape of a stroke `CommandSpec` and the `StrokeScratch` that keeps per-drag memory across the commands sharing one `stroke_id` (`rewind`). The stroke-start snapshot of AC-08 fits there.
- `SampleSource::{CurrentLayer, AllLayers}` and `render::compose` in `tools/fill.rs` are the existing *Sample* semantics (AC-09).
- The UI side follows Smudge: `smudgeStroke` in `ui/src/modes/image/core/controller.ts`, the `BRUSH_TOOLS` set and the press / move handling in `ui/src/modes/image/tools/pointer.ts`, the rail groups in `ImageEditor.svelte`, keys in `TOOL_GROUP_KEYS` (`ui/src/lib/shell/shortcuts.ts`).
- The on-screen modifier toggles (`ui/src/lib/shell/modifiers.svelte.ts`) offer only Shift and Alt, so a pen or touch user has no Ctrl. *Set source* (AC-04) fills that gap.
- Crosshair look (a design default, not an AC): two perpendicular lines about 16 CSS px across, centred on the source position, drawn as a dark line with a light outline so it shows on any image.
- The overlay (`CanvasOverlay.svelte`) is a canvas. An e2e test checks the crosshair (AC-15) by comparing element screenshots, never by reading pixels from app code (AGENTS.md §9). Note the Gotcha "Canvas screenshots include overlays".
- `draw.clone` belongs in `crates/pincel-agent/src/image/draw.rs` next to `draw.stroke`, builds the same `CommandSpec` the editor sends, and is listed in `guide.md`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Source point equals the pixel under the first stroke's press (offset 0) | Every pixel is composited onto itself; opaque pixels stay as they are |
| The stroke paints over its own source area (offset smaller than the brush) | Copies the stroke-start state (AC-08), no repeating smear |
| Source pixels are transparent | Source-over with alpha 0 leaves the destination unchanged |
| Offset so large that every source position is outside the canvas | No pixel changes (AC-11) |
| Ctrl / ⌘ is pressed only after the stroke started | The stroke goes on painting; only the press decides whether a source is set |
| Right-button press with the Clone Stamp | Paints like the left button, because the tool has no second color |
| Ctrl+click while a source is already set | Replaces the source point; the next stroke fixes a new offset (AC-07) |
| Active layer switched between strokes, *Sample: Current Layer* | The next stroke reads from the new active layer at the same offset, because *Current Layer* means the layer being painted, as for the Paint Bucket and the Magic Wand; *All Layers* reaches the other layers |
| Canvas resized or cropped after the source point was set | Source point and offset stay, because they are a tool setting the user chose, and moving them silently would copy from a place the user did not pick; positions now outside the canvas follow AC-11 |
| Stroke cancelled (pointercancel, Escape) mid-drag | Pixels already written stay, as one undo step, as for the Brush |
| Pen with pressure | Size and opacity follow pressure as for the Brush (AC-06) |
| Space-drag or middle-drag while the Clone Stamp is active | Pans the view, sets no source and paints nothing |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The source pixel is composited source-over by the effective coverage, not written as a replacement | Matches how the Brush lays down color, so AC-06 holds and soft tips blend. Photoshop's Clone Stamp in Normal mode behaves this way | If paint.net replaces transparent destination pixels instead, cloning transparency would differ from paint.net |
| A stroke reads the stroke-start snapshot of the source image (AC-08), not the pixels as they change | The result then does not depend on how the browser batches pointer events, as for Smudge's carry, and the tool cannot smear its own output | A user who expects Photoshop's live sampling (repeating patterns when painting towards the offset) sees a single copy |
| ⌘+click replaces Ctrl+click on macOS | On macOS Ctrl+click is the system's secondary click, and ⌘ is the platform's `mod` key everywhere else in Pincel | None known |
| The source point lives in per-tab editor state, not in the document or the undo history | It is a tool setting, like brush size (§6 undoes document changes only), and paint.net does not save it either | None known |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `the Clone Stamp is a rail tool with key C in Image mode only (AC-01)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-02 | `the options bar shares the Brush tip and adds Sample and Set source (AC-02)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-03 | `Ctrl+click sets the source; strokes copy at the fixed offset …`; `on macOS › ⌘+click sets the source, Ctrl+click does not, and the hint names ⌘ (AC-03, AC-13)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-04 | `Set source makes the next press set the source and switches off (AC-04)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-05 | `clone_composites_each_source_pixel_through_the_dabs` (crates/pincel-image-core/src/tools/clone_stamp.rs); `Ctrl+click sets the source; strokes copy at the fixed offset …` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-06 | `a_uniform_source_paints_like_the_brush_in_that_color`, `a_hard_full_opacity_tip_copies_the_source_exactly` (crates/pincel-image-core/src/tools/clone_stamp.rs) | built |        |
| AC-07 | `Ctrl+click sets the source; strokes copy at the fixed offset …` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-08 | `a_stroke_reads_the_source_as_it_was_at_the_press`, `a_split_stroke_matches_the_same_segments_in_one_command`, `a_split_stroke_that_forgets_its_snapshot_differs` (crates/pincel-image-core/src/tools/clone_stamp.rs); `clone_stroke_split_across_commands_equals_one_command`, `clone_stroke_with_a_new_id_reads_the_canvas_afresh` (crates/pincel-image-core/src/command/clone_spec.rs) | built |        |
| AC-09 | `all_layers_copies_the_visible_composite_into_the_target_layer` (crates/pincel-image-core/src/tools/clone_stamp.rs); `clone_stroke_defaults_to_the_current_layer` (crates/pincel-image-core/src/command/clone_spec.rs); `Sample All Layers copies the visible composite into the active layer (AC-09)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-10 | `the_selection_bounds_the_writes_and_feathers_like_the_brush` (crates/pincel-image-core/src/tools/clone_stamp.rs) | built |        |
| AC-11 | `a_source_outside_the_canvas_leaves_the_pixel_unchanged`, `transparent_or_identical_sources_change_nothing` (crates/pincel-image-core/src/tools/clone_stamp.rs) | built |        |
| AC-12 | `clone_stroke_segments_merge_into_one_undo_step` (crates/pincel-image-core/src/command/clone_spec.rs); `clone_is_undoable_byte_for_byte` (crates/pincel-image-core/src/tools/clone_stamp.rs); `Ctrl+click sets the source; strokes copy at the fixed offset …` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-13 | `a stroke without a source paints nothing and says how to set one (AC-13)`; `on macOS › ⌘+click sets the source, Ctrl+click does not, and the hint names ⌘ (AC-03, AC-13)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-14 | `a locked layer refuses the stroke with the Brush notice; the source can still be set (AC-14)` (ui/e2e/image-clone-stamp.spec.ts); `clone_stroke_on_a_locked_layer_is_refused` (crates/pincel-image-core/src/command/clone_spec.rs) | built |        |
| AC-15 | `the crosshair marks the source at a constant size, then follows the pointer (AC-15)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-16 | `each tab keeps its own source through tool, layer and tab switches (AC-16)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-17 | `Ctrl+press or Set source beside the canvas changes nothing (AC-17)` (ui/e2e/image-clone-stamp.spec.ts) | built |        |
| AC-18 | `in German the tool, its options and its notice are translated (AC-18)` (ui/e2e/image-clone-stamp.spec.ts); step name: `clone_stroke_segments_merge_into_one_undo_step` (crates/pincel-image-core/src/command/clone_spec.rs), `image_calls_name_their_step_after_the_editor_action` (crates/pincel-agent/src/tests/history.rs), `the Clone Stamp step is named in English and German (AC-18)` (ui/e2e/image-clone-stamp-names.spec.ts) | built |        |
| AC-19 | `draw_clone_is_listed_for_image_documents`, `draw_clone_equals_the_editors_clone_stamp`, `draw_clone_samples_the_layer_or_the_visible_composite`, `draw_clone_is_one_undo_step_per_changing_call` (crates/pincel-agent/src/tests/image_clone.rs) | built |        |
| AC-20 | `draw_clone_refuses_bad_arguments_and_targets` (crates/pincel-agent/src/tests/image_clone.rs) | built |        |

## Decisions

- 2026-11-05: Aligned cloning only — because paint.net's Clone Stamp has only this behavior (see Out of scope).
- 2026-11-05: A *Set source* toggle in the options bar, in addition to Ctrl / ⌘+click — because §9.4 promises that touch and pen users can reach every tool, and the on-screen modifier toggles have no Ctrl.
- 2026-11-05: The selection limits only where the stroke writes, not where it reads — because a retouch often copies texture from outside the area being repaired, and the Brush and Smudge already mask only the destination.
- 2026-11-05: The Clone Stamp's tool-group key is `C` (owner's choice) — because it is easy to remember, GIMP uses it for its Clone tool, and paint.net's `L` and Photoshop's `S` are taken by Line and Smudge. Pixel mode's Slice tool has no key today and cannot get `C` later (§9.4: the same key means the same thing in both modes).
- 2026-11-05: The MCP tool is a new `draw.clone` (owner's choice), not a `tool` value of `draw.stroke` — because `source` and `sample` mean nothing for the other stroke tools and `color` means nothing for cloning, so a separate schema stays unambiguous for models. The drawing-parity specs give Smudge and Recolor their own tools by the same rule.
- 2026-11-05 (clarification pass): Positions snap with floor to the pixel under the point, for the source point and the press alike — because a click addresses the pixel whose area it lands in, and the brush hands each dab its pixels by integer coordinate, so p + offset is always a pixel.
- 2026-11-05 (clarification pass): The source is composited per dab with build-up, like the Brush, not once per stroke — because AC-06 then holds exactly, and soft and low-opacity tips behave as users know them from the Brush.
- 2026-11-05 (clarification pass): Split-independence (AC-08) and Brush equality (AC-06) are stated for the same segment sequence — because the Brush itself restarts dab spacing at each segment and stamps shared end points twice, so a stroke is only reproducible from its segments, not from its raw point list.
- 2026-11-05 (clarification pass): The Clone Stamp shares the tip controls (angle and tilt angle included) with the other brush tools and keeps *Sample* as its own setting — because the Image editor keeps one set of tip values for every brush tool today, while each sampling tool (Paint Bucket, Magic Wand, Eyedropper) has its own *Sample*.
- 2026-11-05 (clarification pass): The locked-layer notice wins over the missing-source notice (AC-13, AC-14) — because the user has to unlock first in any case, and a source hint would send them the wrong way.
- 2026-11-05 (clarification pass): `draw.clone` takes `angle`, `layer` and the no-op rule of `mcp-parity-drawing-image` AC-13, and `source` as two integers — because `mcp-parity-drawing-image` gives every stroke tool `draw.stroke`'s brush arguments and undo rule, and an integer pixel leaves no doubt which pixel is copied.
- 2026-11-05 (clarification pass): The crosshair's look is a design default in Technical notes, not an AC — because its exact size and colors are visual design, while its presence, position and constant on-screen size (AC-15) are what can be tested.
- 2026-11-05: `draw.clone`'s `sample` takes `current_layer` / `all_layers` — because the other Image tools that sample (`draw.fill`, `select.wand`) use these values, and the MCP tool parity rule (Decision Log 2026-11-05) gives one capability one name across tools.
- 2026-11-06 (spec-build): The core takes the offset, not the source point (`clone_stroke.offset`), and the UI fixes it at the first stroke's press — because the offset is the per-tab tool state of AC-07, and `draw.clone` derives the same offset from `source` and the first point.
- 2026-11-06 (spec-build): A stroke command that changes no pixel returns no command (no undo step) — because AC-19 asks it of `draw.clone`, and an empty step from the editor would be an undo that does nothing.
- 2026-11-06 (spec-build): The stroke-start snapshot is the whole source image (the layer, or the composite for *All Layers*), taken by the stroke's first command and kept in `StrokeScratch` — because the Smudge / Recolor per-drag memory already lives there and a lazy per-tile snapshot is an optimisation the ACs do not need.
- 2026-11-06 (spec-build): The source point lives in a per-tab-id map (`ui/src/modes/image/tools/clone-source.svelte.ts`), dropped when the tab's document is released — because it is tool state, not part of `docInit` mirrored from the core.
- 2026-11-06 (spec-build): The crosshair is an SVG element in the viewport (`CloneCrosshair.svelte`), not drawn on `CanvasOverlay` — because a DOM element keeps its 16 CSS px at every zoom by construction and the e2e test can check its box without screenshots.
- 2026-11-06 (spec-build): The status notice of AC-13 names the platform key in each language (Ctrl / Strg, ⌘) — because German keyboards label Ctrl "Strg", as the other German hints do.
- 2026-11-06 (spec-build): `pointer.ts` was over 800 lines before the tool; its drag-geometry helpers moved to `tools/drag-geometry.ts` (separate refactor commit) so the file stays under the limit.
- 2026-11-06 (spec-build): Closing a tab drops its source (AC-16) by tab id; tab ids never repeat within a page load, so no later document can see it. The e2e test checks the per-tab separation and the source surviving another tab's close; the drop itself is not observable from the UI.
- 2026-11-06 (spec-build) red runs: core — coverage ignored (src_over at alpha 1) failed AC-05/06/10 tests; snapshot retaken per command failed both split tests; All Layers read as the layer failed the AC-09 test; wrapped source coordinates failed AC-11 tests; always returning a command failed the no-change tests; spec — fresh snapshot per command failed the split test, no snapshot clear on a new stroke id failed the new-id test, no `stroke_id` merge failed the one-step test. Agent — `round` instead of `floor` for the first point, no canvas check on `source`, `sample` ignored each failed its test. UI — offset re-fixed every stroke failed AC-07; Set source not switched off failed AC-04; no notice failed AC-13; source press after the lock check, and the missing-source check before it, each failed AC-14; a crosshair scaled with the zoom and one that ignored the pointer each failed AC-15; one source for all tabs failed AC-16; the source press before the on-canvas check failed AC-17; `sample` not sent failed the AC-09 e2e test; *Sample* bound to the Paint Bucket's failed AC-02; an English German tool name failed AC-18; no `C` key failed AC-01.
- 2026-11-06 (spec-check rework) red runs: `clone-press.ts` with `e.ctrlKey` and `clone.noSource` on every platform failed the macOS e2e test (`⌘+click sets the source, Ctrl+click does not, and the hint names ⌘`); `CloneStamp::stroke_with_pen` without `.with_kind(HistoryKind::CloneStamp)` failed `clone_stroke_segments_merge_into_one_undo_step` and `image_calls_name_their_step_after_the_editor_action` ("draw_pixels" instead of "clone_stamp"). The step's English and German names (`history.clone_stamp`) are pinned by `ui/e2e/image-clone-stamp-names.spec.ts`; an English German label failed it. The optional save-as-ORA / reopen e2e for AC-16 was left out: it needs the faked file-system picker and a reload flow, and adds little over the per-tab test.
