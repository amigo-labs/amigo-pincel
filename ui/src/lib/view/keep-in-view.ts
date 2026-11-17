// Keeps a zoomed / panned document from leaving the canvas area.

/** CSS px of the document that stay in view on each axis (all of it when
 *  the document is smaller). */
export const KEEP_IN_VIEW = 48;

/**
 * The offset of a document's leading edge, clamped so that at least
 * `keep` CSS px of it (or all of it, when it or the viewport is smaller)
 * overlap a viewport `viewSize` px long. `size` is the document's length
 * on screen. One axis; call once for x and once for y.
 */
export function keepInView(
  offset: number,
  size: number,
  viewSize: number,
  keep: number = KEEP_IN_VIEW,
): number {
  const k = Math.max(0, Math.min(keep, size, viewSize));
  return Math.min(Math.max(offset, k - size), viewSize - k);
}
