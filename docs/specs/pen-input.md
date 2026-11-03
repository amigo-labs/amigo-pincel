---
status: implemented
created: 2026-10-29
size: M
mode: both
design: [pincel.md §9.4, pincel.md §17.4]
---

# Pen, touch and viewport input

## Context

Pincel targets the iPad / Apple Pencil audience Piskel never served (`pincel.md` §17.4). Both editors take input through the Pointer Events API: Image-mode brushes read stylus pressure and tilt, two fingers pinch-zoom and pan in both modes, the editor frame shows latching Shift / Alt toggles for keyboard-less devices, and the view zooms with the wheel, the keyboard and the floating zoom control, and fits a document when it opens. This spec records that behavior as it exists on 2026-10-29.

Already decided in §17.4: Pixel-mode tools stay pressure-free; Image-mode Pencil and Eraser scale size and / or opacity with pressure, each switchable; the flat tip follows the stylus azimuth while "Tilt turns the nib" is on. The planned `pixel-pressure` and `tablet-gestures` specs cover future work and are not described here.

## Scope

### In scope

- Image-mode pen pressure (size, opacity) and tilt for the pressure-aware brush tools
- Two-finger pinch zoom and pan in both modes, and the rollback of a gesture the second finger interrupts
- The on-screen Shift / Alt toggles: when they appear and how they latch
- Wheel zoom anchored at the cursor, keyboard zoom keys, the floating zoom control, Space-drag pan
- Fit-to-view when a document opens

### Out of scope

- Pen pressure in Pixel mode — because §17.4 keeps Pixel tools pressure-free; the planned `pixel-pressure` spec covers it
- Long-press menus, three-finger undo and other tablet gestures — because they are not built; the planned `tablet-gestures` spec covers them
- What Shift / Alt do per tool — because `pixel-drawing-tools`, `pixel-selection-move` and `image-editing` cover tool behavior
- Image-mode Pan and Zoom rail tools — because they are tools; `image-editing` covers them
- Brush size keys `[` / `]` — because they are a keyboard shortcut of the Image-mode brush (covered by `image-editing`), not pen or viewport input; Pixel mode does not bind them yet (planned `pixel-pencil`)

## Acceptance criteria

- [x] AC-01 (event): When two fingers spread apart on the Pixel-mode canvas, the zoom readout increases, and no pixel is painted and no undo entry is added.
- [x] AC-02 (event): When two fingers spread apart on the Image-mode canvas, the zoom readout increases, and no pixel is painted and no undo entry is added.
- [x] AC-03 (event): When a second finger touches down while the first finger's stroke is in progress, the edits that stroke made are rolled back.
- [x] AC-04 (state): While two fingers are down, moving them together pans the view by the movement of their midpoint, and pinching zooms about that midpoint so the document point under it stays under it.
- [x] AC-05 (state): While a pinch has run and at least one finger is still down, the remaining finger neither paints nor selects; touch input acts again once every finger has lifted.
- [x] AC-06 (always): Mouse and pen pointers never start a pinch, however many are down.
- [x] AC-07 (event): When the mouse wheel turns over the Pixel-mode canvas, the zoom steps in (wheel up) or out (wheel down) to the next integer zoom between 1× and 64×, and the sprite pixel under the cursor stays under the cursor; a purely horizontal wheel gesture does not zoom.
- [x] AC-08 (event): When the mouse wheel turns over the Image-mode canvas, the zoom changes smoothly between 2 % and 6400 %, and the document point under the cursor stays under the cursor.
- [x] AC-09 (event): When `+` or `=` is pressed the view zooms in, `-` or `_` zooms out, `0` fits the document to the view and `1` shows it at 1:1 — in both modes, while focus is not in a text field.
- [x] AC-10 (event): When Actual size is pressed on the floating zoom control, the zoom becomes 100 % (1:1), and Zoom out then lowers it.
- [x] AC-11 (event): When a document is created, opened or recovered, it is shown whole and centred: in Pixel mode at the largest integer zoom (1–64) that leaves at least 24 CSS px on every side, in Image mode at the largest zoom (up to 6400 %) that leaves 16 CSS px on every side.
- [x] AC-12 (state): While the Image-mode view is in fit mode, resizing the viewport fits the document again; any explicit zoom or pan leaves fit mode.
- [x] AC-13 (state): While Space is held (focus not in a text field), dragging on the canvas pans the view with any tool and changes no pixel; in Image mode a middle-button drag pans too.
- [x] AC-14 (option): Where pressure → Size is on, an Image-mode Pencil or Eraser stroke drawn with a light pen paints a smaller dab than the same stroke at full pressure, and a stroke at full pressure paints exactly what the same stroke without pressure readings paints.
- [x] AC-15 (option): Where pressure → Size is off, pen pressure does not change the painted pixels of an Image-mode stroke.
- [x] AC-16 (option): Where pressure → Opacity is on, a dab at pressure p paints with opacity p (alpha ≈ 128 at p = 0.5 on a transparent layer); where it is off, the dab opacity does not depend on pressure.
- [x] AC-17 (always): Mouse and touch strokes paint at full pressure in Image mode, whatever the pressure options are.
- [x] AC-18 (option): Where the tip is Flat and "Tilt turns the nib" is on, the nib follows the azimuth of a tilted pen per sample; where the option is off, or the pen is upright or reports no tilt, the nib keeps the Angle option.
- [x] AC-19 (event): When the Image-mode Pencil, Eraser, Recolor or Smudge tool is active, the options bar shows Pressure with a Size checkbox (on by default) and an Opacity checkbox (off by default), plus "Tilt turns the nib" (on by default) while the tip is Flat.
- [x] AC-20 (state): While only a mouse has been used on a desktop with a fine pointer, no Shift / Alt toggles are shown; once a touch or pen pointer touches the editor stage, or on a coarse-pointer device from the start, the Modifier keys group with Shift and Alt is shown.
- [x] AC-21 (event): When the on-screen Shift or Alt toggle is tapped, it latches (pressed) and acts as the held key for every following gesture until it is tapped again.

## Technical notes

> Paths and line numbers are hints from 2026-10-29 and drift — grep for the names.

**Pinch** (`ui/src/lib/view/pinch.ts`): `PinchTracker` tracks only `pointerType === 'touch'`; `down(e)` returns `true` when the second finger lands, `move(e)` returns a `PinchFrame { scale, midX, midY, dx, dy }`, `suppressing` stays `true` until every finger has lifted.

- Pixel: `onPointerDown` / `onPointerMove` / `onPointerUp` in `ui/src/modes/pixel/pointer.ts`; the second finger calls `cancelGesture()`, which undoes back to `live.gestureUndoDepth`; `applyPinch(f)` in `view-ops.ts` rounds `live.pinchZoom0 * f.scale` to an integer zoom and adds `f.dx` / `f.dy` to the pan.
- Image: capture-phase handlers in `ui/src/modes/image/components/canvas/MainCanvas.svelte` (`onPointerDownCapture`, …) call `gestureControl.cancel()` and `undo()` back to `gestureUndoDepth`, then `zoomAround(pinchZoom0 * f.scale, mid)`.

**Zoom** — Pixel (`ui/src/modes/pixel/view-ops.ts`): `MIN_ZOOM = 1`, `MAX_ZOOM = 64`, integer only; `zoomIn()` ×2, `zoomOut()` ÷2; `onWheel` steps `round(zoom × 1.25)` / `round(zoom / 1.25)` (at least ±1), ignores `deltaY === 0`, anchors by adjusting `panX` / `panY`; `fitView()` uses `fitZoom(stageW, stageH, w, h, 1, 64, FIT_MARGIN = 24)` (`ui/src/lib/view/fit.ts`) and is called on new / open / recover (`doc-ops.ts`) and by `resetView()`; readout `N×`. Image (`MainCanvas.svelte`, bounds in `ui/src/modes/image/stores/editor.svelte.ts`): `MIN_ZOOM = 0.02`, `MAX_ZOOM = 64`; wheel factor `exp(-deltaY × 0.0015)` about the cursor; `viewCommands.zoomIn/zoomOut` ×1.25 / ÷1.25 about the viewport centre; `fitView()` pads 32 px in total and sets `view.fit`; a `ResizeObserver` refits while `view.fit`; readout in % on the "Fit to window" button. Keys come from `SHORTCUTS` (`zoomIn` `+` `=`, `zoomOut` `-` `_`, `zoomFit` `0`, `zoomActual` `1`) in `ui/src/lib/shell/shortcuts.ts`.

**Pan**: Pixel — `view.spaceDown` + drag, and the Move tool without a selection (`pointer.ts`); state cleared by `onWindowBlur`. Image — Space or middle button (`startPan` in `MainCanvas.svelte`).

**Pressure / tilt** (Image mode): `pressureOf(e)` (pen → `e.pressure` clamped to 0–1, else 1) and `azimuthOf(e)` (`azimuthAngle` when `altitudeAngle < π/2`, else from `tiltX` / `tiltY`, `null` when upright) in `ui/src/modes/image/tools/pointer.ts`; samples include `getCoalescedEvents()`. Options `tool.pressureSize` (default `true`), `tool.pressureOpacity` (`false`), `tool.tiltAngle` (`true`) in the image editor store; `penFields()` in `core/controller.ts` sends `pressure`, `pressure_size`, `pressure_opacity`, `azimuth`, `tilt_angle` on the `pencil_stroke` / `eraser_stroke` / recolor / smudge `CommandSpec`. Core: `PressureDynamics { size, opacity }`, `Brush::dab_radius(p)` (min radius `MIN_PRESSURE_RADIUS = 0.5`), `Brush::dab_strength(p)`, `Brush::with_follow_tilt` in `crates/pincel-image-core/src/tools/mod.rs`.

**Modifier toggles** (`ui/src/lib/shell/modifiers.svelte.ts`): `modifiers = $state({ shift, alt, shown: coarsePointer() })`, `shiftHeld(e)`, `altHeld(e)`, `notePointer(e)` wired as `onpointerdowncapture` on the stage in `EditorFrame.svelte`.

**Touch**: both canvases carry `touch-none` so one-finger drags reach the tools instead of scrolling the page; Pixel mode suppresses the context menu on the canvas.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pinch beyond the zoom bounds | The zoom stays at the bound (1–64× Pixel, 2 %–6400 % Image) |
| Window loses focus while Space is held | Space and any running pan are released |
| A Zoom-tool release in Image mode leaves a touch registered | The pointer is always retired, so the next tap is not counted as a second finger |
| Pen reports pressure 0 at the start of a stroke | The dab shrinks to the minimum radius, never vanishes, while Size is on |
| Pen without tilt support (tiltX = tiltY = 0) | The nib keeps the Angle option |
| Wheel over the Pixel canvas with only horizontal delta | No zoom; the page may scroll |
| Fit before the stage has been measured (Pixel) | Falls back to 8× |

## Summary

> Completed: 2026-11-03

- Image-mode pen pressure (size, opacity) and tilt for the pressure-aware brushes, with mouse and touch at full pressure and the options at their defaults
- Two-finger pinch zoom and pan about the midpoint in both modes; the finger left after a pinch neither paints nor selects; mouse and pen never pinch
- Wheel zoom about the cursor (integer steps in Pixel mode, smooth in Image mode), keyboard zoom keys, Space-drag and middle-button pan
- New, opened and recovered documents are fitted and centred in the stage their panels leave (fixed for Pixel mode), and fit mode refits on resize until an explicit zoom or pan

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/pinch.spec.ts › pixel mode: two-finger pinch zooms in and paints nothing` | verified | ba53299 |
| AC-02 | `ui/e2e/pinch.spec.ts › image mode: two-finger pinch zooms in and paints nothing` | verified | ba53299 |
| AC-03 | `ui/e2e/pinch.spec.ts › pixel mode: two-finger pinch zooms in and paints nothing` | verified | 6aed618 |
| AC-04 | `ui/e2e/pinch.spec.ts › pixel mode: two fingers pan by their midpoint and zoom about it`; `ui/e2e/pinch.spec.ts › image mode: two fingers pan by their midpoint and zoom about it` | verified | ba53299 |
| AC-05 | `ui/e2e/pinch.spec.ts › pixel mode: the finger left after a pinch neither paints nor selects`; `ui/e2e/pinch.spec.ts › image mode: the finger left after a pinch neither paints nor selects` | verified | 35f631c |
| AC-06 | `ui/e2e/pinch.spec.ts › pixel mode: a mouse and a pen down together never pinch`; `ui/e2e/pinch.spec.ts › image mode: a mouse and a pen down together never pinch` | verified | 6aed618 |
| AC-07 | `ui/e2e/viewport.spec.ts › pixel mode: the wheel steps integer zooms about the cursor, 1× to 64×` | verified | 35f631c |
| AC-08 | `ui/e2e/viewport.spec.ts › image mode: the wheel zooms smoothly about the cursor, 2 % to 6400 %` | verified | 9af6b69 |
| AC-09 | `ui/e2e/viewport.spec.ts › pixel mode: keys zoom in, out, to fit and to 1:1 outside text fields`; `ui/e2e/viewport.spec.ts › image mode: keys zoom in, out, to fit and to 1:1 outside text fields` | verified | 9a7f4f0 |
| AC-10 | `ui/e2e/tools.spec.ts › image mode: the zoom buttons still work with the Zoom tool selected`; `ui/e2e/large-image.spec.ts › image mode: a canvas wider than 8192 px opens, renders and paints` | verified | c43a845 |
| AC-11 | `ui/e2e/viewport.spec.ts › pixel mode: a new document is fitted and centred`, `› pixel mode: an opened document is fitted and centred`, `› pixel mode: a recovered document is fitted and centred`; the same three for `image mode` | verified | 5dd2afe |
| AC-12 | `ui/e2e/viewport.spec.ts › image mode: fit mode refits on resize until an explicit zoom or pan` | verified | 22f5a52 |
| AC-13 | `ui/e2e/viewport.spec.ts › pixel mode: Space-drag pans with any tool and paints nothing`; `ui/e2e/viewport.spec.ts › image mode: Space-drag pans with any tool and paints nothing`; `ui/e2e/viewport.spec.ts › image mode: a middle-button drag pans and paints nothing` | verified | 3678aba |
| AC-14 | `ui/e2e/pressure.spec.ts › image mode: a light pen stroke is thinner than a firm one`; `crates/pincel-image-core/src/tools/mod.rs › tests::stroke_with_light_pressure_paints_a_smaller_dab`, `tests::stroke_with_full_pressure_matches_plain_stroke` | verified | 48b5d53 |
| AC-15 | `ui/e2e/pressure.spec.ts › image mode: a light pen stroke is thinner than a firm one` | verified | 6aed618 |
| AC-16 | `crates/pincel-image-core/src/tools/mod.rs › tests::stroke_with_pressure_opacity_lowers_alpha_only_when_enabled` | verified | 35f631c |
| AC-17 | `ui/e2e/pressure.spec.ts › touch device › image mode: mouse and touch strokes paint at full pressure` | verified | b5e6482 |
| AC-18 | `ui/e2e/pressure.spec.ts › image mode: tilt turns a flat nib only while the option is on`; `crates/pincel-image-core/src/tools/mod.rs › tests::flat_brush_follows_azimuth_only_when_enabled` | verified | 6aed618 |
| AC-19 | `ui/e2e/pressure.spec.ts › image mode: brush tools show the pressure and tilt options with their defaults` | verified | 6aed618 |
| AC-20 | `ui/e2e/modifiers.spec.ts › a mouse-only desktop shows no modifier toggles`; `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key` | verified | 6aed618 |
| AC-21 | `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key`; `ui/e2e/modifiers.spec.ts › touch device › image mode: the latched Shift squares a shape like the Shift key` | verified | dff3361 |

## Decisions

- 2026-10-29: Extracted from the code as it stands; inconsistencies between the two editors' viewports are not ACs — see the extraction report.
- 2026-11-03: Closed the test gaps of AC-04 – AC-09, AC-11 – AC-13, AC-17 and AC-19. The behavior existed, so each new test was run red against a deliberate, reverted break: the pinch pan delta zeroed, and the pinch midpoint moved off the fingers (AC-04); `PinchTracker` suppression cleared on the first lift together with Pixel's `cancelGesture` leaving the stroke alive (AC-05 — removing the suppression alone stays green, because the cancelled gesture also keeps the remaining finger from painting); the touch-only check dropped from `PinchTracker.down` (AC-06); the cursor anchor removed from Pixel's `onWheel` (AC-07); Image's wheel anchored at the viewport centre (AC-08); `=` unbound from zoom in (AC-09); Image's fit padding 40 instead of 32 px (AC-11); the refit dropped from the `ResizeObserver`, and `zoomAround` no longer leaving fit mode (AC-12); Space pan off in both modes and the middle button no longer panning (AC-13); `pressureOf` reading mouse and touch pressure (AC-17); `tiltAngle` off by default (AC-19).
- 2026-11-03: AC-11 found a bug: Pixel's fit on create, open and recover used the stage size bound through a `ResizeObserver`, which still held the size from before the document's panels filled in (the timeline's frame row), so a 64 × 48 sprite came up at 10× where only 9× leaves 24 px on every side. Fixed in `fix(ui): fit a new Pixel document to the stage its panels leave`: `fitView` measures the stage element, and `fitNewDocument` fits again once the DOM shows the new document.
