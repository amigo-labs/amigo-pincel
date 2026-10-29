---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §9.2, pincel.md §17.5]
---

# Animation preview panel

## Context

Animators judge motion at real size. Aseprite has a preview window (F7); Pixelorama and Piskel have a live preview. In Pincel, playback drives the main canvas itself — the play loop changes the current frame — so the user can either watch or draw, never both at once.

A **Preview** panel in Pixel mode shows the animation at 1×, 2×, 4× or Fit and plays it on its own loop **while the user keeps drawing on the zoomed-in main canvas**. Paused, it shows the current frame. It has its own play button and follows the timeline's loop-tag setting. It never shows onion-skin ghosts.

## Scope

### In scope

- A docked, collapsible side panel "Preview" in Pixel mode, collapsed by default, placed above Layers
- Its own play / pause and zoom (1×, 2×, 4×, Fit), independent of the main canvas's current frame
- Playback range and direction from the timeline's loop tag, frame timing from frame durations
- Compositing without onion skin for the preview (public wasm API addition)
- en + de strings; §9.2 panel list updated

### Out of scope

- A floating, undockable, resizable preview window — because floating panels don't exist yet and need their own Decision Log entry; it is a follow-up.
- Previewing tile-wrapped output — because it belongs to `symmetry-tiled-mode`.
- Exporting from the preview — because export already has its own menu and dialogs.
- A preview in Image mode — because Image documents have no frames.

## Acceptance criteria

- [ ] AC-01 (always): Pixel mode has a side panel named "Preview", placed above the Layers panel and collapsed on a profile with no stored layout; Image mode has no Preview panel.
- [ ] AC-02 (state): While the preview is paused, it shows the main canvas's current frame and follows when the user changes the current frame (the preview's shown frame index, exposed as `data-frame`, equals the timeline's current frame).
- [ ] AC-03 (event): When the user presses the preview's play button, then the preview's shown frame advances over time while the timeline's current frame stays unchanged.
- [ ] AC-04 (state): While the preview plays, a pencil stroke on the main canvas lands on the timeline's current frame and is one undo step, as without the preview.
- [ ] AC-05 (event): When the document changes, then the preview shows the changed content: immediately while paused, and the next time the changed frame comes up while playing (an element screenshot of the paused preview differs before and after a stroke).
- [ ] AC-06 (state): While a loop tag is set in the timeline, preview playback stays within that tag's frames and follows its direction (forward, reverse, ping-pong); without a loop tag it plays all frames forward.
- [ ] AC-07 (always): During preview playback each frame stays shown for its frame duration: with frame durations of 100 ms and 1000 ms, the 1000 ms frame is shown for clearly longer than the 100 ms frame.
- [ ] AC-08 (always): The preview never shows onion-skin ghosts: with onion skin on in the timeline, an element screenshot of the paused preview equals the one with onion skin off.
- [ ] AC-09 (always): The public wasm `compose(frame, zoom)` keeps its signature and output (onion skin applied as configured); compositing a frame without onion skin returns the same bytes as `compose` with onion skin disabled.
- [ ] AC-10 (option): Where the preview zoom is 1×, 2× or 4×, the preview's displayed size is the sprite size times that factor in CSS px; where it is Fit, the preview is the largest whole-sprite size that fits the panel, keeping the aspect ratio.
- [ ] AC-11 (always, manual: visual rendering quality): The preview shows crisp, unsmoothed sprite pixels at every zoom, without blur at pixel edges.
- [ ] AC-12 (always): Main-canvas playback keeps working as before: Enter or the timeline's play button still advances the timeline's current frame.
- [ ] AC-13 (event): When the user collapses the Preview panel while it plays, then preview playback stops.
- [ ] AC-14 (always): On a phone-width viewport the Preview panel is reachable as a sheet tab like the other panels.
- [ ] AC-15 (error): If the frame the preview shows is deleted, or the loop tag it follows is deleted, while it plays, then the preview continues with a frame that still exists (within the remaining range) and no error is raised.
- [ ] AC-16 (option): Where the UI language is German, the panel title, play / pause and zoom labels are German.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Playback (pure functions).** `ui/src/modes/pixel/playback.ts`: `PlayRange { from, to, direction }`, `startCursor` (≈L23), `advance(range, cursor)` (≈L33) for forward, reverse and ping-pong. Reuse them.

**Playback loop.** `ui/src/modes/pixel/doc-ops.ts`: `playRange()` (≈L174) takes `play.loopTag` or all frames; `stopPlayback()` (≈L185); `togglePlay()` (≈L196) is a rAF loop on wall-clock time using `ed.doc.frameDuration(frame)` and calls `setFrame(cursor.frame)` (≈L204). The preview needs its own loop that never calls `setFrame` / touches `ed.currentFrame`.

**State.** `ui/src/modes/pixel/stores/editor.svelte.ts`: `play` (`playInit()` ≈L111), `ed.currentFrame` / `ed.frameCount`, `live.playHandle` (≈L155). Plan design: a `preview` state `{ playing, cursor, zoom: 1 | 2 | 4 | 'fit' }`.

**Compose.** `ui/src/modes/pixel/render.ts` `recompose()` calls `ed.doc.compose(ed.currentFrame, 1)`, `renderer.draw(frame)`, `frame.free()`. wasm `compose(frame, zoom)` in `crates/pincel-wasm/src/lib.rs` (≈L470) always applies the document's onion skin (`request.onion_skin = self.onion_skin`, ≈L515); the onion settings come from `set_onion_skin` in `crates/pincel-wasm/src/timeline.rs` (≈L203). Onion math tests live in `crates/pincel-core/src/render/onion.rs`. Plan recommendation: a new `composeFrame(frame: u32, zoom: u32, onion: bool)` with `compose` delegating to it, so the existing npm API (§17.5) stays unchanged; test name suggestion `compose_frame_without_onion_ignores_onion_settings`.

**Cost.** Compose only when the shown frame or the document changes; cache the last image per frame and invalidate on any edit (find the dirty / revision signal `render.ts` uses — `recomposeDirty`, `canRecomposeDirty` ≈L130–141, and the `dirty-canvas` / `dirty-rect` events ≈L182). Stop the loop when the panel is collapsed or `document.visibilityState` is hidden.

**Blitting.** `blitFrame(canvas, frame)` in `ui/src/lib/render/canvas2d.ts` (≈L14) resizes the canvas and `putImageData`s; CSS width sets the zoom.

**Panels.** `PanelDef { id, title, icon, body, placement?: 'side' | 'bottom', grow?, defaultCollapsed? }` in `ui/src/lib/components/layout/panels.svelte.ts` (≈L12). Collapse state persists per mode (`layout:pixel` pref). The Pixel panel list is in `PixelEditor.svelte` (≈L765–776). On phones every panel becomes a sheet tab (`EditorFrame.svelte`).

**Tests.** e2e `ui/e2e/preview.spec.ts`: a 2-frame sprite with different colors, open the panel, play, the timeline's current frame (`aria-current` in `TimelinePanel.svelte` ≈L290) stays, the preview's `data-frame` changes. Never read canvas pixels from app code; element screenshots in e2e are fine (AGENTS.md §7.4).

**i18n.** Plan keys: `panel.preview`, `preview.play`, `preview.pause`, `preview.zoom`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Single-frame sprite, preview playing | Shows frame 1 continuously; no error |
| Frame or loop tag deleted while previewing | Continues with an existing frame (AC-15) |
| Main-canvas playback and preview playback both running | Both run; each follows its own cursor |
| Sprite larger than the panel at 1× (e.g. 512×512) | Shown at 1× within the panel's scroll / clip area; Fit shrinks it to fit |
| Onion skin toggled while the preview plays | Preview output unchanged (AC-08) |
| Browser tab hidden while previewing | No preview frames are composed while hidden |
| Frame duration edited while playing | The next cycle uses the new duration |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Expanding the panel again after AC-13 shows the paused preview (playback does not resume by itself) | The plan says only "stop the loop when the panel is collapsed" | Low |
| Fit allows non-integer scales | The plan lists Fit next to integer zooms without a rule | Low — pixelated rendering still applies |

## Open questions

- Decision Log entry not yet written: the docked side panel (instead of a floating window) and the new public wasm API for compositing without onion skin (§17.5). Confirm the API shape — a new `composeFrame(frame, zoom, onion)` (plan recommendation) or an `onion` parameter on `compose`.
- Default shortcut for the Preview panel: none, or F7 (Aseprite's key, which browsers and OSes may grab)? The plan leaves it to the session.

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
| AC-16 |      | open   |        |

## Decisions

- 2026-10-28 (legacy plan): The preview is a docked side panel using the existing panel system — responsive, persisted and collapsible without new layout infrastructure.
- 2026-10-28 (legacy plan): The preview has its own cursor and loop and never changes the timeline's current frame — that is the point of watching while drawing.
- 2026-10-28 (legacy plan): The shown frame index is exposed as a `data-frame` attribute for tests — tests must not read canvas pixels.
