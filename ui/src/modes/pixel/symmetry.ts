// Symmetry drawing and tiled mode (docs/specs/symmetry-tiled-mode.md):
// axis positions, the margin handles that move them, and the stroke
// options both aids hand to the core. Mirroring and wrapping happen in
// `PencilStroke`, so a stroke stays one undo entry.

import type { Document } from '../../lib/core';
import { type AxisMode, type Pt, ed, live, toolState, view } from './stores/editor.svelte';

/** The mode covers the x axis (left–right / tiled X). */
export function coversX(mode: AxisMode): boolean {
  return mode === 'x' || mode === 'xy';
}

/** The mode covers the y axis (top–bottom / tiled Y). */
export function coversY(mode: AxisMode): boolean {
  return mode === 'y' || mode === 'xy';
}

/** Vertical axis in half pixels from the left edge; unmoved = centre (AC-04). */
export function axisX2(): number {
  return toolState.symmetryX ?? ed.canvasW;
}

/** Horizontal axis in half pixels from the top edge; unmoved = centre (AC-04). */
export function axisY2(): number {
  return toolState.symmetryY ?? ed.canvasH;
}

/**
 * Apply symmetry and tiled mode to the stroke `beginStroke` /
 * `beginShadingStroke` just armed, before its first `strokeTo`.
 */
export function armStroke(doc: Document): void {
  const s = toolState.symmetry;
  if (s !== 'none') {
    doc.setStrokeMirror(coversX(s) ? axisX2() : null, coversY(s) ? axisY2() : null);
  }
  const tiled = view.tiled;
  if (tiled !== 'none') doc.setStrokeWrap(coversX(tiled), coversY(tiled));
}

/**
 * The point lies on the canvas or on one of its repeated copies (AC-16):
 * one canvas size beyond each edge along the tiled axes.
 */
export function onTiledArea(p: Pt | null): boolean {
  if (!p) return false;
  const w = ed.canvasW;
  const h = ed.canvasH;
  const x0 = coversX(view.tiled) ? -w : 0;
  const y0 = coversY(view.tiled) ? -h : 0;
  return p.x >= x0 && p.y >= y0 && p.x < w - x0 && p.y < h - y0;
}

/** Offsets (in canvas sizes) of the repeated copies tiled mode shows (AC-13). */
export function tileOffsets(mode: AxisMode): { dx: number; dy: number }[] {
  const xs = coversX(mode) ? [-1, 0, 1] : [0];
  const ys = coversY(mode) ? [-1, 0, 1] : [0];
  return ys.flatMap((dy) => xs.map((dx) => ({ dx, dy }))).filter((o) => o.dx !== 0 || o.dy !== 0);
}

/**
 * Keep moved axes on a resized canvas: clamp them to 0 … 2 × the new size;
 * unmoved ones follow the centre on their own (AC-20).
 */
export function clampAxes(w: number, h: number): void {
  if (toolState.symmetryX !== null && toolState.symmetryX > 2 * w) toolState.symmetryX = 2 * w;
  if (toolState.symmetryY !== null && toolState.symmetryY > 2 * h) toolState.symmetryY = 2 * h;
}

/** Another document: both axes back to the centre (AC-20). */
export function resetAxes(): void {
  toolState.symmetryX = null;
  toolState.symmetryY = null;
}

/** Half-pixel axis position under the pointer, clamped to the canvas. */
function axisAt(axis: 'x' | 'y', e: PointerEvent): number | null {
  const rect = view.canvas?.getBoundingClientRect();
  if (!rect || rect.width === 0 || rect.height === 0) return null;
  const size = axis === 'x' ? ed.canvasW : ed.canvasH;
  const frac = axis === 'x' ? (e.clientX - rect.left) / rect.width : (e.clientY - rect.top) / rect.height;
  return Math.max(0, Math.min(2 * size, Math.round(frac * 2 * size)));
}

/** Press on an axis handle: start moving that axis, whatever the tool (AC-09). */
export function startAxisDrag(axis: 'x' | 'y', e: PointerEvent): void {
  if (e.button !== 0) return;
  e.stopPropagation();
  (e.currentTarget as Element).setPointerCapture(e.pointerId);
  live.axisDrag = axis;
}

/** Drag an axis handle: the axis follows in half-pixel steps. */
export function moveAxisDrag(axis: 'x' | 'y', e: PointerEvent): void {
  if (live.axisDrag !== axis) return;
  const at = axisAt(axis, e);
  if (at === null) return;
  if (axis === 'x') toolState.symmetryX = at;
  else toolState.symmetryY = at;
}

/** Release an axis handle. */
export function endAxisDrag(e: PointerEvent): void {
  const target = e.currentTarget as Element;
  if (target.hasPointerCapture(e.pointerId)) target.releasePointerCapture(e.pointerId);
  live.axisDrag = null;
}

/** Double-click on a handle: its axis returns to the canvas centre. */
export function resetAxis(axis: 'x' | 'y'): void {
  if (axis === 'x') toolState.symmetryX = null;
  else toolState.symmetryY = null;
}
