// When the editor cannot take a live agent call right now
// (docs/specs/mcp-live-desktop.md): during a user stroke a call waits for
// pointerup (AC-17); while a modal dialog is open it fails at once (AC-18).
// Both editors draw their canvas inside the shared editor frame's stage
// (`[data-stage]`), so one window-level watch covers both modes.

/** The pointer pressing on the stage, `null` between strokes. */
let pressing: number | null = null;
/** Calls waiting for the stroke to end. */
let waiters: (() => void)[] = [];
/** Native (OS) dialogs a call has open; they are not in the DOM. */
let hostDialogs = 0;

function release(): void {
  pressing = null;
  const ready = waiters;
  waiters = [];
  // After the editor's own pointerup handler committed the stroke.
  setTimeout(() => {
    for (const run of ready) run();
  }, 0);
}

/** Watches presses on the editor stage; returns the function that stops. */
export function watchStrokes(): () => void {
  const down = (e: PointerEvent): void => {
    const target = e.target instanceof Element ? e.target : null;
    if (!target?.closest('[data-stage]') || target.closest('[role="dialog"]')) return;
    if (e.button === 0 || e.pointerType !== 'mouse') pressing = e.pointerId;
  };
  const up = (e: PointerEvent): void => {
    if (pressing === e.pointerId) release();
  };
  const blur = (): void => {
    if (pressing !== null) release();
  };
  window.addEventListener('pointerdown', down, true);
  window.addEventListener('pointerup', up, true);
  window.addEventListener('pointercancel', up, true);
  window.addEventListener('blur', blur);
  return () => {
    window.removeEventListener('pointerdown', down, true);
    window.removeEventListener('pointerup', up, true);
    window.removeEventListener('pointercancel', up, true);
    window.removeEventListener('blur', blur);
    if (pressing !== null) release();
  };
}

/** Resolves at once between strokes, else after the stroke's pointerup. */
export function strokeDone(): Promise<void> {
  if (pressing === null) return Promise.resolve();
  return new Promise((resolve) => waiters.push(resolve));
}

/** A modal dialog is open: one of the app's dialogs (not the panel sheet
 *  inside the stage on narrow screens) or a native file dialog a call
 *  opened. */
export function dialogOpen(): boolean {
  if (hostDialogs > 0) return true;
  if (document.querySelector('[aria-modal="true"], [role="alertdialog"]')) return true;
  return Array.from(document.querySelectorAll('[role="dialog"]')).some(
    (el) => !el.closest('[data-stage]'),
  );
}

/** Runs `show` (a native dialog) counted as an open dialog. */
export async function withHostDialog<T>(show: () => Promise<T>): Promise<T> {
  hostDialogs += 1;
  try {
    return await show();
  } finally {
    hostDialogs -= 1;
  }
}
