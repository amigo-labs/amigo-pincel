import { expect, test } from '@playwright/test';
import { createDocument, gotoStart } from './helpers';

// UI language (spec §9.5): browser language by default, switchable on the
// start screen and under View ▸ Language, remembered across reloads.

test.describe('German browser', () => {
  test.use({ locale: 'de-DE' });

  test('starts in German', async ({ page }) => {
    await page.goto('./');
    await expect(page.getByRole('button', { name: 'Neues Pixel-Sprite' })).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  });
});

test('start-screen switch persists across reloads', async ({ page }) => {
  await gotoStart(page);
  await page.getByLabel('Language').selectOption('de');
  await expect(page.getByRole('button', { name: 'Neues Bild' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Neues Bild' })).toBeVisible();
  await page.getByLabel('Sprache').selectOption('en');
  await expect(page.getByRole('button', { name: 'New image' })).toBeVisible();
});

test('View ▸ Language switches the editor', async ({ page }) => {
  await createDocument(page, 'pixel');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await expect(page.getByRole('button', { name: 'Vergrößern' })).toBeVisible();
});
