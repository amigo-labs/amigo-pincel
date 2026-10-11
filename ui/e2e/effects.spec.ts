import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas, pixelCanvas, menu } from './helpers';

test('image mode: effects and adjustments come from the shared catalog', async ({ page }) => {
  await createDocument(page, 'image', { width: 48, height: 32 });
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  await menu(page, 'Effects', 'Blur', 'Gaussian Blur…');
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('heading', { name: 'Gaussian Blur' })).toBeVisible();
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);

  await menu(page, 'Adjustments', 'Levels…');
  await expect(page.getByRole('dialog').getByLabel('Input white', { exact: true })).toHaveValue(
    '255',
  );
  await page.getByRole('dialog').getByRole('button', { name: 'OK' }).click();

  // Stroke, blur and levels are three undo steps.
  await undo.click();
  await undo.click();
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
});

test('pixel mode: the Levels adjustment applies', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await menu(page, 'Adjustments', 'Levels…');
  await page.getByRole('dialog').getByRole('button', { name: 'OK' }).click();
  await expect(page.getByText(/Levels applied/)).toBeVisible();
});
