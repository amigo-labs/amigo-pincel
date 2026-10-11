// Pixel-mode tool ids, their classification and the single-key shortcut
// map (spec §5). Pure data and functions, shared by PixelEditor and the
// overlay painter.

/** Every Pixel-mode tool. */
export type Tool =
  | 'pencil'
  | 'eraser'
  | 'eyedropper'
  | 'bucket'
  | 'line'
  | 'rectangle'
  | 'rectangle-fill'
  | 'ellipse'
  | 'ellipse-fill'
  | 'rounded-rect'
  | 'polygon-shape'
  | 'selection-rect'
  | 'selection-ellipse'
  | 'selection-lasso'
  | 'selection-polygon'
  | 'selection-wand'
  | 'text'
  | 'move'
  | 'tilemap-stamp'
  | 'slice';

/**
 * Tools that use the press / drag / release pipeline (a start point
 * captured on `pointerdown`, a live endpoint tracked on `pointermove`,
 * committed on `pointerup`). The Selection (Rect / Ellipse) tools share
 * the shape so a Shift constraint applies uniformly; the Slice tool
 * commits via `addSlice` / `setSliceKey` on release.
 */
export function isDragShapeTool(t: Tool): boolean {
  return (
    t === 'line' ||
    t === 'rectangle' ||
    t === 'rectangle-fill' ||
    t === 'ellipse' ||
    t === 'ellipse-fill' ||
    t === 'rounded-rect' ||
    t === 'polygon-shape' ||
    t === 'selection-rect' ||
    t === 'selection-ellipse' ||
    t === 'slice'
  );
}

/** Outline / fill shape tools committed through `applyShape`. */
export function isShapeTool(t: Tool): boolean {
  return t === 'rectangle' || t === 'ellipse' || t === 'rounded-rect' || t === 'polygon-shape';
}

/** Tools whose drag a held Shift constrains to a square bounding box. */
export function isSquareConstrainable(t: Tool): boolean {
  return (
    t === 'rectangle' ||
    t === 'rectangle-fill' ||
    t === 'ellipse' ||
    t === 'ellipse-fill' ||
    t === 'rounded-rect' ||
    t === 'polygon-shape'
  );
}

/**
 * Single-key tool shortcuts, aligned with Aseprite defaults where they
 * don't collide. Each key maps to a tool group: the first press selects
 * the group's first tool, repeated presses cycle through it (the Aseprite
 * pattern — `U` walks the shape variants).
 */
export const TOOL_KEYS: Record<string, Tool[]> = {
  b: ['pencil'],
  e: ['eraser'],
  i: ['eyedropper'],
  g: ['bucket'],
  l: ['line'],
  u: ['rectangle', 'rectangle-fill', 'ellipse', 'ellipse-fill', 'rounded-rect', 'polygon-shape'],
  m: ['selection-rect', 'selection-ellipse', 'selection-lasso', 'selection-polygon'],
  w: ['selection-wand'],
  t: ['text'],
  v: ['move'],
};

/** The tool a bare `key` press switches to from `current`, or `null`. */
export function toolForKey(key: string, current: Tool): Tool | null {
  const group = TOOL_KEYS[key.toLowerCase()];
  if (!group) return null;
  // `indexOf` is -1 when the current tool isn't in the group, so the first
  // press lands on the group's first entry.
  return group[(group.indexOf(current) + 1) % group.length] ?? null;
}

/** A point in sprite pixels. */
export interface Point {
  x: number;
  y: number;
}

/**
 * The drag endpoint after the Shift constraint: square bounding box for
 * rectangles, ellipses, rounded rectangles and polygons.
 */
export function constrainEndpoint(tool: Tool, start: Point, preview: Point, shift: boolean): Point {
  if (!shift || !isSquareConstrainable(tool)) return preview;
  const dx = preview.x - start.x;
  const dy = preview.y - start.y;
  const side = Math.max(Math.abs(dx), Math.abs(dy));
  return { x: start.x + side * (dx < 0 ? -1 : 1), y: start.y + side * (dy < 0 ? -1 : 1) };
}

/**
 * Every pixel on the Bresenham line from `a` to `b`, both ends included.
 * Pencil / eraser drags paint it between consecutive pointer samples so
 * a fast stroke stays connected.
 */
export function linePoints(a: Point, b: Point): Point[] {
  const out: Point[] = [];
  let { x, y } = a;
  const dx = Math.abs(b.x - a.x);
  const dy = -Math.abs(b.y - a.y);
  const sx = a.x < b.x ? 1 : -1;
  const sy = a.y < b.y ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    out.push({ x, y });
    if (x === b.x && y === b.y) return out;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
}
