import { expect, test, type Locator, type Page } from '@playwright/test';

/** Opens the app on the start screen; `url` carries an optional query (`./?renderer=canvas2d`). */
export async function gotoStart(page: Page, url = './'): Promise<void> {
  await page.goto(url);
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
}

/**
 * The value of the IndexedDB pref `key`, read straight from the `prefs`
 * store (`undefined` when unset). Prefs are written fire-and-forget, so a
 * spec that reloads or navigates right after a change polls this first, or
 * the navigation can abort the write.
 */
export async function storedPref(page: Page, key: string): Promise<unknown> {
  return page.evaluate(
    (k) =>
      new Promise<unknown>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const get = open.result.transaction('prefs').objectStore('prefs').get(k);
          get.onsuccess = () => resolve((get.result as { value?: unknown } | undefined)?.value);
          get.onerror = () => resolve(null);
        };
        open.onerror = () => resolve(null);
      }),
    key,
  );
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
    // The dialog sizes a new image from a clipboard image asynchronously (the
    // clipboard outlives the test within a worker's browser). A prefill that
    // lands inside `fill()` drops its select-all and the typed digits append
    // ("3232"), so fill until the field holds the value; once it differs from
    // the default, the prefill leaves the size alone.
    for (const [label, value] of [
      ['Width', String(size.width)],
      ['Height', String(size.height)],
    ] as const) {
      const field = card.getByLabel(label);
      await expect(async () => {
        await field.fill(value);
        await expect(field).toHaveValue(value, { timeout: 500 });
      }).toPass();
    }
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

/** Raster formats the Image editor opens (besides OpenRaster). */
export type RasterKind = 'png' | 'jpeg' | 'webp' | 'bmp' | 'tiff' | 'gif';

/** A 24-bit 4×3 BMP, bottom-up rows padded to 4 bytes. */
function bmpFile(): Buffer {
  const [w, h] = [4, 3];
  const row = Math.ceil((w * 3) / 4) * 4;
  const buf = Buffer.alloc(54 + row * h);
  buf.write('BM', 0, 'latin1');
  buf.writeUInt32LE(buf.length, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(w, 18);
  buf.writeInt32LE(h, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(row * h, 34);
  for (let i = 54; i < buf.length; i += 1) buf[i] = 0x80;
  return buf;
}

/** An uncompressed 8-bit grayscale 4×3 TIFF (little endian). */
function tiffFile(): Buffer {
  const [w, h] = [4, 3];
  const entries: [number, number, number, number][] = [
    // tag, type (3 = SHORT, 4 = LONG), count, value
    [256, 3, 1, w],
    [257, 3, 1, h],
    [258, 3, 1, 8],
    [259, 3, 1, 1],
    [262, 3, 1, 1],
    [273, 4, 1, 0], // strip offset, patched below
    [277, 3, 1, 1],
    [278, 3, 1, h],
    [279, 4, 1, w * h],
  ];
  const dataAt = 8 + 2 + entries.length * 12 + 4;
  const buf = Buffer.alloc(dataAt + w * h, 0x60);
  buf.write('II*\0', 0, 'latin1');
  buf.writeUInt32LE(8, 4);
  buf.writeUInt16LE(entries.length, 8);
  entries.forEach(([tag, type, count, value], i) => {
    const o = 10 + i * 12;
    buf.writeUInt16LE(tag, o);
    buf.writeUInt16LE(type, o + 2);
    buf.writeUInt32LE(count, o + 4);
    const v = tag === 273 ? dataAt : value;
    if (type === 3) buf.writeUInt16LE(v, o + 8);
    else buf.writeUInt32LE(v, o + 8);
  });
  buf.writeUInt32LE(0, 10 + entries.length * 12);
  return buf;
}

/** A 1×1 GIF. */
const GIF_FILE = Buffer.from(
  'R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==',
  'base64',
);

/** Encodes an 8×6 test image in the browser (PNG / JPEG / WebP). */
async function canvasFile(page: Page, mime: string): Promise<Buffer> {
  const bytes = await page.evaluate(async (type) => {
    const canvas = new OffscreenCanvas(8, 6);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.fillStyle = '#c03030';
    ctx.fillRect(0, 0, 8, 6);
    ctx.fillStyle = '#3050c0';
    ctx.fillRect(2, 1, 4, 3);
    const blob = await canvas.convertToBlob({ type, quality: 0.9 });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  }, mime);
  return Buffer.from(bytes);
}

export const RASTER_MIME: Record<RasterKind, string> = {
  png: 'image/png',
  jpeg: 'image/jpeg',
  webp: 'image/webp',
  bmp: 'image/bmp',
  tiff: 'image/tiff',
  gif: 'image/gif',
};

/** A small sample file of a raster format the Image editor opens. */
export async function rasterFile(page: Page, kind: RasterKind): Promise<Buffer> {
  if (kind === 'bmp') return bmpFile();
  if (kind === 'tiff') return tiffFile();
  if (kind === 'gif') return GIF_FILE;
  return canvasFile(page, RASTER_MIME[kind]);
}
