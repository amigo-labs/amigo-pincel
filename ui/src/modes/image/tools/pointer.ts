// Pointer-event handling for the tool suite. Pointer events (not mouse events)
// are required for stylus/touch support (CLAUDE.md §9). The active tool (editor
// store) selects the behavior; all mutation goes through the controller.
import {
  paintStroke,
  eraseStroke,
  recolorStroke,
  smudgeStroke,
  fillAt,
  moveLayer,
  sampleColor,
  selectRectangle,
  selectEllipse,
  selectPolygon,
  moveSelectionContent,
  moveSelectionMask,
  deselect,
} from '../core/controller';
import {
  editor,
  isShapeTool,
  live,
  shapeOf,
  tool,
  selectionPreview,
  shapePreview,
  view,
  type ToolKind,
} from '../stores/editor.svelte';
import type { PointPair, SelectionMode } from '../core/wasm';
import { beginGradient, beginShape, cancelLiveGesture, commitLive, editLive } from '../core/live';
import { cursorOf, dragPoints, hitLive, type LiveHit, type Pt } from './live-handles';
import { editRefused } from '../core/layer-tree';
import {
  caretToPoint,
  hitText,
  pressText,
  requestTextFocus,
} from '../core/live-text.svelte';
import { t } from '../../../lib/i18n/index.svelte';
import { altHeld, shiftHeld } from '../../../lib/shell/modifiers.svelte';
import { gestureControl } from './gesture-control';
import { frameBatch } from './frame-batch';
import { wandClick } from './wand-link';
import { constrainShape, rectFromCorners, snappedCorners } from './drag-geometry';
import { cloneStroke } from '../core/clone-stroke';
import { attachCloneHover, beginCloneStroke, pressCloneSource } from './clone-press';

/** A live text's box corner moved by the drag `start` → `p`, rounded to
 *  whole document pixels (docs/specs/image-live-text.md AC-07). */
function wholeMove(from: PointPair, start: Pt, p: Pt): PointPair {
  const dx = Math.round(p[0] - start[0]);
  const dy = Math.round(p[1] - start[1]);
  const [a] = from;
  return [
    [a[0] + dx, a[1] + dy],
    [a[0] + dx, a[1] + dy],
  ];
}

/** Converts a pointer event into document-pixel coordinates. `surface` is
 *  the element that covers the document on screen (any size / transform). */
function toCanvasPoint(surface: HTMLElement, e: PointerEvent): [number, number] {
  const rect = surface.getBoundingClientRect();
  const scaleX = editor.width / rect.width;
  const scaleY = editor.height / rect.height;
  return [(e.clientX - rect.left) * scaleX, (e.clientY - rect.top) * scaleY];
}

/** Selection combine mode from the keyboard or on-screen modifiers (spec §8.2). */
function selectionModeOf(e: PointerEvent): SelectionMode {
  const shift = shiftHeld(e);
  const alt = altHeld(e);
  if (shift && alt) return 'intersect';
  if (shift) return 'add';
  if (alt) return 'subtract';
  return 'replace';
}

/** Pen pressure in `[0, 1]`; mouse and touch paint at full pressure. */
function pressureOf(e: PointerEvent): number {
  return e.pointerType === 'pen' ? Math.min(1, Math.max(0, e.pressure)) : 1;
}

/** Pen azimuth in radians (0 = +x, clockwise), or `null` when the input
 *  is not a pen or the pen stands upright / reports no tilt. Uses the
 *  Pointer Events 3 angles where the browser has them, else derives the
 *  azimuth from `tiltX` / `tiltY`. */
function azimuthOf(e: PointerEvent): number | null {
  if (e.pointerType !== 'pen') return null;
  const angles = e as PointerEvent & { altitudeAngle?: number; azimuthAngle?: number };
  if (typeof angles.altitudeAngle === 'number' && typeof angles.azimuthAngle === 'number') {
    return angles.altitudeAngle < Math.PI / 2 - 1e-3 ? angles.azimuthAngle : null;
  }
  if (e.tiltX === 0 && e.tiltY === 0) return null;
  const rad = Math.PI / 180;
  return Math.atan2(Math.tan(e.tiltY * rad), Math.tan(e.tiltX * rad));
}

/** Tools that paint along a polyline, coalesced pen samples and all. */
const BRUSH_TOOLS: ReadonlySet<ToolKind> = new Set(['pencil', 'eraser', 'recolor', 'smudge', 'clone']);

/** The three drag-to-move tools: the layer, the selected pixels, the
 *  outline alone. `move_mask` writes no pixels, so it stays out of
 *  `WRITES_PIXELS` and works on a locked layer. */
const MOVE_TOOLS: ReadonlySet<ToolKind> = new Set(['move', 'move_selection', 'move_mask']);

/** Tools that write into the active layer's pixels. */
const WRITES_PIXELS: ReadonlySet<ToolKind> = new Set([
  'pencil',
  'eraser',
  'fill',
  'gradient',
  'recolor',
  'smudge',
  'clone',
  'move',
  'move_selection',
  'text',
  'line',
  'rectangle',
  'rounded_rect',
  'ellipse',
  'polygon',
]);

/** Tools that drag out a selection shape (rubber band / freehand). */
const DRAG_SELECT: ReadonlySet<ToolKind> = new Set(['rect_select', 'ellipse_select', 'lasso']);

export { gestureControl };

/** Tools that may start on the stage beside the canvas (image-editing AC-29, AC-32). */
const STAGE_TOOLS: ReadonlySet<ToolKind> = new Set([
  'rect_select',
  'ellipse_select',
  'lasso',
  'polygon_lasso',
  'magic_wand',
]);

function onCanvas(p: [number, number]): boolean {
  return p[0] >= 0 && p[1] >= 0 && p[0] < editor.width && p[1] < editor.height;
}

/** The bounding box of `points` misses the canvas entirely. */
function offCanvas(points: ReadonlyArray<[number, number]>): boolean {
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  return (
    Math.max(...xs) <= 0 ||
    Math.max(...ys) <= 0 ||
    Math.min(...xs) >= editor.width ||
    Math.min(...ys) >= editor.height
  );
}

/**
 * A selection gesture that selects no canvas pixel (a click, or a shape off
 * the canvas): Deselect in replace mode, nothing otherwise (AC-30) —
 * combining an empty shape would empty the selection under intersect.
 */
function missedCanvas(mode: SelectionMode): void {
  if (mode === 'replace' && editor.hasSelection) deselect();
}

/** A press on a control floating over the stage (zoom buttons, text entry). */
function onControl(e: PointerEvent): boolean {
  const el = e.target;
  return el instanceof Element && el.closest('button, input, select, textarea, a') !== null;
}

/**
 * Attaches the active tool's pointer behavior to a canvas. Returns a teardown.
 * `canvas` covers the document and maps pointer coordinates; `stage`, the
 * viewport around it, takes the events, so a selection gesture can start
 * beside the canvas (spec §9.4).
 *
 * Paint tools draw incrementally over a drag (one undo step per drag). Fill and
 * Eyedropper act on click. Selection tools drag a rubber band (rect/ellipse),
 * trace a freehand path (lasso), click vertices (polygonal lasso), or click a
 * region (magic wand); Shift/Alt set the combine mode. A redraw callback runs
 * after each mutation so the canvas stays live.
 */
export function attachTools(
  canvas: HTMLElement,
  stage: HTMLElement,
  redraw: () => void,
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
  let lastPressure = 1;
  let lastAzimuth: number | null = null;
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
  // Brush samples wait for the next animation frame: one stroke command and
  // one presentation per frame, however many events arrive
  // (performance-input-path). Flushed before the gesture ends.
  const brushFrames = frameBatch((b) => {
    if (gestureKind === 'pencil')
      paintStroke(b.points, strokeId, useBackground, b.pressure, b.azimuth);
    else if (gestureKind === 'recolor') recolorStroke(b.points, strokeId, b.pressure, b.azimuth);
    else if (gestureKind === 'smudge') smudgeStroke(b.points, strokeId, b.pressure, b.azimuth);
    else if (gestureKind === 'clone') cloneStroke(b.points, strokeId, b.pressure, b.azimuth);
    else eraseStroke(b.points, strokeId, b.pressure, b.azimuth);
  }, redraw);

  // A handle or body drag on the live object: one gesture id per drag, so
  // the core keeps the drag as one undo step (image-live-objects AC-03).
  let liveDrag: {
    pointer: number;
    hit: LiveHit;
    from: PointPair;
    start: Pt;
    gesture: number;
    pending: PointPair | null;
    frame: number;
    /** A live text moves by whole document pixels (image-live-text AC-07). */
    whole: boolean;
  } | null = null;
  // A press-and-drag placing the caret and selecting characters in the live
  // text (image-live-text AC-06): the pointer that owns it.
  let textSelect: number | null = null;

  /** Sends the drag's latest points, at most once per frame. */
  const flushLive = (): void => {
    if (liveDrag === null || liveDrag.pending === null) return;
    const points = liveDrag.pending;
    liveDrag.pending = null;
    if (!editLive({ gesture: liveDrag.gesture, points })) {
      // Committed elsewhere meanwhile (AC-19): drop the drag silently.
      endLiveDrag();
    }
    redraw();
  };

  const endLiveDrag = (): void => {
    if (liveDrag === null) return;
    cancelAnimationFrame(liveDrag.frame);
    if (stage.hasPointerCapture(liveDrag.pointer)) stage.releasePointerCapture(liveDrag.pointer);
    liveDrag = null;
  };

  /** Drops the drag in progress and its step (Escape, Undo: AC-11). */
  const cancelLiveDrag = (): boolean => {
    if (liveDrag === null) return false;
    const gesture = liveDrag.gesture;
    endLiveDrag();
    cancelLiveGesture(gesture);
    redraw();
    return true;
  };

  /**
   * A press while an object is live (image-live-objects-ui AC-02 – AC-05,
   * AC-09, AC-10): a handle or the body starts a drag; a Line's stroke is
   * reserved; anywhere else commits. `true` when the press is used up.
   */
  const pressLive = (e: PointerEvent, point: Pt): boolean => {
    const obj = live.object;
    if (obj === null) return false;
    if (obj.type === 'text') {
      if (e.button !== 0) return true;
      // The move handle drags the text; inside the box a press places the
      // caret; outside it commits (image-live-text AC-06, AC-07, AC-11).
      const hit = hitText(point, view.zoom, e.pointerType !== 'mouse');
      if (hit === 'handle') {
        const at: [number, number] = [obj.x, obj.y];
        liveDrag = {
          pointer: e.pointerId,
          hit: { kind: 'body' },
          from: [at, [at[0], at[1]]],
          start: point,
          gesture: nextStrokeId++,
          pending: null,
          frame: 0,
          whole: true,
        };
        stage.setPointerCapture(e.pointerId);
        requestTextFocus();
        return true;
      }
      if (hit === 'inside') {
        startTextSelect(e, point);
        return true;
      }
      commitLive();
      redraw();
      return false;
    }
    // The right button does nothing while an object is live.
    if (e.button !== 0) return true;
    const hit = hitLive(obj, point, view.zoom, e.pointerType !== 'mouse');
    if (hit?.kind === 'stroke') return true;
    if (hit !== null) {
      liveDrag = {
        pointer: e.pointerId,
        hit,
        from: obj.points,
        start: point,
        gesture: nextStrokeId++,
        pending: null,
        frame: 0,
        whole: false,
      };
      stage.setPointerCapture(e.pointerId);
      return true;
    }
    commitLive();
    redraw();
    return false;
  };

  /** Places the caret in the open text box and starts a selecting drag. */
  const startTextSelect = (e: PointerEvent, point: Pt): void => {
    caretToPoint(point, shiftHeld(e));
    textSelect = e.pointerId;
    stage.setPointerCapture(e.pointerId);
    requestTextFocus();
  };

  const endTextSelect = (): void => {
    if (textSelect !== null && stage.hasPointerCapture(textSelect)) {
      stage.releasePointerCapture(textSelect);
    }
    textSelect = null;
  };

  /** Commits the placed polygon if it has enough vertices, then resets. */
  const commitPolygon = (): void => {
    if (polygonPoints.length >= 3) {
      if (offCanvas(polygonPoints)) missedCanvas(polygonMode);
      else selectPolygon(polygonPoints, polygonMode);
    }
    polygonPoints = [];
    selectionPreview.value = null;
  };

  const onDown = (e: PointerEvent): void => {
    if ((e.button !== 0 && e.button !== 2) || onControl(e)) {
      return;
    }
    // Ignore additional pointers while a gesture is in flight.
    if (active || liveDrag !== null || textSelect !== null) {
      return;
    }
    if (pressLive(e, toCanvasPoint(canvas, e))) {
      return;
    }
    // Selection, shapes and text respond to the primary button only; the right
    // button is reserved for the background-color paint variants (spec §9.2).
    const selectionOrPlacement =
      DRAG_SELECT.has(tool.kind) ||
      tool.kind === 'polygon_lasso' ||
      tool.kind === 'magic_wand' ||
      isShapeTool(tool.kind) ||
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
    const inside = onCanvas(point);
    // Beside the canvas only the selection gestures and a text box start
    // (AC-32; image-live-text AC-01).
    if (!inside && !STAGE_TOOLS.has(tool.kind) && tool.kind !== 'text') {
      return;
    }
    // Ctrl / ⌘ or Set source sets the Clone Stamp's source, even on a
    // locked layer (docs/specs/clone-stamp.md AC-03, AC-04, AC-14).
    if (tool.kind === 'clone' && pressCloneSource(e, point)) return;

    // Pixel-writing tools refuse a locked layer, a layer in a locked group
    // and a group (spec §3.2, docs/specs/image-layer-groups.md AC-11,
    // AC-12); Move (layer) moves a group's content. Say why.
    if (WRITES_PIXELS.has(tool.kind)) {
      const layer = editor.layers[editor.activeLayer];
      if (layer && editRefused(editor.layers, editor.activeLayer, tool.kind === 'move')) {
        editor.notice = t('layers.locked', { name: layer.name });
        return;
      }
    }
    editor.notice = null;
    if (tool.kind === 'clone' && !beginCloneStroke(point)) return;

    if (tool.kind === 'magic_wand') {
      wandClick(inside ? [point[0], point[1]] : null, selectionModeOf(e));
      return;
    }

    // Text opens a box (image-live-text AC-01); inside an empty box the
    // press keeps it.
    if (tool.kind === 'text') {
      if (hitText(point, view.zoom, e.pointerType !== 'mouse') !== null) {
        startTextSelect(e, point);
      } else {
        void pressText(point);
      }
      return;
    }

    // The gradient drags like a shape and previews as the ramp's own line.
    const shape = tool.kind === 'gradient' ? 'line' : shapeOf(tool.kind);
    if (shape !== null) {
      active = true;
      activePointer = e.pointerId;
      gestureKind = tool.kind;
      start = point;
      last = point;
      stage.setPointerCapture(e.pointerId);
      shapePreview.value = {
        kind: shape,
        a: point,
        b: point,
        sides: tool.shapeSides,
        cornerRadius: tool.cornerRadius,
      };
      return;
    }

    if (tool.kind === 'polygon_lasso') {
      // Click near the first vertex closes the polygon.
      const first = polygonPoints[0];
      if (polygonPoints.length >= 3 && first !== undefined) {
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
      stage.setPointerCapture(e.pointerId);
      if (tool.kind === 'lasso') {
        lassoPath = [point];
        selectionPreview.value = { shape: 'lasso', points: lassoPath };
      } else {
        const shape = tool.kind === 'ellipse_select' ? 'ellipse' : 'rect';
        selectionPreview.value = { shape, points: snappedCorners(point, point) };
      }
      return;
    }

    active = true;
    activePointer = e.pointerId;
    gestureKind = tool.kind;
    useBackground = e.button === 2;
    last = point;
    lastPressure = pressureOf(e);
    lastAzimuth = azimuthOf(e);
    start = point;
    strokeId = nextStrokeId++;
    stage.setPointerCapture(e.pointerId);

    switch (tool.kind) {
      // The press dab is its own command (a click is one dab); it shows in
      // the next frame together with whatever the drag adds by then.
      case 'pencil':
        paintStroke([point], strokeId, useBackground, [lastPressure], [lastAzimuth]);
        brushFrames.present();
        break;
      case 'eraser':
        eraseStroke([point], strokeId, [lastPressure], [lastAzimuth]);
        brushFrames.present();
        break;
      case 'recolor':
        recolorStroke([point], strokeId, [lastPressure], [lastAzimuth]);
        brushFrames.present();
        break;
      case 'smudge':
        // A single dab only picks colour up; the stroke starts on the move.
        smudgeStroke([point], strokeId, [lastPressure], [lastAzimuth]);
        break;
      case 'clone':
        cloneStroke([point], strokeId, [lastPressure], [lastAzimuth]);
        brushFrames.present();
        break;
      case 'fill':
        fillAt(point[0], point[1], useBackground);
        redraw();
        break;
      case 'eyedropper':
        sampleColor(point[0], point[1], useBackground);
        break;
      case 'move':
      case 'move_selection':
      case 'move_mask':
        break;
    }
  };

  const onMove = (e: PointerEvent): void => {
    if (liveDrag !== null) {
      if (e.pointerId !== liveDrag.pointer) return;
      const p = toCanvasPoint(canvas, e);
      liveDrag.pending = liveDrag.whole
        ? wholeMove(liveDrag.from, liveDrag.start, p)
        : dragPoints(liveDrag.hit, liveDrag.from, liveDrag.start, p, shiftHeld(e));
      if (liveDrag.frame === 0) {
        liveDrag.frame = requestAnimationFrame(() => {
          if (liveDrag !== null) liveDrag.frame = 0;
          flushLive();
        });
      }
      return;
    }
    if (textSelect !== null) {
      if (e.pointerId === textSelect) caretToPoint(toCanvasPoint(canvas, e), true);
      return;
    }
    // The cursor over the live object's handles and body (AC-13).
    if (!active) {
      const obj = live.object;
      const p = toCanvasPoint(canvas, e);
      const touch = e.pointerType !== 'mouse';
      if (obj === null) live.cursor = null;
      else if (obj.type === 'text') {
        live.cursor = hitText(p, view.zoom, touch) === 'handle' ? 'move' : null;
      } else live.cursor = cursorOf(hitLive(obj, p, view.zoom, touch));
    }
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
        selectionPreview.value = { shape, points: snappedCorners(start, point) };
      }
      last = point;
      return;
    }

    const dragShape = gestureKind === 'gradient' ? 'line' : shapeOf(gestureKind);
    if (dragShape !== null && start) {
      const b = constrainShape(start, point, dragShape, shiftHeld(e));
      shapePreview.value = {
        kind: dragShape,
        a: start,
        b,
        sides: tool.shapeSides,
        cornerRadius: tool.cornerRadius,
      };
      last = b;
      return;
    }

    if (BRUSH_TOOLS.has(gestureKind)) {
      // A pen reports several samples per frame; paint them all so fast
      // strokes keep their shape and pressure.
      const samples = e.getCoalescedEvents?.() ?? [];
      const events = samples.length > 0 ? samples : [e];
      const points = events.map((ev) => toCanvasPoint(canvas, ev));
      const pressure = events.map(pressureOf);
      const azimuth = events.map(azimuthOf);
      brushFrames.queue(
        { points: [last], pressure: [lastPressure], azimuth: [lastAzimuth] },
        { points, pressure, azimuth },
      );
      last = points[points.length - 1] ?? point;
      lastPressure = pressure[pressure.length - 1] ?? 1;
      lastAzimuth = azimuth[azimuth.length - 1] ?? null;
      return;
    }

    switch (gestureKind) {
      case 'eyedropper':
        sampleColor(point[0], point[1], useBackground);
        break;
      default:
        break;
    }
    last = point;
  };

  const onUp = (e: PointerEvent): void => {
    if (liveDrag !== null && e.pointerId === liveDrag.pointer) {
      cancelAnimationFrame(liveDrag.frame);
      flushLive();
      endLiveDrag();
      return;
    }
    if (textSelect !== null && e.pointerId === textSelect) {
      endTextSelect();
      return;
    }
    if (!active || e.pointerId !== activePointer) {
      return;
    }
    // Samples queued since the last frame belong to the stroke.
    brushFrames.flush();
    if (DRAG_SELECT.has(gestureKind) && start && last) {
      if (gestureKind === 'lasso') {
        if (lassoPath.length >= 3 && !offCanvas(lassoPath)) {
          selectPolygon(lassoPath, selMode);
        } else {
          missedCanvas(selMode);
        }
        lassoPath = [];
      } else {
        const r = rectFromCorners(start, last);
        const missed =
          r.w === 0 ||
          r.h === 0 ||
          offCanvas([
            [r.x, r.y],
            [r.x + r.w, r.y + r.h],
          ]);
        if (missed) {
          missedCanvas(selMode);
        } else if (gestureKind === 'ellipse_select') {
          selectEllipse(r.x, r.y, r.w, r.h, selMode);
        } else {
          selectRectangle(r.x, r.y, r.w, r.h, selMode);
        }
      }
      selectionPreview.value = null;
    } else if (gestureKind === 'gradient' && start && last) {
      // The object stays live until it is committed (image-live-objects-ui).
      beginGradient(start, last);
      redraw();
    } else if (isShapeTool(gestureKind) && start && last) {
      // The shape the gesture began with, not whatever `U` has selected
      // since: the object has to match the preview being dragged.
      const drawn = shapeOf(gestureKind);
      if (drawn !== null) {
        beginShape(start, last, drawn);
        redraw();
      }
    } else if (MOVE_TOOLS.has(gestureKind) && start && last) {
      const dx = Math.round(last[0] - start[0]);
      const dy = Math.round(last[1] - start[1]);
      if (gestureKind === 'move_selection') moveSelectionContent(dx, dy);
      else if (gestureKind === 'move_mask') moveSelectionMask(dx, dy);
      else moveLayer(dx, dy);
      redraw();
    }
    // Always clear the shape preview, even if the tool changed mid-drag, so a
    // stale rubber-band can never stay on the overlay.
    shapePreview.value = null;
    active = false;
    activePointer = null;
    last = null;
    start = null;
    if (stage.hasPointerCapture(e.pointerId)) {
      stage.releasePointerCapture(e.pointerId);
    }
  };

  /**
   * Abandons the in-flight gesture without committing: commit-on-release
   * gestures (selection/shape/move) are discarded, previews cleared, and any
   * placed polygon vertices dropped. Pencil/Eraser paint incrementally, so
   * pixels already applied stay — as a single undoable stroke (stroke_id).
   */
  const abort = (): boolean => {
    if (cancelLiveDrag()) return true;
    if (textSelect !== null) {
      endTextSelect();
      return true;
    }
    const inProgress = active || polygonPoints.length > 0;
    // Brush pixels painted so far stay; so do the samples still queued.
    if (active) brushFrames.flush();
    if (activePointer !== null && stage.hasPointerCapture(activePointer)) {
      stage.releasePointerCapture(activePointer);
    }
    active = false;
    activePointer = null;
    last = null;
    start = null;
    lassoPath = [];
    polygonPoints = [];
    selectionPreview.value = null;
    shapePreview.value = null;
    return inProgress;
  };

  // A cancelled pointer (touch scroll, palm rejection, window loss) abandons
  // the in-flight gesture — unless the cancel is for a bystander pointer.
  const onCancel = (e: PointerEvent): void => {
    if (textSelect !== null) {
      if (e.pointerId === textSelect) endTextSelect();
      return;
    }
    if (liveDrag !== null) {
      if (e.pointerId === liveDrag.pointer) {
        cancelAnimationFrame(liveDrag.frame);
        flushLive();
        endLiveDrag();
      }
      return;
    }
    if (active && e.pointerId !== activePointer) {
      return;
    }
    void abort();
  };

  // Double-click closes a polygonal-lasso selection.
  const onDblClick = (): void => {
    if (tool.kind === 'polygon_lasso') {
      commitPolygon();
    }
  };

  // Suppress the context menu so right-button paint/sample works.
  const onContextMenu = (e: Event): void => e.preventDefault();

  stage.addEventListener('pointerdown', onDown);
  stage.addEventListener('pointermove', onMove);
  stage.addEventListener('pointerup', onUp);
  stage.addEventListener('pointercancel', onCancel);
  stage.addEventListener('dblclick', onDblClick);
  stage.addEventListener('contextmenu', onContextMenu);
  gestureControl.cancel = abort;
  gestureControl.cancelLiveDrag = cancelLiveDrag;
  const detachCloneHover = attachCloneHover(stage, (e) => toCanvasPoint(canvas, e));

  return () => {
    detachCloneHover();
    brushFrames.dispose();
    gestureControl.cancel = () => false;
    gestureControl.cancelLiveDrag = () => false;
    endLiveDrag();
    stage.removeEventListener('pointerdown', onDown);
    stage.removeEventListener('pointermove', onMove);
    stage.removeEventListener('pointerup', onUp);
    stage.removeEventListener('pointercancel', onCancel);
    stage.removeEventListener('dblclick', onDblClick);
    stage.removeEventListener('contextmenu', onContextMenu);
  };
}
