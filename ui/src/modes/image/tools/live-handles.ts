// Handles of a live object (docs/specs/image-live-objects-ui.md): where they
// are, what a press hits, and how a drag reshapes the object. Pure geometry
// over the core's reported `LiveObjectSpec`, so the overlay and the pointer
// code agree and Undo / MCP changes move the handles.
import type { LiveObjectSpec, PointPair } from '../core/wasm';

/** A live object with points and handles: a Line, box shape or Gradient
 *  (a live text has its own box, caret and handle). */
export type ShapeObject = Exclude<LiveObjectSpec, { type: 'text' }>;

/** A point in canvas (document) space. */
export type Pt = [number, number];

/** A box handle: corners and edge midpoints of the normalized box. */
export type BoxHandle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

/** What a press on a live object grabs. */
export type LiveHit =
  | { kind: 'box'; handle: BoxHandle }
  | { kind: 'end'; index: 0 | 1 }
  /** A Line's control point `index` (docs/specs/image-shapes-curves.md). */
  | { kind: 'ctrl'; index: number }
  | { kind: 'grip' }
  | { kind: 'body' }
  | { kind: 'stroke' };

/** Most points a line may have, endpoints included (image-shapes-curves
 *  AC-15; `MAX_LINE_POINTS` in the core). */
export const MAX_LINE_POINTS = 64;

/** A live line's geometry: its endpoints and its control points. */
export interface LineGeom {
  points: PointPair;
  control: Pt[];
}

/** A live Line (or box shape) as the core reports it. */
type LiveShape = Extract<ShapeObject, { type: 'shape' }>;

/** Whether the object is a live Line. */
export function isLine(obj: ShapeObject): obj is LiveShape & { shape: 'line' } {
  return obj.type === 'shape' && obj.shape === 'line';
}

/** The distance between two points. */
function dist(a: Pt, b: Pt): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1]);
}

/** Hit radius around a handle in CSS px: mouse, and touch / pen (AC-05). */
export const HANDLE_REACH = { mouse: 8, touch: 16 } as const;
/** Distance of a Line's move grip from the segment's midpoint, CSS px. */
export const GRIP_OFFSET = 16;
/** Hit band around a Line / Gradient segment, CSS px (Terms: Body). */
export const SEGMENT_REACH = 8;

/** Whether the object is a box shape (not a Line, not a Gradient). */
export function isBox(obj: ShapeObject): boolean {
  return obj.type === 'shape' && obj.shape !== 'line';
}

/** The normalized box `[x0, y0, x1, y1]` of two corners. */
function box(points: PointPair): [number, number, number, number] {
  const [a, b] = points;
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}

/** Where each box handle sits, in canvas space. */
export function boxHandles(points: PointPair): Array<[BoxHandle, Pt]> {
  const [x0, y0, x1, y1] = box(points);
  const mx = (x0 + x1) / 2;
  const my = (y0 + y1) / 2;
  return [
    ['nw', [x0, y0]],
    ['ne', [x1, y0]],
    ['se', [x1, y1]],
    ['sw', [x0, y1]],
    ['n', [mx, y0]],
    ['e', [x1, my]],
    ['s', [mx, y1]],
    ['w', [x0, my]],
  ];
}

/** A Line's move grip: `GRIP_OFFSET` CSS px beside the midpoint,
 *  perpendicular to the segment. `zoom` is CSS px per canvas px. */
export function gripOf(points: PointPair, zoom: number): Pt {
  const [a, b] = points;
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = Math.hypot(dx, dy) || 1;
  const off = GRIP_OFFSET / zoom;
  return [(a[0] + b[0]) / 2 - (dy / len) * off, (a[1] + b[1]) / 2 + (dx / len) * off];
}

/** A line's move grip: `GRIP_OFFSET` CSS px beside the curve's midpoint by
 *  arc length, along the normal there, as the core reports them
 *  (image-shapes-curves AC-22). */
export function lineGripOf(anchor: Pt, normal: Pt, zoom: number): Pt {
  const off = GRIP_OFFSET / zoom;
  return [anchor[0] + normal[0] * off, anchor[1] + normal[1] * off];
}

/** Every handle of `obj` with its id, in canvas space (for drawing). Handles
 *  come before the grip, so a handle wins over the grip on a tie. */
export function handlesOf(obj: ShapeObject, zoom: number): Array<{ hit: LiveHit; at: Pt }> {
  if (isBox(obj)) {
    return boxHandles(obj.points).map(([handle, at]) => ({ hit: { kind: 'box', handle }, at }));
  }
  const [a, b] = obj.points;
  const out: Array<{ hit: LiveHit; at: Pt }> = [
    { hit: { kind: 'end', index: 0 }, at: a },
    { hit: { kind: 'end', index: 1 }, at: b },
  ];
  if (isLine(obj)) {
    obj.control.forEach((at, index) => out.push({ hit: { kind: 'ctrl', index }, at }));
    const grip =
      obj.polyline.length > 0
        ? lineGripOf(obj.grip_anchor, obj.grip_normal, zoom)
        : gripOf(obj.points, zoom);
    out.push({ hit: { kind: 'grip' }, at: grip });
  } else if (obj.type === 'shape') {
    out.push({ hit: { kind: 'grip' }, at: gripOf(obj.points, zoom) });
  }
  return out;
}

/** The nearest segment of `poly` to `p`: its index and the distance. */
function nearestSegment(poly: Pt[], p: Pt): { index: number; d: number } {
  let best = { index: 0, d: Infinity };
  poly.forEach((a, i) => {
    const b = poly[i + 1] ?? a;
    const d = segmentDistance(p, a, b);
    if (d < best.d && (i + 1 < poly.length || poly.length === 1)) best = { index: i, d };
  });
  return best;
}

/** The line's curve as the core draws it (its endpoints when unknown). */
function curveOf(obj: LiveShape): Pt[] {
  return obj.polyline.length > 0 ? obj.polyline : [obj.points[0], obj.points[1]];
}

/**
 * Where a control point pressed onto the stroke at `p` goes in the
 * line's `control` list: between the two points of the line it lies between
 * (image-shapes-curves AC-02).
 */
export function insertIndex(obj: ShapeObject, p: Pt): number {
  if (!isLine(obj)) return 0;
  const pts: Pt[] = [obj.points[0], ...obj.control, obj.points[1]];
  const poly = curveOf(obj);
  if (poly.length <= 2) {
    // A straight curve: order by the position along it.
    const [a, b] = obj.points;
    const ux = b[0] - a[0];
    const uy = b[1] - a[1];
    const along = (q: Pt) => (q[0] - a[0]) * ux + (q[1] - a[1]) * uy;
    return obj.control.filter((c) => along(c) < along(p)).length;
  }
  // Each point of the line is a vertex of the polyline, in order.
  const at: number[] = [];
  let from = 0;
  for (const q of pts) {
    let best = from;
    let bestD = Infinity;
    poly.forEach((v, i) => {
      const d = dist(v, q);
      if (i >= from && d < bestD) {
        best = i;
        bestD = d;
      }
    });
    at.push(best);
    from = best;
  }
  const seg = nearestSegment(poly, p).index;
  // The first point of the line whose vertex lies past the segment ends it.
  const next = at.findIndex((v, i) => i > 0 && seg < v);
  return next === -1 ? obj.control.length : next - 1;
}

/** Distance from `p` to the segment `a`–`b`. */
function segmentDistance(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  const t =
    len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2));
  return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
}

/**
 * What a press at canvas point `p` grabs on `obj`, or `null` when it misses
 * (beside the object: a commit). `zoom` is CSS px per canvas px; `touch` is
 * a touch or pen press. The nearest handle in reach wins; corners win ties.
 */
export function hitLive(obj: ShapeObject, p: Pt, zoom: number, touch: boolean): LiveHit | null {
  const reach = (touch ? HANDLE_REACH.touch : HANDLE_REACH.mouse) / zoom;
  let best: { hit: LiveHit; d: number } | null = null;
  for (const h of handlesOf(obj, zoom)) {
    const d = Math.hypot(p[0] - h.at[0], p[1] - h.at[1]);
    if (d <= reach && (best === null || d < best.d)) best = { hit: h.hit, d };
  }
  if (best) return best.hit;
  if (isBox(obj)) {
    const [x0, y0, x1, y1] = box(obj.points);
    return p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1 ? { kind: 'body' } : null;
  }
  const [a, b] = obj.points;
  const half = obj.type === 'shape' ? obj.stroke_width / 2 : 0;
  // The stroke reaches half the stroke width or 8 CSS px (16 for touch and
  // pen), whichever is larger (image-shapes-curves AC-02).
  const css = touch ? HANDLE_REACH.touch : SEGMENT_REACH;
  const band = Math.max(css / zoom, half);
  if (isLine(obj)) return nearestSegment(curveOf(obj), p).d <= band ? { kind: 'stroke' } : null;
  if (segmentDistance(p, a, b) > band) return null;
  return obj.type === 'shape' ? { kind: 'stroke' } : { kind: 'body' };
}

/** The CSS cursor over a hit (AC-13); `null` keeps the tool's cursor. */
export function cursorOf(hit: LiveHit | null): string | null {
  if (hit === null || hit.kind === 'stroke') return null;
  if (hit.kind === 'body' || hit.kind === 'grip') return 'move';
  if (hit.kind === 'end' || hit.kind === 'ctrl') return 'crosshair';
  switch (hit.handle) {
    case 'nw':
    case 'se':
      return 'nwse-resize';
    case 'ne':
    case 'sw':
      return 'nesw-resize';
    case 'n':
    case 's':
      return 'ns-resize';
    default:
      return 'ew-resize';
  }
}

/** 45° snap of `p` around `origin` (Shift on a Line / Gradient end). */
function snap45(origin: Pt, p: Pt): Pt {
  const dx = p[0] - origin[0];
  const dy = p[1] - origin[1];
  const angle = (Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI) / 4;
  const len = Math.hypot(dx, dy);
  return [origin[0] + Math.cos(angle) * len, origin[1] + Math.sin(angle) * len];
}

/** Squares the box spanned by `anchor` and `p` on its longer side. */
function square(anchor: Pt, p: Pt): Pt {
  const dx = p[0] - anchor[0];
  const dy = p[1] - anchor[1];
  const side = Math.max(Math.abs(dx), Math.abs(dy));
  return [anchor[0] + Math.sign(dx || 1) * side, anchor[1] + Math.sign(dy || 1) * side];
}

/**
 * The object's new points when the drag that grabbed `hit` at `start` is
 * at `p`. `from` are the points when the drag began. A corner or edge
 * dragged past the opposite one flips the box (the core normalizes it).
 */
export function dragPoints(
  hit: LiveHit,
  from: PointPair,
  start: Pt,
  p: Pt,
  shift: boolean,
): PointPair {
  const dx = p[0] - start[0];
  const dy = p[1] - start[1];
  const [a, b] = from;
  if (hit.kind === 'body' || hit.kind === 'grip') {
    return [
      [a[0] + dx, a[1] + dy],
      [b[0] + dx, b[1] + dy],
    ];
  }
  if (hit.kind === 'end') {
    const other = hit.index === 0 ? b : a;
    const moved: Pt = [from[hit.index][0] + dx, from[hit.index][1] + dy];
    const end = shift ? snap45(other, moved) : moved;
    return hit.index === 0 ? [end, [b[0], b[1]]] : [[a[0], a[1]], end];
  }
  if (hit.kind === 'stroke' || hit.kind === 'ctrl') return from;
  // The edges as they started (x0 / y0 the low ones), moved by the drag; a
  // dragged edge may cross the opposite one.
  let [x0, y0, x1, y1] = box(from);
  const h = hit.handle;
  if (h.includes('w')) x0 += dx;
  if (h.includes('e')) x1 += dx;
  if (h.includes('n')) y0 += dy;
  if (h.includes('s')) y1 += dy;
  if (shift && h.length === 2) {
    // Square anchored at the opposite corner.
    const anchor: Pt = [h.includes('w') ? x1 : x0, h.includes('n') ? y1 : y0];
    const corner = square(anchor, [h.includes('w') ? x0 : x1, h.includes('n') ? y0 : y1]);
    if (h.includes('w')) x0 = corner[0];
    else x1 = corner[0];
    if (h.includes('n')) y0 = corner[1];
    else y1 = corner[1];
  }
  // Each point keeps the edges it sat on, so the first point — the drag
  // start that orients a Block Arrow or a Callout — moves with its edges,
  // also past the opposite one (image-shapes-curves-shapes AC-20).
  const onLow = (p: Pt, other: Pt, i: 0 | 1) => p[i] < other[i] || (p[i] === other[i] && p === a);
  const place = (p: Pt, other: Pt): Pt => [
    onLow(p, other, 0) ? x0 : x1,
    onLow(p, other, 1) ? y0 : y1,
  ];
  return [place(a, b), place(b, a)];
}

/** A live text's box corner moved by the drag `start` → `p`, rounded to
 *  whole document pixels (docs/specs/image-live-text.md AC-07). */
export function wholeMove(from: PointPair, start: Pt, p: Pt): PointPair {
  const dx = Math.round(p[0] - start[0]);
  const dy = Math.round(p[1] - start[1]);
  const [a] = from;
  return [
    [a[0] + dx, a[1] + dy],
    [a[0] + dx, a[1] + dy],
  ];
}

/**
 * A live line's new geometry while the drag that grabbed `hit` at `start`
 * is at `p` (image-shapes-curves AC-02, AC-03, AC-22). `from` is the
 * geometry when the drag began. With Shift an endpoint snaps the segment
 * from its neighbouring point (the adjacent control point, or the other
 * endpoint) to 45° steps; a control point ignores Shift.
 */
export function dragLine(hit: LiveHit, from: LineGeom, start: Pt, p: Pt, shift: boolean): LineGeom {
  const dx = p[0] - start[0];
  const dy = p[1] - start[1];
  const [a, b] = from.points;
  const move = (q: Pt): Pt => [q[0] + dx, q[1] + dy];
  if (hit.kind === 'grip' || hit.kind === 'body') {
    return { points: [move(a), move(b)], control: from.control.map(move) };
  }
  if (hit.kind === 'ctrl') {
    const control = from.control.map((q, i) => (i === hit.index ? move(q) : q));
    return { points: [a, b], control };
  }
  if (hit.kind === 'end') {
    const last = from.control.length - 1;
    const neighbour = hit.index === 0 ? (from.control[0] ?? b) : (from.control[last] ?? a);
    const moved = move(hit.index === 0 ? a : b);
    const end = shift ? snap45(neighbour, moved) : moved;
    return { points: hit.index === 0 ? [end, b] : [a, end], control: from.control };
  }
  return from;
}
