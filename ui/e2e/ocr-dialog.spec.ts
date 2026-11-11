import { expect, test, type Page } from '@playwright/test';
import { createDocument, menu, storedPref } from './helpers';
import {
  type ModelAnswer,
  ocrDialog,
  ocrField,
  ocrStatus,
  openOcr,
  openPng,
  pagePng,
  pageSnapshot,
  recognizedText,
  recordSteps,
  recordedSteps,
  serveModels,
  wordsPng,
} from './ocr-helpers';

// Tools ▸ Recognize Text… in Image mode (docs/specs/ocr-dialog.md): the
// menu, the dialog, the model download on first use and the recognition.
// The models are served from the committed files (`serveModels`).

const words = (text: string): string[] => text.split(/\s+/).filter((w) => w !== '');

/** A 640 × 200 page with three words in one row. */
async function openThreeWords(page: Page): Promise<void> {
  const png = await wordsPng(page, 640, 200, [
    { text: 'apple', x: 30, y: 70 },
    { text: 'river', x: 250, y: 70 },
    { text: 'garden', x: 450, y: 70 },
  ]);
  await openPng(page, 'words.png', png);
}

test('Image mode has a Tools menu between Effects and Help; Pixel mode has none (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const bar = page.getByRole('menubar');
  const tops = await bar.getByRole('menuitem').allTextContents();
  const names = tops.map((s) => s.trim());
  const tools = names.indexOf('Tools');
  expect(tools).toBeGreaterThan(names.indexOf('Effects'));
  expect(tools).toBeLessThan(names.indexOf('Help'));
  await bar.getByRole('menuitem', { name: 'Tools', exact: true }).click();
  await expect(
    page.getByRole('menu', { name: 'Tools', exact: true }).getByRole('menuitem'),
  ).toHaveText(['Recognize Text…']);
  await page.keyboard.press('Escape');

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(page.getByRole('menubar').getByRole('menuitem', { name: 'Tools' })).toHaveCount(0);
});

test('first use downloads the model with progress, then recognizes the image (AC-03, AC-04, AC-07)', async ({
  page,
}) => {
  const requests = await serveModels(page);
  await openThreeWords(page);
  await recordSteps(page);
  await openOcr(page);
  const dialog = ocrDialog(page);
  await expect(dialog.getByRole('combobox', { name: 'Language' })).toHaveValue('eng');
  const text = await recognizedText(page);
  expect(words(text)).toEqual(['apple', 'river', 'garden']);

  const steps = await recordedSteps(page);
  const download = steps.filter((s) => s.startsWith('Downloading English model…'));
  expect(download.length).toBeGreaterThan(0);
  // Bytes downloaded and the total.
  expect(download.at(-1)).toMatch(/Downloading English model… \d+(\.\d)? MB of 4\.1 MB/);
  const after = steps.slice(steps.indexOf(download.at(-1) ?? ''));
  expect(after).toContain('Finding text…');
  expect(after.some((s) => /^Recognizing line 1 of \d+$/.test(s))).toBe(true);
  expect(requests.count('eng')).toBe(1);

  // The field: the page text, cursor at its start, Copy and Close.
  const field = ocrField(page);
  await expect(field).toBeFocused();
  expect(
    await field.evaluate((el: HTMLTextAreaElement) => [el.selectionStart, el.selectionEnd]),
  ).toEqual([0, 0]);
  await expect(dialog.getByRole('button', { name: 'Copy' })).toBeEnabled();
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();

  // A later open reads the stored model: no second download.
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  expect(requests.count('eng')).toBe(1);
});

test('without a usable total the download shows only the bytes received (AC-04)', async ({
  page,
}) => {
  // A chunked answer: no Content-Length.
  const requests = await serveModels(page, () => 'chunked');
  await openThreeWords(page);
  await recordSteps(page);
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  const download = (await recordedSteps(page)).filter((s) =>
    s.startsWith('Downloading English model…'),
  );
  expect(download.length).toBeGreaterThan(0);
  for (const step of download) expect(step).toMatch(/^Downloading English model… \d+\.\d MB$/);
  expect(download.at(-1)).toBe('Downloading English model… 4.1 MB');
  expect(requests.count('eng')).toBe(1);
});

test('during work the dialog shows the step and a Cancel button (AC-03)', async ({ page }) => {
  let release = () => {};
  const held = new Promise<void>((resolve) => (release = resolve));
  await serveModels(page, async (): Promise<ModelAnswer> => {
    await held;
    return 'serve';
  });
  await openThreeWords(page);
  await openOcr(page);
  const dialog = ocrDialog(page);
  await expect(ocrStatus(page)).toHaveText(/Downloading English model…/);
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeVisible();
  release();
  await recognizedText(page);
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toHaveCount(0);
});

test('an empty page shows "No text found." (AC-07)', async ({ page }) => {
  await serveModels(page);
  await openPng(page, 'blank.png', await wordsPng(page, 200, 120, []));
  await openOcr(page);
  await expect(ocrDialog(page).getByText('No text found.')).toBeVisible({ timeout: 60_000 });
  await expect(ocrField(page)).toHaveCount(0);
});

test('the language of the last recognition is preselected, across sessions (AC-02)', async ({
  page,
}) => {
  const requests = await serveModels(page);
  await openThreeWords(page);
  await openOcr(page);
  const language = ocrDialog(page).getByRole('combobox', { name: 'Language' });
  await expect(language).toHaveValue('eng');
  await recognizedText(page);
  await language.selectOption('deu');
  // Picking another language downloads and reads with its model.
  await expect.poll(() => requests.count('deu')).toBe(1);
  await recognizedText(page);
  await expect.poll(() => storedPref(page, 'ocr.language')).toBe('deu');

  await openThreeWords(page);
  await openOcr(page);
  await expect(ocrDialog(page).getByRole('combobox', { name: 'Language' })).toHaveValue('deu');
});

test('the benchmark screenshot reads exactly as the native snapshot (AC-17)', async ({ page }) => {
  test.setTimeout(180_000);
  await serveModels(page);
  await openPng(page, 'ui_light.png', pagePng('eng', 'ui_light'));
  await openOcr(page);
  const text = await recognizedText(page, 150_000);
  expect(`${text}\n`).toBe(pageSnapshot('eng', 'ui_light'));
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('menu, dialog and messages are German; the text is not translated (AC-02, AC-16)', async ({
    page,
  }) => {
    await serveModels(page);
    await openThreeWords(page);
    await recordSteps(page);
    await menu(page, 'Extras', 'Text erkennen…');
    const dialog = ocrDialog(page, 'Text erkennen');
    await expect(dialog).toBeVisible();
    const language = dialog.getByRole('combobox', { name: 'Sprache' });
    await expect(language).toHaveValue('deu');
    await expect(language.locator('option')).toHaveText(['Deutsch', 'Englisch']);
    const field = dialog.getByRole('textbox', { name: 'Erkannter Text' });
    await expect(field).toBeVisible({ timeout: 60_000 });
    expect(words(await field.inputValue())).toEqual(['apple', 'river', 'garden']);
    await expect(dialog.getByRole('button', { name: 'Kopieren' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Schließen' })).toBeVisible();
    const steps = await recordedSteps(page);
    expect(steps.some((s) => /^Deutsches Modell wird geladen… [\d,]+ MB von 1,5 MB$/.test(s))).toBe(
      true,
    );
    expect(steps).toContain('Text wird gesucht…');
    expect(steps.some((s) => /^Zeile 1 von \d+ wird erkannt$/.test(s))).toBe(true);
  });
});
