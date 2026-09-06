// Pointer-event handling for the tool suite. Pointer events (not mouse events)
// are required for stylus/touch support (CLAUDE.md §9). The active tool (editor
// store) selects the behavior; all mutation goes through the controller.
import {
  paintStroke,
  eraseStroke,
  fillAt,
  moveLayer,
  sampleColor,
  selectRectangle,
  selectEllipse,
  selectPolygon,
  selectWand,
  drawShape,
} from '../core/controller';
import { tool, selectionPreview, shapePreview, type ToolKind } from '../stores/editor.svelte';
import type { SelectionMode, ShapeKind } from '../core/wasm';

/** Converts a pointer event into canvas-pixel coordinates. */
function toCanvasPoint(canvas: HTMLCanvasElement, e: PointerEvent): [number, number] {
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width / rect.width;
  const scaleY = canvas.height / rect.height;
  return [(e.clientX - rect.left) * scaleX, (e.clientY - rect.top) * scaleY];
}

/** Selection combine mode from keyboard modifiers (spec §8.2). */
function selectionModeOf(e: PointerEvent): SelectionMode {
  if (e.shiftKey && e.altKey) return 'intersect';
  if (e.shiftKey) return 'add';
  if (e.altKey) return 'subtract';
  return 'replace';
}

/** Tools that drag out a selection shape (rubber band / freehand). */
const DRAG_SELECT: ReadonlySet<ToolKind> = new Set(['rect_select', 'ellipse_select', 'lasso']);

/**
 * Cancels the in-flight gesture without committing (Escape). Wired up by
 * `attachTools`; a no-op while no canvas is mounted.
 */
export const gestureControl = { cancel: (): void => {} };

/**
 * Normalizes two corners into a positive-size rect.
 *
 * Shift is reserved for the selection combine mode (add), so rectangle/ellipse
 * do not also constrain to a square on Shift — that would conflict with the
 * documented modifier mapping (spec §8.2 vs §9.3).
 */
function rectFromCorners(
  a: [number, number],
  b: [number, number],
): { x: number; y: number; w: number; h: number } {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const x = Math.round(Math.min(a[0], a[0] + dx));
  const y = Math.round(Math.min(a[1], a[1] + dy));
  return { x, y, w: Math.round(Math.abs(dx)), h: Math.round(Math.abs(dy)) };
}

/**
 * Applies the Shapes-tool Shift constraint to the drag endpoint (spec §9.2):
 * a square box for area shapes, a 45°-snapped segment for a line.
 */
function constrainShape(
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

/**
 * Attaches the active tool's pointer behavior to a canvas. Returns a teardown.
 *
 * Paint tools draw incrementally over a drag (one undo step per drag). Fill and
 * Eyedropper act on click. Selection tools drag a rubber band (rect/ellipse),
 * trace a freehand path (lasso), click vertices (polygonal lasso), or click a
 * region (magic wand); Shift/Alt set the combine mode. A redraw callback runs
 * after each mutation so the canvas stays live.
 */
export function attachTools(
  canvas: HTMLCanvasElement,
  redraw: () => void,
  onPlaceText?: (cx: number, cy: number, e: PointerEvent) => void,
): () => void {
  let active = false;
  // The pointer that owns the in-flight gesture: a second pointer coming down
  // mid-drag (second touch finger, other mouse button) must not clobber the
  // gesture state or steal the capture.
  let activePointer: number | null = null;
  // The tool that started the drag: keyboard tool switches mid-drag must not
  // change how the in-flight gesture is interpreted or committed.
  let gestureKind: ToolKind = 'pencil';
  let last: [number, number] | null = null;
  let start: [number, number] | null = null;
  let useBackground = false;
  let selMode: SelectionMode = 'replace';
  // Lasso freehand path accumulated during a drag.
  let lassoPath: Array<[number, number]> = [];
  // Polygonal-lasso vertices placed across separate clicks.
  let polygonPoints: Array<[number, number]> = [];
  let polygonMode: SelectionMode = 'replace';
  // Monotonic id per pointer drag so the core merges a drag's segments into one
  // undo step but keeps separate strokes separate.
  let nextStrokeId = 1;
  let strokeId = 0;

  /** Commits the placed polygon if it has enough vertices, then resets. */
  const commitPolygon = (): void => {
    if (polygonPoints.length >= 3) {
      selectPolygon(polygonPoints, polygonMode);
    }
    polygonPoints = [];
    selectionPreview.value = null;
  };

  const onDown = (e: PointerEvent): void => {
    if (e.button !== 0 && e.button !== 2) {
      return;
    }
    // Ignore additional pointers while a gesture is in flight.
    if (active) {
      return;
    }
    // Selection, shapes and text respond to the primary button only; the right
    // button is reserved for the background-color paint variants (spec §9.2).
    const selectionOrPlacement =
      DRAG_SELECT.has(tool.kind) ||
      tool.kind === 'polygon_lasso' ||
      tool.kind === 'magic_wand' ||
      tool.kind === 'shapes' ||
      tool.kind === 'text';
    if (selectionOrPlacement && e.button !== 0) {
      return;
    }
    // Abandon any in-progress polygon when switching away from that tool.
    if (tool.kind !== 'polygon_lasso' && polygonPoints.length > 0) {
      polygonPoints = [];
      selectionPreview.value = null;
    }

    const point = toCanvasPoint(canvas, e);

    if (tool.kind === 'magic_wand') {
      selectWand(point[0], point[1], selectionModeOf(e));
      return;
    }

    // Text places a cursor for the entry overlay; it has no drag gesture here.
    if (tool.kind === 'text') {
      onPlaceText?.(point[0], point[1], e);
      return;
    }

    if (tool.kind === 'shapes') {
      active = true;
      activePointer = e.pointerId;
      gestureKind = tool.kind;
      start = point;
      last = point;
      canvas.setPointerCapture(e.pointerId);
      shapePreview.value = {
        kind: tool.shapeKind,
        a: point,
        b: point,
        sides: tool.shapeSides,
        cornerRadius: tool.cornerRadius,
      };
      return;
    }

    if (tool.kind === 'polygon_lasso') {
      // Click near the first vertex closes the polygon.
      if (polygonPoints.length >= 3) {
        const first = polygonPoints[0];
        const near = Math.hypot(point[0] - first[0], point[1] - first[1]) < 8;
        if (near) {
          commitPolygon();
          return;
        }
      }
      if (polygonPoints.length === 0) {
        polygonMode = selectionModeOf(e);
      }
      polygonPoints = [...polygonPoints, point];
      selectionPreview.value = { shape: 'polygon', points: [...polygonPoints, point] };
      return;
    }

    if (DRAG_SELECT.has(tool.kind)) {
      active = true;
      activePointer = e.pointerId;
      gestureKind = tool.kind;
      start = point;
      last = point;
      selMode = selectionModeOf(e);
      canvas.setPointerCapture(e.pointerId);
      if (tool.kind === 'lasso') {
        lassoPath = [point];
        selectionPreview.value = { shape: 'lasso', points: lassoPath };
      } else {
        const shape = tool.kind === 'ellipse_select' ? 'ellipse' : 'rect';
        selectionPreview.value = { shape, points: [point, point] };
      }
      return;
    }

    active = true;
    activePointer = e.pointerId;
    gestureKind = tool.kind;
    useBackground = e.button === 2;
    last = point;
    start = point;
    strokeId = nextStrokeId++;
    canvas.setPointerCapture(e.pointerId);

    switch (tool.kind) {
      case 'pencil':
        paintStroke([point], strokeId, useBackground);
        redraw();
        break;
      case 'eraser':
        eraseStroke([point], strokeId);
        redraw();
        break;
      case 'fill':
        fillAt(point[0], point[1], useBackground);
        redraw();
        break;
      case 'eyedropper':
        sampleColor(point[0], point[1], useBackground);
        break;
      case 'move':
        break;
    }
  };

  const onMove = (e: PointerEvent): void => {
    // Only the pointer that started the gesture may drive it.
    if (active && e.pointerId !== activePointer) {
      return;
    }
    const point = toCanvasPoint(canvas, e);

    // Polygonal lasso tracks a rubber line to the cursor between clicks.
    if (tool.kind === 'polygon_lasso' && polygonPoints.length > 0) {
      selectionPreview.value = { shape: 'polygon', points: [...polygonPoints, point] };
      return;
    }

    if (!active || !last) {
      return;
    }

    if (DRAG_SELECT.has(gestureKind) && start) {
      if (gestureKind === 'lasso') {
        lassoPath = [...lassoPath, point];
        selectionPreview.value = { shape: 'lasso', points: lassoPath };
      } else {
        const shape = gestureKind === 'ellipse_select' ? 'ellipse' : 'rect';
        selectionPreview.value = { shape, points: [start, point] };
      }
      last = point;
      return;
    }

    if (gestureKind === 'shapes' && start) {
      const b = constrainShape(start, point, tool.shapeKind, e.shiftKey);
      shapePreview.value = {
        kind: tool.shapeKind,
        a: start,
        b,
        sides: tool.shapeSides,
        cornerRadius: tool.cornerRadius,
      };
      last = b;
      return;
    }

    switch (gestureKind) {
      case 'pencil':
        paintStroke([last, point], strokeId, useBackground);
        redraw();
        break;
      case 'eraser':
        eraseStroke([last, point], strokeId);
        redraw();
        break;
      case 'eyedropper':
        sampleColor(point[0], point[1], useBackground);
        break;
      default:
        break;
    }
    last = point;
  };

  const onUp = (e: PointerEvent): void => {
    if (!active || e.pointerId !== activePointer) {
      return;
    }
    if (DRAG_SELECT.has(gestureKind) && start && last) {
      if (gestureKind === 'lasso') {
        if (lassoPath.length >= 3) {
          selectPolygon(lassoPath, selMode);
        }
        lassoPath = [];
      } else {
        const r = rectFromCorners(start, last);
        if (r.w > 0 && r.h > 0) {
          if (gestureKind === 'ellipse_select') {
            selectEllipse(r.x, r.y, r.w, r.h, selMode);
          } else {
            selectRectangle(r.x, r.y, r.w, r.h, selMode);
          }
        }
      }
      selectionPreview.value = null;
    } else if (gestureKind === 'shapes' && start && last) {
      drawShape(start, last);
      redraw();
    } else if (gestureKind === 'move' && start && last) {
      const dx = Math.round(last[0] - start[0]);
      const dy = Math.round(last[1] - start[1]);
      moveLayer(dx, dy);
      redraw();
    }
    // Always clear the shape preview, even if the tool changed mid-drag, so a
    // stale rubber-band can never stay on the overlay.
    shapePreview.value = null;
    active = false;
    activePointer = null;
    last = null;
    start = null;
    if (canvas.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId);
    }
  };

  /**
   * Abandons the in-flight gesture without committing: commit-on-release
   * gestures (selection/shape/move) are discarded, previews cleared, and any
   * placed polygon vertices dropped. Pencil/Eraser paint incrementally, so
   * pixels already applied stay — as a single undoable stroke (stroke_id).
   */
  const abort = (): void => {
    if (activePointer !== null && canvas.hasPointerCapture(activePointer)) {
      canvas.releasePointerCapture(activePointer);
    }
    active = false;
    activePointer = null;
    last = null;
    start = null;
    lassoPath = [];
    polygonPoints = [];
    selectionPreview.value = null;
    shapePreview.value = null;
  };

  // A cancelled pointer (touch scroll, palm rejection, window loss) abandons
  // the in-flight gesture — unless the cancel is for a bystander pointer.
  const onCancel = (e: PointerEvent): void => {
    if (active && e.pointerId !== activePointer) {
      return;
    }
    abort();
  };

  // Double-click closes a polygonal-lasso selection.
  const onDblClick = (): void => {
    if (tool.kind === 'polygon_lasso') {
      commitPolygon();
    }
  };

  // Suppress the context menu so right-button paint/sample works.
  const onContextMenu = (e: Event): void => e.preventDefault();

  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onCancel);
  canvas.addEventListener('dblclick', onDblClick);
  canvas.addEventListener('contextmenu', onContextMenu);
  gestureControl.cancel = abort;

  return () => {
    gestureControl.cancel = () => {};
    canvas.removeEventListener('pointerdown', onDown);
    canvas.removeEventListener('pointermove', onMove);
    canvas.removeEventListener('pointerup', onUp);
    canvas.removeEventListener('pointercancel', onCancel);
    canvas.removeEventListener('dblclick', onDblClick);
    canvas.removeEventListener('contextmenu', onContextMenu);
  };
}
