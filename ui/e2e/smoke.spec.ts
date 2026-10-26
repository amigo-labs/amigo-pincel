import { expect, test } from '@playwright/test';
import { createDocument, drag, gotoStart, imageCanvas, pixelCanvas } from './helpers';

test('start screen offers both modes', async ({ page }) => {
  await gotoStart(page);
  await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'New image' })).toBeEnabled();
});

test('start screen: the New cards open in place, one at a time', async ({ page }) => {
  await gotoStart(page);
  const pixelHead = page.getByRole('button', { name: 'New pixel sprite' });
  const imageHead = page.getByRole('button', { name: 'New image' });
  const pixelForm = page.getByRole('form', { name: 'New pixel sprite' });
  const imageForm = page.getByRole('form', { name: 'New image' });
  await expect(pixelForm).toHaveCount(0);
  await expect(imageForm).toHaveCount(0);

  await imageHead.click();
  await expect(imageHead).toHaveAttribute('aria-expanded', 'true');
  await expect(imageForm.getByLabel('Width')).toHaveValue('800');
  await expect(imageForm.getByLabel('Height')).toHaveValue('600');
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await pixelHead.click();
  await expect(imageForm).toHaveCount(0);
  await expect(pixelForm.getByLabel('Width')).toHaveValue('64');
  await expect(pixelForm.getByLabel('Width')).toBeFocused();

  await page.keyboard.press('Escape');
  await expect(pixelForm).toHaveCount(0);
  await expect(pixelHead).toBeFocused();
  await expect(pixelHead).toHaveAttribute('aria-expanded', 'false');
});

test('pixel mode: a pencil stroke is undoable', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const canvas = pixelCanvas(page);
  await expect(canvas).toBeVisible();
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();

  await page.getByTitle('Pencil (B)').click();
  await drag(page, canvas, [0.2, 0.2], [0.8, 0.6]);
  await expect(undo).toBeEnabled();

  await undo.click();
  await expect(undo).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeEnabled();
});

test('image mode: a brush stroke is undoable and the pixel wasm is not loaded', async ({
  page,
}) => {
  const wasmRequests: string[] = [];
  page.on('request', (req) => {
    if (req.url().endsWith('.wasm')) wasmRequests.push(req.url());
  });
  await createDocument(page, 'image', { width: 64, height: 48 });
  const canvas = imageCanvas(page);
  await expect(canvas).toBeVisible();
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();

  await drag(page, canvas, [0.1, 0.5], [0.9, 0.5]);
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();

  expect(wasmRequests.some((u) => u.includes('pincel_image_wasm'))).toBe(true);
  expect(wasmRequests.some((u) => u.includes('pincel_wasm'))).toBe(false);
});
