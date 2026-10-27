import { expect, type Locator, type Page } from '@playwright/test';

/** Opens the app on the start screen. */
export async function gotoStart(page: Page): Promise<void> {
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** Creates a new document of `mode` through the start screen's New card. */
export async function createDocument(
  page: Page,
  mode: 'pixel' | 'image',
  size?: { width: number; height: number },
): Promise<void> {
  await gotoStart(page);
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  await expect(card).toBeVisible();
  if (size) {
    await card.getByLabel('Width').fill(String(size.width));
    await card.getByLabel('Height').fill(String(size.height));
  }
  await card.getByRole('button', { name: 'Create' }).click();
  await expect(card).toBeHidden();
  // The editor's core is loaded once the top bar names the document.
  // (Huge canvases take a while to allocate on the unoptimized CI wasm.)
  await expect(page.getByRole('banner').getByText('No document')).toHaveCount(0, {
    timeout: 60_000,
  });
  await expect(page.getByRole('menubar').or(page.getByRole('button', { name: 'Menu' }))).toBeVisible();
  // The top bar names the document a moment before the editor mounts its
  // drawing surface — Image mode keeps the stage at `display: none` until
  // the core hands back a document. Wait for the surface, or the first
  // `drag()` of a spec races it and reads a null bounding box.
  // Same generous budget as the banner above: a huge canvas takes a while
  // to allocate on the unoptimized CI wasm.
  await expect(mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page)).toBeVisible({
    timeout: 60_000,
  });
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
  await target.waitFor({ state: 'visible' });
  const box = await target.boundingBox();
  if (!box) throw new Error('drag target is not visible');
  const at = ([fx, fy]: [number, number]) =>
    [box.x + box.width * fx, box.y + box.height * fy] as const;
  const [x0, y0] = at(from);
  const [x1, y1] = at(to);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 });
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
}

/**
 * Runs a menu-bar command: `menu(page, 'File', 'Export', 'PNG')` opens
 * File, then the Export submenu, then clicks PNG.
 */
export async function menu(page: Page, top: string, ...path: string[]): Promise<void> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  let scope = top;
  for (const label of path) {
    await page
      .getByRole('menu', { name: scope, exact: true })
      .getByRole('menuitem', { name: label, exact: true })
      .click();
    scope = label;
  }
}
