import { expect, type Locator, type Page } from '@playwright/test';

/** Opens the app on the start screen. */
export async function gotoStart(page: Page): Promise<void> {
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** Creates a new document of `mode` through the start screen and the New dialog. */
export async function createDocument(
  page: Page,
  mode: 'pixel' | 'image',
  size?: { width: number; height: number },
): Promise<void> {
  await gotoStart(page);
  await page.getByRole('button', { name: mode === 'pixel' ? 'New pixel sprite' : 'New image' }).click();
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await expect(dialog).toBeVisible();
  if (size) {
    await dialog.getByLabel('Width').fill(String(size.width));
    await dialog.getByLabel('Height').fill(String(size.height));
  }
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(dialog).toBeHidden();
}

/** The Pixel-mode drawing surface. */
export function pixelCanvas(page: Page): Locator {
  return page.getByLabel('Pincel canvas');
}

/** The Image-mode drawing surface. */
export function imageCanvas(page: Page): Locator {
  return page.getByLabel('Image canvas');
}

/** Drags the primary pointer across `target` between two fractional positions. */
export async function drag(
  page: Page,
  target: Locator,
  from: [number, number],
  to: [number, number],
): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('drag target is not visible');
  const at = ([fx, fy]: [number, number]) => [box.x + box.width * fx, box.y + box.height * fy] as const;
  const [x0, y0] = at(from);
  const [x1, y1] = at(to);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 });
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
}
