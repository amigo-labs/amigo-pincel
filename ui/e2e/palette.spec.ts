import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
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

/** The Palette panel, its swatches and the foreground / background controls. */
function paletteUi(page: Page) {
  const panel = page.getByRole('region', { name: 'Palette' });
  const alpha = page.getByRole('slider', { name: 'Foreground alpha' });
  return {
    panel,
    swatches: panel.getByRole('group', { name: 'Palette colors' }).getByRole('button'),
    editor: panel.getByLabel('Palette entry editor'),
    add: panel.getByRole('button', { name: 'Add current color to palette' }),
    fg: page.getByLabel('Foreground color', { exact: true }),
    bg: page.getByLabel('Background color', { exact: true }),
    hex: page.getByLabel('Foreground color hex value'),
    alpha,
    alphaShown: page.locator('label', { has: alpha }),
  };
}

test('add appends the foreground color and alpha and selects it', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const ui = paletteUi(page);
  await ui.hex.fill('#123456');
  await ui.alpha.fill('128');
  await ui.add.click();
  await expect(ui.swatches).toHaveCount(33);
  await expect(ui.swatches.nth(32)).toHaveAttribute('title', /#123456/);
  await expect(ui.editor.getByText('#32', { exact: true })).toBeVisible();
  await expect(ui.swatches.nth(32)).toHaveClass(/swatch-selected/);
  // The entry kept the foreground alpha: picking another entry resets the
  // slider, picking the new one brings 128 back.
  await ui.swatches.nth(0).click();
  await expect(ui.alpha).toHaveValue('255');
  await ui.swatches.nth(32).click();
  await expect(ui.alpha).toHaveValue('128');
});

test('clicking a swatch picks the foreground and opens the entry editor', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const ui = paletteUi(page);
  await ui.hex.fill('#123456');
  await ui.alpha.fill('128');
  await ui.add.click();
  await ui.hex.blur();
  await page.keyboard.press('d');
  await expect(ui.fg).toHaveValue('#000000');

  await ui.swatches.nth(1).click();
  await expect(ui.fg).toHaveValue('#222034');
  await expect(ui.alpha).toHaveValue('255');
  await expect(ui.editor.getByText('#1', { exact: true })).toBeVisible();

  await ui.swatches.nth(32).click();
  await expect(ui.fg).toHaveValue('#123456');
  await expect(ui.alpha).toHaveValue('128');
  await expect(ui.alphaShown).toContainText('128');
  await expect(ui.editor.getByText('#32', { exact: true })).toBeVisible();
});

test('right-clicking a swatch picks the background without a context menu', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const ui = paletteUi(page);
  await ui.hex.fill('#123456');
  await ui.alpha.fill('128');
  await ui.add.click();
  await ui.hex.blur();
  await page.keyboard.press('d');
  await expect(ui.fg).toHaveValue('#000000');
  await expect(ui.bg).toHaveValue('#ffffff');
  // Record whether the browser's context menu would open (not prevented).
  await page.evaluate(() => {
    const w = window as { menuPrevented?: boolean[] };
    w.menuPrevented = [];
    window.addEventListener('contextmenu', (e) => w.menuPrevented?.push(e.defaultPrevented));
  });

  await ui.swatches.nth(32).click({ button: 'right' });
  await expect(ui.bg).toHaveValue('#123456');
  await expect(ui.fg).toHaveValue('#000000');
  expect(
    await page.evaluate(() => (window as { menuPrevented?: boolean[] }).menuPrevented),
  ).toEqual([true]);
  // X brings the background (color and alpha) to the foreground controls.
  await page.keyboard.press('x');
  await expect(ui.fg).toHaveValue('#123456');
  await expect(ui.alpha).toHaveValue('128');
});

test('swatches matching the foreground RGB are marked active', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const ui = paletteUi(page);
  const pressed = ui.panel.locator('[aria-pressed="true"]');
  await ui.hex.fill('#000000');
  await expect(pressed).toHaveCount(1);
  await expect(ui.swatches.nth(0)).toHaveAttribute('aria-pressed', 'true');
  // Alpha is not compared.
  await ui.alpha.fill('10');
  await expect(ui.swatches.nth(0)).toHaveAttribute('aria-pressed', 'true');

  await ui.hex.fill('#222034');
  await expect(ui.swatches.nth(0)).toHaveAttribute('aria-pressed', 'false');
  await expect(ui.swatches.nth(1)).toHaveAttribute('aria-pressed', 'true');
  await expect(pressed).toHaveCount(1);
  // A second entry with the same RGB (alpha 10, entry 1 is opaque) is
  // marked too.
  await ui.add.click();
  await expect(ui.swatches).toHaveCount(33);
  await expect(pressed).toHaveCount(2);
  await expect(ui.swatches.nth(32)).toHaveAttribute('aria-pressed', 'true');
});

test('dragging a swatch onto another moves the entry there', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const { swatches } = paletteUi(page);
  // DB32 starts #000000, #222034, #45283c, #663931, #8f563b.
  await swatches.nth(0).dragTo(swatches.nth(3));
  const titles = ['#222034', '#45283c', '#663931', '#000000', '#8f563b'];
  for (const [i, css] of titles.entries()) {
    await expect(swatches.nth(i)).toHaveAttribute('title', new RegExp(css));
  }
  await expect(swatches).toHaveCount(32);
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

  test('the add button is disabled while the palette holds 256 entries', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    const { panel, swatches, add } = paletteUi(page);
    await panel.getByRole('button', { name: 'Palette options' }).click();
    const [chooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.getByRole('menuitem', { name: 'Import .gpl…' }).click(),
    ]);
    const rows = Array.from({ length: 255 }, (_, i) => `${i} 0 0`).join('\n');
    await chooser.setFiles({
      name: 'reds.gpl',
      mimeType: 'text/plain',
      buffer: Buffer.from(`GIMP Palette\n${rows}\n`),
    });
    await expect(swatches).toHaveCount(255);
    await expect(add).toBeEnabled();
    await add.click();
    await expect(swatches).toHaveCount(256);
    await expect(add).toBeDisabled();
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
