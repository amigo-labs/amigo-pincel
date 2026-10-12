// Pixel-mode tool ids, their classification and the tool-rail groups
// (spec §5, §9.2). Pure data and functions, shared by PixelEditor and the
// overlay painter.

import type { ToolGroup } from '../../lib/components/layout/tools';
import { toolHint } from '../../lib/shell/shortcuts';
import { t } from '../../lib/i18n/index.svelte';

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
    { id: 'move', label: t('tool.move'), tools: [{ id: 'move', label: t('tool.move'), icon: 'move' }] },
    {
      id: 'select',
      label: t('tool.marquee'),
      tools: [
        { id: 'selection-rect', label: t('tool.rectSelect'), icon: 'select-rect' },
        { id: 'selection-ellipse', label: t('tool.ellipseSelect'), icon: 'select-ellipse' },
      ],
    },
    {
      id: 'lasso',
      label: t('tool.lasso'),
      tools: [
        { id: 'selection-lasso', label: t('tool.lasso'), icon: 'lasso' },
        {
          id: 'selection-polygon',
          label: t('tool.polygonLasso'),
          icon: 'polygon-lasso',
          title: t('tool.polygonLassoHint'),
        },
      ],
    },
    {
      id: 'wand',
      label: t('tool.wand'),
      tools: [{ id: 'selection-wand', label: t('tool.wand'), icon: 'wand' }],
    },
    { id: 'pencil', label: t('tool.pencil'), tools: [{ id: 'pencil', label: t('tool.pencil'), icon: 'pencil' }] },
    { id: 'eraser', label: t('tool.eraser'), tools: [{ id: 'eraser', label: t('tool.eraser'), icon: 'eraser' }] },
    { id: 'fill', label: t('tool.fill'), tools: [{ id: 'bucket', label: t('tool.fill'), icon: 'bucket' }] },
    {
      id: 'eyedropper',
      label: t('tool.eyedropper'),
      tools: [{ id: 'eyedropper', label: t('tool.eyedropper'), icon: 'eyedropper' }],
    },
    { id: 'line', label: t('tool.line'), tools: [{ id: 'line', label: t('tool.line'), icon: 'line' }] },
    {
      id: 'shapes',
      label: t('tool.shapes'),
      tools: [
        { id: 'rectangle', label: t('tool.rectangle'), icon: 'rect' },
        { id: 'rectangle-fill', label: t('tool.rectangleFill'), icon: 'rect-fill' },
        { id: 'ellipse', label: t('tool.ellipse'), icon: 'ellipse' },
        { id: 'ellipse-fill', label: t('tool.ellipseFill'), icon: 'ellipse-fill' },
        { id: 'rounded-rect', label: t('tool.roundedRect'), icon: 'rounded-rect' },
        { id: 'polygon-shape', label: t('tool.polygon'), icon: 'polygon' },
      ],
    },
    { id: 'text', label: t('tool.text'), tools: [{ id: 'text', label: t('tool.text'), icon: 'text' }] },
    {
      id: 'stamp',
      label: t('tool.stamp'),
      tools: [
        {
          id: 'tilemap-stamp',
          label: t('tool.stamp'),
          icon: 'stamp',
          disabled: !stampTile,
          title: stampTile
            ? t('tool.stampHint', { tile: stampTile.tileId, tileset: stampTile.tilesetId })
            : t('tool.stampPick'),
        },
      ],
    },
    { id: 'slice', label: t('tool.slice'), tools: [{ id: 'slice', label: t('tool.slice'), icon: 'slice' }] },
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
 * rectangles, ellipses, rounded rectangles and polygons; the nearest 45°
 * direction for lines.
 */
export function constrainEndpoint(tool: Tool, start: Point, preview: Point, shift: boolean): Point {
  if (!shift) return preview;
  const dx = preview.x - start.x;
  const dy = preview.y - start.y;
  if (tool === 'line') {
    // Snap to the nearest 45° direction, as in Image mode.
    if (Math.abs(dx) > 2 * Math.abs(dy)) return { x: preview.x, y: start.y };
    if (Math.abs(dy) > 2 * Math.abs(dx)) return { x: start.x, y: preview.y };
    const d = Math.max(Math.abs(dx), Math.abs(dy));
    return { x: start.x + d * (dx < 0 ? -1 : 1), y: start.y + d * (dy < 0 ? -1 : 1) };
  }
  if (!isSquareConstrainable(tool)) return preview;
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
