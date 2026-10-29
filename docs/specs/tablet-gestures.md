---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.4, pincel.md §17.4, pincel.md §14]
---

# Tablet gestures: two- and three-finger tap, pen eraser end

## Context

The iPad is a declared target (§17.4, §14 question 5). Since Procreate, two-finger-tap undo is the gesture iPad artists expect; Pixaki, Resprite and Procreate all have it. Pens with an eraser end (Surface Pen, Wacom) report it as `button === 5` / `buttons & 32`, but both modes drop those pointer events today (only buttons 0 and 2 are accepted).

In both modes:
- a **two-finger tap** undoes one step, a **three-finger tap** redoes one step;
- the pen's **eraser end** (or eraser barrel button) erases for as long as it touches; afterwards the previously selected tool is still active.

Both gestures are on by default; View ▸ Touch Gestures turns the taps off for users who rest fingers on the screen.

## Scope

### In scope

- Two-finger tap → Undo, three-finger tap → Redo, in Pixel and Image mode, with fixed recognition thresholds
- Pen eraser end / eraser button erases for that stroke without changing the selected tool, in both modes
- View ▸ Touch Gestures checkbox (on by default), persisted in IndexedDB prefs
- en + de strings; §9.4 touch sentence updated

### Out of scope

- Long-press menus — because they are a separate interaction design.
- Pinch-to-rotate the canvas — because the view has no rotation.
- Apple Pencil squeeze and double-tap — because Safari does not expose them to web apps.
- Four-finger gestures — because no common editor convention exists for them.
- Pressure for the eraser end in Pixel mode — because Pixel mode has no pen pressure until `pixel-pressure` lands.

## Acceptance criteria

- [ ] AC-01 (event): When two touch points go down on the canvas within 150 ms of each other, neither moves more than 10 CSS px from where it started, and both are lifted within 300 ms of the first touch, then the editor undoes one step — in Pixel and in Image mode.
- [ ] AC-02 (event): When three touch points meet the same conditions, then the editor redoes one step — in Pixel and in Image mode.
- [ ] AC-03 (event): When a two-finger touch breaks any tap condition — a touch moves more than 10 CSS px, the pinch scale changes by more than ±2 %, the second touch lands more than 150 ms after the first, or a touch is still down 300 ms after the first went down — then no undo or redo happens and two-finger pan / zoom behaves as before.
- [ ] AC-04 (always): No touch of a two- or three-finger tap leaves a mark in the document.
- [ ] AC-05 (event): When the first finger of a two-finger tap had already started a stroke before the second finger landed, then after the tap that stroke is gone and the previous step is undone as well (after strokes A and B, a tap whose first finger began stroke C leaves only A).
- [ ] AC-06 (error): If there is nothing to undo (or, for a three-finger tap, nothing to redo), then the tap changes nothing and raises no error.
- [ ] AC-07 (event): When a pen touches the canvas with its eraser end or with the eraser button held (pointer type pen with button 5, or with bit 32 set in `buttons`), then that stroke erases exactly as the Eraser tool would — painted pixels under it become transparent — whatever tool is selected, in Pixel and in Image mode.
- [ ] AC-08 (event): When the eraser-end stroke ends or is cancelled, then the previously selected tool is still the active tool, and the tool rail showed that tool during the stroke.
- [ ] AC-09 (state): While erasing with the eraser end in Image mode, pen pressure affects the stroke as it does for the Eraser tool.
- [ ] AC-10 (event): When the user undoes after an eraser-end stroke, then the whole stroke is removed in one step.
- [ ] AC-11 (always): A new profile has View ▸ Touch Gestures checked.
- [ ] AC-12 (option): Where View ▸ Touch Gestures is unchecked, two- and three-finger taps neither undo nor redo, while two-finger pan / zoom and the pen eraser end keep working.
- [ ] AC-13 (event): When the user changes View ▸ Touch Gestures and reloads the app, then the setting is kept.
- [ ] AC-14 (option): Where the UI language is German, the View ▸ Touch Gestures entry is German.
- [ ] AC-15 (event, manual: real tablet hardware): When a user on an iPad two-finger-taps after a stroke, and on a Windows tablet with a Surface or Wacom pen erases with the eraser end, then the results match AC-01 and AC-07 on the real devices.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Pinch tracking.** `ui/src/lib/view/pinch.ts`, shared `PinchTracker` (≈L24): tracks `pointerType === 'touch'` only, reports when exactly two fingers are down (scale, midpoint, delta); `suppressing` stays on until all fingers lift. No tap timing, no movement threshold. Plan design: a `TapTracker` in `ui/src/lib/view/taps.ts` next to it, fed the same touch events by both modes; thresholds as named constants (150 ms, 10 CSS px, 300 ms, ±2 %).

**Pixel mode.** `ui/src/modes/pixel/pointer.ts`: `onPointerDown` (≈L203) calls `live.pinch.down(e)`, then `cancelGesture()` (≈L49), which rolls back to `live.gestureUndoDepth`; then `if (e.button !== 0 && e.button !== 2) return;` (≈L214) and `live.useBg = e.button === 2`. Handlers are bound in `PixelEditor.svelte`. Plan design: a per-gesture `live.toolOverride = 'eraser'` used instead of `toolState.tool` for that drag, cleared on pointerup / cancel.

**Image mode.** Capture-phase handlers in `ui/src/modes/image/components/canvas/MainCanvas.svelte` (≈L87–175: `pinch.down`, `gestureControl.cancel()`, undo back to `gestureUndoDepth`). Tool dispatcher `ui/src/modes/image/tools/pointer.ts`: same button filter (≈L198), plus `if (selectionOrPlacement && e.button !== 0)` (≈L213); `gestureKind = tool.kind` latched per drag (≈L302); `useBackground = e.button === 2`. Plan design: `gestureKind = 'eraser'` for that drag.

**Ordering.** When the second finger lands, `cancelGesture()` already rolls back the first finger's stroke; the tap's undo then undoes the previous step (AC-05). Test this explicitly.

**Undo / redo.** Pixel `undo()` / `redo()` in `ui/src/modes/pixel/doc-ops.ts` (≈L243 / L259); Image `undo()` / `redo()` in `ui/src/modes/image/core/controller.ts` (≈L840 / L847).

**Transient input pattern.** `ui/src/lib/shell/modifiers.svelte.ts` (`shiftHeld`, `altHeld`, `notePointer`), Decision Log 2026-10-12.

**Tests.** e2e `ui/e2e/gestures.spec.ts` following `ui/e2e/pinch.spec.ts`: `test.use({ hasTouch: true })`, CDP `Input.dispatchTouchEvent` with `touchPoints`. Undo state via the top-bar Undo button. Pen eraser: synthetic `PointerEvent` sequence (`pointerType: 'pen'`, `button: 5`, `buttons: 32`) on the canvas; check transparency with the Eyedropper read-back pattern of `ui/e2e/pencil.spec.ts`, never by reading canvas pixels in app code. Image-mode pressure e2e pattern: `ui/e2e/pressure.spec.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Two-finger tap with an empty undo history | Nothing happens (AC-06) |
| Two fingers land, then slowly pinch | Pinch zoom, no undo (AC-03) |
| A third finger lands during a pinch | No redo; pinch ends as today |
| Eraser end used while a selection tool, the Eyedropper or Move is selected | The stroke erases; the selected tool stays (AC-07, AC-08) |
| Pen driver reports the eraser only via `buttons` bit 32, with `button` 0 | Treated as eraser end |
| Eraser-end stroke cancelled by a second touch (palm) | Stroke rolled back like any cancelled gesture; selected tool unchanged |
| Touch Gestures off, user taps with two fingers | No undo; no mark left either |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Taps are recognised only on the canvas area, not on panels or menus | Both modes feed touch events to the trackers from the canvas handlers only | Low |
| The eraser end uses the Eraser tool's current settings (size, hardness, opacity) | The plan says "erases as the eraser"; it names no separate settings | Medium |

## Open questions

- Decision Log entry not yet written: the tap thresholds (150 ms / 10 CSS px / 300 ms / ±2 %), the View ▸ Touch Gestures toggle (on by default), and that Apple Pencil squeeze / double-tap are not reachable from Safari — confirm and add to `pincel.md` §15 with the §9.4 touch sentence.
- The plan lists an optional eraser badge on the cursor during an eraser-end stroke — build it or not?

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

- 2026-10-28 (legacy plan): The tap's undo runs after the existing gesture rollback, so it undoes the step before the cancelled stroke — that is what the user means by "undo".
- 2026-10-28 (legacy plan): Include the View ▸ Touch Gestures toggle (plan recommendation: "include it; it's cheap") — users who rest fingers on the screen need a way out.
- 2026-10-28 (legacy plan): The eraser end overrides the tool per stroke only; the rail keeps showing the selected tool — the eraser is a transient input state, like the Shift / Alt toggles (Decision Log 2026-10-12).
