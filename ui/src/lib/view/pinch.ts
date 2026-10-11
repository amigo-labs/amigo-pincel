// Two-finger pinch / pan tracking for touch input (spec §17.4, §9.4:
// "Two-finger pan/zoom is part of the standard input handling"). Shared
// by both editors: feed it every pointer event the canvas sees; it only
// tracks `pointerType === 'touch'` pointers and reports a gesture frame
// once two fingers are down. Pen and mouse input pass straight through.

/** One step of a two-finger gesture. */
export interface PinchFrame {
  /** Finger distance relative to when the pinch started (1 = unchanged). */
  scale: number;
  /** Current midpoint between the fingers, in client coordinates. */
  midX: number;
  midY: number;
  /** Midpoint movement since the previous frame (the pan delta). */
  dx: number;
  dy: number;
}

interface Point {
  x: number;
  y: number;
}

export class PinchTracker {
  private readonly touches = new Map<number, Point>();
  private startDistance = 0;
  private lastMid: Point = { x: 0, y: 0 };
  /** Set while a pinch ran; cleared when every finger has lifted. */
  private consumed = false;

  /** `true` while two or more fingers are down. */
  get active(): boolean {
    return this.touches.size >= 2;
  }

  /**
   * `true` from the moment a pinch starts until the last finger lifts —
   * the editor ignores the remaining finger so the end of a pinch does
   * not turn into a stray stroke.
   */
  get suppressing(): boolean {
    return this.consumed;
  }

  /** Track a pointer-down. Returns `true` when this finger starts a pinch. */
  down(e: PointerEvent): boolean {
    if (e.pointerType !== 'touch') return false;
    this.touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (this.touches.size !== 2) return false;
    const [a, b] = this.pair();
    this.startDistance = Math.max(1, distance(a, b));
    this.lastMid = midpoint(a, b);
    this.consumed = true;
    return true;
  }

  /** Track a pointer-move; returns a frame while pinching, else `null`. */
  move(e: PointerEvent): PinchFrame | null {
    if (!this.touches.has(e.pointerId)) return null;
    this.touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (!this.active) return null;
    const [a, b] = this.pair();
    const mid = midpoint(a, b);
    const frame: PinchFrame = {
      scale: distance(a, b) / this.startDistance,
      midX: mid.x,
      midY: mid.y,
      dx: mid.x - this.lastMid.x,
      dy: mid.y - this.lastMid.y,
    };
    this.lastMid = mid;
    return frame;
  }

  /** Track a pointer-up / cancel. */
  up(e: PointerEvent): void {
    this.touches.delete(e.pointerId);
    if (this.touches.size === 0) this.consumed = false;
  }

  private pair(): [Point, Point] {
    const [a, b] = [...this.touches.values()];
    return [a ?? { x: 0, y: 0 }, b ?? { x: 0, y: 0 }];
  }
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function midpoint(a: Point, b: Point): Point {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}
