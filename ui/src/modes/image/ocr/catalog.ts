// The OCR language models (docs/specs/ocr-dialog.md): one `tessdata_fast`
// model per language, served from Pincel's website at a path that names
// the `tessdata_fast` version, and checked against its pinned SHA-256
// (Decision Log 2026-11-08). Adding a language is an entry here. The files
// are the ones committed in `crates/pincel-ocr/models/`; scripts/web-site.sh
// adds them to the released site. Rune-free, so a Node-side Playwright spec can import it.

/** A recognition language, named like its model file. */
export type OcrLang = 'deu' | 'eng';

/** The languages in the order the dialog lists them. */
export const OCR_LANGS: readonly OcrLang[] = ['deu', 'eng'];

/** One language's model. */
export interface OcrModelEntry {
  /** The `tessdata_fast` release the file comes from. */
  readonly version: string;
  /** SHA-256 of the file, lower-case hex. */
  readonly sha256: string;
}

/** The pinned models. A newer pinned version replaces a stored older one. */
export const OCR_MODELS: Readonly<Record<OcrLang, OcrModelEntry>> = {
  deu: {
    version: '4.1.0',
    sha256: '19d219bbb6672c869d20a9636c6816a81eb9a71796cb93ebe0cb1530e2cdb22d',
  },
  eng: {
    version: '4.1.0',
    sha256: '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2',
  },
};

/** Pincel's website, where the desktop app fetches the models. */
export const MODEL_ORIGIN = 'https://pincel.amigo-labs.dev';

/** The model's path on the website. */
export function modelPath(lang: OcrLang): string {
  return `/ocr/tessdata_fast-${OCR_MODELS[lang].version}/${lang}.traineddata`;
}

/**
 * The model URL: on the PWA's own origin, and absolute in the desktop app,
 * whose bundle is served from its own (non-web) origin.
 */
export function modelUrl(lang: OcrLang, desktop: boolean): string {
  return desktop ? `${MODEL_ORIGIN}${modelPath(lang)}` : modelPath(lang);
}

/** Narrows a stored pref value to a language. */
export function isOcrLang(v: unknown): v is OcrLang {
  return v === 'deu' || v === 'eng';
}
