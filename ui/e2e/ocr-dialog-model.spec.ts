import { expect, test, type Page } from '@playwright/test';
import { menu } from './helpers';
import {
  installOcrFaults,
  modelBytes,
  ocrDialog,
  ocrStatus,
  openOcr,
  openPng,
  putStoredModel,
  recognizedText,
  serveModels,
  storedModels,
  wordsPng,
} from './ocr-helpers';

// The language model's life in the Recognize Text dialog
// (docs/specs/ocr-dialog.md): a download that fails or does not verify,
// a recognition that fails, storage that cannot be used, a damaged stored
// model, and the removal of an older stored version.

const words = (text: string): string[] => text.split(/\s+/).filter((w) => w !== '');

const ENG_SIZE = 4_113_088;

async function openThreeWords(page: Page): Promise<void> {
  const png = await wordsPng(page, 640, 200, [
    { text: 'apple', x: 30, y: 70 },
    { text: 'river', x: 250, y: 70 },
    { text: 'garden', x: 450, y: 70 },
  ]);
  await openPng(page, 'words.png', png);
}

/** The English model with one byte changed: same size, wrong hash. */
function damagedModel(): Buffer {
  const bytes = Buffer.from(modelBytes('eng'));
  bytes[bytes.length - 100] = (bytes[bytes.length - 100] ?? 0) ^ 0xff;
  return bytes;
}

test('a download that does not match its hash is not stored; Retry downloads again (AC-05)', async ({
  page,
}) => {
  const requests = await serveModels(page, (_lang, n) =>
    n === 0 ? { body: damagedModel() } : 'serve',
  );
  await openThreeWords(page);
  await openOcr(page);
  const dialog = ocrDialog(page);
  await expect(ocrStatus(page)).toHaveText('The language model could not be verified.');
  expect(await storedModels(page)).toEqual([]);
  await dialog.getByRole('button', { name: 'Retry' }).click();
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  expect(requests.count('eng')).toBe(2);
  expect(await storedModels(page)).toEqual([{ lang: 'eng', version: '4.1.0', size: ENG_SIZE }]);
});

for (const [name, failure] of [
  ['offline', 'abort'],
  ['a server error', { status: 503 }],
] as const) {
  test(`a failed download (${name}) stores nothing; Retry downloads again (AC-06)`, async ({
    page,
  }) => {
    const requests = await serveModels(page, (_lang, n) => (n === 0 ? failure : 'serve'));
    await openThreeWords(page);
    await openOcr(page);
    await expect(ocrStatus(page)).toHaveText(
      'The language model could not be downloaded. Check the connection and try again.',
    );
    expect(await storedModels(page)).toEqual([]);
    await ocrDialog(page).getByRole('button', { name: 'Retry' }).click();
    expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
    expect(requests.count('eng')).toBe(2);
  });
}

test('a recognition that fails shows an error with Retry; the editor stays usable (AC-18)', async ({
  page,
}) => {
  await installOcrFaults(page);
  await serveModels(page);
  await openThreeWords(page);
  await page.evaluate(() => {
    (window as unknown as { __ocrFault: string | null }).__ocrFault = 'crash';
  });
  await openOcr(page);
  const dialog = ocrDialog(page);
  await expect(ocrStatus(page)).toHaveText('Text recognition failed.');
  await expect(dialog.getByRole('button', { name: 'Retry' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();
  // The editor takes edits again.
  const undo = page.getByRole('button', { name: 'Undo', exact: true }).first();
  await expect(undo).toBeDisabled();
  await menu(page, 'Image', 'Flip Horizontal');
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
  await page.evaluate(() => {
    (window as unknown as { __ocrFault: string | null }).__ocrFault = null;
  });
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
});

test('a model the engine rejects is removed, so Retry downloads it again (AC-18)', async ({
  page,
}) => {
  await installOcrFaults(page);
  const requests = await serveModels(page);
  await openThreeWords(page);
  await page.evaluate(() => {
    (window as unknown as { __ocrFault: string | null }).__ocrFault = 'model';
  });
  await openOcr(page);
  await expect(ocrStatus(page)).toHaveText('Text recognition failed.');
  await expect.poll(() => storedModels(page)).toEqual([]);
  await page.evaluate(() => {
    (window as unknown as { __ocrFault: string | null }).__ocrFault = null;
  });
  await ocrDialog(page).getByRole('button', { name: 'Retry' }).click();
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  expect(requests.count('eng')).toBe(2);
});

test('storage that cannot be read or written: the model serves this recognition, without an error (AC-19)', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const refuse = (name: string) => {
      throw new DOMException(`${name} refused`, 'QuotaExceededError');
    };
    const { get, put } = IDBObjectStore.prototype;
    IDBObjectStore.prototype.get = function (this: IDBObjectStore, ...args) {
      if (this.name === 'ocr_models') refuse('read');
      return get.apply(this, args);
    };
    IDBObjectStore.prototype.put = function (this: IDBObjectStore, ...args) {
      if (this.name === 'ocr_models') refuse('write');
      return put.apply(this, args);
    };
  });
  const requests = await serveModels(page);
  await openThreeWords(page);
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  await ocrDialog(page).getByRole('button', { name: 'Close' }).click();
  // Not stored, so the next use downloads it again.
  await openOcr(page);
  await recognizedText(page);
  expect(requests.count('eng')).toBe(2);
});

test('a stored model that no longer matches its hash is removed and downloaded again (AC-20)', async ({
  page,
}) => {
  const requests = await serveModels(page);
  await openThreeWords(page);
  await putStoredModel(page, 'eng', '4.1.0', [1, 2, 3, 4, 5]);
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  expect(requests.count('eng')).toBe(1);
  expect(await storedModels(page)).toEqual([{ lang: 'eng', version: '4.1.0', size: ENG_SIZE }]);
});

test('storing a newer version removes the older one of that language', async ({ page }) => {
  await serveModels(page);
  await openThreeWords(page);
  await putStoredModel(page, 'eng', '4.0.0', [9, 9, 9]);
  await putStoredModel(page, 'deu', '4.0.0', [7, 7]);
  await openOcr(page);
  await recognizedText(page);
  expect(await storedModels(page)).toEqual([
    { lang: 'deu', version: '4.0.0', size: 2 },
    { lang: 'eng', version: '4.1.0', size: ENG_SIZE },
  ]);
});
