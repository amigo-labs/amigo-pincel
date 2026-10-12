import { expect, test } from '@playwright/test';
import { createDocument, drag, pixelCanvas } from './helpers';

test.describe('phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('tools sit in a bottom strip, menus behind ☰, panels in a sheet', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // No docked sidebar: the canvas has the width.
    await expect(page.getByRole('complementary', { name: 'Panels' })).toHaveCount(0);
    const tools = page.getByRole('toolbar', { name: 'Tools' });
    await expect(tools).toHaveAttribute('aria-orientation', 'horizontal');

    await tools.getByRole('button', { name: 'Pencil' }).click();
    await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await expect(undo).toBeEnabled();

    // The ☰ sheet lists every menu; Undo from there reverts the stroke.
    await page.getByRole('button', { name: 'Menu' }).click();
    await page.getByRole('menu', { name: 'Menu' }).getByRole('menuitem', { name: 'Undo' }).click();
    await expect(undo).toBeDisabled();

    // Panels open as a bottom sheet with one tab per panel.
    await page.getByRole('button', { name: 'Panels' }).click();
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await sheet.getByRole('tab', { name: 'Layers' }).click();
    await sheet.getByRole('button', { name: 'Add layer' }).click();
    await expect(sheet.getByRole('button', { name: /^Hide Layer/ })).toHaveCount(2);
    await sheet.getByRole('button', { name: 'Close panels' }).click();
    await expect(sheet).toHaveCount(0);
  });
});

test.describe('tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } });

  test('the sidebar is a drawer toggled from the top bar', async ({ page }) => {
    await createDocument(page, 'image', { width: 64, height: 48 });
    await expect(page.getByRole('toolbar', { name: 'Tools' })).toHaveAttribute(
      'aria-orientation',
      'vertical',
    );
    const drawer = page.getByRole('complementary', { name: 'Panels' });
    await expect(drawer).toHaveCount(0);
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    await expect(drawer.getByRole('region', { name: 'Layers' })).toBeVisible();
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    await expect(drawer).toHaveCount(0);
  });
});

test('desktop: panels collapse and stay collapsed across sessions', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const palette = page.getByRole('region', { name: 'Palette' });
  await expect(palette.getByRole('group', { name: 'Palette colors' })).toBeVisible();
  await palette.getByRole('button', { name: /^Palette/, expanded: true }).click();
  await expect(palette.getByRole('group', { name: 'Palette colors' })).toHaveCount(0);

  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await expect(
    page.getByRole('region', { name: 'Palette' }).getByRole('group', { name: 'Palette colors' }),
  ).toHaveCount(0);
});
