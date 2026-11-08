// Touch gestures both editors share (docs/specs/tablet-gestures.md): two- and
// three-finger taps for Undo / Redo, and the pen's eraser end. Fed the same
// pointer events as the `PinchTracker`; rune-free so node-side specs can
// test it.

/** Recognition thresholds of a multi-finger tap. */
export const TAP_LIMITS = {
  /** Every finger lands within this many ms of the first. */
  landMs: 150,
  /** No finger moves farther than this many CSS px from where it landed. */
  moveCss: 10,
  /** Every finger is lifted within this many ms of the first touch. */
  liftMs: 300,
  /** The two-finger distance changes by at most this fraction. */
  scale: 0.02,
} as const;

/** What a finished tap asks for. */
export type TapAction = 'undo' | 'redo';

interface Touch {
  x0: number;
  y0: number;
  x: number;
  y: number;
}

/**
 * Recognises a two-finger tap (Undo) and a three-finger tap (Redo): all
 * fingers land within `landMs`, none moves more than `moveCss`, a pair's
 * distance stays within `scale`, and all are lifted within `liftMs` of the
 * first touch. Only `pointerType === 'touch'` counts.
 */
export class TapTracker {
  private readonly touches = new Map<number, Touch>();
  private firstAt = 0;
  private most = 0;
  private broken = false;
  private pairDistance = 0;

  /** Track a pointer-down. */
  down(e: PointerEvent): void {
    if (e.pointerType !== 'touch') return;
    if (this.touches.size === 0) {
      this.firstAt = e.timeStamp;
      this.most = 0;
      this.broken = false;
    } else if (e.timeStamp - this.firstAt > TAP_LIMITS.landMs) {
      this.broken = true;
    }
    this.touches.set(e.pointerId, { x0: e.clientX, y0: e.clientY, x: e.clientX, y: e.clientY });
    this.most = Math.max(this.most, this.touches.size);
    if (this.touches.size > 3) this.broken = true;
    if (this.touches.size === 2) this.pairDistance = this.distance();
  }

  /** Track a pointer-move. */
  move(e: PointerEvent): void {
    const t = this.touches.get(e.pointerId);
    if (!t) return;
    t.x = e.clientX;
    t.y = e.clientY;
    if (Math.hypot(t.x - t.x0, t.y - t.y0) > TAP_LIMITS.moveCss) this.broken = true;
    if (this.touches.size === 2 && this.pairDistance > 0) {
      const ratio = this.distance() / this.pairDistance;
      if (Math.abs(ratio - 1) > TAP_LIMITS.scale) this.broken = true;
    }
  }

  /** Track a pointer-up; the tap's action once its last finger lifts. */
  up(e: PointerEvent): TapAction | null {
    if (!this.touches.delete(e.pointerId)) return null;
    if (this.touches.size > 0) return null;
    if (this.broken || e.timeStamp - this.firstAt > TAP_LIMITS.liftMs) return null;
    if (this.most === 2) return 'undo';
    if (this.most === 3) return 'redo';
    return null;
  }

  /** Track a pointer-cancel: the tap is off. */
  cancel(e: PointerEvent): void {
    if (!this.touches.delete(e.pointerId)) return;
    this.broken = true;
  }

  private distance(): number {
    const [a, b] = [...this.touches.values()];
    return a && b ? Math.hypot(a.x - b.x, a.y - b.y) : 0;
  }
}

/** A pen touching with its eraser end, or with the eraser button held
 *  (button 5, or bit 32 of `buttons`; tablet-gestures AC-07). */
export function isEraserEnd(e: PointerEvent): boolean {
  return e.pointerType === 'pen' && (e.button === 5 || (e.buttons & 32) !== 0);
}
