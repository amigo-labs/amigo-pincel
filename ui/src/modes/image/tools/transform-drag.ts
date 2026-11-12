// Pointer gestures on the Move tools' transform box
// (docs/specs/image-transform-handles.md AC-02 – AC-06, AC-08): which part a
// press hits decides the drag; every pointer move updates the live
// transform (at most once a frame), the whole drag as one undo step. A
// press outside the box that moves less than `CLICK_SLOP` screen pixels is
// a click and commits; one that moves farther, or any right-button drag,
// rotates. Wired into `pointer.ts`.
import { applyTransform, commitTransform, transformBox } from '../core/transform';
import { live, view } from '../stores/editor.svelte';
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
  pending: BoxState | null;
  frame: number;
}

export interface TransformDrags {
  /** A press with a Move tool while the box shows; `true` when used. */
  press(e: PointerEvent, point: Pt, pixels: boolean): boolean;
  /** A pointer move; `true` when it belonged to a box drag. */
  move(e: PointerEvent, point: Pt, shift: boolean): boolean;
  /** A release; `true` when it ended a box drag. */
  up(e: PointerEvent): boolean;
  /** Ends a drag in flight, keeping what it applied; `true` when one was. */
  abort(): boolean;
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

  const flush = (): void => {
    if (drag === null || drag.pending === null) return;
    const next = drag.pending;
    drag.pending = null;
    applyTransform(next, drag.pixels, drag.gesture);
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

  const end = (): void => {
    if (drag === null) return;
    cancelAnimationFrame(drag.frame);
    if (stage.hasPointerCapture(drag.pointer)) stage.releasePointerCapture(drag.pointer);
    drag = null;
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
        frame: 0,
      };
      stage.setPointerCapture(e.pointerId);
      return true;
    },

    move(e, point, shift) {
      if (drag === null || e.pointerId !== drag.pointer) return drag !== null;
      if (drag.mode === 'outside') {
        const moved = Math.hypot(e.clientX - drag.screenFrom[0], e.clientY - drag.screenFrom[1]);
        if (moved < CLICK_SLOP) return true;
        drag.mode = 'rotate';
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
      flush();
      end();
      // A click outside the box commits (AC-08).
      if (click) {
        commitTransform();
        redraw();
      }
      return true;
    },

    abort() {
      if (drag === null) return false;
      flush();
      end();
      return true;
    },

    busy: () => drag !== null,
  };
}
