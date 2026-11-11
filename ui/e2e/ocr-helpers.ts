import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Locator, type Page, type Route } from '@playwright/test';
import { dragFiles, gotoStart, imageCanvas, menu, waitForEditor } from './helpers';

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

/** Screen point of the document position `(x, y)` on a `w × h` canvas. */
export async function docPoint(
  page: Page,
  size: { width: number; height: number },
  x: number,
  y: number,
): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / size.width, box.y + (y * box.height) / size.height];
}

/** Picks the tool `name` of the group under `key`. */
export async function useTool(page: Page, key: string, name: string): Promise<void> {
  const button = page.getByRole('button', { name: new RegExp(`(^|: )${name}$`) });
  await page.keyboard.press(key);
  if ((await button.count()) === 0) await page.getByRole('radio', { name, exact: true }).click();
  await expect(button).toHaveCount(1);
}

/** Rectangle Select of the document rect from `(x0, y0)` to `(x1, y1)`. */
export async function rectSelect(
  page: Page,
  size: { width: number; height: number },
  [x0, y0]: [number, number],
  [x1, y1]: [number, number],
): Promise<void> {
  await useTool(page, 'm', 'Rectangle Select');
  await page.mouse.move(...(await docPoint(page, size, x0, y0)));
  await page.mouse.down();
  await page.mouse.move(...(await docPoint(page, size, x1, y1)), { steps: 4 });
  await page.mouse.up();
}

/** A freehand Lasso along the document points `path` (closed on release). */
export async function lassoSelect(
  page: Page,
  size: { width: number; height: number },
  path: readonly [number, number][],
): Promise<void> {
  await useTool(page, 'q', 'Lasso');
  const [first, ...rest] = path;
  if (!first) return;
  await page.mouse.move(...(await docPoint(page, size, ...first)));
  await page.mouse.down();
  for (const p of rest) await page.mouse.move(...(await docPoint(page, size, ...p)), { steps: 8 });
  await page.mouse.up();
}

/** A stored model row, as the test reads it from IndexedDB. */
export interface StoredModel {
  readonly lang: string;
  readonly version: string;
  readonly size: number;
}

/** The rows of the `ocr_models` store, sorted by language and version. */
export async function storedModels(page: Page): Promise<StoredModel[]> {
  return page.evaluate(
    () =>
      new Promise<StoredModel[]>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error ?? new Error('open failed'));
        open.onsuccess = () => {
          const db = open.result;
          if (!db.objectStoreNames.contains('ocr_models')) {
            db.close();
            resolve([]);
            return;
          }
          const all = db.transaction('ocr_models').objectStore('ocr_models').getAll();
          all.onerror = () => reject(all.error ?? new Error('read failed'));
          all.onsuccess = () => {
            const rows = (all.result as { lang: string; version: string; bytes: Uint8Array }[])
              .map((r) => ({ lang: r.lang, version: r.version, size: r.bytes.byteLength }))
              .sort((a, b) => `${a.lang}${a.version}`.localeCompare(`${b.lang}${b.version}`));
            db.close();
            resolve(rows);
          };
        };
      }),
  );
}

/** Puts `bytes` into the `ocr_models` store as `lang`'s model `version`. */
export async function putStoredModel(
  page: Page,
  lang: string,
  version: string,
  bytes: number[],
): Promise<void> {
  await page.evaluate(
    ([l, v, b]) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error ?? new Error('open failed'));
        open.onsuccess = () => {
          const db = open.result;
          const tx = db.transaction('ocr_models', 'readwrite');
          tx.objectStore('ocr_models').put({ lang: l, version: v, bytes: new Uint8Array(b) });
          tx.oncomplete = () => {
            db.close();
            resolve();
          };
          tx.onerror = () => reject(tx.error ?? new Error('write failed'));
        };
      }),
    [lang, version, bytes] as const,
  );
}

/**
 * Lets a test break the OCR worker: after this init script,
 * `window.__ocrFault = 'crash'` makes the next OCR worker fail to start and
 * `'model'` makes it answer as if the engine rejected the model.
 */
export async function installOcrFaults(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as unknown as { __ocrFault: string | null; Worker: typeof Worker };
    w.__ocrFault = null;
    const Original = w.Worker;
    const scripts: Record<string, string> = {
      crash: "throw new Error('the engine could not start');",
      model:
        "onmessage = () => postMessage({ kind: 'error', failure: 'model', message: 'not a .traineddata file' });",
    };
    w.Worker = class extends Original {
      constructor(url: string | URL, options?: WorkerOptions) {
        const fault = w.__ocrFault;
        const script =
          fault !== null && String(url).includes('ocr-worker') ? scripts[fault] : undefined;
        if (script === undefined) super(url, options);
        else super(URL.createObjectURL(new Blob([script], { type: 'text/javascript' })));
      }
    };
  });
}
