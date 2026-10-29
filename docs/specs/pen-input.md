---
status: todo
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

- [ ] AC-01 (event): When two fingers spread apart on the Pixel-mode canvas, the zoom readout increases, and no pixel is painted and no undo entry is added.
- [ ] AC-02 (event): When two fingers spread apart on the Image-mode canvas, the zoom readout increases, and no pixel is painted and no undo entry is added.
- [ ] AC-03 (event): When a second finger touches down while the first finger's stroke is in progress, the edits that stroke made are rolled back.
- [ ] AC-04 (state): While two fingers are down, moving them together pans the view by the movement of their midpoint, and pinching zooms about that midpoint so the document point under it stays under it.
- [ ] AC-05 (state): While a pinch has run and at least one finger is still down, the remaining finger neither paints nor selects; touch input acts again once every finger has lifted.
- [ ] AC-06 (always): Mouse and pen pointers never start a pinch, however many are down.
- [ ] AC-07 (event): When the mouse wheel turns over the Pixel-mode canvas, the zoom steps in (wheel up) or out (wheel down) to the next integer zoom between 1× and 64×, and the sprite pixel under the cursor stays under the cursor; a purely horizontal wheel gesture does not zoom.
- [ ] AC-08 (event): When the mouse wheel turns over the Image-mode canvas, the zoom changes smoothly between 2 % and 6400 %, and the document point under the cursor stays under the cursor.
- [ ] AC-09 (event): When `+` or `=` is pressed the view zooms in, `-` or `_` zooms out, `0` fits the document to the view and `1` shows it at 1:1 — in both modes, while focus is not in a text field.
- [ ] AC-10 (event): When Actual size is pressed on the floating zoom control, the zoom becomes 100 % (1:1), and Zoom out then lowers it.
- [ ] AC-11 (event): When a document is created, opened or recovered, it is shown whole and centred: in Pixel mode at the largest integer zoom (1–64) that leaves at least 24 CSS px on every side, in Image mode at the largest zoom (up to 6400 %) that leaves 16 CSS px on every side.
- [ ] AC-12 (state): While the Image-mode view is in fit mode, resizing the viewport fits the document again; any explicit zoom or pan leaves fit mode.
- [ ] AC-13 (state): While Space is held (focus not in a text field), dragging on the canvas pans the view with any tool and changes no pixel; in Image mode a middle-button drag pans too.
- [ ] AC-14 (option): Where pressure → Size is on, an Image-mode Pencil or Eraser stroke drawn with a light pen paints a smaller dab than the same stroke at full pressure, and a stroke at full pressure paints exactly what the same stroke without pressure readings paints.
- [ ] AC-15 (option): Where pressure → Size is off, pen pressure does not change the painted pixels of an Image-mode stroke.
- [ ] AC-16 (option): Where pressure → Opacity is on, a dab at pressure p paints with opacity p (alpha ≈ 128 at p = 0.5 on a transparent layer); where it is off, the dab opacity does not depend on pressure.
- [ ] AC-17 (always): Mouse and touch strokes paint at full pressure in Image mode, whatever the pressure options are.
- [ ] AC-18 (option): Where the tip is Flat and "Tilt turns the nib" is on, the nib follows the azimuth of a tilted pen per sample; where the option is off, or the pen is upright or reports no tilt, the nib keeps the Angle option.
- [ ] AC-19 (event): When the Image-mode Pencil, Eraser, Recolor or Smudge tool is active, the options bar shows Pressure with a Size checkbox (on by default) and an Opacity checkbox (off by default), plus "Tilt turns the nib" (on by default) while the tip is Flat.
- [ ] AC-20 (state): While only a mouse has been used on a desktop with a fine pointer, no Shift / Alt toggles are shown; once a touch or pen pointer touches the editor stage, or on a coarse-pointer device from the start, the Modifier keys group with Shift and Alt is shown.
- [ ] AC-21 (event): When the on-screen Shift or Alt toggle is tapped, it latches (pressed) and acts as the held key for every following gesture until it is tapped again.

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

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/pinch.spec.ts › pixel mode: two-finger pinch zooms in and paints nothing` | built | |
| AC-02 | `ui/e2e/pinch.spec.ts › image mode: two-finger pinch zooms in and paints nothing` | built | |
| AC-03 | `ui/e2e/pinch.spec.ts › pixel mode: two-finger pinch zooms in and paints nothing` | built | |
| AC-04 |  | open | |
| AC-05 |  | open | |
| AC-06 |  | open | |
| AC-07 |  | open | |
| AC-08 |  | open | |
| AC-09 |  | open | |
| AC-10 | `ui/e2e/tools.spec.ts › image mode: the zoom buttons still work with the Zoom tool selected`; `ui/e2e/large-image.spec.ts › image mode: a canvas wider than 8192 px opens, renders and paints` | built | |
| AC-11 |  | open | |
| AC-12 |  | open | |
| AC-13 |  | open | |
| AC-14 | `ui/e2e/pressure.spec.ts › image mode: a light pen stroke is thinner than a firm one`; `crates/pincel-image-core/src/tools/mod.rs › tests::stroke_with_light_pressure_paints_a_smaller_dab`, `tests::stroke_with_full_pressure_matches_plain_stroke` | built | |
| AC-15 | `ui/e2e/pressure.spec.ts › image mode: a light pen stroke is thinner than a firm one` | built | |
| AC-16 | `crates/pincel-image-core/src/tools/mod.rs › tests::stroke_with_pressure_opacity_lowers_alpha_only_when_enabled` | built | |
| AC-17 |  | open | |
| AC-18 | `ui/e2e/pressure.spec.ts › image mode: tilt turns a flat nib only while the option is on`; `crates/pincel-image-core/src/tools/mod.rs › tests::flat_brush_follows_azimuth_only_when_enabled` | built | |
| AC-19 |  | open | |
| AC-20 | `ui/e2e/modifiers.spec.ts › a mouse-only desktop shows no modifier toggles`; `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key` | built | |
| AC-21 | `ui/e2e/modifiers.spec.ts › touch device › pixel mode: the latched Shift snaps a line like the Shift key`; `ui/e2e/modifiers.spec.ts › touch device › image mode: the latched Shift squares a shape like the Shift key` | built | |

## Decisions

- 2026-10-29: Extracted from the code as it stands; inconsistencies between the two editors' viewports are not ACs — see the extraction report.
