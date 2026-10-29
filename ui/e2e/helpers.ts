import { expect, type Locator, type Page } from '@playwright/test';

/** Opens the app on the start screen; `url` carries an optional query (`./?renderer=canvas2d`). */
export async function gotoStart(page: Page, url = './'): Promise<void> {
  await page.goto(url);
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** Creates a new document of `mode` through the start screen's New card. */
export async function createDocument(
  page: Page,
  mode: 'pixel' | 'image',
  size?: { width: number; height: number },
  url = './',
): Promise<void> {
  await gotoStart(page, url);
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
  await waitForEditor(page, mode);
}

/**
 * Waits until the editor of `mode` takes input: its document is loaded
 * into the core, the view is fitted and the canvas has drawn it. Both
 * editors mount their canvas (and menus, panels, key handlers) before the
 * core hands back a document — input or a `boundingBox()` taken before
 * `data-ready` hits a placeholder canvas or a document-less editor.
 */
export async function waitForEditor(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  // Huge canvases take a while to allocate on the unoptimized CI wasm.
  await expect(mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page)).toHaveAttribute(
    'data-ready',
    'true',
    { timeout: 60_000 },
  );
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
