import { expect, test, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';

// Image mode: Edit ▸ Paste puts a clipboard image on a new layer; a larger
// image asks whether to grow the canvas, like paint.net (spec §5.4).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** Puts a `width` × `height` PNG on the clipboard (test-side canvas). */
async function copyImage(page: Page, width: number, height: number): Promise<void> {
  await page.evaluate(
    async ([w, h]) => {
      const canvas = new OffscreenCanvas(w, h);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = '#ff004d';
      ctx.fillRect(0, 0, w, h);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
    },
    [width, height] as const,
  );
}

const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });

test('a larger image expands the canvas in one undo step', async ({ page }) => {
  await createDocument(page, 'image', { width: 80, height: 60 });
  await copyImage(page, 100, 70);
  await menu(page, 'Edit', 'Paste');
  const dialog = page.getByRole('dialog', { name: 'Paste larger image' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Expand canvas' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText('100 × 70 · sRGB')).toBeVisible();
  await expect(layerRows(page)).toHaveCount(2);

  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByText('80 × 60 · sRGB')).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
});

test('Keep canvas size crops the pasted image', async ({ page }) => {
  await createDocument(page, 'image', { width: 80, height: 60 });
  await copyImage(page, 100, 70);
  await menu(page, 'Edit', 'Paste');
  await page
    .getByRole('dialog', { name: 'Paste larger image' })
    .getByRole('button', { name: 'Keep canvas size' })
    .click();
  await expect(page.getByText('80 × 60 · sRGB')).toBeVisible();
  await expect(layerRows(page)).toHaveCount(2);
});

test('a smaller image pastes without asking', async ({ page }) => {
  await createDocument(page, 'image', { width: 80, height: 60 });
  await copyImage(page, 20, 10);
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByRole('dialog', { name: 'Paste larger image' })).toHaveCount(0);
  await expect(page.getByText('80 × 60 · sRGB')).toBeVisible();
});

test('Ctrl+V pastes the image a paste event carries', async ({ page }) => {
  await createDocument(page, 'image', { width: 80, height: 60 });
  // The browser fires `paste` for Ctrl+V; dispatch one carrying a PNG.
  await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(30, 20);
    canvas.getContext('2d')?.fillRect(0, 0, 30, 20);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    const data = new DataTransfer();
    data.items.add(new File([blob], 'clip.png', { type: 'image/png' }));
    window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data }));
  });
  await expect(layerRows(page)).toHaveCount(2);
});

test('a paste inside the shell New dialog does not reach the canvas', async ({ page }) => {
  await createDocument(page, 'image', { width: 80, height: 60 });
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  const box = dialog.getByLabel('Open like this on startup');
  await box.focus();
  await box.evaluate(async (el) => {
    const canvas = new OffscreenCanvas(30, 20);
    canvas.getContext('2d')?.fillRect(0, 0, 30, 20);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    const data = new DataTransfer();
    data.items.add(new File([blob], 'clip.png', { type: 'image/png' }));
    el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true }));
  });
  // Give a (wrongly) accepted paste time to decode and land before checking.
  await page.waitForTimeout(500);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(layerRows(page)).toHaveCount(1);
});
