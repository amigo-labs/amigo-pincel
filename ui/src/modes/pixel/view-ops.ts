// Viewport: zoom, fit, wheel and pinch.

import { fitZoom } from '../../lib/view/fit';
import type { PinchFrame } from '../../lib/view/pinch';
import { ed, live, view } from './stores/editor.svelte';

/** Integer display-zoom range (sprite px → CSS px). */
export const MIN_ZOOM = 1;
export const MAX_ZOOM = 64;

// CSS px of breathing room left on each edge when auto-fitting a
// sprite to the viewport (see `fitView`).
export const FIT_MARGIN = 24;

// Clamp + apply a new zoom level. Pan offset stays in CSS pixels,
// so the canvas's flex-centered position keeps the sprite center
// anchored across zoom changes; further pan offsets shift uniformly
// from there.
export function setZoom(next: number) {
  next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, Math.floor(next)));
  if (next === view.zoom) return;
  view.zoom = next;
}

export function zoomIn() {
  setZoom(view.zoom * 2);
}

export function zoomOut() {
  setZoom(view.zoom >>> 1);
}

// Fit the whole sprite to the viewport: the largest integer zoom that
// shows it with a small margin, then re-center (pan 0). Runs on every
// document replacement (new / open / recover) so a freshly-loaded
// sprite always lands fully in view regardless of its dimensions, and
// backs the "Reset" control + the `0` shortcut. Falls back to the
// historical 8× default when the stage hasn't been measured yet
// (e.g. first paint before layout settles).
export function fitView() {
  view.zoom =
    view.stageW > 0 && view.stageH > 0
      ? fitZoom(view.stageW, view.stageH, ed.canvasW, ed.canvasH, MIN_ZOOM, MAX_ZOOM, FIT_MARGIN)
      : 8;
  view.panX = 0;
  view.panY = 0;
}

// 1:1 — one screen pixel per sprite pixel, re-centered.
export function zoomActual() {
  setZoom(1);
  view.panX = 0;
  view.panY = 0;
}

// "Reset view" — re-centers and fits the sprite to the viewport.
// Wired to the toolbar button and the View ▸ Reset Zoom menu item.
export function resetView() {
  fitView();
}

// Cursor-anchored mouse-wheel zoom. The canvas is flex-centered, so it
// scales about its own center for a fixed pan — which means keeping the
// sprite pixel under the cursor put is a pure pan adjustment, computed
// from the live rect with no async DOM read. Wheel up zooms in, down
// zooms out; steps are multiplicative but nudged by ±1 so low zoom
// levels still respond. See docs/specs/pincel.md §5 (Move/zoom).
export function onWheel(e: WheelEvent) {
  if (!view.canvas) return;
  // Ignore purely horizontal gestures (deltaY === 0): they aren't a
  // zoom intent, and swallowing them would suppress horizontal
  // trackpad scroll over the canvas.
  if (e.deltaY === 0) return;
  // Stop the gesture from scrolling the page / parent containers.
  e.preventDefault();
  const rect = view.canvas.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;
  const oldZoom = view.zoom;
  let next = e.deltaY < 0 ? Math.round(oldZoom * 1.25) : Math.round(oldZoom / 1.25);
  if (next === oldZoom) next = oldZoom + (e.deltaY < 0 ? 1 : -1);
  next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
  if (next === oldZoom) return;
  const ratio = next / oldZoom;
  const rectCx = rect.left + rect.width / 2;
  const rectCy = rect.top + rect.height / 2;
  view.panX += (rectCx - e.clientX) * (ratio - 1);
  view.panY += (rectCy - e.clientY) * (ratio - 1);
  view.zoom = next;
}

export function applyPinch(f: PinchFrame) {
  if (!view.canvas) return;
  const rect = view.canvas.getBoundingClientRect();
  const next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, Math.round(live.pinchZoom0 * f.scale)));
  if (next !== view.zoom && rect.width > 0) {
    // Same anchoring as `onWheel`: keep the sprite pixel under the
    // fingers' midpoint in place.
    const ratio = next / view.zoom;
    view.panX += (rect.left + rect.width / 2 - f.midX) * (ratio - 1);
    view.panY += (rect.top + rect.height / 2 - f.midY) * (ratio - 1);
    view.zoom = next;
  }
  view.panX += f.dx;
  view.panY += f.dy;
}
