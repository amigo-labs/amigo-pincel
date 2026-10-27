import { expect, test } from '@playwright/test';
import { createDocument, drag, gotoStart, imageCanvas, menu, pixelCanvas } from './helpers';

// The Help menu exists wherever the OS draws no native menu bar — the web
// build included, which is what these tests run against (spec §11.3).
// On the macOS desktop app it is absent and the native Help menu serves
// About; that half is only verifiable by hand.

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Help ▸ About Pincel names the version`, async ({ page }) => {
    await createDocument(page, mode);
    await expect(
      page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }),
    ).toBeVisible();

    await menu(page, 'Help', 'About Pincel');
    const dialog = page.getByRole('dialog', { name: 'About Pincel' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/^Version /)).toBeVisible();

    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();
  });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: the About dialog holds the keyboard`, async ({ page }) => {
    await createDocument(page, mode, { width: 64, height: 64 });
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    await drag(page, canvas, [0.2, 0.5], [0.8, 0.5]);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await expect(undo).toBeEnabled();

    await menu(page, 'Help', 'About Pincel');
    const dialog = page.getByRole('dialog', { name: 'About Pincel' });
    await expect(dialog).toBeVisible();

    // Neither an undo nor a tool key may reach the editor behind the dialog.
    await page.keyboard.press('Control+z');
    await page.keyboard.press('e');
    await expect(dialog).toBeVisible();
    await expect(undo).toBeEnabled();

    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(undo).toBeEnabled();
  });
}

test('the start screen reaches About without a menu bar', async ({ page }) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'About Pincel' }).click();
  await expect(page.getByRole('dialog', { name: 'About Pincel' })).toBeVisible();
});

test('the web build offers no updater entries', async ({ page }) => {
  // They belong to the desktop app (`updater::supported()`); a browser
  // has nothing to install.
  await createDocument(page, 'image');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }).click();
  const help = page.getByRole('menu', { name: 'Help', exact: true });
  await expect(help.getByRole('menuitem', { name: 'About Pincel' })).toBeVisible();
  await expect(help.getByRole('menuitem', { name: /Updates/ })).toHaveCount(0);
});
