// Drag geometry of the Image tools: snapped selection rectangles and the
// Shift constraint of the shape tools (spec §8.2, §9.2).
import type { ShapeKind } from '../core/wasm';

/**
 * Normalizes two corners into a positive-size rect, each corner snapped to
 * the nearest pixel grid line on its own (rounding the size instead would
 * drop the column under a far edge).
 *
 * Shift is reserved for the selection combine mode (add), so rectangle/ellipse
 * do not also constrain to a square on Shift — that would conflict with the
 * documented modifier mapping (spec §8.2 vs §9.3).
 */
export function rectFromCorners(
  a: [number, number],
  b: [number, number],
): { x: number; y: number; w: number; h: number } {
  const x0 = Math.round(Math.min(a[0], b[0]));
  const y0 = Math.round(Math.min(a[1], b[1]));
  const x1 = Math.round(Math.max(a[0], b[0]));
  const y1 = Math.round(Math.max(a[1], b[1]));
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** The rubber band of a rect/ellipse drag: the snapped rect it will commit. */
export function snappedCorners(a: [number, number], b: [number, number]): [number, number][] {
  const r = rectFromCorners(a, b);
  return [
    [r.x, r.y],
    [r.x + r.w, r.y + r.h],
  ];
}

/**
 * Applies the Shapes-tool Shift constraint to the drag endpoint (spec §9.2):
 * a square box for area shapes, a 45°-snapped segment for a line.
 */
export function constrainShape(
  a: [number, number],
  p: [number, number],
  kind: ShapeKind,
  shift: boolean,
): [number, number] {
  if (!shift) {
    return p;
  }
  const dx = p[0] - a[0];
  const dy = p[1] - a[1];
  if (kind === 'line') {
    const angle = (Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI) / 4;
    const len = Math.hypot(dx, dy);
    return [a[0] + Math.cos(angle) * len, a[1] + Math.sin(angle) * len];
  }
  const side = Math.max(Math.abs(dx), Math.abs(dy));
  return [a[0] + Math.sign(dx || 1) * side, a[1] + Math.sign(dy || 1) * side];
}
