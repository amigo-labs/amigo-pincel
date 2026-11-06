// Pencil / Eraser samples of one animation frame, handed to the core in one
// `strokeToBatch` call by the render tick (docs/specs/performance-input-path.md).

import { t } from '../../lib/i18n/index.svelte';
import { ed, live } from './stores/editor.svelte';

/** Sprite-space points queued since the last flush, as `x0, y0, x1, y1, …`. */
const pendingXY: number[] = [];
/** Pen pressure per queued point; `undefined` means full pressure. */
const pendingPressure: Array<number | undefined> = [];

/** Queue one point of the stroke in progress for the next flush. */
export function queueStrokePoint(x: number, y: number, pressure: number | undefined): void {
  pendingXY.push(x, y);
  pendingPressure.push(pressure);
}

/** Drop the queued points without painting them (the gesture is abandoned). */
export function discardStrokePoints(): void {
  pendingXY.length = 0;
  pendingPressure.length = 0;
}

/** Paint every queued point in one core call. Runs once per animation frame
 *  and before a stroke ends, so no sample is lost. */
export function flushStrokePoints(): void {
  if (pendingXY.length === 0) return;
  const xy = Int32Array.from(pendingXY);
  const pressure = pendingPressure.some((p) => p !== undefined)
    ? Float32Array.from(pendingPressure, (p) => p ?? 1)
    : new Float32Array(0);
  discardStrokePoints();
  if (!ed.doc || !live.painting) return;
  try {
    ed.doc.strokeToBatch(xy, pressure);
  } catch (err) {
    // Missing layer, locked target, …: a real failure. Stop the gesture so
    // the rest of the drag doesn't repeat it, and surface it.
    live.painting = false;
    ed.doc.endStroke();
    const msg = err instanceof Error ? err.message : String(err);
    console.error('paint stroke failed', err);
    ed.status = t('status.failed', { action: t('act.paint'), error: msg });
  }
}
