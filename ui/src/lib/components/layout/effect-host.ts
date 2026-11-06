// How the editor frame hosts the open effect's panel (effect-panel spec,
// revision 2026-11-05): floating over the canvas area at 640 px and more,
// docked to the bottom edge below. The frame provides an `EffectHost`
// through context; `EffectPanel` reads it to style itself and to wire its
// title bar, which drags the panel (pointer) and moves it (arrow keys).

import { getContext, setContext } from 'svelte';
import type { Attachment } from 'svelte/attachments';

/** What the frame tells the effect panel it hosts. */
export interface EffectHost {
  /** Floating over the canvas area; `false` = docked to the bottom edge. */
  readonly floating: boolean;
  /** Wires the panel's title bar: drag and arrow keys move the panel. */
  readonly titleBar: Attachment<HTMLElement>;
}

const HOST = Symbol('pc-effect-host');

/** Provide the effect host for panels rendered inside the caller. */
export function provideEffectHost(host: EffectHost): void {
  setContext(HOST, host);
}

/** The effect host of the enclosing frame, if any. */
export function effectHost(): EffectHost | undefined {
  return getContext<EffectHost | undefined>(HOST);
}

/** What a title bar reports; distances are screen px since the press
 *  (`dx` positive = right, `dy` positive = down). */
export interface MoverEvents {
  /** Moving is possible now (the floating layout). */
  enabled(): boolean;
  /** A drag starts. */
  begin(): void;
  /** The pointer moved during a drag. */
  move(dx: number, dy: number): void;
  /** The drag ended (release, cancel or lost capture). */
  end(): void;
  /** An arrow key: one step of `dx`, `dy` px. */
  step(dx: number, dy: number): void;
}

/** Arrow-key step of the title bar in px (effect-panel AC-20). */
export const MOVE_STEP = 16;

const ARROWS: Readonly<Record<string, readonly [number, number]>> = {
  ArrowLeft: [-MOVE_STEP, 0],
  ArrowRight: [MOVE_STEP, 0],
  ArrowUp: [0, -MOVE_STEP],
  ArrowDown: [0, MOVE_STEP],
};

/** Attachment wiring a title bar to `events`: focusable, dragged by one
 *  pointer at a time (mouse, pen or touch; a press on a button inside
 *  starts nothing), moved by the arrow keys. It lives in an attachment
 *  for the same reason as `splitter`: Svelte's a11y check flags
 *  `tabindex` and key handlers on the title bar's `group` role in markup. */
export function mover(events: MoverEvents): Attachment<HTMLElement> {
  return (node) => {
    node.tabIndex = 0;
    let drag: { pointer: number; x: number; y: number } | null = null;

    const down = (e: PointerEvent) => {
      if (drag !== null || !events.enabled()) return;
      if (e.pointerType === 'mouse' && e.button !== 0) return;
      if (e.target instanceof Element && e.target.closest('button')) return;
      e.preventDefault();
      node.setPointerCapture(e.pointerId);
      drag = { pointer: e.pointerId, x: e.clientX, y: e.clientY };
      events.begin();
    };
    const move = (e: PointerEvent) => {
      if (drag?.pointer === e.pointerId) events.move(e.clientX - drag.x, e.clientY - drag.y);
    };
    const end = (e: PointerEvent) => {
      if (drag?.pointer !== e.pointerId) return;
      drag = null;
      events.end();
    };
    const key = (e: KeyboardEvent) => {
      const step = e.target === node ? ARROWS[e.key] : undefined;
      if (!step || !events.enabled()) return;
      // The editors' arrow-key shortcuts are not for the title bar.
      e.preventDefault();
      e.stopPropagation();
      events.step(step[0], step[1]);
    };

    node.addEventListener('pointerdown', down);
    node.addEventListener('pointermove', move);
    node.addEventListener('pointerup', end);
    node.addEventListener('pointercancel', end);
    node.addEventListener('lostpointercapture', end);
    node.addEventListener('keydown', key);
    return () => {
      node.removeEventListener('pointerdown', down);
      node.removeEventListener('pointermove', move);
      node.removeEventListener('pointerup', end);
      node.removeEventListener('pointercancel', end);
      node.removeEventListener('lostpointercapture', end);
      node.removeEventListener('keydown', key);
    };
  };
}
