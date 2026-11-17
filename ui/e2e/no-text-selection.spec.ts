import { expect, test } from '@playwright/test';
import { gotoStart } from './helpers';

const selectedText = (page: import('@playwright/test').Page): Promise<string> =>
  page.evaluate(() => window.getSelection()?.toString() ?? '');

test('UI text cannot be selected by triple-click or drag', async ({ page }) => {
  await gotoStart(page);
  const heading = page.getByRole('heading', { name: 'Pincel', exact: true });

  await heading.click({ clickCount: 3 });
  expect(await selectedText(page)).toBe('');

  const box = await heading.boundingBox();
  if (!box) throw new Error('heading has no box');
  await page.mouse.move(box.x + 1, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 600, box.y + 400, { steps: 8 });
  await page.mouse.up();
  expect(await selectedText(page)).toBe('');
});

test('text fields stay selectable', async ({ page }) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'New image' }).click();
  const field = page.getByRole('form', { name: 'New image' }).getByRole('spinbutton').first();
  await expect(field).toBeVisible();
  expect(await field.evaluate((el) => getComputedStyle(el).userSelect)).not.toBe('none');
});
