// Image mode's Colors-panel palette state (docs/specs/image-palette-gradient.md).
// One palette for Image mode as a whole: every Image tab shows the same one
// (AC-16), it is not document state, so no undo step and no unsaved change
// (AC-15). `custom` is `null` while the user has not loaded a palette or
// added a color since the last reset: then nothing is stored and the
// built-in default of the running version shows (AC-22).

import { getPref, removePref, setPref } from '../../../lib/idb/prefs';
import {
  DEFAULT_ENTRIES,
  PALETTE_PREF,
  readStoredPalette,
  toStored,
  type PaletteEntry,
} from '../palette';

export const imagePalette = $state<{ custom: PaletteEntry[] | null }>({ custom: null });

/** The entries the Colors panel shows. */
export function paletteEntries(): readonly PaletteEntry[] {
  return imagePalette.custom ?? DEFAULT_ENTRIES;
}

let loading: Promise<void> | null = null;
// Set by the first change, so a slow store read cannot overwrite it.
let changed = false;

/** Restores the stored palette once per page load; an unreadable value or
 *  an unavailable store leaves the default palette, silently (AC-13, AC-14). */
export function loadImagePalette(): Promise<void> {
  loading ??= getPref(PALETTE_PREF)
    .then((stored) => {
      if (!changed) imagePalette.custom = readStoredPalette(stored);
    })
    .catch(() => {});
  return loading;
}

/** Shows `entries` (`null`: the built-in default) and stores them;
 *  resolves `false` when storing failed — the change stays in effect until
 *  the app reloads (AC-25). */
export async function setImagePalette(entries: PaletteEntry[] | null): Promise<boolean> {
  changed = true;
  imagePalette.custom = entries;
  try {
    if (entries === null) await removePref(PALETTE_PREF);
    else await setPref(PALETTE_PREF, toStored(entries));
    return true;
  } catch {
    return false;
  }
}
