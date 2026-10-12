import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

test('palette: DB32 by default, add / recolor / move / delete with undo', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const panel = page.getByRole('region', { name: 'Palette' });
  const swatches = panel.getByRole('group', { name: 'Palette colors' }).getByRole('button');
  await expect(swatches).toHaveCount(32);

  await panel.getByRole('button', { name: 'Add current color to palette' }).click();
  await expect(swatches).toHaveCount(33);

  await swatches.nth(1).click();
  const editor = panel.getByLabel('Palette entry editor');
  await expect(editor.getByText('#1')).toBeVisible();
  await editor.getByLabel('Entry color').fill('#123456');
  await expect(swatches.nth(1)).toHaveAttribute('title', /#123456/);

  await editor.getByRole('button', { name: 'Move entry left' }).click();
  await expect(swatches.nth(0)).toHaveAttribute('title', /#123456/);

  await editor.getByRole('button', { name: 'Delete entry' }).click();
  await expect(swatches).toHaveCount(32);

  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(swatches).toHaveCount(33);
  await expect(swatches.nth(0)).toHaveAttribute('title', /#123456/);
});

test.describe('palette presets and .gpl', () => {
  // Force the download / <input type=file> fallbacks (see ora.spec).
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  test('a preset replaces the palette in one undo step', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    const panel = page.getByRole('region', { name: 'Palette' });
    const swatches = panel.getByRole('group', { name: 'Palette colors' }).getByRole('button');
    await panel.getByRole('button', { name: 'Palette options' }).click();
    await page.getByRole('menuitem', { name: 'PICO-8' }).click();
    await expect(swatches).toHaveCount(16);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(swatches).toHaveCount(32);
  });

  test('.gpl import and export', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    const panel = page.getByRole('region', { name: 'Palette' });
    const swatches = panel.getByRole('group', { name: 'Palette colors' }).getByRole('button');

    await panel.getByRole('button', { name: 'Palette options' }).click();
    const [chooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.getByRole('menuitem', { name: 'Import .gpl…' }).click(),
    ]);
    const gpl =
      'GIMP Palette\nName: Test\n# four greys\n0 0 0 Black\n85 85 85\n170 170 170\n255 255 255 White\n';
    await chooser.setFiles({ name: 'grey.gpl', mimeType: 'text/plain', buffer: Buffer.from(gpl) });
    await expect(swatches).toHaveCount(4);
    await expect(page.getByText('imported 4 palette colours')).toBeVisible();

    await panel.getByRole('button', { name: 'Palette options' }).click();
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('menuitem', { name: 'Export .gpl…' }).click(),
    ]);
    expect(download.suggestedFilename()).toMatch(/\.gpl$/);
    const text = await readFile(await download.path(), 'utf8');
    expect(text.startsWith('GIMP Palette\n')).toBe(true);
    expect(text).toContain('255 255 255\tWhite');
  });
});
