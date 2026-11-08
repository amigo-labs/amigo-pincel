// Reading pointer events for the tool suite (`pointer.ts`): document
// coordinates, the selection combine mode, pen pressure and azimuth, and a
// touch press held back until it is clear that no pinch follows.
import { editor } from '../stores/editor.svelte';
import type { SelectionMode } from '../core/wasm';
import { altHeld, shiftHeld } from '../../../lib/shell/modifiers.svelte';

/** Converts a pointer event into document-pixel coordinates. `surface` is
 *  the element that covers the document on screen (any size / transform). */
export function toCanvasPoint(surface: HTMLElement, e: PointerEvent): [number, number] {
  const rect = surface.getBoundingClientRect();
  const scaleX = editor.width / rect.width;
  const scaleY = editor.height / rect.height;
  return [(e.clientX - rect.left) * scaleX, (e.clientY - rect.top) * scaleY];
}

/** Selection combine mode from the keyboard or on-screen modifiers (spec §8.2). */
export function selectionModeOf(e: PointerEvent): SelectionMode {
  const shift = shiftHeld(e);
  const alt = altHeld(e);
  if (shift && alt) return 'intersect';
  if (shift) return 'add';
  if (alt) return 'subtract';
  return 'replace';
}

/** Pen pressure in `[0, 1]`; mouse and touch paint at full pressure. */
export function pressureOf(e: PointerEvent): number {
  return e.pointerType === 'pen' ? Math.min(1, Math.max(0, e.pressure)) : 1;
}

/** Pen azimuth in radians (0 = +x, clockwise), or `null` when the input
 *  is not a pen or the pen stands upright / reports no tilt. Uses the
 *  Pointer Events 3 angles where the browser has them, else derives the
 *  azimuth from `tiltX` / `tiltY`. */
export function azimuthOf(e: PointerEvent): number | null {
  if (e.pointerType !== 'pen') return null;
  const angles = e as PointerEvent & { altitudeAngle?: number; azimuthAngle?: number };
  if (typeof angles.altitudeAngle === 'number' && typeof angles.azimuthAngle === 'number') {
    return angles.altitudeAngle < Math.PI / 2 - 1e-3 ? angles.azimuthAngle : null;
  }
  if (e.tiltX === 0 && e.tiltY === 0) return null;
  const rad = Math.PI / 180;
  return Math.atan2(Math.tan(e.tiltY * rad), Math.tan(e.tiltX * rad));
}

/** How far (CSS px) a held finger may drift and still count as a tap. */
const TOUCH_SLOP = 8;

/** A touch press held back by `touchHold`. */
export interface TouchHold {
  /** Whether a press is held. */
  readonly held: boolean;
  /** Holds the press `e` instead of running it. */
  hold(e: PointerEvent): void;
  /** A pointer moved while a press is held: `true` while the press stays
   *  held (the move is used up); once the held finger passes the slop the
   *  press runs and `false` lets the move run after it. */
  moved(e: PointerEvent): boolean;
  /** A pointer lifted: a held press of that finger runs first. */
  lifted(e: PointerEvent): void;
  /** Drops the held press without running it (a second finger starts a
   *  pinch, the pointer is cancelled); `true` when one was held. */
  drop(): boolean;
}

/**
 * The first finger of a pinch reaches the tools as an ordinary press, and
 * the second one only arrives later. While a live object or a text box is
 * open such a press would commit it, open a new text box or move the caret,
 * none of which the pinch's rollback can undo (docs/specs/image-live-text.md
 * AC-12). So the press is held until its finger lifts or moves past the
 * slop — then `run` gets it — or a second finger drops it.
 */
export function touchHold(run: (down: PointerEvent) => void): TouchHold {
  let held: PointerEvent | null = null;
  const release = (): void => {
    const down = held;
    held = null;
    if (down !== null) run(down);
  };
  return {
    get held(): boolean {
      return held !== null;
    },
    hold(e: PointerEvent): void {
      held = e;
    },
    moved(e: PointerEvent): boolean {
      if (held === null) return false;
      if (e.pointerId !== held.pointerId) return true;
      if (Math.hypot(e.clientX - held.clientX, e.clientY - held.clientY) < TOUCH_SLOP) return true;
      release();
      return false;
    },
    lifted(e: PointerEvent): void {
      if (held !== null && e.pointerId === held.pointerId) release();
    },
    drop(): boolean {
      const had = held !== null;
      held = null;
      return had;
    },
  };
}
