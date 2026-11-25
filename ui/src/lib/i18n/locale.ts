// Shell side of the UI language (spec §9.5): restore the saved choice,
// persist a new one, and keep the native Tauri menu in the same language.
// The user picks it on the start screen or in Edit ▸ Settings…
// (docs/specs/settings-dialog.md).

import { invoke } from '@tauri-apps/api/core';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { isTauri } from '../platform';
import {
  LANGUAGE_PREF,
  applyLocale,
  browserLocale,
  isLocale,
  locale,
  type Locale,
} from './index.svelte';

/** How long the first render waits for the saved language before using the browser's. */
const READ_LIMIT_MS = 400;

function syncNativeMenu(next: Locale): void {
  if (!isTauri()) return;
  invoke('set_menu_language', { lang: next }).catch((err: unknown) => {
    console.error('set_menu_language failed', err);
  });
}

async function savedLocale(): Promise<Locale | null> {
  if (!isIdbAvailable()) return null;
  const read = getPref(LANGUAGE_PREF).then(
    (saved) => (isLocale(saved) ? saved : null),
    (err: unknown) => {
      console.error('getPref language failed', err);
      return null;
    },
  );
  const limit = new Promise<null>((resolve) => setTimeout(() => resolve(null), READ_LIMIT_MS));
  return Promise.race([read, limit]);
}

/**
 * Activate the saved language, falling back to the browser's, and load its
 * dictionary — awaited before the first render, so the start screen never
 * shows another language first.
 */
export async function initLocale(): Promise<void> {
  const next = (await savedLocale()) ?? browserLocale();
  try {
    await applyLocale(next);
  } catch (err) {
    console.error(`loading the ${next} dictionary failed`, err);
  }
  syncNativeMenu(locale());
}

/** Switch the UI language and remember the choice. */
export function setLocale(next: Locale): void {
  applyLocale(next).then(
    () => syncNativeMenu(locale()),
    (err: unknown) => {
      console.error(`loading the ${next} dictionary failed`, err);
    },
  );
  if (isIdbAvailable()) {
    setPref(LANGUAGE_PREF, next).catch((err: unknown) => {
      console.error('setPref language failed', err);
    });
  }
}

