// Pixel-mode tool ids, their classification and the tool-rail groups
// (spec §5, §9.2). Pure data and functions, shared by PixelEditor and the
// overlay painter.

import type { ToolGroup } from '../../lib/components/layout/tools';
import { toolHint } from '../../lib/shell/shortcuts';

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
 * The tool rail (spec §9.2): same group ids, order and keys as Image mode
 * (lib/shell/shortcuts.ts `TOOL_GROUP_KEYS`). Repeated key presses cycle a
 * group's variants — `U` walks the shapes, `M` the marquee shapes, `Q`
 * the lasso kinds. The Stamp needs a tile picked in the Tilesets panel.
 */
export function toolGroups(stampTile: { tilesetId: number; tileId: number } | null): ToolGroup<Tool>[] {
  const groups: ToolGroup<Tool>[] = [
    { id: 'move', label: 'Move', tools: [{ id: 'move', label: 'Move', icon: 'move' }] },
    {
      id: 'select',
      label: 'Marquee',
      tools: [
        { id: 'selection-rect', label: 'Rectangle Select', icon: 'select-rect' },
        { id: 'selection-ellipse', label: 'Ellipse Select', icon: 'select-ellipse' },
      ],
    },
    {
      id: 'lasso',
      label: 'Lasso',
      tools: [
        { id: 'selection-lasso', label: 'Lasso', icon: 'lasso' },
        {
          id: 'selection-polygon',
          label: 'Polygonal Lasso',
          icon: 'polygon-lasso',
          title: 'Polygonal Lasso — click vertices, double-click or Enter to close',
        },
      ],
    },
    {
      id: 'wand',
      label: 'Magic Wand',
      tools: [{ id: 'selection-wand', label: 'Magic Wand', icon: 'wand' }],
    },
    { id: 'pencil', label: 'Pencil', tools: [{ id: 'pencil', label: 'Pencil', icon: 'pencil' }] },
    { id: 'eraser', label: 'Eraser', tools: [{ id: 'eraser', label: 'Eraser', icon: 'eraser' }] },
    { id: 'fill', label: 'Fill', tools: [{ id: 'bucket', label: 'Fill', icon: 'bucket' }] },
    {
      id: 'eyedropper',
      label: 'Eyedropper',
      tools: [{ id: 'eyedropper', label: 'Eyedropper', icon: 'eyedropper' }],
    },
    { id: 'line', label: 'Line', tools: [{ id: 'line', label: 'Line', icon: 'line' }] },
    {
      id: 'shapes',
      label: 'Shapes',
      tools: [
        { id: 'rectangle', label: 'Rectangle', icon: 'rect' },
        { id: 'rectangle-fill', label: 'Filled Rectangle', icon: 'rect-fill' },
        { id: 'ellipse', label: 'Ellipse', icon: 'ellipse' },
        { id: 'ellipse-fill', label: 'Filled Ellipse', icon: 'ellipse-fill' },
        { id: 'rounded-rect', label: 'Rounded Rectangle', icon: 'rounded-rect' },
        { id: 'polygon-shape', label: 'Polygon', icon: 'polygon' },
      ],
    },
    { id: 'text', label: 'Text', tools: [{ id: 'text', label: 'Text', icon: 'text' }] },
    {
      id: 'stamp',
      label: 'Tile Stamp',
      tools: [
        {
          id: 'tilemap-stamp',
          label: 'Tile Stamp',
          icon: 'stamp',
          disabled: !stampTile,
          title: stampTile
            ? `Tile Stamp — tile ${stampTile.tileId} of tileset ${stampTile.tilesetId}`
            : 'Tile Stamp — pick a tile in the Tilesets panel first',
        },
      ],
    },
    { id: 'slice', label: 'Slice', tools: [{ id: 'slice', label: 'Slice', icon: 'slice' }] },
  ];
  return groups.map((g) => ({ ...g, shortcut: toolHint(g.id) }));
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
