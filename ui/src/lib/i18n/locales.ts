// The supported UI languages and the browser-language match
// (spec §9.5, docs/specs/more-languages.md AC-01, AC-03). Rune-free, so a
// Node-side spec can test it.

/** Supported UI languages, as BCP 47 tags. */
export type Locale = 'en' | 'de' | 'es' | 'ja' | 'fr' | 'pt-BR';

/** Language picker entries, each labelled in its own language. */
export const LOCALES: readonly { id: Locale; label: string }[] = [
  { id: 'en', label: 'English' },
  { id: 'de', label: 'Deutsch' },
  { id: 'es', label: 'Español' },
  { id: 'ja', label: '日本語' },
  { id: 'fr', label: 'Français' },
  { id: 'pt-BR', label: 'Português (Brasil)' },
];

/** Narrow a stored pref value to a locale. */
export function isLocale(v: unknown): v is Locale {
  return LOCALES.some((l) => l.id === v);
}

/**
 * The first entry of `langs` (the browser's list, most preferred first)
 * that a supported locale matches — by full tag first, then by base
 * language, with every Portuguese tag (`pt`, `pt-PT`, …) taken as
 * `pt-BR` — or English when none does (AC-03).
 */
export function pickLocale(langs: readonly string[]): Locale {
  for (const tag of langs) {
    const lower = tag.toLowerCase();
    const exact = LOCALES.find((l) => l.id.toLowerCase() === lower);
    if (exact) return exact.id;
    const base = lower.split('-')[0];
    if (base === 'pt') return 'pt-BR';
    const byBase = LOCALES.find((l) => l.id.toLowerCase() === base);
    if (byBase) return byBase.id;
  }
  return 'en';
}

/** Whether `locale` writes decimals with a comma ("1,5"): German, Spanish,
 *  French and Portuguese do, English and Japanese write a point. */
export function usesDecimalComma(locale: Locale): boolean {
  return locale === 'de' || locale === 'es' || locale === 'fr' || locale === 'pt-BR';
}
