// UI language (spec §9.5): a typed dictionary per locale, no i18n
// dependency (Decision Log 2026-10-12 "Own DE/EN dictionary"). `en.ts` is
// the source of the keys; every other locale is typed against it, so a
// missing translation fails `pnpm check`. `t()` reads the reactive locale,
// so templates and `$derived` menus re-render when it changes.

import { de } from './de';
import { en, type MessageKey } from './en';
import { es } from './es';
import { fr } from './fr';
import { ja } from './ja';
import { isLocale, LOCALES, pickLocale, type Locale } from './locales';
import { ptBr } from './pt-br';
import { formatDateTime, formatFixed, formatMessage, formatPercent } from './format';

export type { MessageKey } from './en';

export { isLocale, LOCALES, type Locale };

/** IndexedDB pref holding the user's explicit choice. */
export const LANGUAGE_PREF = 'language';

const DICTS: Record<Locale, Record<MessageKey, string>> = {
  en,
  de,
  es,
  ja,
  fr,
  'pt-BR': ptBr,
};

/** The locale the browser asks for (docs/specs/more-languages.md AC-03). */
export function browserLocale(): Locale {
  const langs =
    typeof navigator === 'undefined' ? [] : (navigator.languages ?? [navigator.language]);
  return pickLocale(langs);
}

/** `true` when `key` names a message (for keys built at runtime). */
export function isMessageKey(key: string): key is MessageKey {
  return key in en;
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
 * filled from `params` (unknown ones stay as written) and
 * `{n, plural, one {…} other {…}}` blocks follow the locale's plural rules
 * (`format.ts`).
 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  return formatMessage(DICTS[state.locale][key], params, state.locale);
}

// Reverse lookups per locale: a message (parameterless) → its key.
const REVERSE: Partial<Record<Locale, Readonly<Record<string, MessageKey>>>> = {};

/**
 * The English text of `label`, a message of the active locale without
 * parameters (`Nach unten vereinen` → `Merge Down`, `Belichtung…` →
 * `Exposure…`); `null` when `label` is no such message (a file name, a filled-in template). The command
 * palette matches English queries in every locale with it.
 */
export function englishLabel(label: string): string | null {
  const loc = state.locale;
  let map = REVERSE[loc];
  if (!map) {
    // Reversed, so the first key with a given text wins.
    const pairs = (Object.entries(DICTS[loc]) as [MessageKey, string][]).reverse();
    map = Object.fromEntries(pairs.map(([key, text]) => [text, key])) as Record<string, MessageKey>;
    REVERSE[loc] = map;
  }
  const key = Object.hasOwn(map, label) ? map[label] : undefined;
  if (key !== undefined) return en[key];
  // A menu entry that opens a panel or dialog appends "…" to its message
  // (`Belichtung…`): look up the message and keep the suffix.
  if (label.endsWith('…') && label.length > 1) {
    const base = englishLabel(label.slice(0, -1));
    return base === null ? null : `${base}…`;
  }
  return null;
}

/** `value` (0–100) as a percentage in the active locale ("100 %" in
 *  French; docs/specs/more-languages.md AC-09). Reactive like `t()`. */
export function percent(value: number): string {
  return formatPercent(value, state.locale);
}

/** `value` with exactly `digits` decimals in the active locale ("1,5"). */
export function decimal(value: number, digits: number): string {
  return formatFixed(value, state.locale, digits);
}

/** `time` (ms since the epoch) as a date and time in the active locale. */
export function dateTime(time: number): string {
  return formatDateTime(time, state.locale);
}
