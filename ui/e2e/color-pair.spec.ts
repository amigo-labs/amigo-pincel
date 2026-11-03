import { expect, test, type Page } from '@playwright/test';
import { createDocument, storedPref } from './helpers';

// The Pixel editor's foreground / background pair (docs/specs/pixel-palette.md).

function colorPair(page: Page) {
  const alpha = page.getByRole('slider', { name: 'Foreground alpha' });
  return {
    fg: page.getByLabel('Foreground color', { exact: true }),
    bg: page.getByLabel('Background color', { exact: true }),
    hex: page.getByLabel('Foreground color hex value'),
    alpha,
    alphaShown: page.locator('label', { has: alpha }),
  };
}

test('D resets to opaque black / white, X swaps colors and alphas', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const c = colorPair(page);
  await c.hex.fill('#112233');
  await c.hex.blur();
  await c.alpha.fill('100');
  await c.bg.fill('#445566');

  await page.keyboard.press('x');
  await expect(c.fg).toHaveValue('#445566');
  await expect(c.bg).toHaveValue('#112233');
  await expect(c.alpha).toHaveValue('255');
  await page.keyboard.press('x');
  await expect(c.fg).toHaveValue('#112233');
  await expect(c.bg).toHaveValue('#445566');
  await expect(c.alpha).toHaveValue('100');

  // Make both alphas translucent (background 100, foreground 50), then reset.
  await page.keyboard.press('x');
  await c.alpha.fill('50');
  await page.keyboard.press('d');
  await expect(c.fg).toHaveValue('#000000');
  await expect(c.bg).toHaveValue('#ffffff');
  await expect(c.alpha).toHaveValue('255');
  await page.keyboard.press('x');
  await expect(c.fg).toHaveValue('#ffffff');
  await expect(c.alpha).toHaveValue('255');
});

test('the hex field sets the foreground from a full hex value only', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const c = colorPair(page);
  await c.hex.click();
  await c.hex.press('ControlOrMeta+a');
  await c.hex.pressSequentially('#a1b2c3');
  await expect(c.fg).toHaveValue('#a1b2c3');
  await c.hex.fill('00FF00');
  await expect(c.fg).toHaveValue('#00ff00');

  await c.hex.fill('#12');
  await expect(c.hex).toHaveValue('#12');
  await expect(c.fg).toHaveValue('#00ff00');
  await c.hex.fill('12345');
  await expect(c.fg).toHaveValue('#00ff00');
  await c.hex.blur();
  await expect(c.hex).toHaveValue('#00ff00');
});

test('the alpha slider sets the foreground alpha and shows it', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const c = colorPair(page);
  await expect(c.alpha).toHaveValue('255');
  await expect(c.alphaShown).toContainText('255');
  await c.alpha.fill('128');
  await expect(c.alphaShown).toContainText('128');
  await c.alpha.press('ArrowLeft');
  await expect(c.alpha).toHaveValue('127');
  await expect(c.alphaShown).toContainText('127');
  await c.alpha.fill('0');
  await expect(c.alphaShown).toContainText('0');
  // The foreground alpha is what the palette's + appends.
  await page.getByRole('button', { name: 'Add current color to palette' }).click();
  await page.keyboard.press('d');
  await expect(c.alpha).toHaveValue('255');
  const swatches = page
    .getByRole('region', { name: 'Palette' })
    .getByRole('group', { name: 'Palette colors' })
    .getByRole('button');
  await swatches.nth(32).click();
  await expect(c.alpha).toHaveValue('0');
});

test('foreground and background colors survive a reload, alphas reset', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const c = colorPair(page);
  await c.hex.fill('#123456');
  await c.hex.blur();
  await c.bg.fill('#abcdef');
  await c.alpha.fill('100');
  await expect.poll(() => storedPref(page, 'lastColor')).toBe('#123456');
  await expect.poll(() => storedPref(page, 'lastBgColor')).toBe('#abcdef');

  await page.reload();
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await expect(c.fg).toHaveValue('#123456');
  await expect(c.bg).toHaveValue('#abcdef');
  await expect(c.alpha).toHaveValue('255');
  await page.keyboard.press('x');
  await expect(c.alpha).toHaveValue('255');
});
