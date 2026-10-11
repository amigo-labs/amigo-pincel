import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

test('palette: DB32 by default, add / recolor / move / delete with undo', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const panel = page.getByRole('complementary', { name: 'Palette' });
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
