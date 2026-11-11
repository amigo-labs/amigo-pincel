import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Locator, type Page, type Route } from '@playwright/test';
import { dragFiles, gotoStart, menu, waitForEditor } from './helpers';

// Helpers for the Recognize Text dialog (docs/specs/ocr-dialog.md). The
// language models are the files committed with `pincel-ocr`; the specs
// serve them through `page.route`, so no test needs the network.

/** The committed `pincel-ocr` crate. */
const OCR_CRATE = join(import.meta.dirname, '..', '..', 'crates', 'pincel-ocr');

/** Requests for any model URL. */
export const MODEL_ROUTE = '**/ocr/tessdata_fast-4.1.0/*.traineddata';

/** The two languages and their model names. */
export type OcrLang = 'deu' | 'eng';

/** The committed model file of `lang`. */
export function modelBytes(lang: OcrLang): Buffer {
  return readFileSync(join(OCR_CRATE, 'models', `${lang}.traineddata`));
}

/** The committed page-text snapshot of a corpus page (`ocr-page` AC-14). */
export function pageSnapshot(lang: OcrLang, page: string): string {
  return readFileSync(join(OCR_CRATE, 'tests', 'pages', lang, `${page}.snapshot.txt`), 'utf8');
}

/** The PNG of a corpus page. */
export function pagePng(lang: OcrLang, page: string): Buffer {
  return readFileSync(join(OCR_CRATE, 'tests', 'pages', lang, `${page}.png`));
}

/** What a model route answers; `serve` is the committed file. */
export type ModelAnswer = 'serve' | 'abort' | { status: number } | { body: Buffer };

/** The model requests the page made, by language. */
export interface ModelRequests {
  readonly urls: string[];
  count(lang: OcrLang): number;
}

/**
 * Serves the models at their URL; `answer` decides per request (default:
 * the committed file). Returns the log of requests.
 */
export async function serveModels(
  page: Page,
  answer: (lang: OcrLang, n: number) => ModelAnswer | Promise<ModelAnswer> = () => 'serve',
): Promise<ModelRequests> {
  const urls: string[] = [];
  const langOf = (url: string): OcrLang => (url.endsWith('/deu.traineddata') ? 'deu' : 'eng');
  await page.route(MODEL_ROUTE, async (route: Route) => {
    const url = route.request().url();
    const lang = langOf(url);
    const n = urls.filter((u) => langOf(u) === lang).length;
    urls.push(url);
    const a = await answer(lang, n);
    if (a === 'abort') return route.abort('connectionreset');
    if (a === 'serve') {
      return route.fulfill({
        status: 200,
        contentType: 'application/octet-stream',
        body: modelBytes(lang),
      });
    }
    if ('status' in a) return route.fulfill({ status: a.status, body: 'error' });
    return route.fulfill({ status: 200, contentType: 'application/octet-stream', body: a.body });
  });
  return { urls, count: (lang) => urls.filter((u) => langOf(u) === lang).length };
}

/** A word drawn on the generated test page. */
export interface PlacedWord {
  readonly text: string;
  readonly x: number;
  readonly y: number;
}

/** Draws `words` in black on white (bold sans-serif, 44 px) into a PNG. */
export async function wordsPng(
  page: Page,
  width: number,
  height: number,
  words: readonly PlacedWord[],
  background = '#ffffff',
): Promise<number[]> {
  return page.evaluate(
    async ([w, h, list, bg]) => {
      const canvas = new OffscreenCanvas(w, h);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      if (bg !== 'transparent') {
        ctx.fillStyle = bg;
        ctx.fillRect(0, 0, w, h);
      }
      ctx.fillStyle = '#000000';
      ctx.font = 'bold 44px sans-serif';
      ctx.textBaseline = 'top';
      for (const word of list) ctx.fillText(word.text, word.x, word.y);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      return Array.from(new Uint8Array(await blob.arrayBuffer()));
    },
    [width, height, words, background] as const,
  );
}

/** Opens `bytes` as the PNG `name` by dropping it on the start screen. */
export async function openPng(page: Page, name: string, bytes: Buffer | number[]): Promise<void> {
  await gotoStart(page);
  await dragFiles(page.getByRole('main'), 'drop', [{ name, bytes }]);
  await waitForEditor(page, 'image');
}

/** The OCR dialog (English UI). */
export function ocrDialog(page: Page, title = 'Recognize Text'): Locator {
  return page.getByRole('dialog', { name: title });
}

/** The dialog's status line. */
export function ocrStatus(page: Page): Locator {
  return page.getByTestId('ocr-status');
}

/** The editable result field. */
export function ocrField(page: Page): Locator {
  return page.getByRole('textbox', { name: 'Recognized text' });
}

/** Opens Tools ▸ Recognize Text…. */
export async function openOcr(page: Page): Promise<void> {
  await menu(page, 'Tools', 'Recognize Text…');
  await expect(ocrDialog(page)).toBeVisible();
}

/**
 * Starts recording every text the status line shows (into
 * `window.__ocrSteps`); read them with `recordedSteps`.
 */
export async function recordSteps(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __ocrSteps: string[] };
    w.__ocrSteps = [];
    let last = '';
    const note = () => {
      const el = document.querySelector('[data-testid="ocr-status"]');
      const text = el?.textContent?.trim() ?? '';
      if (text !== '' && text !== last) w.__ocrSteps.push(text);
      last = text;
    };
    new MutationObserver(note).observe(document.body, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  });
}

/** The status texts recorded since `recordSteps`. */
export async function recordedSteps(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __ocrSteps: string[] }).__ocrSteps);
}

/** Waits for the recognized text and returns it (up to `timeout` ms). */
export async function recognizedText(page: Page, timeout = 60_000): Promise<string> {
  await expect(ocrField(page)).toBeVisible({ timeout });
  return ocrField(page).inputValue();
}
