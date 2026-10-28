---
id: todo-tablet-gestures
title: Tablet gestures — two/three-finger tap undo/redo, pen eraser end
status: todo
size: M (one session)
mode: Pixel + Image
depends_on: []
created: 2026-10-28
---

# Tablet gestures: two- and three-finger tap, pen eraser end

## Goal

In both modes:
- **Two-finger tap** → Undo.
- **Three-finger tap** → Redo.
- With a pen, the **eraser end**, or the eraser barrel button, erases for as long as it is in use. Afterwards the previous tool is back.

## Why

The iPad is a declared target (§17.4, §14 question 5). Since Procreate, two-finger-tap undo is the gesture iPad artists expect. Pixaki, Resprite and Procreate all have it.

Pens with an eraser end (Apple Pencil Pro squeeze aside; Surface Pen, Wacom) report `button === 5` / `buttons & 32`. Today **both modes drop those events**: `if (e.button !== 0 && e.button !== 2) return;`.

## Current state (read these first)

**Pinch tracking.** `ui/src/lib/view/pinch.ts`, shared `PinchTracker`
- Tracks `pointerType === 'touch'` only (`down` ≈L46–55).
- Reports when **exactly two** fingers are down: scale, midpoint, delta.
- `suppressing` stays on until all fingers lift (≈L38–40, L76–79).
- No tap timing, no movement threshold.

**Pixel mode.** `ui/src/modes/pixel/pointer.ts`
- `onPointerDown` (≈L203–214): `live.pinch.down(e)`, then `cancelGesture()`, which rolls back to `live.gestureUndoDepth` (≈L49–69).
- Then `if (e.button !== 0 && e.button !== 2) return;` (≈L214) and `live.useBg = e.button === 2` (≈L215).
- Moves ≈L481, ups ≈L546.
- Handlers are bound in `PixelEditor.svelte` (≈L903–907).

**Image mode.**
- `ui/src/modes/image/components/canvas/MainCanvas.svelte` capture-phase handlers (≈L87–175): `pinch.down`, then `gestureControl.cancel()` and an undo back to `gestureUndoDepth`.
- The tool dispatcher in `ui/src/modes/image/tools/pointer.ts`:
  - the same button filter (≈L198);
  - `gestureKind = tool.kind` (≈L302), latched per drag;
  - `useBackground = e.button === 2` (≈L303).

**Undo and redo.**
- Pixel: `undo()` / `redo()` in `ui/src/modes/pixel/doc-ops.ts` (≈L243 / L259).
- Image: `controller.ts` (≈L840 / L847).

**Tools.** Pixel `toolState.tool` (`Tool` includes `'eraser'`); Image `tool.kind` (`ToolKind` includes `'eraser'`).

**Pattern for a transient input state.** `ui/src/lib/shell/modifiers.svelte.ts` (`modifiers`, `shiftHeld`, `altHeld`, `notePointer`), Decision Log 2026-10-12.

**e2e.** `ui/e2e/pinch.spec.ts`
- `test.use({ hasTouch: true })`.
- A CDP session sends `Input.dispatchTouchEvent` (`touchStart` / `touchMove` / `touchEnd` with `touchPoints`).

## Design

### Taps

New `ui/src/lib/view/taps.ts`, a `TapTracker` next to `PinchTracker`, fed the same touch pointer events by both modes. A tap is recognized when all of these hold:
1. 2 or 3 touch pointers go down within **150 ms** of the first.
2. None moves more than **10 CSS px** from its start.
3. All are lifted within **300 ms** of the first down.
4. No pinch frame was emitted in between (scale change within ±2 %).

On a tap with n = 2 call `undo()`; with n = 3 call `redo()`.

Order relative to the existing gesture rollback: when the second finger lands, `cancelGesture()` already rolls back the first finger's stroke. The tap's undo then undoes the **previous** step, which is what the user means. Test this case explicitly.

Make the thresholds constants, and record them in a Decision Log entry.

### Pen eraser end

- The pointerdown filter accepts `button === 5` when `e.pointerType === 'pen'`. Also accept `(e.buttons & 32) !== 0`, because some drivers report the eraser through `buttons` only.
- **Pixel mode:** a per-gesture tool override, `live.toolOverride = 'eraser'`, which `paintAt` and its callers use instead of `toolState.tool` for that drag. Clear it on pointerup / cancel. The rail shows the real tool. Optional: a subtle eraser badge on the cursor.
- **Image mode:** set `gestureKind = 'eraser'` for that drag. It is already latched per drag, so this is a small change.
- Pressure still applies (Image mode; and Pixel mode once `02-pixel-pressure.md` is done).

### Settings

Both gestures are on by default. A user who rests fingers on the screen may want taps off: add View ▸ Touch Gestures (a checkbox), persisted in the IndexedDB prefs. Recommendation: include it; it's cheap.

## Tasks (one M session)

1. **`docs(spec)`.** In §9.4, add the taps, their thresholds and the pen eraser end to the touch sentence. Decision Log entry for the thresholds and the toggle.
2. **`feat(ui)`.** `TapTracker`, wired into both modes; the pen eraser override in both modes; the View toggle and i18n.
3. **e2e** `ui/e2e/gestures.spec.ts`, using `hasTouch` and CDP touch events as in `pinch.spec.ts`. In both modes:
   - draw a stroke with the mouse, then a two-finger tap → the Undo button is disabled again;
   - three-finger tap → Redo applied.
   - Pen eraser: dispatch a synthetic `PointerEvent` sequence (`pointerType: 'pen'`, `button: 5`, `buttons: 32`) on the canvas over painted pixels. The pixels become transparent (Pixel mode: Eyedropper read-back as in `ui/e2e/pencil.spec.ts`), and the rail still shows the previous tool afterwards.

## Out of scope

- Long-press menus.
- Pinch-to-rotate the canvas.
- Apple Pencil squeeze and double-tap: Safari does not expose them to web apps. Record that.
- Four-finger gestures.
