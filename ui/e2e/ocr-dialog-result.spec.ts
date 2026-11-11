import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, menu } from './helpers';
import { historyRows, openHistory } from './history-helpers';
import {
  lassoSelect,
  ocrDialog,
  ocrField,
  openOcr,
  openPng,
  recognizedText,
  rectSelect,
  serveModels,
  wordsPng,
} from './ocr-helpers';

// The Recognize Text dialog's result (docs/specs/ocr-dialog.md): Copy with
// the user's edits, a failing clipboard, what the source area reads with a
// selection or hidden layers, and a document the dialog leaves unchanged.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const words = (text: string): string[] => text.split(/\s+/).filter((w) => w !== '');

const ROW = { width: 640, height: 200 };

/** A 640 × 200 page with three words in one row. */
async function openThreeWords(page: Page): Promise<void> {
  const png = await wordsPng(page, ROW.width, ROW.height, [
    { text: 'apple', x: 30, y: 70 },
    { text: 'river', x: 250, y: 70 },
    { text: 'garden', x: 450, y: 70 },
  ]);
  await openPng(page, 'words.png', png);
}

/** The clipboard's plain text; Windows reads line breaks back as CRLF. */
const clipboardText = async (page: Page): Promise<string> =>
  (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/g, '\n');

test('Copy puts the edited text on the clipboard; Copied lasts until the next edit (AC-08)', async ({
  page,
}) => {
  await serveModels(page);
  await openThreeWords(page);
  await openOcr(page);
  await recognizedText(page);
  const dialog = ocrDialog(page);
  const field = ocrField(page);
  const copy = dialog.getByRole('button', { name: 'Copy' });

  await field.press('Control+End');
  await field.pressSequentially(' tree');
  const edited = await field.inputValue();
  expect(edited.endsWith('garden tree')).toBe(true);
  await copy.click();
  await expect.poll(() => clipboardText(page)).toBe(edited);
  await expect(dialog.getByText('Copied', { exact: true })).toBeVisible();
  await expect(dialog).toBeVisible();

  await field.pressSequentially('s');
  await expect(dialog.getByText('Copied', { exact: true })).toBeHidden();

  await field.fill('');
  await expect(copy).toBeDisabled();
  await field.fill('  \n\t ');
  await expect(copy).toBeDisabled();
  await field.fill('x');
  await expect(copy).toBeEnabled();
});

test('a clipboard that refuses the text shows an error and keeps the text (AC-09)', async ({
  page,
}) => {
  await page.addInitScript(() => {
    navigator.clipboard.writeText = () =>
      Promise.reject(new DOMException('Write permission denied.', 'NotAllowedError'));
  });
  await serveModels(page);
  await openThreeWords(page);
  await openOcr(page);
  const text = await recognizedText(page);
  const dialog = ocrDialog(page);
  await dialog.getByRole('button', { name: 'Copy' }).click();
  await expect(dialog.getByText('Could not copy to the clipboard.')).toBeVisible();
  await expect(ocrField(page)).toHaveValue(text);
});

test('with a selection only the selected word is read (AC-10)', async ({ page }) => {
  // Three words in a column, gaps of more than one word height.
  const size = { width: 400, height: 400 };
  await serveModels(page);
  const png = await wordsPng(page, size.width, size.height, [
    { text: 'apple', x: 40, y: 30 },
    { text: 'river', x: 40, y: 170 },
    { text: 'garden', x: 40, y: 310 },
  ]);
  await openPng(page, 'column.png', png);
  await rectSelect(page, size, [20, 150], [380, 240]);
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['river']);
});

test('a lasso leaves out a word inside its bounding box (AC-10)', async ({ page }) => {
  const size = { width: 600, height: 400 };
  await serveModels(page);
  const png = await wordsPng(page, size.width, size.height, [
    { text: 'apple', x: 30, y: 30 },
    { text: 'garden', x: 30, y: 310 },
    { text: 'river', x: 400, y: 310 },
  ]);
  await openPng(page, 'corner.png', png);
  // An L: the top band and the left column; "river" (bottom right) lies
  // inside its bounding box but outside the lasso.
  await lassoSelect(page, size, [
    [5, 5],
    [595, 5],
    [595, 120],
    [270, 120],
    [270, 395],
    [5, 395],
    [5, 10],
  ]);
  await openOcr(page);
  const read = words(await recognizedText(page));
  expect(read).toEqual(['apple', 'garden']);
});

test('a word on a hidden layer is not read, one on a visible layer above is (AC-11)', async ({
  page,
}) => {
  await serveModels(page);
  const bottom = await wordsPng(page, ROW.width, ROW.height, [{ text: 'apple', x: 30, y: 70 }]);
  await openPng(page, 'layers.png', bottom);
  // The top layer: "river" on transparency, pasted as a new layer.
  const top = await wordsPng(
    page,
    ROW.width,
    ROW.height,
    [{ text: 'river', x: 360, y: 70 }],
    'transparent',
  );
  await page.evaluate(async (bytes) => {
    const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
  }, top);
  await menu(page, 'Edit', 'Paste');
  const hide = page.getByRole('button', { name: /^Hide / });
  await expect(hide).toHaveCount(2);
  // Rows run top to bottom: hide the bottom layer.
  await hide.last().click();
  await expect(page.getByRole('button', { name: /^Show / })).toHaveCount(1);
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['river']);
});

test('the dialog leaves the document, selection, history and dirty state unchanged (AC-13)', async ({
  page,
}) => {
  await serveModels(page);
  await openThreeWords(page);
  await rectSelect(page, ROW, [230, 50], [420, 150]);
  await openHistory(page);
  const state = async () => ({
    rows: (await historyRows(page).allTextContents()).map((s) => s.trim()),
    undo: await page.getByRole('button', { name: 'Undo', exact: true }).first().isEnabled(),
    redo: await page.getByRole('button', { name: 'Redo', exact: true }).first().isEnabled(),
    dirty: await page.getByTitle('words.png — unsaved changes', { exact: true }).count(),
  });
  const copySelection = async () => {
    await clearClipboard(page);
    await menu(page, 'Edit', 'Copy');
    return copiedImage(page);
  };
  const before = await state();
  const pixels = await copySelection();

  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['river']);
  await ocrField(page).pressSequentially(' edited');
  await ocrDialog(page).getByRole('button', { name: 'Copy' }).click();
  await ocrDialog(page).getByRole('button', { name: 'Close' }).click();
  await expect(ocrDialog(page)).toBeHidden();
  await openOcr(page);
  await recognizedText(page);
  await page.keyboard.press('Escape');
  await expect(ocrDialog(page)).toBeHidden();

  expect(await state()).toEqual(before);
  expect(await copySelection()).toEqual(pixels);
  // The selection still limits what is read.
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['river']);
});
