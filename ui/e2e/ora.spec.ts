import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { createDocument, drag, gotoStart, imageCanvas, menu } from './helpers';

// Force the download / <input type=file> fallbacks: the File System Access
// pickers would open native dialogs the test cannot drive.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

test('image mode: Save writes an OpenRaster file that reopens with its layers', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 40, height: 30 });
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await page.getByTitle('Add layer').click();
  await expect(page.getByRole('button', { name: /Hide layer/ })).toHaveCount(2);
  await expect(page.getByTitle(/unsaved changes/)).toBeVisible();

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    menu(page, 'File', 'Save'),
  ]);
  expect(download.suggestedFilename()).toBe('image.ora');
  const bytes = await readFile(await download.path());
  expect(bytes.subarray(30, 54).toString('latin1')).toBe('mimetypeimage/openraster');
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);

  // Reopen through the start screen's Open… (input fallback).
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name: 'image.ora', mimeType: 'image/openraster', buffer: bytes });
  await expect(page.getByText('40 × 30 · sRGB')).toBeVisible();
  await expect(page.getByRole('button', { name: /Hide layer/ })).toHaveCount(2);
});
