---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6.1, pincel.md §6.2, pincel.md §6.3, pincel.md §9.3, pincel.md §15 (2026-10-27 Image mode gains Gradient, Recolor and Smudge), pincel.md §15 (2026-11-05 MCP tool parity), pincel.md §15 (2026-11-05 History entries carry a kind from the core, on branch feat/history-panel)]
depends_on: []
---

# Image-mode live objects: core

## Context

In paint.net a shape, a line or a gradient stays editable after the drag that drew it: handles move its corners or endpoints, and the toolbar and the colors change its style, until the user commits it. Pincel's Image mode rasterizes at once: the pointer-up of a Line, Shapes or Gradient drag sends one `draw_shape` / `draw_gradient` command (`ui/src/modes/image/core/controller.ts` `drawShape` / `drawGradient`), and during the drag the overlay only draws a rubber band (`shapePreview` in `ui/src/modes/image/stores/editor.svelte.ts`), so the pixels appear only on release and can never be changed afterwards.

This is the owner's paint.net parity goal for Image mode (2026-11-05). The topic is split in two specs, each at most L:

- **This spec** — the core side in `pincel-image-core` / `pincel-image-wasm`: the live object, how it is drawn, how its edits, commit and cancel go through the undo history, and which other actions commit it.
- **`image-live-objects-ui`** — what the user sees and does: handles, toolbar and color changes, commit triggers, Escape, and the interplay with save, autosave, tabs and effects. It depends on this spec.

The two follow-ups `image-live-text` (live Text) and `image-shapes-curves` (Bézier curves, more shapes) build on the same live object and are out of scope here.

Where the uncommitted object lives was decided by the owner on 2026-11-05 (Decisions): the core draws the object into its layer at every change and keeps its parameters beside the document on the command bus. The same model serves `image-live-text` and `image-transform-handles`.

**Terms.**

- *Object kind*: Line, Gradient and the box shapes, plus the kinds later specs add on the same record: Text (`image-live-text`) and Selection transform (`image-transform-handles`). This spec specifies the first three; the others follow its rules unless their spec says otherwise.
- *Box shape*: every shape of the Shapes group that is defined by a box (two opposite corners) — today Rectangle, Rounded Rectangle, Ellipse and Polygon; `image-shapes-curves-shapes` adds more.
- *Parameters*: what the immediate draw command of that kind takes today — for shapes and Line the geometry (two points: endpoints, or opposite corners of the box; a Polygon is the regular polygon inscribed in that box, as `drawShape` builds it today) and the style (mode, stroke width, stroke and fill colors, dash, anti-alias, corner radius, sides); for Gradient the two drag points, kind, channel, the two colors and the opacity.
- *Live object*: an object of a kind whose parameters can still change. A document has at most one live object of any kind at a time: beginning one of any kind commits the one that is live (AC-08), whatever its kind.
- *Pre-object pixels*: the layer's pixels before the live object was begun.
- *Immediate draw*: the result of today's `draw_shape` / `draw_gradient` with the same parameters on the same pixels and selection.
- *Object step*: an undo step that begins or edits the live object.
- *Commit*: the object stops being live; its pixels stay. *Cancel*: the object disappears as if it had never been drawn.

## Scope

### In scope

- A live object of each kind on a raster layer, begun, edited (geometry, style, and the shape among the box shapes), committed and cancelled through the core
- Its pixels in the layer, the composite, the render cache, saves and exports, as the immediate draw would give them
- Object steps on the undo history: undo / redo of single edits, folding into one step on commit, removal on cancel, the undo capacity
- Committing the live object before every other document change and before an active-layer change, whoever issues it (tool, menu, effect, paste, MCP call)
- Reading the live object's kind, layer and parameters through `pincel-image-wasm`, so the UI can draw handles and hit-test them
- Benchmarks for re-rasterizing a live shape and a live gradient

### Out of scope

- Handles, pointer input, toolbar, commit triggers in the UI and Escape — because `image-live-objects-ui` owns everything the user operates
- Live Text — because `image-live-text` builds on this spec and needs font layout of its own
- Bézier curves, arrows and other new shapes, rotating a shape — because `image-shapes-curves` owns new geometry; today's `Shape` enum is axis-aligned and has no curve
- Live objects in Pixel mode — because Pixel mode's Line and Rectangle tools follow Aseprite, which commits at once
- Storing a live object in `.ora` or autosave so that it can be edited after reopening — because ORA has no field for it and paint.net does not keep it either; a reopened document has the object as plain pixels (AC-13)
- MCP tools for live objects — because an agent's draw call is final (Decision Log 2026-11-05 "MCP tool parity" maps a capability to the result of the command the UI issues, which is the committed draw); an agent's document change commits the user's live object (AC-08)
- Changing an object's layer while it is live — because changing the active layer commits it (AC-08), as in paint.net

## Acceptance criteria

- [ ] AC-01 (event): When a live object is begun on a layer with given parameters, then the layer's pixels and the composite equal the immediate draw of those parameters, one undo step is added, and the core reports a live object with that kind, layer and parameters. This holds also when the object changes no pixel (it lies off the canvas, outside the selection, or the selection is empty): it is live and can be moved into view; AC-12 applies if it is committed so.
- [ ] AC-02 (event): When the live object's parameters are edited (any geometry or style field), then the layer's pixels equal the immediate draw of the new parameters on the pre-object pixels — no trace of earlier parameters remains, and a Transparency-channel gradient fades the pre-object pixels, not the previous ramp — and one undo step is added.
- [ ] AC-03 (event): When several edits are sent between the start and the end of one gesture (one handle or body drag, press to release), then they form one undo step; the gesture also ends at any edit outside it, an Undo, Redo, commit or cancel. When consecutive edits outside a gesture each change only the same *parameter group* with no other step in between, then they also form one undo step. The groups are: geometry; shape kind; mode; stroke width; dash; anti-alias; corner radius; sides; colors (stroke and fill color including their alpha, which carries the tool opacity of shapes and Line); gradient kind; gradient channel; gradient colors; gradient opacity.
- [ ] AC-04 (event): When Undo runs while the top step is an object step, then the object's pixels and reported parameters return to those before that step and the object stays live; Redo re-applies it. When Undo reverts the step that began the object, then the layer shows the pre-object pixels and no object is live; Redo makes it live again with the parameters it had.
- [ ] AC-05 (event): When the live object is committed, then the layer's pixels do not change, no object is live, and the object's steps are replaced by one undo step: one Undo restores the pre-object pixels and Redo restores the committed pixels, with no object live after either.
- [ ] AC-06 (event): When the live object is cancelled, then the layer shows the pre-object pixels, the object's steps are gone from the undo history, the redo history is empty, and the undo depth equals the depth before the object was begun, less any steps below the object that the capacity dropped meanwhile (AC-14); Cancel does not bring those back.
- [ ] AC-07 (event): When the live object's kind is edited from one box shape to any other box shape of the Shapes group, then it keeps its box and every style field, including those the old kind ignored (a Rectangle turned into a Polygon uses the sides value the object was begun or last edited with), and the pixels equal the immediate draw of the new kind, as one undo step. If the edit asks for a change between a box shape, Line and Gradient, then it is refused with an error and nothing changes.
- [ ] AC-08 (always): Any document change other than an edit, commit or cancel of the live object, an Undo or a Redo — a pixel tool, a layer, selection, canvas or image command, an effect or adjustment, a paste, a cut, an MCP tool call, beginning another live object — and any change of the active layer to a different layer first commits the live object: afterwards the history holds the object's committed step (or none, AC-12) below the new change's step, and the new change sees the object's pixels. Setting the active layer to the layer that is already active commits nothing. Undo and Redo never commit; while an object is live, the top undo step and the top redo step (if any) are its own steps (AC-04).
- [ ] AC-09 (error): If a live object is begun on a locked layer, a group or a layer index that does not exist, then it is refused with the same error a draw command gets today (`"layer is locked"` for a locked layer) and no step is added; if its geometry is degenerate (a zero-length Line or Gradient, a box of zero width or height), then nothing is begun, no step is added and no object is live.
- [ ] AC-10 (always): A live object keeps its full geometry wherever it is: moved partly or entirely off the canvas and back, its pixels equal the immediate draw at the final position. If an edit would put any of its points more than 32 768 px outside the canvas, then the edit is refused with an error and the object keeps its previous parameters.
- [ ] AC-11 (always): A live object is clipped by the selection as the immediate draw is, feathered edges included; since a selection change commits it (AC-08), the clip it is edited with is the selection it was begun with.
- [ ] AC-12 (event): When a live object is committed whose pixels equal the pre-object pixels (off the canvas, fully outside the selection, an empty selection, or degenerate geometry after an edit), then its steps are removed and no step is added, as for a draw command that changes nothing today.
- [ ] AC-13 (always): The core's save, export, autosave encoding, copy and composite calls, run while an object is live, contain the object as currently drawn and by themselves neither commit it nor change the undo history; whether an action commits first is the editor's rule (`image-live-objects-ui` AC-09: Save, Save As, Export and Copy commit, autosave does not). a project saved while an object is live reopens with the object as plain pixels and no live object.
- [ ] AC-14 (always): The undo capacity never drops a live object's steps: while the history is full, the oldest steps below the object are dropped first; if the object's own steps exceed the capacity, its oldest edit steps fold into the step that began it, so Cancel (AC-06) still restores the pre-object pixels.
- [ ] AC-15 (always): After every begin, edit, undo, redo, commit and cancel, the region composite the viewport renderer reads equals a fresh compose of the document, and the reported live object (kind, layer, parameters, or none) matches the document state.
- [ ] AC-16 (error): If an edit, commit or cancel is sent while no object is live, then it is refused with an error and nothing changes; if an edit carries a parameter outside the range the draw command accepts, then it is clamped as the draw command clamps it today. An edit that makes the geometry degenerate (an endpoint dragged onto the other, a box of zero width or height) is accepted: the object stays live and draws nothing until a later edit makes it non-degenerate.
- [ ] AC-17 (event): When `cargo bench -p pincel-image-core --bench compose` runs, then it also reports a median for one edit that moves a live 1024 × 1024 filled, anti-aliased ellipse with a 4 px outline by 1 px on a 4096 × 4096, 5-layer document, and for one edit that moves the end point of a live linear gradient by 1 px on a 2048 × 2048, 5-layer document, each including the region compose of what changed.

## Technical notes

Hints, not contracts, verified 2026-11-05; grep for the names, line numbers drift.

- **Drawing today.** `CommandSpec::DrawShape` / `DrawGradient` (`crates/pincel-image-core/src/command/spec.rs`) build `Shapes::new(shape, style).draw(layer, &doc)` (`tools/shapes.rs`) and `Gradient::draw(layer, a, b, &doc)` (`tools/gradient.rs`), each returning a `SetPixels` (before / after of the changed rect, `command/pixels.rs`) or `None` for a no-op. A live edit can be "revert the current object's `SetPixels`, which restores the pre-object pixels of its rect, then draw the new parameters" — no full-layer snapshot is needed, and the pre-object pixels of a growing shape are still the layer's pixels outside the old rect.
- **Where the record lives.** A `live: Option<LiveObject { kind, layer, params, ... }>` on `CommandBus` (`command/mod.rs`), not on `Document`, keeps the codecs (`codec/ora`) untouched (AC-13). `UndoStack::push` already merges via `Command::merge_with`; folding on commit (AC-05) and removal on cancel (AC-06) need a way to replace the top n entries. `UndoStack::push` drops `undo.remove(0)` when full — AC-14 changes which entry goes.
- **Gestures (AC-03).** The UI can pass a gesture id on the edits of one drag and end the gesture on pointer-up; `SetPixels::merge_with` by `stroke_id` is the existing pattern.
- **Commit before others (AC-08).** One check in `CommandBus::apply` (and in `pincel-image-wasm` paths that bypass `CommandSpec`: `apply_effect`, `paste_layer`, `cut_selection`, `set_active_layer`) covers the editor, the menus, effects and the MCP tools of `pincel-agent`, which go through the same core.
- **Damage.** A shape edit damages the union of the old and new object rects; a gradient fills the layer inside the selection, so its edits damage that whole region (`command/damage.rs` `Damage::Rects`). The render cache (`performance-image-cache`) must see both on undo too (AC-15).
- **wasm (the API shape is spec-build's choice within the Decision Log row).** Either new `CommandSpec` variants (`begin_live` with the `draw_shape` / `draw_gradient` fields, `edit_live` with a gesture id and the changed fields, `commit_live`, `cancel_live`) so the ts-rs mirror in `ui/src/modes/image/core/generated/` follows, plus `live_object(handle)` returning JSON or `null`; or the same as separate `#[wasm_bindgen]` functions. The live object can also join `get_document_info`.
- **History kinds.** Once `history-panel` lands (Decision Log 2026-11-05 "History entries carry a kind from the core"), every object step and the folded step report the kind of the object's tool (Line, Rectangle, …), as paint.net names them; the folded step replaces the object's rows.
- **Benchmark (AC-17).** `crates/pincel-image-core/benches/compose.rs` already has the `image_*` groups of `performance-benchmarks`; a budget for the two new scenarios (proposal: 8 ms for the ellipse, 16 ms for the gradient, native) can join that spec's budget list as `pending`.
- **Follow-ups.** `image-live-text` and `image-shapes-curves` add object kinds with their own parameters (text runs, control points); the record should hold kind-specific parameters rather than a fixed shape struct.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Begin a second live object while one is live | The first is committed (one step), then the second begins (AC-08) |
| Undo past the begin step, then draw something else | The redo branch, including the object, is discarded as for any new edit |
| Undo past the begin step, then Commit or Cancel | Refused: no object is live (AC-16) |
| Live gradient in the Transparency channel, edited ten times | Fades the pre-object pixels once, never compounding (AC-02) |
| Live object on a layer that is then hidden or has its opacity changed | Changing a layer property is a document change: the object is committed first (AC-08) |
| Live object moved entirely off the canvas, then committed | No step remains (AC-12) |
| Live object moved off the canvas and back before commit | Drawn whole at the final position (AC-10) |
| 150 handle drags on one live object (capacity 100) | The oldest edits fold into the begin step; Cancel restores the pre-object pixels (AC-14) |
| The layer is deleted while an object is live | Deleting is a document change: the object is committed first, then the layer goes; Undo restores the layer with the committed object |
| An MCP tool call in live desktop mode while the user has a live object | The object is committed first; the agent's change sees its pixels (AC-08) |
| Polygon box that is not square | The regular polygon stays inscribed in the shorter side, centred, as `drawShape` builds it today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Undo after a commit removes the whole object in one step and does not reopen it for editing | The topic says commit is one step; reopening would need the parameters kept on a committed step and a second notion of "live" | Low: a later spec can add "edit again" without changing this one |
| Cancel cannot be undone | Cancel is an explicit "never mind"; keeping a redo of a cancelled object would put a step on the history that the user just removed | Low |
| Edits of one style field merge while nothing else happens in between | A toolbar slider sends a value per input event; one step per value would flood the 100-step history, as `SetLayerOpacity` merging already avoids | Low |
| The live record is not stored in autosave or `.ora` | Recovery restores pixels; an editable shape after a crash is not a paint.net behavior | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `crates/pincel-image-core/tests/live_objects.rs` `begin_live_draws_the_immediate_draw_and_adds_one_step`, `begin_live_off_canvas_is_live_without_changing_pixels` | built |        |
| AC-02 | `crates/pincel-image-core/tests/live_objects.rs` `edit_live_redraws_on_the_pre_object_pixels`, `edit_live_transparency_gradient_fades_the_pre_object_pixels_once` | built |        |
| AC-03 | `crates/pincel-image-core/tests/live_objects.rs` `edit_live_of_one_gesture_is_one_step`, `edit_live_gesture_ends_at_undo_and_at_other_edits`, `edit_live_consecutive_edits_of_one_group_merge` | built |        |
| AC-04 | `crates/pincel-image-core/tests/live_objects.rs` `undo_and_redo_walk_the_live_steps` | built |        |
| AC-05 | `crates/pincel-image-core/tests/live_objects.rs` `commit_live_folds_the_steps_into_one` | built |        |
| AC-06 | `crates/pincel-image-core/tests/live_objects.rs` `cancel_live_restores_the_pre_object_state` | built |        |
| AC-07 | `crates/pincel-image-core/tests/live_objects.rs` `edit_live_converts_between_box_shapes_only`; `crates/pincel-image-core/src/command/live_spec.rs` `live_object_spec_reports_the_wire_parameters_of_a_live_shape` | built |        |
| AC-08 | `crates/pincel-image-core/tests/live_objects.rs` `other_document_changes_commit_the_live_object_first`, `set_active_layer_commits_only_on_a_change`, `undo_and_redo_never_commit`; `crates/pincel-image-wasm/src/lib.rs` `live_tests::paths_beside_apply_command_commit_the_live_object_first` | built |        |
| AC-09 | `crates/pincel-image-core/tests/live_objects.rs` `begin_live_is_refused_on_locked_group_and_missing_layers`, `begin_live_with_degenerate_geometry_begins_nothing`; `live_spec.rs` `begin_live_on_a_locked_layer_is_refused_like_a_draw` | built |        |
| AC-10 | `crates/pincel-image-core/tests/live_objects.rs` `live_object_moved_off_canvas_and_back_is_drawn_whole`, `edit_live_beyond_the_reach_is_refused` | built |        |
| AC-11 | `crates/pincel-image-core/tests/live_objects.rs` `live_object_is_clipped_by_a_feathered_selection` | built |        |
| AC-12 | `crates/pincel-image-core/tests/live_objects.rs` `commit_live_that_changes_no_pixel_leaves_no_step` | built |        |
| AC-13 | `crates/pincel-image-core/tests/live_objects.rs` `save_and_composite_while_live_neither_commit_nor_touch_history`; `crates/pincel-image-wasm/src/lib.rs` `live_tests::save_and_export_while_live_do_not_commit` | built |        |
| AC-14 | `crates/pincel-image-core/tests/live_objects.rs` `capacity_drops_steps_below_the_object_first` | built |        |
| AC-15 | `crates/pincel-image-core/tests/live_objects.rs` `render_cache_matches_a_fresh_compose_after_every_live_operation` | built |        |
| AC-16 | `crates/pincel-image-core/tests/live_objects.rs` `live_operations_without_a_live_object_are_refused`, `edit_live_clamps_like_the_draw_command`, `edit_live_to_degenerate_geometry_stays_live_and_draws_nothing` | built |        |
| AC-17 | `crates/pincel-image-core/benches/compose.rs` `image_live/ellipse_move_1024_4k_5layers` (median 200 ms native, 2026-11-05), `image_live/gradient_end_move_2k_5layers` (median 767 ms) | built |        |

## Decisions

- 2026-11-05 — Split from the UI part (`image-live-objects-ui`) because the whole feature (new core module, wasm surface, generated types, overlay, pointer, toolbar, editor commit hooks, e2e) is larger than L; the split follows the `vectorize-core` / `vectorize-dialog` precedent.
- 2026-11-05 — Changing the shape among the box shapes converts the live object (AC-07), because paint.net changes a live shape's type from its toolbar and every box shape shares the same box; Line and Gradient have different parameters, so a change to or from them is not a conversion.
- 2026-11-05 — An agent's document change commits the user's live object instead of being refused (AC-08), because it is the same rule a menu command follows and refusing would make live MCP calls fail for reasons the agent cannot see.
- 2026-11-05 — Architecture: the core draws the live object into its layer at every change, keeps its parameters beside the document on `CommandBus` (not in `.ora`), records every edit as a normal undo step, folds the object's steps into one on commit, removes them on cancel, and commits before any other command or active-layer change (option (c)). Owner's choice, because one undo stack and one render path let save, autosave, export, effects, MCP and the history panel see exactly what the canvas shows, and the same record serves `image-live-text` and `image-transform-handles`.
- 2026-11-05 — Escape cancels and Enter commits a live object in every live-object spec (for live text: Ctrl+Enter commits, Enter is a new line). Owner's choice, because one rule across shapes, gradients, text and transforms is predictable; the keys themselves are `image-live-objects-ui`'s.
- 2026-11-05 (clarification) — A begin that changes no pixel still makes the object live (AC-01), and an edit to degenerate geometry is accepted (AC-16), because the user can still drag the object into view or out again, and a handle that refused to pass through zero would stick mid-drag. Committing such an object leaves no step (AC-12).
- 2026-11-05 (clarification) — Merging of consecutive option edits works on parameter groups, and the tool opacity of shapes and Line falls into the colors group (AC-03), because the opacity travels in the stroke and fill alpha the draw command takes; a color change right after an opacity change merges with it and still undoes to the state before both.
- 2026-11-05 (clarification) — Geometry is bounded at 32 768 px outside the canvas (AC-10), because a bound far beyond any viewport keeps `f32` canvas coordinates exact without reshaping the object.
- 2026-11-05 (clarification) — The core never commits on save, export, copy or autosave by itself (AC-13); the editor decides (`image-live-objects-ui` AC-09), because only the editor knows whether a call is the user's Save or the autosave timer. Cut changes the document and commits in the core (AC-08).
- 2026-11-05 (spec-build) — Wire API: `CommandSpec` variants `begin_live_shape`, `begin_live_gradient`, `edit_live` (optional fields plus a `gesture` id), `commit_live`, `cancel_live`, and `live_object(handle)` in `pincel-image-wasm` returning a ts-rs-generated `LiveObjectSpec` or `null`, because the command spec already carries every draw command and its TypeScript mirror keeps the UI in sync. Core API (new public items): `CommandBus::{begin_live, edit_live, commit_live, cancel_live, live_object, set_active_layer}`, `LiveParams` / `LiveShape` / `LiveShapeKind` / `BoxShape` / `LiveGradient` / `LiveEdit` / `LiveObject`, `command::live_spec`, `DocumentError::{LayerLocked, NoLiveObject, LiveEditMismatch, LiveGeometryOutOfRange}`.
- 2026-11-05 (spec-build) — The object is live exactly while the top undo steps are its steps (`LiveStep`); each step keeps a shared snapshot of the layer from before the object and redraws from it in both directions, because that makes undo, redo, merge and the capacity fold stateless and keeps one copy of the pre-object pixels per object.
- 2026-11-05 (spec-build) — A begin on a missing layer is refused with `LayerIndexOutOfBounds`, and a begin on a locked layer or a group with `LayerLocked` ("layer is locked") also in the core, not only in the wasm `refused` check, so every host (editor, MCP) gets the same refusal (AC-09).
- 2026-11-05 (spec-build) — An edit whose result equals the current parameters adds no step, because a step that changes nothing would make Undo look broken (same reason as `image-live-objects-ui` AC-07).
- 2026-11-05 (spec-build) — Degenerate begin commits the previously live object (it is a begin attempt) and then begins nothing, because AC-09 asks that afterwards no object is live.
- 2026-11-05 (spec-build) — Test order: the core module was written before its tests in this block. The red runs were (1) the tests against the core without the live API (105 compile errors: no `begin_live` / `edit_live` / … ), and (2) deliberate temporary breaks, each reverted: no commit in `CommandBus::apply` (AC-08 red), no capacity fold (AC-14 red), no restore of the pre-object pixels / no merge / no clamp (AC-02, AC-03, AC-04, AC-10, AC-11, AC-16 red), commit and cancel keeping steps and pixels / no reach, lock or degenerate check (AC-06, AC-09, AC-10, AC-12 red), no damage of live steps (AC-15 red), the wasm `set_active_layer` bypassing the bus (wasm AC-08 test red).
- 2026-11-05 (spec-build) — AC-17's medians are far above the proposed 8 ms / 16 ms budgets; not profiled further — the region compose of five layers in linear light is the likely main cost, judging by the `image_compose` medians. No budget row was added; the compose cost belongs to the performance specs.
