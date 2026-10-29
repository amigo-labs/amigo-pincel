import { expect, test, type Locator, type Page } from '@playwright/test';

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

/** Upper bound for one editor start-up (`waitForEditor`). */
const EDITOR_START_BUDGET = 60_000;

/**
 * Waits until the editor of `mode` takes input: its document is loaded
 * into the core, the view is fitted and the canvas has drawn it. Both
 * editors mount their canvas (and menus, panels, key handlers) before the
 * core hands back a document — input or a `boundingBox()` taken before
 * `data-ready` hits a placeholder canvas or a document-less editor.
 */
export async function waitForEditor(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  // Editor start-up has its own budget: fetching and compiling the
  // unoptimized wasm (5–9 MiB) per browser context takes 10–15 s once a
  // dozen workers share the CPU, and a huge canvas takes a while to
  // allocate. Lend the test that budget while waiting, then charge it
  // only the time start-up actually took, so the test timeout keeps
  // bounding the spec's own steps.
  const info = test.info();
  const base = info.timeout;
  info.setTimeout(base + EDITOR_START_BUDGET);
  const started = Date.now();
  await expect(mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page)).toHaveAttribute(
    'data-ready',
    'true',
    { timeout: EDITOR_START_BUDGET },
  );
  info.setTimeout(base + (Date.now() - started));
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
 * Draws a real (CDP) stylus stroke across the middle row of `target`,
 * from 10 % to 90 % of its width, at pen pressure `force` and with the
 * given tilt. `pointerType: 'mouse'` sends the same path as a mouse,
 * which carries no pressure.
 */
export async function penStroke(
  page: Page,
  target: Locator,
  force: number,
  tilt: { tiltX: number; tiltY: number } = { tiltX: 0, tiltY: 0 },
  pointerType: 'pen' | 'mouse' = 'pen',
): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  const cdp = await page.context().newCDPSession(page);
  const at = (fx: number) => ({ x: box.x + box.width * fx, y: box.y + box.height * 0.5 });
  const send = (type: 'mousePressed' | 'mouseMoved' | 'mouseReleased', fx: number) =>
    cdp.send('Input.dispatchMouseEvent', {
      type,
      ...at(fx),
      button: 'left',
      buttons: type === 'mouseReleased' ? 0 : 1,
      clickCount: 1,
      pointerType,
      force,
      ...tilt,
    });
  await send('mousePressed', 0.1);
  for (const fx of [0.3, 0.5, 0.7, 0.9]) await send('mouseMoved', fx);
  await send('mouseReleased', 0.9);
  await cdp.detach();
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

/** A decoded clipboard image: size plus straight RGBA bytes. */
export interface ClipImage {
  width: number;
  height: number;
  data: number[];
}

/** RGBA of pixel `(x, y)` of a decoded clipboard image. */
export function pixelOf(img: ClipImage, x: number, y: number): number[] {
  const i = (y * img.width + x) * 4;
  return img.data.slice(i, i + 4);
}

/**
 * The `image/png` on the system clipboard, decoded test-side (the app never
 * reads canvas pixels); `null` when the clipboard holds no PNG. Needs the
 * `clipboard-read` permission.
 */
export async function readClipboardImage(page: Page): Promise<ClipImage | null> {
  return page.evaluate(async () => {
    for (const item of await navigator.clipboard.read()) {
      if (!item.types.includes('image/png')) continue;
      const bitmap = await createImageBitmap(await item.getType('image/png'), {
        premultiplyAlpha: 'none',
        colorSpaceConversion: 'none',
      });
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
      return { width: bitmap.width, height: bitmap.height, data: Array.from(data) };
    }
    return null;
  });
}

/** Replaces the system clipboard with plain text, so no image is on it. */
export async function clearClipboard(page: Page): Promise<void> {
  await page.evaluate(() => navigator.clipboard.writeText('-'));
}

/**
 * Waits for the image an app Copy / Cut writes (the write is async) and
 * returns it decoded. Call `clearClipboard` before the copy.
 */
export async function copiedImage(page: Page): Promise<ClipImage> {
  let img: ClipImage | null = null;
  await expect
    .poll(async () => {
      img = await readClipboardImage(page);
      return img !== null;
    })
    .toBe(true);
  if (!img) throw new Error('no image on the clipboard');
  return img;
}

/** Puts a `width` × `height` PNG filled with `color` on the system clipboard. */
export async function putClipboardImage(
  page: Page,
  width: number,
  height: number,
  color: string,
): Promise<void> {
  await page.evaluate(
    async ([w, h, c]) => {
      const canvas = new OffscreenCanvas(w, h);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = c;
      ctx.fillRect(0, 0, w, h);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
    },
    [width, height, color] as const,
  );
}
