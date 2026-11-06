// Image-mode view aids (docs/specs/image-view-aids.md): rulers, pixel grid
// and units, per user for every Image tab, plus the pointer position the
// rulers and the status bar follow. View state only: no command, no undo
// step, nothing in any file (AC-14, AC-15).

import { getPref, setPref } from '../../../lib/idb/prefs';
import {
  DEFAULT_IMAGE_VIEW,
  IMAGE_VIEW_PREF,
  parseImageViewPref,
  type ImageViewPref,
  type ViewUnit,
} from '../view-units';

/** The on / off states, the unit and the pointer over the canvas area. */
export const viewAids = $state<{
  showRulers: boolean;
  showPixelGrid: boolean;
  units: ViewUnit;
  /** The image coordinate under the pointer, or `null` outside the canvas
   *  area (AC-10, AC-24). */
  pointer: { x: number; y: number } | null;
}>({ ...DEFAULT_IMAGE_VIEW, pointer: null });

let loaded = false;

/** Applies the stored pref once per session; a missing or broken one keeps
 *  the defaults (AC-13, AC-20). */
export async function loadViewAids(): Promise<void> {
  if (loaded) return;
  loaded = true;
  let raw: unknown = null;
  try {
    raw = await getPref(IMAGE_VIEW_PREF);
  } catch {
    // IndexedDB unavailable: the defaults apply (AC-25).
  }
  const pref = parseImageViewPref(raw);
  viewAids.showRulers = pref.showRulers;
  viewAids.showPixelGrid = pref.showPixelGrid;
  viewAids.units = pref.units;
}

/** Changes the view state and stores it; a failed write keeps the change
 *  for the session without a message (AC-25). */
export function setViewAids(change: Partial<ImageViewPref>): void {
  Object.assign(viewAids, change);
  const pref: ImageViewPref = {
    showRulers: viewAids.showRulers,
    showPixelGrid: viewAids.showPixelGrid,
    units: viewAids.units,
  };
  setPref(IMAGE_VIEW_PREF, pref).catch(() => {});
}
