// Shell side of the UI language (spec §9.5): restore the saved choice,
// persist a new one, keep the native Tauri menu in the same language, and
// the View ▸ Language submenu both editors list.

import { invoke } from '@tauri-apps/api/core';
import type { MenuSubmenu } from '../components/layout/menu';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { isTauri } from '../platform';
import { LANGUAGE_PREF, LOCALES, applyLocale, isLocale, locale, t, type Locale } from './index.svelte';

function syncNativeMenu(next: Locale): void {
  if (!isTauri()) return;
  invoke('set_menu_language', { lang: next }).catch((err: unknown) => {
    console.error('set_menu_language failed', err);
  });
}

/** Apply the saved language, falling back to the browser's (already active). */
export async function initLocale(): Promise<void> {
  if (isIdbAvailable()) {
    try {
      const saved = await getPref(LANGUAGE_PREF);
      if (isLocale(saved)) applyLocale(saved);
    } catch (err) {
      console.error('getPref language failed', err);
    }
  }
  applyLocale(locale());
  syncNativeMenu(locale());
}

/** Switch the UI language and remember the choice. */
export function setLocale(next: Locale): void {
  applyLocale(next);
  syncNativeMenu(next);
  if (isIdbAvailable()) {
    setPref(LANGUAGE_PREF, next).catch((err: unknown) => {
      console.error('setPref language failed', err);
    });
  }
}

/** View ▸ Language, shared by both editors. */
export function languageMenu(): MenuSubmenu {
  return {
    kind: 'submenu',
    label: t('menu.language'),
    items: LOCALES.map((l) => ({
      label: l.label,
      checked: locale() === l.id,
      run: () => setLocale(l.id),
    })),
  };
}
