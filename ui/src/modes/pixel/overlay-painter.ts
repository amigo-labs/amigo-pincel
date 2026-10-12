// Pixel-mode overlay layer (spec §4.4, M12.5): the transparent Canvas2D
// stacked over the base canvas carries drag-shape previews, the selection
// marquee, the tilemap stamp grid and the active slice's accents, so the
// base layer can be WebGPU. Pure painting: the editor hands in a snapshot
// of the state that decides what is live.

import type { Document } from '../../lib/core';
import {
  paintEllipsePreview,
  paintLinePreview,
  paintMaskMarquee,
  paintPivotCrosshair,
  paintPolylinePreview,
  paintRectOutline,
  paintRectanglePreview,
  paintSelectionMarquee,
} from '../../lib/render/canvas2d';
import type { Point, Tool } from './tools';

/** A rect in sprite pixels. */
export interface SpriteRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Everything the overlay paints from. */
export interface OverlayState {
  doc: Document | null;
  tool: Tool;
  /** Foreground `#RRGGBB` for shape previews. */
  color: string;
  /** Marching-ants dash phase. */
  marchPhase: number;
  canvasW: number;
  canvasH: number;
  /** In-flight drag shape (endpoint already constrained), if any. */
  drag: { tool: Tool; start: Point; end: Point } | null;
  /** Lasso / polygon in progress; `hover` is the polygon's floating edge. */
  polyline: { points: Point[]; hover: Point | null } | null;
  /** Committed selection bounds, with the shaped mask when not a rect. */
  selection: SpriteRect | null;
  selectionMask: Uint8Array | null;
  /** Live Move-tool translation of the selection. */
  moveDelta: Point;
  stampTile: { tilesetId: number; tileId: number } | null;
  stampHover: Point | null;
  activeSliceId: number | null;
}

/** Clear `overlay` and repaint every live overlay source. */
export function paintOverlays(overlay: HTMLCanvasElement, s: OverlayState): void {
  const ctx = overlay.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, overlay.width, overlay.height);
  if (s.drag) {
    // On release the editor commits through the matching wasm method and
    // the composed cel shows the same pixels; the next paint clears this.
    paintDragPreview(overlay, s.drag, s.color, s.marchPhase);
  } else if (s.polyline) {
    // Lasso / polygon in progress: outline so far plus the floating edge
    // to the cursor (polygon), closed back to the start.
    paintPolylinePreview(overlay, s.polyline.points, s.polyline.hover, true);
  } else if (s.selection) {
    // A live Move-tool drag paints a ghost marquee at the translated
    // position; shaped selections trace the mask edge.
    const { x: dx, y: dy } = s.moveDelta;
    if (s.selectionMask) {
      paintMaskMarquee(
        overlay,
        s.selectionMask,
        s.canvasW,
        s.canvasH,
        s.marchPhase,
        dx,
        dy,
        s.selection,
      );
    } else {
      const r = s.selection;
      paintSelectionMarquee(overlay, r.x + dx, r.y + dy, r.w, r.h, s.marchPhase);
    }
  }
  // Tile grid after the marquee so a marquee + stamp combo keeps both.
  if (s.tool === 'tilemap-stamp' && s.stampTile && s.doc) {
    paintTileGrid(overlay, s.doc, s.stampTile.tilesetId, s.stampHover);
  }
  // Active slice last so its geometry stays above grid / marquee.
  if (s.doc && s.activeSliceId !== null) {
    paintActiveSlice(overlay, s.doc, s.activeSliceId, s.marchPhase);
  }
}

/** Wipe the overlay (the dirty fast path's companion to a sub-rect blit). */
export function clearOverlay(overlay: HTMLCanvasElement): void {
  overlay.getContext('2d')?.clearRect(0, 0, overlay.width, overlay.height);
}

/**
 * True when no overlay source is live, i.e. a sub-rect blit cannot leave
 * stale overlay pixels behind.
 */
export function overlayIsIdle(s: OverlayState): boolean {
  if (s.selection) return false;
  if (s.drag || s.polyline || s.moveDelta.x !== 0 || s.moveDelta.y !== 0) return false;
  if (s.tool === 'tilemap-stamp' && s.stampTile) return false;
  return s.activeSliceId === null;
}

function paintDragPreview(
  overlay: HTMLCanvasElement,
  drag: { tool: Tool; start: Point; end: Point },
  color: string,
  marchPhase: number,
): void {
  const { start: a, end: b } = drag;
  switch (drag.tool) {
    case 'line':
      paintLinePreview(overlay, a.x, a.y, b.x, b.y, color);
      break;
    case 'rectangle':
    case 'rounded-rect':
    case 'polygon-shape':
      paintRectanglePreview(overlay, a.x, a.y, b.x, b.y, color, false);
      break;
    case 'rectangle-fill':
      paintRectanglePreview(overlay, a.x, a.y, b.x, b.y, color, true);
      break;
    case 'ellipse':
      paintEllipsePreview(overlay, a.x, a.y, b.x, b.y, color, false);
      break;
    case 'ellipse-fill':
      paintEllipsePreview(overlay, a.x, a.y, b.x, b.y, color, true);
      break;
    case 'selection-ellipse':
      paintEllipsePreview(overlay, a.x, a.y, b.x, b.y, '#ffffff', false);
      break;
    case 'selection-rect':
    case 'slice': {
      // Inclusive-corner marquee: the rect the release hands to wasm.
      const minX = Math.min(a.x, b.x);
      const minY = Math.min(a.y, b.y);
      paintSelectionMarquee(
        overlay,
        minX,
        minY,
        Math.max(a.x, b.x) - minX + 1,
        Math.max(a.y, b.y) - minY + 1,
        marchPhase,
      );
      break;
    }
    default:
      break;
  }
}

/**
 * Marching ants on the slice's frame-0 key (or key 0), plus its 9-patch
 * center band and pivot crosshair. A slice that vanished mid-frame (undo
 * of an addSlice) is skipped; the editor reconciles the stale id.
 */
function paintActiveSlice(
  overlay: HTMLCanvasElement,
  doc: Document,
  id: number,
  marchPhase: number,
): void {
  try {
    const keyCount = doc.sliceKeyCount(id);
    if (keyCount === 0) return;
    let k = 0;
    for (let i = 0; i < keyCount; i += 1) {
      if (doc.sliceKeyFrame(id, i) === 0) {
        k = i;
        break;
      }
    }
    paintSelectionMarquee(
      overlay,
      doc.sliceKeyX(id, k),
      doc.sliceKeyY(id, k),
      doc.sliceKeyWidth(id, k),
      doc.sliceKeyHeight(id, k),
      marchPhase,
    );
    if (doc.sliceKeyHasCenter(id, k)) {
      paintRectOutline(
        overlay,
        doc.sliceKeyCenterX(id, k),
        doc.sliceKeyCenterY(id, k),
        doc.sliceKeyCenterWidth(id, k),
        doc.sliceKeyCenterHeight(id, k),
        // The slice's own color, opaque, so a 9-patch band reads as
        // distinct from the marquee.
        '#' + ((doc.sliceColor(id) >>> 8) & 0xffffff).toString(16).padStart(6, '0'),
      );
    }
    if (doc.sliceKeyHasPivot(id, k)) {
      paintPivotCrosshair(overlay, doc.sliceKeyPivotX(id, k), doc.sliceKeyPivotY(id, k));
    }
  } catch {
    // Slice disappeared mid-frame.
  }
}

/**
 * A grid at the stamp tileset's tile size plus the hovered cell, in the
 * overlay's intrinsic (sprite) pixel space; CSS does the upscale.
 */
function paintTileGrid(
  overlay: HTMLCanvasElement,
  doc: Document,
  tilesetId: number,
  hover: Point | null,
): void {
  const tileW = doc.tilesetTileWidth(tilesetId);
  const tileH = doc.tilesetTileHeight(tilesetId);
  if (tileW === 0 || tileH === 0) return;
  const ctx = overlay.getContext('2d');
  if (!ctx) return;
  ctx.save();
  ctx.strokeStyle = 'rgba(96, 165, 250, 0.5)';
  ctx.lineWidth = 1;
  for (let gx = tileW; gx < overlay.width; gx += tileW) {
    ctx.beginPath();
    ctx.moveTo(gx + 0.5, 0);
    ctx.lineTo(gx + 0.5, overlay.height);
    ctx.stroke();
  }
  for (let gy = tileH; gy < overlay.height; gy += tileH) {
    ctx.beginPath();
    ctx.moveTo(0, gy + 0.5);
    ctx.lineTo(overlay.width, gy + 0.5);
    ctx.stroke();
  }
  if (hover) {
    const cellX = Math.floor(hover.x / tileW) * tileW;
    const cellY = Math.floor(hover.y / tileH) * tileH;
    ctx.strokeStyle = 'rgba(59, 130, 246, 0.95)';
    ctx.lineWidth = 2;
    ctx.strokeRect(cellX + 0.5, cellY + 0.5, tileW - 1, tileH - 1);
  }
  ctx.restore();
}
