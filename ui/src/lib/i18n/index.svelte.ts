// UI language (spec §9.5): a typed dictionary per locale, no i18n
// dependency (Decision Log 2026-10-12 "Own DE/EN dictionary"). `en.ts` is
// the source of the keys; every other locale is typed against it, so a
// missing translation fails `pnpm check`. `t()` reads the reactive locale,
// so templates and `$derived` menus re-render when it changes.

import { en, type MessageKey } from './en';
import { isLocale, LOCALES, pickLocale, type Locale } from './locales';
import { formatDateTime, formatFixed, formatMessage, formatPercent } from './format';

export type { MessageKey } from './en';

export { isLocale, LOCALES, type Locale };

/** IndexedDB pref holding the user's explicit choice. */
export const LANGUAGE_PREF = 'language';

type Dict = Record<MessageKey, string>;

// English ships in the entry bundle (it is the source of the keys and of
// `englishLabel`); every other dictionary is its own chunk, fetched only
// when that language becomes active — together they were most of the start
// screen's bytes (docs/specs/performance-build-simd.md AC-06).
const DICTS: Partial<Record<Locale, Dict>> = { en };

const LOADERS: Record<Exclude<Locale, 'en'>, () => Promise<Dict>> = {
  de: () => import('./de').then((m) => m.de),
  es: () => import('./es').then((m) => m.es),
  fr: () => import('./fr').then((m) => m.fr),
  ja: () => import('./ja').then((m) => m.ja),
  'pt-BR': () => import('./pt-br').then((m) => m.ptBr),
};

/** Fetches the dictionary of `loc` unless it is loaded already. */
async function loadDict(loc: Locale): Promise<Dict> {
  const loaded = DICTS[loc];
  if (loaded) return loaded;
  const dict = await LOADERS[loc as Exclude<Locale, 'en'>]();
  DICTS[loc] = dict;
  return dict;
}

/** The dictionary of `loc`, English while it is still loading. */
function dictOf(loc: Locale): Dict {
  return DICTS[loc] ?? en;
}

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

// Starts in English: `initLocale` (before the first render) switches to the
// saved or the browser's language once its dictionary is loaded.
const state = $state<{ locale: Locale }>({ locale: 'en' });

// The language asked for last; a slower load of an earlier choice must not
// win over it.
let requested: Locale = 'en';

/** The active locale (reactive). */
export function locale(): Locale {
  return state.locale;
}

/**
 * Switch the UI language once its dictionary is loaded; resolves when it is
 * active. If loading fails the language stays as it was and the promise
 * rejects. Persistence and the native menu are the shell's job.
 */
export async function applyLocale(next: Locale): Promise<void> {
  requested = next;
  await loadDict(next);
  if (requested !== next) return;
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
  return formatMessage(dictOf(state.locale)[key], params, state.locale);
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
    const pairs = (Object.entries(dictOf(loc)) as [MessageKey, string][]).reverse();
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
export function percent(value: number, maxFraction = 0): string {
  return formatPercent(value, state.locale, maxFraction);
}

/** `value` with exactly `digits` decimals in the active locale ("1,5"). */
export function decimal(value: number, digits: number): string {
  return formatFixed(value, state.locale, digits);
}

/** `time` (ms since the epoch) as a date and time in the active locale. */
export function dateTime(time: number): string {
  return formatDateTime(time, state.locale);
}
