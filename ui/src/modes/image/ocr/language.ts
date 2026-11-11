// The language the OCR dialog preselects (docs/specs/ocr-dialog.md AC-02):
// the language of the last recognition the user started, kept in the
// `ocr.language` pref; on first use German for a German interface and
// English otherwise. Rune-free.

import { getPref, setPref } from '../../../lib/idb/prefs';
import { isOcrLang, type OcrLang } from './catalog';

/** The IndexedDB pref holding the last recognition's language. */
export const OCR_LANGUAGE_PREF = 'ocr.language';

/** The first-use language for interface language `ui`. */
export function defaultOcrLanguage(ui: string): OcrLang {
  return ui === 'de' ? 'deu' : 'eng';
}

/** The language to preselect; unreadable storage gives the default. */
export async function preselectedOcrLanguage(ui: string): Promise<OcrLang> {
  try {
    const stored = await getPref(OCR_LANGUAGE_PREF);
    if (isOcrLang(stored)) return stored;
  } catch {
    // Fall back to the interface language.
  }
  return defaultOcrLanguage(ui);
}

/** Remembers `lang` as the last recognition's language. */
export function rememberOcrLanguage(lang: OcrLang): void {
  setPref(OCR_LANGUAGE_PREF, lang).catch(() => undefined);
}
