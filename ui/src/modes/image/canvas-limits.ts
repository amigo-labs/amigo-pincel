// Image-mode canvas size limits (image-files AC-16), rune-free so node-side
// Playwright specs can import them.

/** Largest canvas edge the UI lets the user create / resize / scale to.
 * The renderer only ever draws the visible rect into a viewport-sized
 * canvas, so browser canvas limits no longer cap the document; memory
 * does. The core accepts 32767 per edge, but each layer is width × height
 * × 4 bytes of wasm memory (4 GiB address space in total), so the edge is
 * capped at 16384 and the area at `MAX_CANVAS_AREA`. */
export const MAX_CANVAS_DIM = 16384;

/** Largest canvas area (64 M px = 256 MiB per layer): room for a few
 *  layers, their undo snapshots and the composite inside wasm32 memory. */
export const MAX_CANVAS_AREA = 8192 * 8192;

/** Clamp a requested canvas size to the edge and area limits, scaling
 *  both edges down together when only the area is too large. */
export function clampCanvasSize(width: number, height: number): [number, number] {
  let w = Math.max(1, Math.min(MAX_CANVAS_DIM, Math.round(width)));
  let h = Math.max(1, Math.min(MAX_CANVAS_DIM, Math.round(height)));
  if (w * h > MAX_CANVAS_AREA) {
    const k = Math.sqrt(MAX_CANVAS_AREA / (w * h));
    w = Math.max(1, Math.floor(w * k));
    h = Math.max(1, Math.floor(h * k));
  }
  return [w, h];
}
