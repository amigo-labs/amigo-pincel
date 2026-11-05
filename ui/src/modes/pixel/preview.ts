// Preview panel playback and compositing (docs/specs/animation-preview.md).
// The preview has its own cursor and rAF loop and never touches
// `ed.currentFrame`, so the user can draw while it plays. It composites
// without onion skin through `composeFrame(frame, 1, false)`.

import { advance, startCursor, type PlayCursor, type PlayRange } from './playback';
import { playRange } from './doc-ops';
import { ed, live, preview } from './stores/editor.svelte';

/** Shortest time a frame stays up, so a zero duration cannot spin the loop. */
const MIN_FRAME_MS = 10;

/** `cursor` moved into `range` and onto a frame the document still has. */
function clampCursor(range: PlayRange, cursor: PlayCursor): PlayCursor {
  const last = Math.max(0, ed.frameCount - 1);
  const from = Math.min(range.from, last);
  const to = Math.min(range.to, last);
  return { frame: Math.min(Math.max(cursor.frame, from), to), step: cursor.step };
}

/** Stop preview playback; the panel goes back to the current frame. */
export function stopPreview() {
  preview.playing = false;
  if (live.previewHandle !== null) {
    cancelAnimationFrame(live.previewHandle);
    live.previewHandle = null;
  }
}

/** Start preview playback from the current frame, within the loop tag. */
export function startPreview() {
  if (!ed.doc || preview.playing) return;
  preview.playing = true;
  let cursor = startCursor(playRange(), ed.currentFrame);
  preview.frame = cursor.frame;
  let shownAt = performance.now();
  const duration = (frame: number) =>
    Math.max(MIN_FRAME_MS, ed.doc ? ed.doc.frameDuration(frame) : MIN_FRAME_MS);
  // rAF does not run in a hidden tab, so nothing is composed meanwhile;
  // the resync below keeps it from racing through frames on return.
  const step = (now: number) => {
    if (!preview.playing || !ed.doc) return;
    const range = playRange();
    cursor = clampCursor(range, cursor);
    let ms = duration(cursor.frame);
    while (now - shownAt >= ms) {
      shownAt += ms;
      cursor = clampCursor(range, advance(range, cursor));
      ms = duration(cursor.frame);
      if (now - shownAt > 1000) shownAt = now;
    }
    if (preview.frame !== cursor.frame) preview.frame = cursor.frame;
    live.previewHandle = requestAnimationFrame(step);
  };
  live.previewHandle = requestAnimationFrame(step);
}

export function togglePreview() {
  if (preview.playing) stopPreview();
  else startPreview();
}

/** The frame the preview shows: its own cursor while playing, else the current frame. */
export function previewShownFrame(): number {
  const last = Math.max(0, ed.frameCount - 1);
  return Math.min(preview.playing ? preview.frame : ed.currentFrame, last);
}

// Composites per frame, valid for one `contentRev`: playback revisits
// frames without composing them again until the document changes.
let cache = new Map<number, ImageData>();
let cacheRev = -1;

/** Draw `frame` (onion-free, 1×) into `canvas`, composing it only when needed. */
export function drawPreview(canvas: HTMLCanvasElement, frame: number) {
  const doc = ed.doc;
  if (!doc || frame >= ed.frameCount) return;
  if (cacheRev !== preview.contentRev) {
    cache = new Map();
    cacheRev = preview.contentRev;
  }
  let image = cache.get(frame);
  if (!image) {
    const composed = doc.composeFrame(frame, 1, false);
    try {
      image = new ImageData(
        new Uint8ClampedArray(composed.pixels),
        composed.width,
        composed.height,
      );
    } finally {
      composed.free();
    }
    cache.set(frame, image);
  }
  if (canvas.width !== image.width) canvas.width = image.width;
  if (canvas.height !== image.height) canvas.height = image.height;
  canvas.getContext('2d')?.putImageData(image, 0, 0);
}
