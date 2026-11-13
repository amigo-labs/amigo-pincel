// Geometry of the Move tools' transform box (docs/specs/image-transform-handles.md):
// the box, its eight handles and pivot, hit-testing in screen pixels and
// what a drag of each part does to the box, and the maps the drag preview
// draws with. Pure and rune-free, so a Node-side Playwright spec can drive
// it; the core applies the result (`begin_transform` / `edit_transform`).

type Pt = readonly [number, number];

/** The box: centre, size along its own axes, turn (degrees, clockwise on
 *  screen) and pivot relative to the box (`[0.5, 0.5]` = its centre). */
export interface BoxState {
  readonly center: Pt;
  readonly size: Pt;
  readonly angle: number;
  readonly pivot: Pt;
}

/** Corners in the order top-left, top-right, bottom-right, bottom-left. */
export type Corner = 0 | 1 | 2 | 3;
export type Edge = 'n' | 'e' | 's' | 'w';

/** What a press hits. */
export type BoxHit =
  | { readonly kind: 'corner'; readonly corner: Corner }
  | { readonly kind: 'edge'; readonly edge: Edge }
  | { readonly kind: 'pivot' }
  | { readonly kind: 'inside' }
  | { readonly kind: 'outside' };

/** Hit radius of a handle and of the box outline, screen px (AC-05). */
export const HANDLE_HIT = 12;
/** Hit radius of the pivot marker, screen px. */
export const PIVOT_HIT = 10;
/** A press that moves less than this before release is a click (AC-08). */
export const CLICK_SLOP = 4;

/** The box of `[x, y, w, h]` bounds, untransformed. */
export function boxOfBounds([x, y, w, h]: readonly [number, number, number, number]): BoxState {
  return { center: [x + w / 2, y + h / 2], size: [w, h], angle: 0, pivot: [0.5, 0.5] };
}

function axes(angle: number): { u: Pt; v: Pt } {
  const r = (angle * Math.PI) / 180;
  const [c, s] = [Math.cos(r), Math.sin(r)];
  return { u: [c, s], v: [-s, c] };
}

/** The canvas point of box-local `(lx, ly)` (`(0, 0)` = the centre). */
function toCanvas(b: BoxState, lx: number, ly: number): [number, number] {
  const { u, v } = axes(b.angle);
  return [b.center[0] + u[0] * lx + v[0] * ly, b.center[1] + u[1] * lx + v[1] * ly];
}

/** Box-local coordinates of canvas point `p`. */
function toLocal(b: BoxState, p: Pt): [number, number] {
  const { u, v } = axes(b.angle);
  const d: Pt = [p[0] - b.center[0], p[1] - b.center[1]];
  return [d[0] * u[0] + d[1] * u[1], d[0] * v[0] + d[1] * v[1]];
}

const CORNER_SIGNS: readonly [Pt, Pt, Pt, Pt] = [
  [-1, -1],
  [1, -1],
  [1, 1],
  [-1, 1],
];
const EDGE_SIGNS: Record<Edge, Pt> = { n: [0, -1], e: [1, 0], s: [0, 1], w: [-1, 0] };

/** The four corners on the canvas (top-left, top-right, bottom-right,
 *  bottom-left). */
export function corners(b: BoxState): [number, number][] {
  return CORNER_SIGNS.map(([sx, sy]) => toCanvas(b, (sx * b.size[0]) / 2, (sy * b.size[1]) / 2));
}

/** The pivot on the canvas. */
export function pivotPoint(b: BoxState): [number, number] {
  return toCanvas(b, (b.pivot[0] - 0.5) * b.size[0], (b.pivot[1] - 0.5) * b.size[1]);
}

/** Every handle with its canvas position, corners first. */
export function handles(b: BoxState): { hit: BoxHit; at: [number, number] }[] {
  const out: { hit: BoxHit; at: [number, number] }[] = corners(b).map((at, i) => ({
    hit: { kind: 'corner', corner: i as Corner },
    at,
  }));
  for (const edge of ['n', 'e', 's', 'w'] as const) {
    const [sx, sy] = EDGE_SIGNS[edge];
    out.push({
      hit: { kind: 'edge', edge },
      at: toCanvas(b, (sx * b.size[0]) / 2, (sy * b.size[1]) / 2),
    });
  }
  return out;
}

/** What a press at canvas point `p` hits at `zoom` (screen px per canvas
 *  px): the pivot, a handle, the box (its inside, or within the hit
 *  radius of its outline) or the outside (AC-05). */
export function hitBox(b: BoxState, p: Pt, zoom: number): BoxHit {
  const near = (q: Pt, radius: number): boolean =>
    Math.hypot(q[0] - p[0], q[1] - p[1]) * zoom <= radius;
  if (near(pivotPoint(b), PIVOT_HIT)) return { kind: 'pivot' };
  // The nearest handle within reach: on a small box the hit areas overlap.
  let best: { hit: BoxHit; d: number } | null = null;
  for (const h of handles(b)) {
    const d = Math.hypot(h.at[0] - p[0], h.at[1] - p[1]) * zoom;
    if (d <= HANDLE_HIT && (best === null || d < best.d)) best = { hit: h.hit, d };
  }
  if (best !== null) return best.hit;
  const [lx, ly] = toLocal(b, p);
  const reach = HANDLE_HIT / zoom;
  return Math.abs(lx) <= b.size[0] / 2 + reach && Math.abs(ly) <= b.size[1] / 2 + reach
    ? { kind: 'inside' }
    : { kind: 'outside' };
}

/** The box moved by `(dx, dy)`; the pivot moves with it (AC-02). */
export function translateBox(b: BoxState, dx: number, dy: number): BoxState {
  return { ...b, center: [b.center[0] + dx, b.center[1] + dy] };
}

/** The box with `corner` dragged to `p`: the opposite corner stays, the
 *  box scales along its own axes, never below 1 px and never flipping;
 *  `keepRatio` keeps `start`'s width-to-height ratio (AC-03, AC-04). */
export function dragCorner(start: BoxState, corner: Corner, p: Pt, keepRatio: boolean): BoxState {
  const [sx, sy] = CORNER_SIGNS[corner];
  const fixed = toCanvas(start, (-sx * start.size[0]) / 2, (-sy * start.size[1]) / 2);
  const { u, v } = axes(start.angle);
  const d: Pt = [p[0] - fixed[0], p[1] - fixed[1]];
  let w = Math.max(1, (d[0] * u[0] + d[1] * u[1]) * sx);
  let h = Math.max(1, (d[0] * v[0] + d[1] * v[1]) * sy);
  if (keepRatio) {
    const f = Math.max(w / start.size[0], h / start.size[1]);
    w = Math.max(1, start.size[0] * f);
    h = Math.max(1, start.size[1] * f);
  }
  return withFixedPoint(start, fixed, [sx, sy], [w, h]);
}

/** The box with `edge` dragged to `p`: the opposite edge stays, the box
 *  scales along that axis only, never below 1 px (AC-03). */
export function dragEdge(start: BoxState, edge: Edge, p: Pt): BoxState {
  const [sx, sy] = EDGE_SIGNS[edge];
  const fixed = toCanvas(start, (-sx * start.size[0]) / 2, (-sy * start.size[1]) / 2);
  const { u, v } = axes(start.angle);
  const d: Pt = [p[0] - fixed[0], p[1] - fixed[1]];
  const w = sx === 0 ? start.size[0] : Math.max(1, (d[0] * u[0] + d[1] * u[1]) * sx);
  const h = sy === 0 ? start.size[1] : Math.max(1, (d[0] * v[0] + d[1] * v[1]) * sy);
  return withFixedPoint(start, fixed, [sx, sy], [w, h]);
}

/** The box of `size` whose point at signs `-(sx, sy)` stays at `fixed`. */
function withFixedPoint(start: BoxState, fixed: Pt, [sx, sy]: Pt, size: Pt): BoxState {
  const { u, v } = axes(start.angle);
  const [hx, hy] = [(sx * size[0]) / 2, (sy * size[1]) / 2];
  return {
    ...start,
    size: [size[0], size[1]],
    center: [fixed[0] + u[0] * hx + v[0] * hy, fixed[1] + u[1] * hx + v[1] * hy],
  };
}

/** The box turned about its pivot by the angle the pointer sweeps from
 *  `from` to `to` (clockwise positive); `snap` rounds the box's angle to
 *  multiples of 15° (AC-05). */
export function rotateBox(start: BoxState, from: Pt, to: Pt, snap: boolean): BoxState {
  const pv = pivotPoint(start);
  const a0 = Math.atan2(from[1] - pv[1], from[0] - pv[0]);
  const a1 = Math.atan2(to[1] - pv[1], to[0] - pv[0]);
  let angle = start.angle + ((a1 - a0) * 180) / Math.PI;
  if (snap) angle = Math.round(angle / 15) * 15;
  angle = normalizeAngle(angle);
  const delta = ((angle - start.angle) * Math.PI) / 180;
  const [c, s] = [Math.cos(delta), Math.sin(delta)];
  const d: Pt = [start.center[0] - pv[0], start.center[1] - pv[1]];
  return {
    ...start,
    angle,
    center: [pv[0] + c * d[0] - s * d[1], pv[1] + s * d[0] + c * d[1]],
  };
}

/** `angle` in (-180, 180]. */
export function normalizeAngle(angle: number): number {
  let a = angle % 360;
  if (a <= -180) a += 360;
  if (a > 180) a -= 360;
  return a;
}

/** The box with its pivot moved to canvas point `p`; nothing else changes
 *  (AC-06). */
export function movePivot(b: BoxState, p: Pt): BoxState {
  const [lx, ly] = toLocal(b, p);
  return { ...b, pivot: [lx / b.size[0] + 0.5, ly / b.size[1] + 0.5] };
}

/** A 2-D affine map `[a, b, c, d, e, f]`: `x' = a·x + b·y + c`,
 *  `y' = d·x + e·y + f`. */
export type Matrix = readonly [number, number, number, number, number, number];

/** `m` after `n`: the map that applies `n` first. */
function compose(m: Matrix, n: Matrix): Matrix {
  return [
    m[0] * n[0] + m[1] * n[3],
    m[0] * n[1] + m[1] * n[4],
    m[0] * n[2] + m[1] * n[5] + m[2],
    m[3] * n[0] + m[4] * n[3],
    m[3] * n[1] + m[4] * n[4],
    m[3] * n[2] + m[4] * n[5] + m[5],
  ];
}

/** The map from the box's unit square (`[-0.5, 0.5]²`) to the canvas. */
function frame(b: BoxState): Matrix {
  const { u, v } = axes(b.angle);
  return [
    u[0] * b.size[0],
    v[0] * b.size[1],
    b.center[0],
    u[1] * b.size[0],
    v[1] * b.size[1],
    b.center[1],
  ];
}

/** The inverse of `m`; `null` when it collapses the plane. */
function invert(m: Matrix): Matrix | null {
  const det = m[0] * m[4] - m[1] * m[3];
  if (!Number.isFinite(det) || det === 0) return null;
  const [a, b, d, e] = [m[4] / det, -m[1] / det, -m[3] / det, m[0] / det];
  return [a, b, -(a * m[2] + b * m[5]), d, e, -(d * m[2] + e * m[5])];
}

/** The map from patch pixels (`(0, 0)` = the corner of `bounds`, the
 *  selection's `[x, y, w, h]` when the transform began) to the canvas for
 *  box `b` — the map the core applies (`Base::forward` in
 *  `live_transform.rs`), which the drag preview draws with (AC-20). */
export function patchMatrix(
  b: BoxState,
  bounds: readonly [number, number, number, number],
): Matrix {
  return compose(frame(b), [1 / bounds[2], 0, -0.5, 0, 1 / bounds[3], -0.5]);
}

/** The map that takes box `from` onto box `to`: how the outline moves
 *  during a drag (AC-23); `null` for a degenerate `from`. */
export function relativeMatrix(from: BoxState, to: BoxState): Matrix | null {
  const inv = invert(frame(from));
  return inv === null ? null : compose(frame(to), inv);
}

/** Point `p` under map `m`. */
export function applyMatrix(m: Matrix, p: Pt): [number, number] {
  return [m[0] * p[0] + m[1] * p[1] + m[2], m[3] * p[0] + m[4] * p[1] + m[5]];
}
