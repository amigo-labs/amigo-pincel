// The integer scale of Pixel mode's PNG frame and sprite-sheet export
// (docs/specs/scaled-export.md). One remembered scale serves both dialogs,
// kept in the IndexedDB pref `exportScale`; ×1 without one.

import { isIdbAvailable } from '../../lib/idb/db';
import { getPref, setPref } from '../../lib/idb/prefs';
import { file, live } from './stores/editor.svelte';

/** The Scale choices both export dialogs offer. */
export const EXPORT_SCALES: readonly number[] = [1, 2, 3, 4, 5, 6, 8, 10];

/** IndexedDB pref key of the last used export scale. */
const EXPORT_SCALE_PREF = 'exportScale';

/** A stored value as one of `EXPORT_SCALES`, or `null`. */
function parseScale(v: unknown): number | null {
  return typeof v === 'number' && EXPORT_SCALES.includes(v) ? v : null;
}

/** Restore the last used export scale (×1 stands without one). */
export async function loadExportScale(): Promise<void> {
  try {
    if (!isIdbAvailable()) return;
    const scale = parseScale(await getPref(EXPORT_SCALE_PREF));
    if (scale !== null) file.exportScale = scale;
  } catch (err) {
    console.error('getPref exportScale failed', err);
  } finally {
    live.exportScalePrefLoaded = true;
  }
}

/** Remember `scale` as the one both dialogs preselect next time. */
export function rememberExportScale(scale: number): void {
  if (parseScale(scale) === null) return;
  file.exportScale = scale;
  if (!live.exportScalePrefLoaded || !isIdbAvailable()) return;
  setPref(EXPORT_SCALE_PREF, scale).catch((err: unknown) => {
    console.error('setPref exportScale failed', err);
  });
}
