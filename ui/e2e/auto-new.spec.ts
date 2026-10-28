import { expect, test } from '@playwright/test';
import { gotoStart, menu } from './helpers';

// "Open like this on startup" (spec §9.2, Decision Log 2026-10-28): a New
// card remembers its mode and size, the next launch skips the start
// screen; File ▸ New… switches it off again.

test('a ticked New card opens its document on the next launch', async ({ page }) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'New image' }).click();
  const card = page.getByRole('form', { name: 'New image' });
  await card.getByLabel('Width').fill('320');
  await card.getByLabel('Height').fill('200');
  await card.getByLabel('Open like this on startup').check();
  await card.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByText('320 × 200 · sRGB')).toBeVisible();

  await page.reload();
  await expect(page.getByText('320 × 200 · sRGB')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toHaveCount(0);

  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  const box = dialog.getByLabel('Open like this on startup');
  await expect(box).toBeChecked();
  await box.uncheck();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  await page.reload();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
  await page.getByRole('button', { name: 'New image' }).click();
  await expect(
    page.getByRole('form', { name: 'New image' }).getByLabel('Open like this on startup'),
  ).not.toBeChecked();
});

test('ticking one card clears the other', async ({ page }) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'New pixel sprite' }).click();
  await page
    .getByRole('form', { name: 'New pixel sprite' })
    .getByLabel('Open like this on startup')
    .check();
  await page.getByRole('button', { name: 'New image' }).click();
  const image = page.getByRole('form', { name: 'New image' }).getByLabel('Open like this on startup');
  await expect(image).not.toBeChecked();
  await image.check();
  await page.getByRole('button', { name: 'New pixel sprite' }).click();
  await expect(
    page.getByRole('form', { name: 'New pixel sprite' }).getByLabel('Open like this on startup'),
  ).not.toBeChecked();
});
