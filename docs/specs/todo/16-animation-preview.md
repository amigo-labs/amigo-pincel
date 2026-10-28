---
id: todo-animation-preview
title: Animation preview panel (Pixel mode)
status: todo
size: M (one session; a floating window would be a second)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Animation preview panel (Pixel mode)

## Goal

A **Preview** panel shows the animation at 1:1 (or 2×, 4×, Fit) and plays it on its own loop, **while you keep drawing on the zoomed-in main canvas**. It shows the current frame when paused, has its own play button, and follows the timeline's loop-tag setting.

## Why

Animators judge motion at real size. Aseprite has a preview window (F7), and Pixelorama and Piskel have a live preview. In Pincel, playback today drives the **main canvas** itself: `togglePlay()` changes `ed.currentFrame`. So you can watch or draw, never both at once.

## Current state (read these first)

**Playback** (pure functions). `ui/src/modes/pixel/playback.ts`
- `PlayRange { from, to, direction }`, `startCursor` (≈L23), `advance(range, cursor)` (≈L33): forward, reverse and ping-pong.
- **Reuse these.**

**Playback loop.** `ui/src/modes/pixel/doc-ops.ts`
- `playRange()` (≈L174–183) takes `play.loopTag` (tag from, to, direction) or all frames.
- `togglePlay()` (≈L196–221) is a rAF loop on wall-clock time using `ed.doc.frameDuration(frame)`, and calls **`setFrame(cursor.frame)`**.
- `stopPlayback()` (≈L185).

**State.** `ui/src/modes/pixel/stores/editor.svelte.ts`
- `play = { playing, loopTag, onion }` (≈L111–118)
- `ed.currentFrame` / `ed.frameCount` (≈L47)
- `live.playHandle` (≈L155)

**Compose.**
- `render.ts` `recompose()` (≈L71) calls `ed.doc.compose(ed.currentFrame, 1)`, then `renderer.draw(frame)`, then `frame.free()`.
- wasm `compose(frame, zoom)` (`crates/pincel-wasm/src/lib.rs` ≈L470) **always applies the document's onion skin** (`request.onion_skin = self.onion_skin`, ≈L515). A preview must not show onion ghosts.

**Blitting.** `blitFrame(canvas, frame)` in `ui/src/lib/render/canvas2d.ts` (≈L14–34) resizes the canvas and calls `putImageData`. That is enough for a small preview: CSS `width` sets the zoom, with `image-rendering: pixelated`.

**Panels.**
- `PanelDef { id, title, icon, body, placement?: 'side' | 'bottom', grow?, defaultCollapsed? }` (`ui/src/lib/components/layout/panels.svelte.ts` ≈L12–23).
- Collapse state persists per mode (`layout:pixel` pref).
- **No floating placement exists.**
- The Pixel panel list is in `PixelEditor.svelte` (≈L764–776): colors, palette, layers, tilesets, slices, timeline. On phones every panel becomes a sheet tab automatically (`EditorFrame.svelte` ≈L171–197).

## Design

**Placement.**
- A **docked side panel** "Preview", `defaultCollapsed: true`, placed above Layers. It uses the existing panel system, so it is responsive, persisted and collapsible with no new layout infrastructure.
- A floating, undockable window is a separate follow-up and needs a Decision Log entry for floating panels.

**Compose without onion.**
- Add `composePlain(frame, zoom)` to wasm, or an `onion: bool` parameter on `compose`. **Recommendation:** a new `composeFrame(frame: u32, zoom: u32, onion: bool)`, with `compose` delegating to it, so the existing API stays unchanged.
- The npm API is public (§17.5), so this needs a doc comment and a Decision Log line.

**Own cursor and loop.**
- A `preview` state `{ playing, cursor, zoom: 1 | 2 | 4 | 'fit' }`.
- Its own rAF loop using `startCursor` / `advance` and `frameDuration`. It **never touches `ed.currentFrame`**.
- When paused, it shows `ed.currentFrame`.
- The range follows `play.loopTag`, the same setting as the timeline.
- Main-canvas playback (`togglePlay`) keeps working as it does today.

**Cost.**
- Only compose when the shown frame changes or the document revision changes.
- Cache the last `ImageData` per frame index and invalidate it on any edit. Pixel mode bumps a revision on each command; find the counter render.ts uses for dirty recompose.
- Stop the loop when the panel is collapsed or the tab is hidden (`document.visibilityState`).

**UI.**
- The panel body: the canvas (centered, checkerboard background), a play/pause button, and a zoom select.
- Shortcut: none by default. F7 is Aseprite's key, but browsers and OSes may grab F-keys; decide in the session.
- i18n: `panel.preview`, `preview.play`, `preview.pause`, `preview.zoom`.

## Tasks (one M session)

1. **`docs(spec)`.** §9.2 panel list plus the Preview panel; Decision Log entry for the docked-panel choice and the wasm `composeFrame`.
2. **`feat(pincel-wasm)`.** `composeFrame(frame, zoom, onion)`.
   - Test `compose_frame_without_onion_ignores_onion_settings`, alongside the existing onion tests in pincel-wasm. Find them with `grep -n onion crates/pincel-wasm/src/lib.rs`.
3. **`feat(ui)`.** The preview store, its loop and the panel.
   - e2e `ui/e2e/preview.spec.ts`:
     - a 2-frame sprite with different colors;
     - open the Preview panel and start preview playback;
     - **the main canvas's current frame stays the same** (the timeline highlights frame 1);
     - the preview canvas's `data-frame` attribute changes over time. Expose the shown frame index as a `data-frame` attribute for tests; never read canvas pixels.

## Out of scope

- A floating, resizable window.
- Previewing tile-wrapped output (see `07-symmetry-tiled-mode.md`).
- Exporting from the preview.
