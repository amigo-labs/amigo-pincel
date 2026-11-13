// Pointer gestures on the Move tools' transform box
// (docs/specs/image-transform-handles.md AC-02 – AC-06, AC-08, AC-20,
// AC-23): which part a press hits decides the drag. During the drag the
// box, the outline and — for Move Selected Pixels — the lifted content
// follow the pointer at most once a frame, drawn by the editor from what
// the core handed over when the drag began (`render/transform-preview.ts`);
// the core applies the final box once, on release, as one undo step. A
// press outside the box that moves less than `CLICK_SLOP` screen pixels is
// a click and commits; one that moves farther, or any right-button drag,
// rotates. Wired into `pointer.ts`.
import { applyTransform, commitTransform, previewLift, transformBox } from '../core/transform';
import { dragPreview, type DragPreview } from '../render/transform-preview';
import { live, tool, view } from '../stores/editor.svelte';
import {
  CLICK_SLOP,
  dragCorner,
  dragEdge,
  hitBox,
  movePivot,
  rotateBox,
  translateBox,
  type BoxHit,
  type BoxState,
} from './transform-box';

type Pt = [number, number];

interface Drag {
  readonly pointer: number;
  /** What the drag does; `outside` until it moves past the slop. */
  mode: 'translate' | 'corner' | 'edge' | 'pivot' | 'rotate' | 'outside';
  readonly hit: BoxHit;
  readonly start: BoxState;
  readonly from: Pt;
  readonly screenFrom: Pt;
  readonly pixels: boolean;
  readonly gesture: number;
  /** The box the pointer reached and the frame has not shown yet. */
  pending: BoxState | null;
  /** The last box shown; `null` while the drag has not moved. */
  shown: BoxState | null;
  frame: number;
}

/** The drag preview on the canvas, while a Move Selected Pixels drag runs;
 *  `MainCanvas` draws it instead of the document. */
export const boxPreview: { current: DragPreview | null } = { current: null };

export interface TransformDrags {
  /** A press with a Move tool while the box shows; `true` when used. */
  press(e: PointerEvent, point: Pt, pixels: boolean): boolean;
  /** A pointer move; `true` when it belonged to a box drag. */
  move(e: PointerEvent, point: Pt, shift: boolean): boolean;
  /** A release; `true` when it ended a box drag. */
  up(e: PointerEvent): boolean;
  /** Ends a drag in flight, applying the box it showed; `true` when one
   *  was in flight. */
  abort(): boolean;
  /** Drops a drag in flight without applying it (Undo during the drag);
   *  `true` when one was in flight. */
  drop(): boolean;
  /** Whether a drag is in flight. */
  busy(): boolean;
}

/** Box drags on `stage`; `nextGesture` hands out one id per drag. */
export function transformDrags(
  stage: HTMLElement,
  nextGesture: () => number,
  redraw: () => void,
): TransformDrags {
  let drag: Drag | null = null;

  /** The drag shows itself: the box, and the content for a pixel drag that
   *  can change pixels (a pivot drag changes none, AC-06). */
  const begin = (d: Drag): void => {
    live.transformDrag = { start: d.start, box: d.start };
    if (d.pixels && d.mode !== 'pivot') {
      boxPreview.current = dragPreview(previewLift(true), d.start, tool.moveQuality);
    }
  };

  const flush = (): void => {
    if (drag === null || drag.pending === null) return;
    const next = drag.pending;
    drag.pending = null;
    drag.shown = next;
    live.transformDrag = { start: drag.start, box: next };
    if (boxPreview.current !== null) boxPreview.current.box = next;
    redraw();
  };

  const schedule = (b: BoxState): void => {
    if (drag === null) return;
    drag.pending = b;
    if (drag.frame === 0) {
      drag.frame = requestAnimationFrame(() => {
        if (drag !== null) drag.frame = 0;
        flush();
      });
    }
  };

  /** Ends the drag; `apply` sends the box it reached to the core. */
  const end = (apply: boolean): void => {
    const d = drag;
    if (d === null) return;
    cancelAnimationFrame(d.frame);
    if (stage.hasPointerCapture(d.pointer)) stage.releasePointerCapture(d.pointer);
    drag = null;
    const final = d.pending ?? d.shown;
    // One core apply per drag (AC-23); the preview goes with it.
    if (apply && final !== null) applyTransform(final, d.pixels, d.gesture);
    boxPreview.current = null;
    live.transformDrag = null;
    redraw();
  };

  return {
    press(e, point, pixels) {
      const box = transformBox();
      if (box === null || drag !== null) return false;
      const hit = hitBox(box, point, view.zoom);
      const mode: Drag['mode'] =
        e.button === 2
          ? 'rotate'
          : hit.kind === 'corner'
            ? 'corner'
            : hit.kind === 'edge'
              ? 'edge'
              : hit.kind === 'pivot'
                ? 'pivot'
                : hit.kind === 'inside'
                  ? 'translate'
                  : 'outside';
      drag = {
        pointer: e.pointerId,
        mode,
        hit,
        start: box,
        from: point,
        screenFrom: [e.clientX, e.clientY],
        // A live transform keeps the kind it began with.
        pixels: live.transform?.pixels ?? pixels,
        gesture: nextGesture(),
        pending: null,
        shown: null,
        frame: 0,
      };
      stage.setPointerCapture(e.pointerId);
      if (mode !== 'outside') begin(drag);
      return true;
    },

    move(e, point, shift) {
      if (drag === null || e.pointerId !== drag.pointer) return drag !== null;
      if (drag.mode === 'outside') {
        const moved = Math.hypot(e.clientX - drag.screenFrom[0], e.clientY - drag.screenFrom[1]);
        if (moved < CLICK_SLOP) return true;
        drag.mode = 'rotate';
        begin(drag);
      }
      const { start, from, hit } = drag;
      let next: BoxState;
      switch (drag.mode) {
        case 'translate':
          next = translateBox(start, point[0] - from[0], point[1] - from[1]);
          break;
        case 'corner':
          next = hit.kind === 'corner' ? dragCorner(start, hit.corner, point, shift) : start;
          break;
        case 'edge':
          next = hit.kind === 'edge' ? dragEdge(start, hit.edge, point) : start;
          break;
        case 'pivot':
          next = movePivot(start, point);
          break;
        default:
          next = rotateBox(start, from, point, shift);
      }
      schedule(next);
      return true;
    },

    up(e) {
      if (drag === null || e.pointerId !== drag.pointer) return false;
      const click = drag.mode === 'outside';
      end(true);
      // A click outside the box commits (AC-08).
      if (click) {
        commitTransform();
        redraw();
      }
      return true;
    },

    abort() {
      if (drag === null) return false;
      end(true);
      return true;
    },

    drop() {
      if (drag === null) return false;
      end(false);
      return true;
    },

    busy: () => drag !== null,
  };
}
