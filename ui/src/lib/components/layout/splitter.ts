// Behaviour of a focusable window splitter (WAI-ARIA "Window Splitter"
// pattern; effect-panel AC-07 – AC-09): pointer drag with capture, a
// double-click reset, and Arrow Left / Right steps. The element carries
// `role="separator"` and its `aria-value*` in markup; this attachment makes
// it focusable and wires the input. It lives in an attachment because a
// focusable separator is a widget in WAI-ARIA, while Svelte's a11y check
// counts `separator` as non-interactive and flags `tabindex` / listeners
// on it in markup.

import type { Attachment } from 'svelte/attachments';

/** What the splitter reports; `dx` is the horizontal pointer travel since
 *  the press, in CSS px (positive = to the right). */
export interface SplitterEvents {
  /** A drag starts. */
  begin(): void;
  /** The pointer moved during a drag. */
  move(dx: number): void;
  /** The drag ended (release, cancel or lost capture). */
  end(): void;
  /** Arrow Left (`-1`) or Arrow Right (`+1`). */
  step(direction: -1 | 1): void;
  /** Double-click. */
  reset(): void;
}

/** Attachment wiring a splitter element to `events`. */
export function splitter(events: SplitterEvents): Attachment<HTMLElement> {
  return (node) => {
    node.tabIndex = 0;
    let drag: { pointer: number; x: number } | null = null;

    const down = (e: PointerEvent) => {
      if (e.button !== 0) return;
      e.preventDefault();
      node.setPointerCapture(e.pointerId);
      drag = { pointer: e.pointerId, x: e.clientX };
      events.begin();
    };
    const move = (e: PointerEvent) => {
      if (drag?.pointer === e.pointerId) events.move(e.clientX - drag.x);
    };
    const end = (e: PointerEvent) => {
      if (drag?.pointer !== e.pointerId) return;
      drag = null;
      events.end();
    };
    const key = (e: KeyboardEvent) => {
      const direction = e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0;
      if (direction === 0) return;
      // The editors' arrow-key shortcuts are not for the splitter.
      e.preventDefault();
      e.stopPropagation();
      events.step(direction);
    };
    const dblclick = () => events.reset();

    node.addEventListener('pointerdown', down);
    node.addEventListener('pointermove', move);
    node.addEventListener('pointerup', end);
    node.addEventListener('pointercancel', end);
    node.addEventListener('lostpointercapture', end);
    node.addEventListener('keydown', key);
    node.addEventListener('dblclick', dblclick);
    return () => {
      node.removeEventListener('pointerdown', down);
      node.removeEventListener('pointermove', move);
      node.removeEventListener('pointerup', end);
      node.removeEventListener('pointercancel', end);
      node.removeEventListener('lostpointercapture', end);
      node.removeEventListener('keydown', key);
      node.removeEventListener('dblclick', dblclick);
    };
  };
}
