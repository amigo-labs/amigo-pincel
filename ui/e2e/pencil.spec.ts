import { expect, test } from '@playwright/test';
import { createDocument, pixelCanvas } from './helpers';

test('pixel mode: a fast pencil stroke stays connected', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const canvas = pixelCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');
  const at = (px: number, py: number) =>
    [box.x + ((px + 0.5) * box.width) / 16, box.y + ((py + 0.5) * box.height) / 16] as const;

  await page.getByTitle('Pencil (B)').click();
  const color = page.getByLabel('Color', { exact: true });
  await color.fill('#00ff00');
  // One jump from (1, 1) to (14, 14): no pointer samples in between.
  await page.mouse.move(...at(1, 1));
  await page.mouse.down();
  await page.mouse.move(...at(14, 14), { steps: 1 });
  await page.mouse.up();

  // Sample a pixel in the middle of the diagonal with the eyedropper.
  await color.fill('#000000');
  await page.getByTitle(/Eyedropper/).click();
  await page.mouse.click(...at(7, 7));
  await expect(color).toHaveValue('#00ff00');
});
