import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu } from './helpers';

// A new image takes the size of the image on the clipboard, like
// paint.net (spec §9.2). Chromium only: the read needs the permission.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** Puts a `width` × `height` PNG on the clipboard (test-side canvas). */
async function copyImage(page: Page, width: number, height: number): Promise<void> {
  await page.evaluate(
    async ([w, h]) => {
      const canvas = new OffscreenCanvas(w, h);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = '#29adff';
      ctx.fillRect(0, 0, w, h);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
    },
    [width, height] as const,
  );
}

test('the New image card takes the clipboard image size', async ({ page }) => {
  await gotoStart(page);
  await copyImage(page, 123, 45);
  await page.getByRole('button', { name: 'New image' }).click();
  const card = page.getByRole('form', { name: 'New image' });
  await expect(card.getByLabel('Width')).toHaveValue('123');
  await expect(card.getByLabel('Height')).toHaveValue('45');

  // Pixel sprites keep their own default.
  await page.getByRole('button', { name: 'New pixel sprite' }).click();
  await expect(page.getByRole('form', { name: 'New pixel sprite' }).getByLabel('Width')).toHaveValue(
    '64',
  );
});

test('File ▸ New… takes the clipboard image size', async ({ page }) => {
  await createDocument(page, 'image');
  await copyImage(page, 77, 66);
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await expect(dialog.getByLabel('Width')).toHaveValue('77');
  await expect(dialog.getByLabel('Height')).toHaveValue('66');
});
