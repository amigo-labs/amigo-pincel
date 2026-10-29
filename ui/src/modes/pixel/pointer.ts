// Canvas pointer input: paint, shapes, selections, slices, stamps, pan.

import { packColor, unpackColor } from '../../lib/color';
import { t } from '../../lib/i18n/index.svelte';
import { altHeld, shiftHeld } from '../../lib/shell/modifiers.svelte';
import { constrainedEndpoint, syncMeta } from './render';
import { type SelectionMode, colors, ed, live, toolState, view } from './stores/editor.svelte';
import { openTextAt } from './text-tool';
import { clampBrushSize, isBrushTool, strokePressure } from './brush';
import { type Tool, alongDirection, directionTo, isDragShapeTool, isShapeTool } from './tools';
import { applyPinch } from './view-ops';

export function selectionModeFor(e: { shiftKey: boolean; altKey: boolean }): SelectionMode {
  const shift = shiftHeld(e);
  const alt = altHeld(e);
  if (shift && alt) return 'intersect';
  if (shift) return 'add';
  if (alt) return 'subtract';
  return 'replace';
}

// Tools the right mouse button drives with the background color.
export const BG_TOOLS: ReadonlySet<Tool> = new Set<Tool>([
  'pencil',
  'bucket',
  'eyedropper',
  'line',
  'rectangle',
  'rectangle-fill',
  'ellipse',
  'ellipse-fill',
  'rounded-rect',
  'polygon-shape',
]);

/** Packed RGBA of the color the current gesture paints with. */
export function strokeColor(): number {
  return live.useBg
    ? packColor(colors.bgColor, colors.bgAlpha)
    : packColor(colors.color, colors.alpha);
}

/** Packed RGBA of a filled shape's interior: the other color. */
export function shapeFillColor(): number {
  return live.useBg
    ? packColor(colors.color, colors.alpha)
    : packColor(colors.bgColor, colors.bgAlpha);
}

export function cancelGesture() {
  if (live.painting) {
    live.painting = false;
    ed.doc?.endStroke();
  }
  if (ed.doc && ed.doc.undoDepth > live.gestureUndoDepth) {
    ed.doc.undo();
    syncMeta();
    ed.docRev += 1;
  }
  view.panning = false;
  live.panStartClient = null;
  live.panStartOffset = null;
  toolState.moveSelStart = null;
  toolState.moveSelPreview = null;
  live.lassoPoints = null;
  live.dragStart = null;
  live.dragPreview = null;
  live.dragTool = null;
  live.needsRecompose = true;
}

export function spriteCoord(e: PointerEvent): { x: number; y: number } | null {
  if (!view.canvas) return null;
  const rect = view.canvas.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return null;
  const x = Math.floor(((e.clientX - rect.left) * view.canvas.width) / rect.width);
  const y = Math.floor(((e.clientY - rect.top) * view.canvas.height) / rect.height);
  return { x, y };
}

// Find a tilemap layer bound to `tilesetId`. The Stamp tool needs
// a target layer; today we auto-pick the topmost matching tilemap
// layer (highest z-index), which matches the natural "the last
// tilemap layer you added is the one you draw on" expectation.
// An explicit active-layer selector lands when the Layers panel
// ships.
export function activeTilemapLayerForTileset(tilesetId: number): number | null {
  if (!ed.doc) return null;
  for (let i = ed.doc.layerCount - 1; i >= 0; i -= 1) {
    let layerId: number;
    try {
      layerId = ed.doc.layerIdAt(i);
    } catch {
      continue;
    }
    if (ed.doc.layerKind(layerId) === 'tilemap' && ed.doc.layerTilesetId(layerId) === tilesetId) {
      return layerId;
    }
  }
  return null;
}

// Commit the active stamp tile onto the canvas cell under `point`.
// Resolves the target tilemap layer, the tileset's tile size, and
// forwards the grid cell to wasm `placeTile`. Out-of-grid clicks
// (canvas larger than grid * tile_size) and missing layers surface
// in the status bar rather than throwing.
export function commitStamp(point: { x: number; y: number }) {
  if (!ed.doc || !toolState.stampTile) return;
  const layerId = activeTilemapLayerForTileset(toolState.stampTile.tilesetId);
  if (layerId === null) {
    ed.status = t('px.needTilemap');
    return;
  }
  const tileW = ed.doc.tilesetTileWidth(toolState.stampTile.tilesetId);
  const tileH = ed.doc.tilesetTileHeight(toolState.stampTile.tilesetId);
  if (tileW === 0 || tileH === 0) return;
  if (point.x < 0 || point.y < 0) return;
  const gx = Math.floor(point.x / tileW);
  const gy = Math.floor(point.y / tileH);
  try {
    ed.doc.placeTile(layerId, ed.currentFrame, gx, gy, toolState.stampTile.tileId);
    live.needsRecompose = true;
    syncMeta();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.stamp'), error: msg });
  }
}

export function paintAt(e: PointerEvent) {
  if (!ed.doc) return;
  // Drag-shape tools (Line, Rectangle, …) have their own press /
  // drag / release pipeline below and commit on `pointerup`; a
  // mid-drag tool switch must not route a stroke through here.
  if (isDragShapeTool(toolState.tool) || toolState.tool === 'move') return;
  const point = spriteCoord(e);
  if (!point) return;
  if (toolState.tool === 'eyedropper') {
    // The eyedropper is read-only: sample the composed pixel and
    // bind it to the foreground color picker. Drags keep sampling
    // so the user can scrub for the exact pixel they want.
    try {
      const picked = ed.doc.pickColor(ed.currentFrame, point.x, point.y);
      if (live.useBg) {
        colors.bgColor = unpackColor(picked);
        colors.bgAlpha = picked & 0xff;
      } else {
        colors.color = unpackColor(picked);
        colors.alpha = picked & 0xff;
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('pickColor failed', err);
      ed.status = t('status.failed', { action: t('act.pick'), error: msg });
    }
    return;
  }
  if (!isBrushTool(toolState.tool)) return;
  // Shift (key or latched toggle) constrains the stroke to a 45° ray from
  // the anchor: the press position, or the last point sent before Shift
  // went down (pixel-pencil AC-07). The first movement away from the
  // anchor picks the nearest direction and it holds until Shift is
  // released; releasing continues freehand from where the stroke is.
  const fresh = live.lastPaint === null;
  let target = point;
  if (!shiftHeld(e)) {
    live.shiftAnchor = null;
    live.shiftDir = null;
  } else if (fresh) {
    live.shiftAnchor = point;
    live.shiftDir = null;
  } else {
    live.shiftAnchor ??= live.lastPaint;
    const anchor = live.shiftAnchor;
    if (anchor) {
      live.shiftDir ??= directionTo(anchor, point);
      target = live.shiftDir ? alongDirection(anchor, live.shiftDir, point) : anchor;
    }
  }
  if (live.lastPaint && live.lastPaint.x === target.x && live.lastPaint.y === target.y) return;
  try {
    if (fresh) {
      // The core stamps the brush along the line between consecutive
      // points and skips off-canvas pixels; the whole drag is one undo
      // entry, closed by `endStroke` on pointerup.
      ed.doc.beginStroke(
        toolState.tool,
        strokeColor(),
        clampBrushSize(toolState.brushSize),
        toolState.brushShape,
        toolState.pixelPerfect,
      );
    }
    ed.doc.strokeTo(target.x, target.y, strokePressure(e));
    live.lastPaint = target;
  } catch (err) {
    // Missing layer, unknown tool, …: a real failure. Stop the gesture so
    // the rest of the drag doesn't repeat it, and surface it.
    live.painting = false;
    ed.doc.endStroke();
    const msg = err instanceof Error ? err.message : String(err);
    console.error('paint stroke failed', err);
    ed.status = t('status.failed', { action: t('act.paint'), error: msg });
  }
}

export function lockedEditTarget(): string | null {
  if (!ed.doc) return null;
  if (
    toolState.tool.startsWith('selection') ||
    toolState.tool === 'eyedropper' ||
    toolState.tool === 'slice'
  )
    return null;
  if (toolState.tool === 'move' && !ed.selection) return null;
  const id = toolState.tool === 'tilemap-stamp' ? ed.activeLayerId : ed.doc.paintTargetLayer();
  if (id === null || id === 0xffffffff || !ed.doc.layerLocked(id)) return null;
  return ed.doc.layerName(id);
}

export function onPointerDown(e: PointerEvent) {
  if (live.pinch.down(e)) {
    view.canvas?.setPointerCapture(e.pointerId);
    cancelGesture();
    live.pinchZoom0 = view.zoom;
    return;
  }
  if (live.pinch.suppressing) return;
  // Right button: the painting tools use the background color
  // (Aseprite / paint.net); every other tool ignores it.
  if (e.button === 2 && !BG_TOOLS.has(toolState.tool)) return;
  if (e.button !== 0 && e.button !== 2) return;
  live.useBg = e.button === 2;
  view.canvas?.setPointerCapture(e.pointerId);
  live.gestureUndoDepth = ed.doc?.undoDepth ?? 0;
  // Space-drag always pans, regardless of the active tool (spec §5.2
  // — Move tool "Pans canvas with space-drag"). Move-tool press
  // splits on whether a selection is active: with one, drag the
  // selection content (M7.7b); without one, fall back to viewport
  // pan (M7.7a). spaceDown wins so the user can always pan even
  // when the Move tool would otherwise translate the marquee.
  if (view.spaceDown) {
    view.panning = true;
    live.panStartClient = { x: e.clientX, y: e.clientY };
    live.panStartOffset = { x: view.panX, y: view.panY };
    return;
  }
  const locked = lockedEditTarget();
  if (locked !== null) {
    ed.status = live.lockNotice = t('layers.locked', { name: locked });
    return;
  }
  if (live.lockNotice !== null && ed.status === live.lockNotice) ed.status = '';
  live.lockNotice = null;
  if (toolState.tool === 'move') {
    if (ed.selection) {
      const point = spriteCoord(e);
      if (point) {
        toolState.moveSelStart = point;
        toolState.moveSelPreview = point;
        live.needsRecompose = true;
      }
      return;
    }
    view.panning = true;
    live.panStartClient = { x: e.clientX, y: e.clientY };
    live.panStartOffset = { x: view.panX, y: view.panY };
    return;
  }
  if (toolState.tool === 'selection-lasso') {
    const point = spriteCoord(e);
    if (!point) return;
    live.lassoPoints = [point];
    live.dragSelMode = selectionModeFor(e);
    live.needsRecompose = true;
    return;
  }
  if (toolState.tool === 'selection-polygon') {
    const point = spriteCoord(e);
    if (!point) return;
    if (!toolState.polyPoints) {
      toolState.polyPoints = [point];
      live.dragSelMode = selectionModeFor(e);
    } else if (e.detail >= 2) {
      commitPolygon();
      return;
    } else {
      toolState.polyPoints = [...toolState.polyPoints, point];
    }
    live.polyHover = point;
    live.needsRecompose = true;
    return;
  }
  if (toolState.tool === 'selection-wand') {
    commitWand(e);
    return;
  }
  if (toolState.tool === 'text') {
    const point = spriteCoord(e);
    if (point && !toolState.textAnchor) void openTextAt(point);
    return;
  }
  if (isDragShapeTool(toolState.tool)) {
    const point = spriteCoord(e);
    if (!point) return;
    live.dragStart = point;
    live.dragPreview = point;
    live.dragTool = toolState.tool;
    live.dragShift = shiftHeld(e);
    live.dragSelMode = selectionModeFor(e);
    live.needsRecompose = true;
    return;
  }
  if (toolState.tool === 'bucket') {
    // Bucket commits once per click; entering painting mode would have
    // `pointermove` re-fire `applyBucket` and push a no-op fill onto
    // the bus on every pixel of the drag.
    commitBucket(e);
    return;
  }
  if (toolState.tool === 'tilemap-stamp') {
    // Stamp commits once per click for the same reason as Bucket.
    // Drag-to-paint over tiles can land alongside auto-tile mode
    // in Phase 2 (spec §5.3 / §13.2).
    const point = spriteCoord(e);
    if (point) commitStamp(point);
    return;
  }
  live.painting = true;
  live.lastPaint = null;
  live.shiftAnchor = null;
  live.shiftDir = null;
  paintAt(e);
}

export function commitBucket(e: PointerEvent) {
  if (!ed.doc) return;
  const point = spriteCoord(e);
  if (!point) return;
  try {
    ed.doc.applyBucket(point.x, point.y, strokeColor());
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error('applyBucket failed', err);
    ed.status = t('status.failed', { action: t('act.bucket'), error: msg });
  }
}

// Commit a Slice tool drag from the two corner points of the
// press / drag / release gesture. Routes to `addSlice` when no
// slice is active (creating a new slice with the drawn bounds), or
// to `setSliceKey` when a slice is active (preserving its 9-patch
// center and pivot). Degenerate (single-pixel) drags are accepted
// as 1×1 slices — the underlying commands reject empty bounds, so
// a click without a drag still surfaces a valid 1-pixel slice.
export function commitSliceDrag(x0: number, y0: number, x1: number, y1: number) {
  if (!ed.doc) return;
  const minX = Math.min(x0, x1);
  const minY = Math.min(y0, y1);
  const w = Math.abs(x1 - x0) + 1;
  const h = Math.abs(y1 - y0) + 1;
  try {
    if (ed.activeSliceId === null) {
      const autoName = `Slice ${ed.doc.sliceCount + 1}`;
      const packed = packColor(colors.color, colors.alpha);
      const newId = ed.doc.addSlice(autoName, minX, minY, w, h, packed);
      ed.activeSliceId = newId;
    } else {
      const id = ed.activeSliceId;
      // Preserve the active slice's 9-patch center and pivot
      // around the new bounds. The wasm side validates the
      // partial-quartet invariant on its own.
      let keyIndex = 0;
      const keyCount = ed.doc.sliceKeyCount(id);
      for (let k = 0; k < keyCount; k += 1) {
        if (ed.doc.sliceKeyFrame(id, k) === 0) {
          keyIndex = k;
          break;
        }
      }
      const hasCenter = keyCount > 0 ? ed.doc.sliceKeyHasCenter(id, keyIndex) : false;
      const hasPivot = keyCount > 0 ? ed.doc.sliceKeyHasPivot(id, keyIndex) : false;
      ed.doc.setSliceKey(
        id,
        0,
        minX,
        minY,
        w,
        h,
        hasCenter ? ed.doc.sliceKeyCenterX(id, keyIndex) : undefined,
        hasCenter ? ed.doc.sliceKeyCenterY(id, keyIndex) : undefined,
        hasCenter ? ed.doc.sliceKeyCenterWidth(id, keyIndex) : undefined,
        hasCenter ? ed.doc.sliceKeyCenterHeight(id, keyIndex) : undefined,
        hasPivot ? ed.doc.sliceKeyPivotX(id, keyIndex) : undefined,
        hasPivot ? ed.doc.sliceKeyPivotY(id, keyIndex) : undefined,
      );
    }
    ed.docRev += 1;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error('slice commit failed', err);
    ed.status = t('status.failed', { action: t('act.slice'), error: msg });
  }
}

// Commit a selection marquee from the two corner points of the
// press / drag / release gesture. A no-move click (start === end)
// clears the selection, matching Aseprite's "click outside to
// deselect" UX; otherwise the rect is normalized to min / max
// corners (inclusive) and forwarded to `setSelection`. Sprite-bounds
// clipping is intentionally not applied here — the wasm side stores
// the raw rect and the marching-ants overlay's per-pixel clip keeps
// off-canvas extents from leaking visually.
export function commitSelection(x0: number, y0: number, x1: number, y1: number) {
  if (!ed.doc) return;
  if (x0 === x1 && y0 === y1 && live.dragSelMode === 'replace') {
    ed.doc.clearSelection();
    return;
  }
  const minX = Math.min(x0, x1);
  const minY = Math.min(y0, y1);
  const w = Math.abs(x1 - x0) + 1;
  const h = Math.abs(y1 - y0) + 1;
  ed.doc.selectRect(minX, minY, w, h, live.dragSelMode);
}

// Ellipse marquee: same inclusive-corner gesture as the rect, combined
// per the modifier mode captured on pointerdown.
export function commitEllipseSelection(x0: number, y0: number, x1: number, y1: number) {
  if (!ed.doc) return;
  if (x0 === x1 && y0 === y1 && live.dragSelMode === 'replace') {
    ed.doc.clearSelection();
    return;
  }
  const minX = Math.min(x0, x1);
  const minY = Math.min(y0, y1);
  const w = Math.abs(x1 - x0) + 1;
  const h = Math.abs(y1 - y0) + 1;
  ed.doc.selectEllipse(minX, minY, w, h, live.dragSelMode);
}

// Lasso / polygon commit. Fewer than three vertices is a click: in
// replace mode that deselects (like the rect click), otherwise nothing.
export function commitPolygonPoints(pts: { x: number; y: number }[], mode: SelectionMode) {
  if (!ed.doc) return;
  try {
    if (pts.length < 3) {
      if (mode === 'replace') ed.doc.clearSelection();
      return;
    }
    const flat = new Int32Array(pts.length * 2);
    pts.forEach((p, i) => {
      flat[i * 2] = p.x;
      flat[i * 2 + 1] = p.y;
    });
    ed.doc.selectPolygon(flat, mode);
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.selection'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function commitPolygon() {
  const pts = toolState.polyPoints;
  toolState.polyPoints = null;
  live.polyHover = null;
  if (pts) commitPolygonPoints(pts, live.dragSelMode);
  live.needsRecompose = true;
}

export function cancelPolygon() {
  toolState.polyPoints = null;
  live.polyHover = null;
  live.lassoPoints = null;
  live.needsRecompose = true;
}

export function commitWand(e: PointerEvent) {
  if (!ed.doc) return;
  const point = spriteCoord(e);
  if (!point) return;
  try {
    ed.doc.selectWand(
      point.x,
      point.y,
      Math.max(0, Math.min(255, Math.round(toolState.wandTolerance))),
      toolState.wandContiguous,
      toolState.wandSample,
      selectionModeFor(e),
    );
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.wand'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function onPointerMove(e: PointerEvent) {
  const frame = live.pinch.move(e);
  if (frame) {
    applyPinch(frame);
    return;
  }
  if (live.pinch.suppressing) return;
  if (view.panning && live.panStartClient && live.panStartOffset) {
    view.panX = live.panStartOffset.x + (e.clientX - live.panStartClient.x);
    view.panY = live.panStartOffset.y + (e.clientY - live.panStartClient.y);
    return;
  }
  if (toolState.moveSelStart) {
    const point = spriteCoord(e);
    if (!point) return;
    toolState.moveSelPreview = point;
    live.needsRecompose = true;
    return;
  }
  if (live.lassoPoints) {
    const point = spriteCoord(e);
    if (!point) return;
    const last = live.lassoPoints[live.lassoPoints.length - 1];
    if (!last || last.x !== point.x || last.y !== point.y) {
      live.lassoPoints.push(point);
      live.needsRecompose = true;
    }
    return;
  }
  if (toolState.polyPoints) {
    const point = spriteCoord(e);
    if (!point) return;
    live.polyHover = point;
    live.needsRecompose = true;
    return;
  }
  if (live.dragStart) {
    const point = spriteCoord(e);
    if (!point) return;
    live.dragPreview = point;
    live.dragShift = shiftHeld(e);
    live.needsRecompose = true;
    return;
  }
  if (toolState.tool === 'tilemap-stamp') {
    // Update the hover indicator regardless of button state — the
    // overlay should follow the cursor whether or not the user is
    // mid-click. `dirty = true` triggers a recompose so the grid
    // overlay redraws at the new cell.
    const point = spriteCoord(e);
    toolState.stampHover = point;
    live.needsRecompose = true;
    return;
  }
  if (!live.painting) return;
  // Pens and fast mice report several samples per frame; paint each one
  // so the stroke keeps its shape and its per-sample pressure.
  const samples = e.getCoalescedEvents?.() ?? [];
  for (const sample of samples.length > 0 ? samples : [e]) {
    if (!live.painting) break;
    paintAt(sample);
  }
}

export function onPointerLeave() {
  if (toolState.tool === 'tilemap-stamp' && toolState.stampHover) {
    toolState.stampHover = null;
    live.needsRecompose = true;
  }
}

export function onPointerUp(e: PointerEvent) {
  if (view.canvas?.hasPointerCapture(e.pointerId)) {
    view.canvas.releasePointerCapture(e.pointerId);
  }
  const pinched = live.pinch.suppressing;
  live.pinch.up(e);
  if (pinched) return;
  if (view.panning) {
    view.panning = false;
    live.panStartClient = null;
    live.panStartOffset = null;
    return;
  }
  if (toolState.moveSelStart && toolState.moveSelPreview && ed.doc) {
    const dx = toolState.moveSelPreview.x - toolState.moveSelStart.x;
    const dy = toolState.moveSelPreview.y - toolState.moveSelStart.y;
    if (dx !== 0 || dy !== 0) {
      try {
        ed.doc.applyMoveSelection(dx, dy);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('applyMoveSelection failed', err);
        ed.status = t('status.failed', { action: t('act.move'), error: msg });
      }
    }
    toolState.moveSelStart = null;
    toolState.moveSelPreview = null;
    live.needsRecompose = true;
    syncMeta();
    return;
  }
  if (live.lassoPoints && ed.doc) {
    const pts = live.lassoPoints;
    live.lassoPoints = null;
    commitPolygonPoints(pts, live.dragSelMode);
    live.needsRecompose = true;
    return;
  }
  if (live.dragStart && live.dragPreview && live.dragTool && ed.doc) {
    live.dragShift = shiftHeld(e);
    const end = constrainedEndpoint();
    const packed = strokeColor();
    try {
      if (live.dragTool === 'line') {
        ed.doc.applyLine(live.dragStart.x, live.dragStart.y, end.x, end.y, packed);
      } else if (isShapeTool(live.dragTool)) {
        const kind =
          live.dragTool === 'rectangle'
            ? 'rectangle'
            : live.dragTool === 'ellipse'
              ? 'ellipse'
              : live.dragTool === 'rounded-rect'
                ? 'rounded_rectangle'
                : 'polygon';
        const param =
          live.dragTool === 'rounded-rect' ? toolState.cornerRadius : toolState.polygonSides;
        ed.doc.applyShape(
          kind,
          live.dragStart.x,
          live.dragStart.y,
          end.x,
          end.y,
          toolState.shapeMode,
          Math.max(1, Math.round(toolState.strokeWidth)),
          packed,
          shapeFillColor(),
          param,
        );
      } else if (live.dragTool === 'rectangle-fill') {
        ed.doc.applyRectangle(live.dragStart.x, live.dragStart.y, end.x, end.y, packed, true);
      } else if (live.dragTool === 'ellipse-fill') {
        ed.doc.applyEllipse(live.dragStart.x, live.dragStart.y, end.x, end.y, packed, true);
      } else if (live.dragTool === 'selection-rect') {
        commitSelection(live.dragStart.x, live.dragStart.y, end.x, end.y);
      } else if (live.dragTool === 'selection-ellipse') {
        commitEllipseSelection(live.dragStart.x, live.dragStart.y, end.x, end.y);
      } else if (live.dragTool === 'slice') {
        commitSliceDrag(live.dragStart.x, live.dragStart.y, end.x, end.y);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error(`${live.dragTool} failed`, err);
      ed.status = t('px.toolFailed', { tool: live.dragTool, error: msg });
    }
    live.dragStart = null;
    live.dragPreview = null;
    live.dragTool = null;
    live.dragShift = false;
    live.needsRecompose = true;
    syncMeta();
    return;
  }
  if (live.painting) {
    live.painting = false;
    // Close the gesture on the undo bus so this drag stays one undo
    // entry and the next stroke starts a fresh one.
    ed.doc?.endStroke();
  }
}

// If the window loses focus (alt-tab, DevTools, OS shortcut) between
// a Space keydown and keyup, the keyup never reaches us and
// `spaceDown` stays stuck — and any in-flight pan drag would similarly
// outlive the gesture. Clear both on blur / hidden so the user
// returns to a clean state.
export function onWindowBlur() {
  view.spaceDown = false;
  view.panning = false;
  live.panStartClient = null;
  live.panStartOffset = null;
  toolState.moveSelStart = null;
  toolState.moveSelPreview = null;
}
