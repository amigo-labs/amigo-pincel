// On-screen Shift / Alt for touch and pen (spec §9.4): an iPad without a
// keyboard has no way to hold a modifier while drawing, so the editor frame
// shows two latching toggles. Tool code asks `shiftHeld(e)` / `altHeld(e)`
// instead of reading `e.shiftKey` / `e.altKey`, so a real key and a latched
// toggle mean the same thing in both modes.

function coarsePointer(): boolean {
  if (typeof window === 'undefined') return false;
  return window.matchMedia?.('(pointer: coarse)').matches ?? false;
}

/** Latched modifier toggles and whether their bar is shown. */
export const modifiers = $state({
  shift: false,
  alt: false,
  /** Shown on touch-first devices and once a pen or touch pointer is seen. */
  shown: coarsePointer(),
});

/** Shift is held on the keyboard or latched on screen. */
export function shiftHeld(e: { shiftKey: boolean }): boolean {
  return e.shiftKey || modifiers.shift;
}

/** Alt / Option is held on the keyboard or latched on screen. */
export function altHeld(e: { altKey: boolean }): boolean {
  return e.altKey || modifiers.alt;
}

/** Reveal the toggles once a non-mouse pointer touches the editor. */
export function notePointer(e: PointerEvent): void {
  if (e.pointerType === 'touch' || e.pointerType === 'pen') modifiers.shown = true;
}
