// Image-mode view aids (docs/specs/image-view-aids.md): rulers, pixel grid
// and units, per user for every Image tab, plus the pointer position the
// rulers and the status bar follow. View state only: no command, no undo
// step, nothing in any file (AC-14, AC-15).

import { getPref, setPref } from '../../../lib/idb/prefs';
import { view } from './editor.svelte';
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
  /** The pointer's position inside the canvas area in CSS px, or `null`
   *  outside it (AC-04, AC-24). A screen position, not an image one: the
   *  image pixel under it follows every zoom, pan and tab switch without a
   *  pointer move (`pointerPixel`). */
  pointer: { x: number; y: number } | null;
}>({ ...DEFAULT_IMAGE_VIEW, pointer: null });

/** The image pixel under the pointer through the active tab's current
 *  view, rounded down, or `null` outside the canvas area (AC-10). */
export function pointerPixel(): { x: number; y: number } | null {
  const p = viewAids.pointer;
  if (p === null) return null;
  return {
    x: Math.floor((p.x - view.panX) / view.zoom),
    y: Math.floor((p.y - view.panY) / view.zoom),
  };
}

let loaded = false;
let loading = false;
// What the user changed while the stored pref was still on its way: the
// change wins over the stored value.
let changedEarly: Partial<Record<keyof ImageViewPref, true>> = {};

/** Applies the stored pref once per session; a missing or broken one keeps
 *  the defaults (AC-13, AC-20). A toggle made before it arrived keeps its
 *  value, and the merged state is stored again. */
export async function loadViewAids(): Promise<void> {
  if (loaded) return;
  loaded = true;
  loading = true;
  let raw: unknown = null;
  try {
    raw = await getPref(IMAGE_VIEW_PREF);
  } catch {
    // IndexedDB unavailable: the defaults apply (AC-25).
  }
  loading = false;
  const pref = parseImageViewPref(raw);
  if (!changedEarly.showRulers) viewAids.showRulers = pref.showRulers;
  if (!changedEarly.showPixelGrid) viewAids.showPixelGrid = pref.showPixelGrid;
  if (!changedEarly.units) viewAids.units = pref.units;
  // The early change stored the other fields' defaults: store the merge.
  if (Object.keys(changedEarly).length > 0) store();
  changedEarly = {};
}

/** Changes the view state and stores it; a failed write keeps the change
 *  for the session without a message (AC-25). */
export function setViewAids(change: Partial<ImageViewPref>): void {
  Object.assign(viewAids, change);
  if (loading) {
    for (const key of Object.keys(change) as (keyof ImageViewPref)[]) changedEarly[key] = true;
  }
  store();
}

function store(): void {
  const pref: ImageViewPref = {
    showRulers: viewAids.showRulers,
    showPixelGrid: viewAids.showPixelGrid,
    units: viewAids.units,
  };
  setPref(IMAGE_VIEW_PREF, pref).catch(() => {});
}
