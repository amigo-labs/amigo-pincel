// "Open like this on startup": the New cards on the start screen and the
// New dialog can remember one mode and size; the next launch opens an
// empty document of it instead of the start screen. See
// docs/specs/pincel.md §9.2 and the Decision Log, 2026-10-28.
//
// Stored as one IndexedDB pref; writes are best effort like the other
// prefs (language, panel layout).

import type { NewDocParams } from './types';
import { isIdbAvailable } from '../idb/db';
import { getPref, removePref, setPref } from '../idb/prefs';
import { newDocParams } from './new-doc';

const AUTO_NEW_PREF = 'autoNew';

/** The remembered startup document; `null` when the start screen shows. */
export const autoNew = $state<{ params: NewDocParams | null }>({ params: null });

/** Reads the pref, clamped to today's limits; `null` when unset or unreadable. */
export async function loadAutoNew(): Promise<NewDocParams | null> {
  if (!isIdbAvailable()) return null;
  try {
    const v = await getPref(AUTO_NEW_PREF);
    if (typeof v !== 'object' || v === null) return null;
    const { mode, width, height } = v as Record<string, unknown>;
    if (
      (mode !== 'pixel' && mode !== 'image') ||
      typeof width !== 'number' ||
      typeof height !== 'number' ||
      !Number.isFinite(width) ||
      !Number.isFinite(height)
    ) {
      return null;
    }
    autoNew.params = newDocParams(mode, width, height);
  } catch (err) {
    console.error('getPref autoNew failed', err);
    autoNew.params = null;
  }
  return autoNew.params;
}

/** Remembers `params` for the next launch, or forgets it with `null`. */
export function setAutoNew(params: NewDocParams | null): void {
  autoNew.params = params;
  if (!isIdbAvailable()) return;
  const write = params ? setPref(AUTO_NEW_PREF, { ...params }) : removePref(AUTO_NEW_PREF);
  write.catch((err: unknown) => {
    console.error('setPref autoNew failed', err);
  });
}
