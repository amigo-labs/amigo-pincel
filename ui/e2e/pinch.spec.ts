import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, pixelCanvas } from './helpers';

test.use({ hasTouch: true });

/** Spread two real (CDP) touch points apart over `target`'s centre. */
async function pinchOut(page: Page, target: Locator): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('pinch target is not visible');
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  const cdp = await page.context().newCDPSession(page);
  const points = (spread: number) => [
    { x: cx - spread, y: cy, id: 1 },
    { x: cx + spread, y: cy, id: 2 },
  ];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: points(10) });
  for (const spread of [20, 40, 60, 80]) {
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: points(spread) });
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
}

test('pixel mode: two-finger pinch zooms in and paints nothing', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const readout = page.getByText(/^\d+×$/);
  const before = Number((await readout.textContent())?.replace('×', ''));
  await pinchOut(page, pixelCanvas(page));
  await expect
    .poll(async () => Number((await readout.textContent())?.replace('×', '')))
    .toBeGreaterThan(before);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});

test('image mode: two-finger pinch zooms in and paints nothing', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const readout = page.getByRole('button', { name: 'Fit to window' });
  const before = Number((await readout.textContent())?.replace('%', ''));
  await pinchOut(page, imageCanvas(page));
  await expect
    .poll(async () => Number((await readout.textContent())?.replace('%', '')))
    .toBeGreaterThan(before);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});
