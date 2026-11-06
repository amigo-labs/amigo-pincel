import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  solidPng,
  storedPref,
  waitForEditor,
} from './helpers';
import {
  area,
  check,
  checked,
  decimalUnit,
  expectTicksAligned,
  zoomTo,
} from './view-aids-helpers';

// image-view-aids: the view state is per user, survives a reload, follows
// tab switches, is no edit and never reaches a file; the units use each
// document's own resolution (docs/specs/image-view-aids.md AC-14, AC-15,
// AC-19, AC-20, AC-23, AC-25).

const tabsOf = (page: Page) =>
  page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');

/** File ▸ New… for a second document in the open session. */
async function newTab(
  page: Page,
  mode: 'pixel' | 'image',
  size?: { width: number; height: number },
): Promise<void> {
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: mode === 'image' ? /^Image/ : /^Pixel/ }).click();
  if (size) {
    await dialog.getByLabel('Width').fill(String(size.width));
    await dialog.getByLabel('Height').fill(String(size.height));
  }
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

const undo = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

test('view state persists per user, applies to every Image tab and is no edit (AC-14, AC-15)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  // On and off again through the menu and the keys: no undo step, no
  // unsaved changes.
  await check(page, 'View', 'Rulers');
  await check(page, 'View', 'Pixel Grid');
  await check(page, 'View', 'Units', 'Inches');
  await check(page, 'View', 'Rulers');
  await check(page, 'View', 'Pixel Grid');
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await expect(undo(page)).toBeDisabled();
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);

  await check(page, 'View', 'Rulers');
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await check(page, 'View', 'Units', 'Centimeters');
  await expect
    .poll(() => storedPref(page, 'imageView'))
    .toEqual({ showRulers: true, showPixelGrid: true, units: 'cm' });
  await expect(undo(page)).toBeDisabled();
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);

  // A second Image tab of another size: the states stay, the status follows.
  await newTab(page, 'image', { width: 96, height: 48 });
  await expect(page.getByText('2.54 × 1.27 cm · sRGB', { exact: true })).toBeVisible();
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  expect(await checked(page, 'Centimeters', 'Units')).toBe(true);
  await tabsOf(page).first().click();
  await expect(page.getByText('0.53 × 0.26 cm · sRGB', { exact: true })).toBeVisible();

  // After a reload every Image tab starts from the stored state.
  await createDocument(page, 'image', { width: 20, height: 10 });
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  expect(await checked(page, 'Centimeters', 'Units')).toBe(true);
  await newTab(page, 'image', { width: 96, height: 48 });
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  expect(await checked(page, 'Centimeters', 'Units')).toBe(true);
});

test('Pixel mode keeps its grids when an Image tab toggles its own (AC-14)', async ({ page }) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await newTab(page, 'pixel');
  const pixelShow = await checked(page, 'Show Pixel Grid');
  const pixelGrid = await checked(page, 'Show Grid');
  const pixelPref = await storedPref(page, 'pixelGrid');

  await tabsOf(page).first().click();
  await waitForEditor(page, 'image');
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await check(page, 'View', 'Pixel Grid');
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  await check(page, 'View', 'Pixel Grid');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await expect.poll(async () => storedPref(page, 'imageView')).toMatchObject({ showPixelGrid: true });
  expect(await storedPref(page, 'pixelGrid')).toEqual(pixelPref);

  await tabsOf(page).last().click();
  await waitForEditor(page, 'pixel');
  expect(await checked(page, 'Show Pixel Grid')).toBe(pixelShow);
  expect(await checked(page, 'Show Grid')).toBe(pixelGrid);
  expect(await storedPref(page, 'pixelGrid')).toEqual(pixelPref);
});

test.describe('opened and saved files', () => {
  test.beforeEach(async ({ page }) => {
    // Save falls back to a download named like the opened file.
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  /** File ▸ Save's download; retried while an earlier save finishes. */
  async function saved(page: Page): Promise<Buffer> {
    let bytes = Buffer.alloc(0);
    await expect(async () => {
      const [d] = await Promise.all([
        page.waitForEvent('download', { timeout: 2000 }),
        menu(page, 'File', 'Save'),
      ]);
      bytes = await readFile(await d.path());
    }).toPass();
    return bytes;
  }

  test('a document saved with every aid on is byte-identical to one saved with all off (AC-14)', async ({
    page,
  }) => {
    await gotoStart(page);
    await openPng(page, 'shot.png', await solidPng(page, 24, 16, '#3060c0'));
    const plain = await saved(page);
    await check(page, 'View', 'Rulers');
    await check(page, 'View', 'Pixel Grid');
    await check(page, 'View', 'Units', 'Centimeters');
    await zoomTo(page, 400);
    const aided = await saved(page);
    expect(plain.length).toBeGreaterThan(0);
    expect(aided.equals(plain)).toBe(true);
  });

  test('an opened PNG counts 96 DPI whatever resolution the file declares (AC-23)', async ({
    page,
  }) => {
    await gotoStart(page);
    const png = withDpi(await solidPng(page, 96, 48, '#c03030'), 300);
    expect(png.includes(Buffer.from('pHYs', 'latin1'))).toBe(true);
    await openPng(page, 'print.png', png);
    await check(page, 'View', 'Units', 'Inches');
    // 96 px at 96 DPI, not 0.32 in at 300 DPI.
    await expect(page.getByText('1.00 × 0.50 in · sRGB', { exact: true })).toBeVisible();
    await check(page, 'View', 'Rulers');
    await expect(page.getByTestId('ruler-v')).toBeVisible();
    await expect.poll(async () => (await imageCanvas(page).boundingBox())?.x ?? 0).toBeGreaterThan(
      ((await page.getByTestId('ruler-v').boundingBox())?.x ?? 0) + 20,
    );
    await expectTicksAligned(page, 96, decimalUnit('in', 96));
  });
});

/** Opens `bytes` as `name` through the start screen's Open…. */
async function openPng(page: Page, name: string, bytes: Buffer): Promise<void> {
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name, mimeType: 'image/png', buffer: bytes });
  await waitForEditor(page, 'image');
}

/** CRC-32 of a PNG chunk's type and data. */
function crc32(bytes: Buffer): number {
  let c = 0xffffffff;
  for (const b of bytes) {
    c ^= b;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  }
  return (c ^ 0xffffffff) >>> 0;
}

/** `png` with a pHYs chunk declaring `dpi` right after its IHDR. */
function withDpi(png: Buffer, dpi: number): Buffer {
  const perMeter = Math.round(dpi / 0.0254);
  const body = Buffer.alloc(13);
  body.write('pHYs', 0, 'latin1');
  body.writeUInt32BE(perMeter, 4);
  body.writeUInt32BE(perMeter, 8);
  body.writeUInt8(1, 12);
  const chunk = Buffer.alloc(4 + 13 + 4);
  chunk.writeUInt32BE(9, 0);
  body.copy(chunk, 4);
  chunk.writeUInt32BE(crc32(body), 17);
  // Signature (8) + IHDR (4 length + 4 type + 13 data + 4 CRC).
  const ihdrEnd = 8 + 25;
  return Buffer.concat([png.subarray(0, ihdrEnd), chunk, png.subarray(ihdrEnd)]);
}

test.describe('another resolution', () => {
  // No UI path stores a resolution other than 96 DPI yet, so the image
  // core's `document_dpi` export is wrapped (test-only, the app is
  // unchanged): it reports `__dpi[handle]` where the test set one.
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      type Exports = Record<string, unknown>;
      const w = window as unknown as {
        __dpi: Record<number, number>;
        __dpiHandles: number[];
      };
      w.__dpi = {};
      w.__dpiHandles = [];
      const wrap = (exports: Exports): Exports => {
        const real = exports.document_dpi;
        if (typeof real !== 'function') return exports;
        return {
          ...exports,
          document_dpi: (handle: number): unknown => {
            if (!w.__dpiHandles.includes(handle)) w.__dpiHandles.push(handle);
            const dpi = w.__dpi[handle];
            return dpi === undefined ? real(handle) : [dpi, 0, 0];
          },
        };
      };
      type Source = WebAssembly.Module | BufferSource;
      type Result = WebAssembly.WebAssemblyInstantiatedSource;
      // An instance the glue code takes for a real one (`instanceof`).
      const instance = (real: WebAssembly.Instance): WebAssembly.Instance =>
        Object.defineProperty(Object.create(WebAssembly.Instance.prototype), 'exports', {
          value: wrap(real.exports as Exports),
        }) as WebAssembly.Instance;
      const patch = (r: Result): Result => ({ module: r.module, instance: instance(r.instance) });
      const streaming = WebAssembly.instantiateStreaming.bind(WebAssembly);
      WebAssembly.instantiateStreaming = async (source, imports) =>
        patch(await streaming(source, imports));
      const instantiate = WebAssembly.instantiate.bind(WebAssembly) as (
        s: Source,
        i?: WebAssembly.Imports,
      ) => Promise<Result | WebAssembly.Instance>;
      (WebAssembly as { instantiate: unknown }).instantiate = async (
        source: Source,
        imports?: WebAssembly.Imports,
      ) => {
        const r = await instantiate(source, imports);
        return r instanceof WebAssembly.Instance ? instance(r) : patch(r);
      };
    });
  });

  /** Image handles in the order the editor first asked their resolution. */
  const handles = (page: Page) =>
    page.evaluate(() => (window as unknown as { __dpiHandles: number[] }).__dpiHandles);
  const setDpi = (page: Page, handle: number, dpi: number) =>
    page.evaluate(
      ([h, d]) => {
        (window as unknown as { __dpi: Record<number, number> }).__dpi[h] = d;
      },
      [handle, dpi] as const,
    );

  test('the rulers and the status bar take the resolution again on every tab switch and document change (AC-19, AC-23)', async ({
    page,
  }) => {
    test.setTimeout(120_000);
    await createDocument(page, 'image', { width: 300, height: 150 });
    await check(page, 'View', 'Units', 'Inches');
    await check(page, 'View', 'Rulers');
    // 96 DPI until a resolution is set.
    await expect(page.getByText('3.13 × 1.56 in · sRGB', { exact: true })).toBeVisible();
    await newTab(page, 'image', { width: 384, height: 192 });
    await expect(page.getByText('4.00 × 2.00 in · sRGB', { exact: true })).toBeVisible();
    const [a, b] = await handles(page);
    if (a === undefined || b === undefined) throw new Error('two documents expected');
    await setDpi(page, a, 300);
    await setDpi(page, b, 192);

    // Each tab switch reads the active document's resolution.
    await tabsOf(page).first().click();
    await expect(page.getByText('1.00 × 0.50 in · sRGB', { exact: true })).toBeVisible();
    await expectTicksAligned(page, 300, decimalUnit('in', 300));
    await tabsOf(page).last().click();
    await expect(page.getByText('2.00 × 1.00 in · sRGB', { exact: true })).toBeVisible();
    await expectTicksAligned(page, 384, decimalUnit('in', 192));

    // A document change reads it again.
    await setDpi(page, b, 96);
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.6, 0.5]);
    await expect(undo(page)).toBeEnabled();
    await expect(page.getByText('4.00 × 2.00 in · sRGB', { exact: true })).toBeVisible();
    await expectTicksAligned(page, 384, decimalUnit('in', 96));
  });
});

test('a tab switch moves the rulers and the pixel grid to that tab’s view (AC-19)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  // Tab A zoomed to 400 % or more, tab B fitted below it.
  await createDocument(page, 'image', { width: 200, height: 100 });
  await check(page, 'View', 'Rulers');
  await zoomTo(page, 400);
  await newTab(page, 'image', { width: 800, height: 400 });
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  const grid = await checked(page, 'Pixel Grid');
  expect(grid).toBe(true);

  /** Whether the pixel grid shows: the canvas area differs with it off. */
  async function gridShown(): Promise<boolean> {
    await page.mouse.move(0, 0);
    const on = await area(page).screenshot();
    await page.keyboard.press('ControlOrMeta+Shift+Quote');
    await page.mouse.move(0, 0);
    const off = await area(page).screenshot();
    await page.keyboard.press('ControlOrMeta+Shift+Quote');
    return !on.equals(off);
  }

  expect(await gridShown()).toBe(false);
  await expectTicksAligned(page, 800);
  await tabsOf(page).first().click();
  await expect(page.getByText('200 × 100 px · sRGB', { exact: true })).toBeVisible();
  expect(await gridShown()).toBe(true);
  await expectTicksAligned(page, 200);
  await tabsOf(page).last().click();
  await expect(page.getByText('800 × 400 px · sRGB', { exact: true })).toBeVisible();
  expect(await gridShown()).toBe(false);
  await expectTicksAligned(page, 800);
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  expect(await checked(page, 'Rulers')).toBe(true);
});

test('a malformed pref or a failing store keeps the editor working (AC-20, AC-25)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({ key: 'imageView', value: { units: 'furlong' } });
          tx.oncomplete = () => resolve();
        };
      }),
  );
  await createDocument(page, 'image', { width: 20, height: 10 });
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  expect(await checked(page, 'Pixels', 'Units')).toBe(true);
  expect(await checked(page, 'Pixel Grid')).toBe(false);

  // Store A: rulers on.
  await check(page, 'View', 'Rulers');
  await expect
    .poll(() => storedPref(page, 'imageView'))
    .toEqual({ showRulers: true, showPixelGrid: false, units: 'px' });

  // Every write fails from now on: changes still apply, silently.
  await page.evaluate(() => {
    IDBObjectStore.prototype.put = () => {
      throw new DOMException('blocked', 'InvalidStateError');
    };
  });
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await check(page, 'View', 'Units', 'Centimeters');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await expect(page.getByText('0.53 × 0.26 cm · sRGB', { exact: true })).toBeVisible();
  await expect(page.getByText(/blocked|failed/i)).toHaveCount(0);
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await expect(undo(page)).toBeEnabled();

  // The next launch starts from A, the last state stored.
  await createDocument(page, 'image', { width: 20, height: 10 });
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  expect(await checked(page, 'Pixels', 'Units')).toBe(true);
});

test.describe('without IndexedDB', () => {
  test('the defaults apply and the editor works (AC-25)', async ({ page }) => {
    // A stored state the next launch cannot read.
    await createDocument(page, 'image', { width: 20, height: 10 });
    await check(page, 'View', 'Rulers');
    await expect.poll(() => storedPref(page, 'imageView')).toMatchObject({ showRulers: true });
    await page.addInitScript(() => {
      Object.defineProperty(window, 'indexedDB', { value: undefined, configurable: true });
    });
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    await createDocument(page, 'image', { width: 20, height: 10 });
    await expect(page.getByTestId('ruler-h')).toHaveCount(0);
    expect(await checked(page, 'Pixel Grid')).toBe(false);
    expect(await checked(page, 'Pixels', 'Units')).toBe(true);
    // Toggles still work for the session, silently.
    await check(page, 'View', 'Rulers');
    await expect(page.getByTestId('ruler-h')).toBeVisible();
    await check(page, 'View', 'Units', 'Inches');
    await expect(page.getByText('0.21 × 0.10 in · sRGB', { exact: true })).toBeVisible();
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
    await expect(undo(page)).toBeEnabled();
    await expect(page.getByText(/failed/i)).toHaveCount(0);
    expect(errors).toEqual([]);
  });
});
