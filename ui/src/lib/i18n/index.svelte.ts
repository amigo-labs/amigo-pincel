// UI language (spec §9.5): a typed dictionary per locale, no i18n
// dependency (Decision Log 2026-10-12 "Own DE/EN dictionary"). `en.ts` is
// the source of the keys; every other locale is typed against it, so a
// missing translation fails `pnpm check`. `t()` reads the reactive locale,
// so templates and `$derived` menus re-render when it changes.

import { de } from './de';
import { en, type MessageKey } from './en';

export type { MessageKey } from './en';

/** Supported UI languages. */
export type Locale = 'en' | 'de';

/** Language picker entries, each labelled in its own language. */
export const LOCALES: readonly { id: Locale; label: string }[] = [
  { id: 'en', label: 'English' },
  { id: 'de', label: 'Deutsch' },
];

/** IndexedDB pref holding the user's explicit choice. */
export const LANGUAGE_PREF = 'language';

const DICTS: Record<Locale, Record<MessageKey, string>> = { en, de };

/** The locale a browser asks for: German for any `de-*`, else English. */
export function browserLocale(): Locale {
  const langs = typeof navigator === 'undefined' ? [] : (navigator.languages ?? [navigator.language]);
  for (const l of langs) {
    const base = l.toLowerCase().split('-')[0];
    if (base === 'de' || base === 'en') return base;
  }
  return 'en';
}

/** `true` when `key` names a message (for keys built at runtime). */
export function isMessageKey(key: string): key is MessageKey {
  return key in en;
}

/** Narrow a stored pref value to a locale. */
export function isLocale(v: unknown): v is Locale {
  return v === 'en' || v === 'de';
}

const state = $state<{ locale: Locale }>({ locale: browserLocale() });

/** The active locale (reactive). */
export function locale(): Locale {
  return state.locale;
}

/** Switch the UI language. Persistence and the native menu are the shell's job. */
export function applyLocale(next: Locale): void {
  state.locale = next;
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

/**
 * The message for `key` in the active locale. `{name}` placeholders are
 * filled from `params`; unknown placeholders stay as written.
 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const s = DICTS[state.locale][key];
  if (!params) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}
