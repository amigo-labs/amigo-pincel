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

test('image mode: an effect stays inside the selection', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const canvas = imageCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  // Screenshot clips of a corner (outside the selection) and the centre.
  const clip = (fx: number, fy: number) => ({
    x: box.x + box.width * fx - 3,
    y: box.y + box.height * fy - 3,
    width: 6,
    height: 6,
  });
  // A new image is transparent; fill it so inverting shows.
  await page.keyboard.press('g');
  await canvas.click({ position: { x: 4, y: 4 } });
  const corner = () => page.screenshot({ clip: clip(0.1, 0.15) });
  const centre = () => page.screenshot({ clip: clip(0.5, 0.5) });
  const cornerBefore = await corner();
  const centreBefore = await centre();

  await page.keyboard.press('m');
  await drag(page, canvas, [0.3, 0.3], [0.7, 0.7]);
  await menu(page, 'Adjustments', 'Invert Colors');
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('selection', { exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press('Control+d');

  await expect.poll(async () => (await centre()).equals(centreBefore)).toBe(false);
  expect((await corner()).equals(cornerBefore)).toBe(true);
});
