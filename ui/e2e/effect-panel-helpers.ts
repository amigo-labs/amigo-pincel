import type { Locator, Page } from '@playwright/test';

// Shared steps of the effect-panel specs: the dialog and its title bar, the
// stored `layout:<mode>` pref, region brightness and box geometry.

export type Mode = 'pixel' | 'image';

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The open effect's dialog, found by the effect's name. */
export function effectPanel(page: Page, title: string): Locator {
  return page.getByRole('dialog', { name: title });
}

/** The panel's title bar: the handle that moves it. */
export function titleBar(panel: Locator, name = 'Move dialog'): Locator {
  return panel.getByRole('group', { name, exact: true });
}

/** The bounding box of `target`, or a test failure when it is hidden. */
export async function boxOf(target: Locator): Promise<Box> {
  const box = await target.boundingBox();
  if (!box) throw new Error('element is not visible');
  return box;
}

/** The width of `target`, or `null` while it has no box: for `expect.poll`
 *  across a layout change, where the frame re-mounts the effect panel and
 *  `boundingBox()` of the element it resolved first comes back `null` once
 *  that element is detached. A throw would end the poll at once. */
export async function widthOf(target: Locator): Promise<number | null> {
  return (await target.boundingBox())?.width ?? null;
}

/** True when `inner` lies wholly inside `outer`; `false` while either has
 *  no box, so `expect.poll` keeps polling through a re-mount (see `widthOf`). */
export async function liesInside(inner: Locator, outer: Locator): Promise<boolean> {
  const [a, b] = await Promise.all([inner.boundingBox(), outer.boundingBox()]);
  return a !== null && b !== null && inside(a, b);
}

/** True when `inner` lies wholly inside `outer` (1 px tolerance). */
export function inside(inner: Box, outer: Box): boolean {
  return (
    inner.x >= outer.x - 1 &&
    inner.y >= outer.y - 1 &&
    inner.x + inner.width <= outer.x + outer.width + 1 &&
    inner.y + inner.height <= outer.y + outer.height + 1
  );
}

/** Drags with the mouse from a point on `target`'s left part by `dx`, `dy`. */
export async function dragBy(page: Page, target: Locator, dx: number, dy: number): Promise<void> {
  const box = await boxOf(target);
  const x = box.x + 24;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y + dy / 2, { steps: 4 });
  await page.mouse.move(x + dx, y + dy, { steps: 4 });
  await page.mouse.up();
}

/** The stored `layout:<mode>` pref, read straight from IndexedDB. */
export function storedLayout(page: Page, mode: Mode): Promise<unknown> {
  return page.evaluate(
    (key) =>
      new Promise<unknown>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const get = open.result.transaction('prefs').objectStore('prefs').get(key);
          get.onsuccess = () => resolve((get.result as { value?: unknown } | undefined)?.value);
          get.onerror = () => resolve(null);
        };
        open.onerror = () => resolve(null);
      }),
    `layout:${mode}`,
  );
}

/** Writes `value` as the stored `layout:<mode>` pref. */
export function storeLayout(page: Page, mode: Mode, value: unknown): Promise<void> {
  return page.evaluate(
    ([key, v]) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({ key, value: v });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(new Error('pref write failed'));
        };
        open.onerror = () => reject(new Error('IndexedDB unavailable'));
      }),
    [`layout:${mode}`, value] as const,
  );
}

/** The stored effect-panel position of `mode`, if any. */
export async function storedPos(page: Page, mode: Mode): Promise<unknown> {
  return ((await storedLayout(page, mode)) as { effectPos?: unknown } | undefined)?.effectPos;
}

/** Mean brightness (0–255) of `target` as it shows on screen. */
export async function brightness(page: Page, target: Locator): Promise<number> {
  const png = await target.screenshot({ animations: 'disabled' });
  return page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bmp = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bmp.width, bmp.height);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bmp, 0, 0);
    const data = ctx.getImageData(0, 0, bmp.width, bmp.height).data;
    let sum = 0;
    for (let i = 0; i < data.length; i += 4) sum += (data[i] ?? 0) + (data[i + 1] ?? 0) + (data[i + 2] ?? 0);
    return sum / ((data.length / 4) * 3);
  }, png.toString('base64'));
}

/** Runs a menu path from the phone ☰ sheet: top-level menus are headings
 *  there, submenus expand inline. */
export async function phoneMenu(page: Page, ...path: string[]): Promise<void> {
  // "Menu" / German "Menü".
  await page.getByRole('button', { name: /^Men[uü]$/ }).click();
  const sheet = page.getByRole('menu', { name: /^Men[uü]$/ });
  for (const label of path) {
    await sheet
      .getByRole('menuitem', { name: label, exact: true })
      .or(sheet.getByRole('menuitemcheckbox', { name: label, exact: true }))
      .click();
  }
}
