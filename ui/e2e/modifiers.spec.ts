import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, pixelCanvas } from './helpers';

// On-screen Shift / Alt for touch and pen (spec §9.4): the toggles appear
// once a touch pointer is seen, and a latched toggle acts exactly like the
// held key.

async function revealToggles(page: Page): Promise<void> {
  // Any touch on the editor stage reveals the bar; 1:1 is a harmless target.
  await page.getByRole('button', { name: 'Actual size' }).tap();
  await expect(page.getByRole('group', { name: 'Modifier keys' })).toBeVisible();
}

async function lineShot(page: Page, canvas: Locator, hold: 'none' | 'key'): Promise<Buffer> {
  if (hold === 'key') await page.keyboard.down('Shift');
  await drag(page, canvas, [0.2, 0.2], [0.8, 0.35]);
  if (hold === 'key') await page.keyboard.up('Shift');
  const shot = await canvas.screenshot();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  return shot;
}

test('a mouse-only desktop shows no modifier toggles', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await page.getByRole('button', { name: 'Actual size' }).click();
  await expect(page.getByRole('group', { name: 'Modifier keys' })).toHaveCount(0);
});

test.describe('touch device', () => {
  test.use({ hasTouch: true });

  test('pixel mode: the latched Shift snaps a line like the Shift key', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    await revealToggles(page);
    const canvas = pixelCanvas(page);
    await page.keyboard.press('l');

    const free = await lineShot(page, canvas, 'none');
    const keyed = await lineShot(page, canvas, 'key');
    const shift = page.getByRole('button', { name: 'Shift' });
    await shift.click();
    await expect(shift).toHaveAttribute('aria-pressed', 'true');
    const latched = await lineShot(page, canvas, 'none');

    expect(latched.equals(keyed)).toBe(true);
    expect(latched.equals(free)).toBe(false);

    await shift.click();
    await expect(shift).toHaveAttribute('aria-pressed', 'false');
  });

  test('image mode: the latched Shift squares a shape like the Shift key', async ({ page }) => {
    await createDocument(page, 'image', { width: 64, height: 48 });
    await revealToggles(page);
    const canvas = imageCanvas(page);
    await page.keyboard.press('u');

    const free = await lineShot(page, canvas, 'none');
    const keyed = await lineShot(page, canvas, 'key');
    await page.getByRole('button', { name: 'Shift' }).click();
    const latched = await lineShot(page, canvas, 'none');

    expect(latched.equals(keyed)).toBe(true);
    expect(latched.equals(free)).toBe(false);
  });
});
