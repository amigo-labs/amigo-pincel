---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6.2, pincel.md §9.2, pincel.md §9.4, pincel.md §10.3, pincel.md §15 (2026-11-04 Several documents per editor), pincel.md §15 (2026-11-04 effect panel)]
depends_on: [image-live-objects]
---

# Image-mode live objects: editing in the UI

## Context

`image-live-objects` gives Image mode's core a live object: a Line, a Gradient or a box shape that is drawn into its layer but stays editable until it is committed. This spec is what the user sees and does with it, after paint.net: the drag that draws the object leaves it live with handles; handles, the toolbar and the colors change it; Enter, another tool, a new object, a press beside it, or any action that needs the final pixels commits it; Escape cancels it.

Today the pointer-up of such a drag commits at once (`ui/src/modes/image/tools/pointer.ts` → `drawShape` / `drawGradient` in `core/controller.ts`), the overlay draws only the rubber band of the drag in progress (`shapePreview`), and Escape cancels a drag or deselects (`image-editing` AC-31).

Terms are those of `image-live-objects` (*live object*, *object kind*, *parameters*, *commit*, *cancel*), plus:

- *Object's tool*: the rail tool that draws the object kind (Line, the Shapes tools, Gradient).
- *Box shapes*: every shape of the Shapes group that is defined by a box (two opposite corners) — today Rectangle, Rounded Rectangle, Ellipse and Polygon; `image-shapes-curves-shapes` adds more.
- *Handles*: the grab points drawn over a live object: for a box shape its four corners and four edge midpoints, for a Line and a Gradient its two endpoints, and for a Line also its *move grip*.
- *Move grip*: a handle of a live Line, drawn 16 CSS px beside the midpoint of the segment, perpendicular to it; dragging it moves the whole line.
- *Body*: for a box shape the inside of its box; for a Line or a Gradient the points within 8 CSS px of the segment between its endpoints, or within half the stroke width as shown on screen if that is wider (for a Line: its *stroke*). This is the rule until `image-shapes-curves` lands, which then owns a Line's hit area.
- *Press*: a primary-button mouse press, a touch or a pen contact.

## Scope

### In scope

- The live state after a Line, Shapes or Gradient drag, with handles drawn over the canvas and the stage
- Moving and resizing by handles, by the body of box shapes and gradients, and by a line's move grip, with the Shift constraints of drawing
- Toolbar options, foreground / background colors and the Shapes-group variant applying to the live object
- Commit triggers: Enter, tool change, new object, press beside the object, active layer, tab switch, document-changing actions, effects, save, export, copy, closing
- Escape, Undo and Redo while an object is live
- Autosave, unsaved state and the close prompt while an object is live

### Out of scope

- The core model, undo folding and the commit-before-other-commands rule — because `image-live-objects` owns them
- Live Text, Bézier curves, arrows, more shapes — because `image-live-text` and `image-shapes-curves` build on this spec
- Rotating a live shape — because the core shapes are axis-aligned and paint.net's rotation handle belongs with the new geometry of `image-shapes-curves`
- Nudging a live object with the arrow keys — because nudging is a separate gap shared with Move Selected Pixels and `image-transform-handles`, and one key rule should serve all of them
- Editing an object again after it is committed — because commit is one final step (`image-live-objects` Assumptions)
- Right-button drawing or editing of live objects — because Line, Shapes and Gradient respond to the primary button only today (`pointer.ts`), and swapped-color drawing is a separate gap
- Pixel mode — because its Aseprite-style Line and Rectangle commit at once

## Acceptance criteria

- [ ] AC-01 (event): When a Line, Gradient or box-shape drag ends, then the object is shown in the canvas as it will be committed, it stays live, and its handles are drawn over it; the drag added one undo step.
- [ ] AC-02 (event): When the user drags a handle of a live object, then the canvas shows the object following the pointer during the drag: a corner handle moves that corner, an edge handle moves only that edge, an endpoint handle moves only that endpoint, and a corner or edge dragged past the opposite one flips the box instead of collapsing it. The whole drag is one undo step.
- [ ] AC-03 (event): When the user holds Shift while dragging a handle, then a box shape's box becomes a square anchored at the opposite corner (corner handles), and a Line or Gradient endpoint snaps the segment to the nearest multiple of 45° around the other endpoint — the same constraints as drawing (`image-editing` AC-18).
- [ ] AC-04 (event): When the user drags the body of a live box shape or Gradient (not on a handle), or the move grip of a live Line, then the whole object moves by the drag offset, keeping its size and style; one undo step. A drag on a live Line's stroke does not move the line: it is reserved for inserting a control point (`image-shapes-curves`), and until that spec lands it changes nothing and adds no step.
- [ ] AC-05 (always): A press within 8 CSS px of a handle's centre (16 CSS px for touch and pen) grabs that handle at every zoom level, and handles keep the same on-screen size at every zoom; where two handles are in reach, the nearer one is grabbed. Handles that lie on the stage beside the canvas are drawn and grabbed there as well.
- [ ] AC-06 (event): When the user changes a toolbar option while an object is live — mode (outline / fill / both), stroke width, dash, anti-alias, corner radius, polygon sides, opacity for shapes and Line; kind, channel and opacity for Gradient — then the live object takes the new value at once and stays live; each change is one undo step, except that consecutive changes of the same option merge into one step, as do a slider's moves without letting go (`image-live-objects` AC-03). The toolbar shows only the options of the active tool, as today, so an option the live kind ignores cannot be changed.
- [ ] AC-07 (event): When the user changes the foreground or background color, or swaps them, while an object is live, then the object takes the new colors (shapes and Line: outline = foreground, fill = background; Gradient: start = foreground, end = background), as one undo step that merges with directly preceding color or opacity changes (`image-live-objects` AC-03), and stays live. A color change while a Transparency-channel Gradient is live is not applied to it and adds no step, because that channel ignores the colors.
- [ ] AC-08 (event): When the user picks any other box shape of the Shapes group while a box shape is live (`U` cycling, the rail or the tool flyout), then the live object becomes that shape with the same box and style, as one undo step, and stays live with its handles.
- [ ] AC-09 (event): When, while an object is live, the user presses Enter (focus not in a text field), picks any tool other than the object's tool (AC-08 aside), presses with the object's tool outside its handles and body, presses on the stage outside its handles and body, changes the active layer, switches to another document tab, runs any action that changes the document (a menu item, shortcut, layers-panel action including a confirmed rename, selection action, paste), opens an effect or adjustment, runs Copy, Cut or Copy Merged, or runs Save, Save As or Export, then the object is committed before that action takes effect: the handles disappear, the pixels stay as shown, and the object leaves one undo step, or none if it changes no pixel (`image-live-objects` AC-12). View actions do not commit: Space-drag, middle-button drag, pinch and wheel zoom keep the object live, and a right-button press does nothing.
- [ ] AC-10 (event): When the user presses with the object's tool outside a live object's handles and body and drags, then the live object is committed and the drag draws a new live object; a press without a drag only commits.
- [ ] AC-11 (event): When the user presses Escape (no dialog open, focus not in a text field) during a handle or body drag, then that drag is undone without a redo step, the object returns to how it was before the drag and stays live; Undo pressed during such a drag (keyboard, header button or two-finger tap) does the same and undoes nothing more; when the user presses Escape while an object is live and no drag is in progress, then the object is cancelled: the canvas shows the layer as before the object was drawn, no undo step of the object remains, and a present selection is kept (it is not deselected).
- [ ] AC-12 (event): When the user presses Undo while an object is live, then the last change to it is undone and the handles follow; once the drag that drew it is undone, the object and its handles disappear; Redo brings each step back, and redoing the drawing step shows the object live again with its handles.
- [ ] AC-13 (state): While an object is live, the pointer over a handle shows a resize cursor matching its direction (`nwse-resize`, `nesw-resize`, `ns-resize`, `ew-resize` for box shapes; `crosshair` for a Line or Gradient endpoint), over the move grip and over the body of a box shape or Gradient the `move` cursor; elsewhere, including a Line's stroke, the tool's usual cursor.
- [ ] AC-14 (state): While an object is live, autosave runs as usual and does not commit it — the handles stay and the undo history is unchanged; a document recovered from that autosave contains the object as plain pixels, with no live object.
- [ ] AC-15 (event): When the user closes a tab or the app while an object is live, then the unsaved-changes prompt lists the document (drawing the object made it unsaved); Save commits and then saves, so the file contains the object; Don't Save discards it with the document.
- [ ] AC-16 (error): If a Line, Shapes or Gradient drag starts on a locked layer or on a group layer, then no object becomes live and the existing locked-layer notice appears (`image-layer-groups` AC-12 for groups); if a drag ends with a degenerate object (zero length, or a box of zero width or height), then no object becomes live and no undo step is added.
- [ ] AC-17 (event): When the user cancels an object (AC-11) in a document that had no unsaved changes before the object was drawn, then the document is shown as having no unsaved changes again.
- [ ] AC-18 (state, manual: visual design): While an object is live, its handles are visible against light, dark and transparent (checkerboard) backgrounds at every zoom, use the theme's `pc-*` tokens, and the body of a Line or Gradient is shown by a thin guide line between its endpoints.
- [ ] AC-19 (error): If the core refuses an edit, commit or cancel because no object is live any more (an MCP call or another path committed it since the editor last synced), then the editor drops the handles and any drag in progress, shows no error notice, and the document stays as the core has it.

## Technical notes

Hints, not contracts; verified 2026-11-05, line numbers drift.

- **Pointer input.** `ui/src/modes/image/tools/pointer.ts`: the drag start for shapes and the gradient (`shapeOf`, `constrainShape` for Shift), the pointer-up commit (`drawShape` / `drawGradient`), `gestureControl.cancel()` (Escape), stage presses (`image-editing` AC-29 – AC-32) and `toCanvasPoint()` (unclamped, so handles on the stage map to canvas space). Hit-testing for handles and body happens before a new drag starts. A handle drag sends edits with one gesture id (`image-live-objects` AC-03); coalescing them to one per animation frame keeps a 4096 × 4096 gradient drag responsive.
- **Overlay.** `components/canvas/CanvasOverlay.svelte` already draws the rubber band from `shapePreview` (`stores/editor.svelte.ts`); the handles come from the core's reported live object (kind, parameters), not from UI-side copies, so Undo and MCP changes move them (AC-12). Handles are DOM / overlay drawing only; app code never reads canvas pixels (AGENTS.md §9).
- **Options.** `components/toolbar/ToolOptions.svelte` binds `tool.shapeMode`, `strokeWidth`, `shapeDash`, `shapeSides`, `cornerRadius`, `shapeAntiAlias`, `opacity`, `gradientKind`, `gradientChannel` straight to the `tool` store; a live object needs these changes forwarded to the core (AC-06) and the color store's changes and `swapColors` likewise (AC-07). When an object becomes live by Undo / Redo, the toolbar keeps the user's current values; only the next change is sent.
- **Commit points.** `ImageEditor.svelte` (`runShortcut`, the menus, Escape handling at `e.key === 'Escape'`), `core/controller.ts` (`selectLayer`, `save`, effects, copy / paste), `core/tabs.svelte.ts` (tab switch), the tool rail and `cycleGroup` for tool changes. The core already commits before any other command and before `set_active_layer` (`image-live-objects` AC-08); the UI still commits explicitly where no command follows (Enter, tool change, tab switch, save, export, opening an effect) so that the handles go away and the history shows one step.
- **Unsaved state.** `editor` compares `undoDepth` with the depth at the last save; Cancel returns the depth to its value before the object (`image-live-objects` AC-06), which gives AC-17.
- **E2e.** `ui/e2e/` with `createDocument` / `waitForEditor` and the drag helpers in `helpers.ts`; pixels are checked through element screenshots or the core (§7.4). Deselect is its own undo step in Image mode (AGENTS.md Gotchas), so count steps after it. Canvas screenshots include the overlay: hide or commit the handles before comparing pixels (AGENTS.md Gotchas, "Canvas screenshots include overlays").
- **History panel.** If `history-panel` lands first, its AC-11 ("while … another uncommitted edit is open, a jump does to it first what Undo does") is satisfied by AC-12 here: live steps are ordinary steps.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Live object dragged entirely onto the stage | Handles stay on the stage and can bring it back; committed there it adds no step (`image-live-objects` AC-12) |
| Two handles overlap (a tiny box at low zoom) | The nearer handle wins (AC-05); a press exactly between them takes the corner |
| Press or drag on the stroke of a live Line with the Line tool | Never moves the line, starts a new one or commits (AC-04): once `image-shapes-curves` lands it inserts a control point, until then it does nothing; only the move grip moves the whole line |
| Move grip of a live Line lies on the stage | Drawn and grabbed there (AC-05) |
| Press inside a live rectangle with the Ellipse tool active (AC-08 converted it) | Moves the object; the object's tool is now Ellipse |
| Enter while the Polygonal Lasso is open | Not this spec: no object is live, because picking the Lasso committed it |
| Text field focused (layer rename) and Enter | Enter confirms the rename, which is a layer command, so the object is committed first (AC-09, `image-live-objects` AC-08); Enter in a text field never commits by itself |
| Thick live Line (40 px stroke at zoom 1) and a press on its visible stroke 15 CSS px from the centre line | Counts as the stroke (Terms: *Body*), not as outside: nothing is committed |
| Corner radius set while a Rounded Rectangle is live, then converted to Ellipse and back | The radius is kept through both conversions (`image-live-objects` AC-07) |
| Effect panel opened from the menu | The object is committed first, the effect previews and applies to the committed pixels (AC-09) |
| Undo pressed 3× after a draw plus two handle drags | Back to before the draw; no handles; Redo 3× restores the live object with both edits (AC-12) |
| Switch tab while live, switch back | The object is committed in the first tab before the switch; back there it is plain pixels |
| An agent call in live desktop mode while live | The core commits it (`image-live-objects` AC-08); the handles disappear on the next sync |
| Pinch-zoom while live | View change only; the object stays live and the handles keep their screen size |
| Two-finger tap (Undo) while live | As AC-12 |
| Escape twice during a handle drag | The first cancels the drag (object stays live), the second cancels the object (AC-11) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Hit tolerance 8 CSS px for a mouse, 16 CSS px for touch and pen | A 16 px target for a mouse and a ~32 px target for a fingertip match common touch-target guidance without making small objects impossible to move | Low: values can be tuned in a revision |
| Save, Save As and Export commit, autosave does not | paint.net commits on save; autosave runs on a timer and must not end an edit in progress | Low |
| A tab switch commits instead of keeping one live object per tab | Tool, options and colors are shared across tabs (Decision Log 2026-11-04 "Several documents per editor"), so a hidden tab's live object could not follow the toolbar | Low |
| The toolbar is not set from a live object that reappears through Redo | Overwriting the user's tool options on Undo / Redo would surprise more than it helps; paint.net keeps the toolbar as it is | Medium: owner may prefer the toolbar to follow the object |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/image-live-objects.spec.ts` `a shape, line or gradient drag leaves the object live with handles` | built |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 | `ui/e2e/image-live-objects.spec.ts` `another tool, a new drag or a press beside the object commits it` | built |        |
| AC-11 |      | open   |        |
| AC-12 | `ui/e2e/image-live-objects.spec.ts` `Undo and Redo walk the live object with its handles` | built |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 | `ui/e2e/image-live-objects.spec.ts` `a refused or degenerate drag leaves nothing live` | built |        |
| AC-17 | `ui/e2e/image-live-objects.spec.ts` `Escape cancels the live object and the document is clean again` | built |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |

## Decisions

- 2026-11-05 — Copy, Cut and Copy Merged commit (AC-09), because they read the final pixels and paint.net commits before any command that needs them.
- 2026-11-05 — Architecture as in `image-live-objects` Decisions (the core draws the object into its layer at every change; edits are undo steps; commit folds, cancel removes; any other command commits first). Owner's choice, because one undo stack and render path keep the UI's view, saves and history consistent; the Technical notes describe this model.
- 2026-11-05 — Escape cancels a live object everywhere and Enter commits it (AC-09, AC-11); with no object live, Escape keeps `image-editing` AC-31's behavior. Owner's choice, because one rule across shapes, gradients, live text (where Ctrl+Enter commits and Enter is a new line) and transforms is predictable.
- 2026-11-05 — A plain drag on a live Line's stroke inserts a control point (`image-shapes-curves`), a drag on a box shape's body moves the shape, and a Line moves by a dedicated move grip (AC-04). Owner's choice, because paint.net bends a line where it is dragged and the curve spec needs that gesture. A Gradient has no control points, so a drag on its guide line moves it (decided here, because no gesture competes for it).
- 2026-11-05 (clarification) — View actions (Space-drag, middle-button drag, pinch, wheel) and a right-button press do not commit (AC-09), because paint.net keeps a shape live while the user looks around, and the right button does nothing with these tools today.
- 2026-11-05 (clarification) — Undo during a handle or body drag only cancels that drag (AC-11), because undoing a step under a drag in progress would leave the drag editing an object state that no longer exists.
- 2026-11-05 (clarification) — A color change has no effect on a live Transparency-channel Gradient and adds no step (AC-07), because a step that changes nothing visible would make Undo look broken.
- 2026-11-05 (clarification) — A refused edit after an outside commit is dropped silently (AC-19), because the outcome (the object was committed) is what the user sees anyway, and an error notice would blame them for an agent's action.
- 2026-11-05 (spec-build) — Handles are DOM elements (`components/canvas/LiveHandles.svelte`, `data-live-handle`) positioned in viewport space over the canvas and the stage, with hit-testing in the pure module `tools/live-handles.ts`, because DOM handles keep their screen size without a redraw loop and let e2e specs count and locate them without reading canvas pixels.
- 2026-11-05 (spec-build) — Enter commits before the shared keymap is consulted, because the keymap maps a plain Enter to Pixel mode's Play (`togglePlay`), which Image mode does not serve.
- 2026-11-05 (spec-build) — Escape and Undo during a handle drag drop the drag through a new core command `cancel_live_gesture` (`CommandBus::cancel_live_gesture`), because Undo would leave the dropped drag as a redo step, which AC-11 rules out.
- 2026-11-05 (spec-build) — Progress (paused by the coordinator after the first block): built AC-01, AC-10, AC-12, AC-16, AC-17; the pointer code for handle / body / grip drags, cursors, Escape / Undo during a drag, the box-shape conversion on a tool change and the tool-change commit are in place but not yet covered by tests, so AC-02 – AC-09, AC-11, AC-13 stay `open`. Red run of this block: all five specs of `image-live-objects.spec.ts` failed against the editor before the change (no handles, the drag committed at once).
