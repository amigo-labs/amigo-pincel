import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, dragFiles, gotoStart, imageCanvas, menu, rasterFile, waitForEditor } from './helpers';

// TGA, DDS and ICO in Image mode (docs/specs/image-formats-more.md). The
// files are built here byte by byte; saves fall back to downloads.

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

/** A 4 × 4 uncompressed 32-bit TGA of one color (top-left origin). */
function tga([r, g, b, a]: number[]): Buffer {
  const header = [0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 4, 0, 32, 0x28];
  const px: number[] = [];
  for (let i = 0; i < 16; i += 1) px.push(b ?? 0, g ?? 0, r ?? 0, a ?? 255);
  return Buffer.from([...header, ...px]);
}

/** A 4 × 4 DDS of solid red with FourCC `fourCc` (and DXGI `dxgi` for DX10),
 *  `w` × `h` declared. */
function dds(fourCc = 'DXT1', dxgi?: number, w = 4, h = 4): Buffer {
  const header = Buffer.alloc(124);
  header.writeUInt32LE(124, 0);
  header.writeUInt32LE(0x81007, 4);
  header.writeUInt32LE(h, 8);
  header.writeUInt32LE(w, 12);
  header.writeUInt32LE(32, 72);
  header.writeUInt32LE(4, 76);
  header.write(fourCc, 80, 'latin1');
  header.writeUInt32LE(0x1000, 104);
  const dx10 = Buffer.alloc(dxgi === undefined ? 0 : 20);
  if (dxgi !== undefined) {
    dx10.writeUInt32LE(dxgi, 0);
    dx10.writeUInt32LE(3, 4);
    dx10.writeUInt32LE(1, 12);
  }
  const blocks = Math.max(1, Math.ceil(w / 4) * Math.ceil(h / 4));
  const data = Buffer.alloc(16 * blocks);
  for (let i = 0; i < blocks; i += 1) data.writeUInt16LE(0xf800, i * 8);
  return Buffer.concat([Buffer.from('DDS '), header, dx10, data]);
}

/** An ICO with the given PNG entries ([edge, declared bit depth, png]). */
function ico(entries: Array<[number, number, Buffer]>): Buffer {
  const head = Buffer.alloc(6 + 16 * entries.length);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(entries.length, 4);
  let offset = head.length;
  entries.forEach(([edge, bpp, png], i) => {
    const at = 6 + 16 * i;
    head.writeUInt8(edge % 256, at);
    head.writeUInt8(edge % 256, at + 1);
    head.writeUInt16LE(1, at + 4);
    head.writeUInt16LE(bpp, at + 6);
    head.writeUInt32LE(png.length, at + 8);
    head.writeUInt32LE(offset, at + 12);
    offset += png.length;
  });
  return Buffer.concat([head, ...entries.map(([, , png]) => png)]);
}

/** A solid PNG of `w` × `h`, encoded by the browser. */
async function solidPng(page: Page, w: number, h: number, color: string): Promise<Buffer> {
  const bytes = await page.evaluate(
    async ({ w, h, color }) => {
      const c = new OffscreenCanvas(w, h);
      const ctx = c.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = color;
      ctx.fillRect(0, 0, w, h);
      return Array.from(new Uint8Array(await (await c.convertToBlob()).arrayBuffer()));
    },
    { w, h, color },
  );
  return Buffer.from(bytes);
}

/** Opens `name` through the start screen's Open… (input fallback). */
async function openFile(page: Page, name: string, buffer: Buffer, ok = true): Promise<void> {
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name, mimeType: 'application/octet-stream', buffer });
  if (ok) await waitForEditor(page, 'image');
}

/** The color shown at the canvas center (screen read). */
async function center(page: Page): Promise<string> {
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(async (b64) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
    const c = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = c.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bitmap, 0, 0);
    const [r = 0, g = 0, b = 0] = ctx.getImageData(bitmap.width >> 1, bitmap.height >> 1, 1, 1).data;
    return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
  }, png.toString('base64'));
}

async function download(page: Page, action: () => Promise<void>): Promise<{ name: string; bytes: Buffer }> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return { name: d.suggestedFilename(), bytes: await readFile(await d.path()) };
}

const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });

test('a TGA opens with its pixels and is saved back as TGA (AC-01, AC-06)', async ({ page }) => {
  await openFile(page, 'Shot.TGA', tga([0, 0, 255, 255]));
  await expect(page.getByTitle('Shot.TGA', { exact: true })).toBeVisible();
  await expect.poll(() => center(page)).toBe('#0000ff');
  await expect(page.getByRole('button', { name: 'Hide Background' })).toHaveCount(1);
  // Paint nothing: Save writes the file back in its format.
  const written = await download(page, () => menu(page, 'File', 'Save'));
  expect(written.name).toBe('Shot.TGA');
  expect(written.bytes[2]).toBe(10); // RLE true-colour TGA
});

test('a DDS opens whatever its extension; Save goes to Save As (AC-02, AC-04, AC-09)', async ({
  page,
}) => {
  await openFile(page, 'texture.bin', dds('DX10', 71));
  await expect.poll(() => center(page)).toBe('#ff0000');
  await menu(page, 'File', 'Save');
  const dialog = page.getByRole('dialog', { name: 'Save As' });
  await expect(dialog).toContainText('DDS files cannot be written back');
});

test('an ICO opens its deepest, then largest entry and saves back as ICO (AC-03, AC-07)', async ({
  page,
}) => {
  await gotoStart(page);
  const file = ico([
    [64, 8, await solidPng(page, 64, 64, '#00ff00')],
    [32, 32, await solidPng(page, 32, 32, '#ff00ff')],
    [16, 32, await solidPng(page, 16, 16, '#0000ff')],
  ]);
  await openFile(page, 'app.ico', file);
  await expect.poll(() => center(page)).toBe('#ff00ff');
  const written = await download(page, () => menu(page, 'File', 'Save'));
  expect(written.name).toBe('app.ico');
  expect([...written.bytes.subarray(0, 4)]).toEqual([0, 0, 1, 0]);
});

test('a TGA without its extension is refused; another signature wins over .tga (AC-04, AC-12)', async ({
  page,
}) => {
  await openFile(page, 'noext', tga([0, 0, 255, 255]), false);
  await expect(page.getByText(/noext/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeVisible();
  const png = await rasterFile(page, 'png');
  await openFile(page, 'fake.tga', png);
  await expect(page.getByTitle('fake.tga', { exact: true })).toBeVisible();
});

test('DDS variants outside BC1 – BC3 and odd sizes fail with their own reason (AC-11)', async ({
  page,
}) => {
  await openFile(page, 'bc7.dds', dds('DX10', 98), false);
  await expect(page.getByText(/DDS variant is not supported/)).toBeVisible();
  await openFile(page, 'odd.dds', dds('DXT1', undefined, 30, 30), false);
  await expect(page.getByText(/multiples of 4/)).toBeVisible();
});

test('Save As offers TGA and ICO; ICO refuses more than 256 px (AC-08, AC-10)', async ({ page }) => {
  await createDocument(page, 'image', { width: 300, height: 40 });
  await menu(page, 'File', 'Save As…');
  const dialog = page.getByRole('dialog', { name: 'Save As' });
  const format = dialog.getByLabel('Format');
  await expect(format.locator('option')).toContainText(['TGA', 'ICO (Windows icon)']);
  await format.selectOption('tga');
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(written.name).toBe('image.tga');

  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await menu(page, 'File', 'Save As…');
  await format.selectOption('ico');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('ICO holds at most 256 × 256 px. Choose another format.')).toBeVisible();
  await expect(page.getByTitle('image.tga', { exact: true })).toBeVisible();
  expect(downloads).toBe(0);
  // Save As can run again.
  await menu(page, 'File', 'Save As…');
  await expect(dialog).toBeVisible();
});

test('a dropped TGA, DDS or ICO lands as a new layer (AC-13)', async ({ page }) => {
  await createDocument(page, 'image', { width: 8, height: 8 });
  const ico1 = ico([[4, 32, await solidPng(page, 4, 4, '#ff00ff')]]);
  for (const [name, bytes] of [
    ['a.tga', tga([1, 2, 3, 255])],
    ['b.dds', dds()],
    ['c.ico', ico1],
  ] as const) {
    const before = await layerRows(page).count();
    await dragFiles(imageCanvas(page), 'drop', [{ name, bytes }]);
    const offer = page.getByRole('dialog');
    await offer.getByRole('button', { name: /Add as layer/ }).click();
    await expect(layerRows(page)).toHaveCount(before + 1);
  }
});
