// View ▸ Touch Gestures (docs/specs/tablet-gestures.md AC-11 – AC-13): the
// two- and three-finger taps for Undo / Redo, on by default, stored as one
// IndexedDB pref like `droppedImages`; best effort — without IndexedDB the
// setting holds for the session. Pinch / pan and the pen's eraser end do not
// depend on it.

import type { MenuAction } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';

const TOUCH_GESTURES_PREF = 'touchGestures';

/** Whether multi-finger taps undo and redo. */
export const touchGestures = $state({ enabled: true });

/** Reads the stored setting; keeps the default when unset or unreadable. */
export async function loadTouchGestures(): Promise<void> {
  if (!isIdbAvailable()) return;
  try {
    const v = await getPref(TOUCH_GESTURES_PREF);
    if (typeof v === 'boolean') touchGestures.enabled = v;
  } catch (err) {
    console.error('getPref touchGestures failed', err);
  }
}

/** Turns the taps on or off, also after a restart. */
export function setTouchGestures(enabled: boolean): void {
  touchGestures.enabled = enabled;
  if (!isIdbAvailable()) return;
  setPref(TOUCH_GESTURES_PREF, enabled).catch((err: unknown) => {
    console.error('setPref touchGestures failed', err);
  });
}

/** View ▸ Touch Gestures, checked while the taps are on (both modes). */
export function touchGesturesItem(): MenuAction {
  return {
    label: t('menu.touchGestures'),
    checked: touchGestures.enabled,
    run: () => setTouchGestures(!touchGestures.enabled),
  };
}
