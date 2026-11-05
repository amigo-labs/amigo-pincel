// Routes canvas presses to an open effect dialog instead of the active tool
// (effect-catalog-v2 AC-11, "Pick on canvas"). The dialog arms picking by
// setting `handler`; the editor's canvas calls it with the pointer position
// as a fraction of the image area (unclamped: a press on the pasteboard lies
// outside 0–1) and swallows the press, so no tool sees it. Panning gestures
// stay with the canvas.

/** Which part of the press a pick event reports. */
export type CanvasPickPhase = 'down' | 'move' | 'up';

/** Receives a picked position, `fx` / `fy` in fractions of the image area. */
export type CanvasPickHandler = (fx: number, fy: number, phase: CanvasPickPhase) => void;

/** The armed pick handler, `null` while no dialog is picking. */
export const canvasPick = $state<{ handler: CanvasPickHandler | null }>({ handler: null });
