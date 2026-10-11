import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas } from './helpers';

// CI runs the unoptimized dev wasm; allocating and compositing tens of
// megapixels takes seconds there.
test.setTimeout(120_000);

test('image mode: a canvas wider than 8192 px opens, renders and paints', async ({ page }) => {
  await createDocument(page, 'image', { width: 9000, height: 1000 });
  await expect(page.getByText('9000 × 1000 · sRGB')).toBeVisible({ timeout: 60_000 });

  // The render target stays viewport-sized however large the document is.
  const backing = await page
    .locator('main canvas')
    .first()
    .evaluate((c: HTMLCanvasElement) => ({
      w: c.width,
      h: c.height,
      max: Math.ceil(window.innerWidth * window.devicePixelRatio),
    }));
  expect(backing.w).toBeLessThanOrEqual(backing.max);

  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await drag(page, imageCanvas(page), [0.4, 0.5], [0.6, 0.5]);
  await expect(undo).toBeEnabled({ timeout: 30_000 });

  // 1:1 zoom re-renders just the visible rect.
  await page.getByRole('button', { name: '1:1' }).click();
  await expect(page.getByRole('button', { name: '100%' })).toBeVisible();
});

test('image mode: the New dialog clamps an oversized canvas to the area limit', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16384, height: 16384 });
  await expect(page.getByText('8192 × 8192 · sRGB')).toBeVisible({ timeout: 60_000 });
});
