// The document pixel under the pointer, for the status bar
// (docs/specs/frame-layout.md AC-08).

import { ed, hover, view } from './stores/editor.svelte';

/** The event comes from the stage, the canvas or the canvas's wrapper —
 *  not from another child of the stage (the tile editor, an overlay). */
function fromStage(e: Event): boolean {
  const target = e.target;
  return (
    target === e.currentTarget ||
    target === view.canvas ||
    (view.canvas !== null && target === view.canvas.parentElement)
  );
}

/**
 * Keeps `hover.pixel`: the document pixel under the pointer of `e`, or
 * none — for `null`, beside the document, and over another child of the
 * stage.
 */
export function noteHover(e: PointerEvent | WheelEvent | null): void {
  let pixel: { x: number; y: number } | null = null;
  if (e !== null && fromStage(e) && view.canvas) {
    const rect = view.canvas.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      const x = Math.floor(((e.clientX - rect.left) * view.canvas.width) / rect.width);
      const y = Math.floor(((e.clientY - rect.top) * view.canvas.height) / rect.height);
      if (x >= 0 && y >= 0 && x < ed.canvasW && y < ed.canvasH) pixel = { x, y };
    }
  }
  const now = hover.pixel;
  if (now?.x !== pixel?.x || now?.y !== pixel?.y) hover.pixel = pixel;
}
