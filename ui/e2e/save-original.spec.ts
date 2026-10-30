import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { drag, gotoStart, imageCanvas, menu, waitForEditor } from './helpers';

// Image-mode Save writes back to the opened file in its own format
// (docs/specs/save-original-format.md). The File System Access pickers are
// removed so Save falls back to a download named like the opened file; one
// test fakes an opened FSA handle to prove the in-place write.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

type Kind = 'png' | 'jpeg' | 'webp' | 'bmp' | 'tiff' | 'gif';

/** Leading bytes that identify each written format. */
function formatOf(bytes: Uint8Array): Kind | 'ora' | null {
  const at = (sig: number[], offset = 0) => sig.every((b, i) => bytes[offset + i] === b);
  if (at([0x89, 0x50, 0x4e, 0x47])) return 'png';
  if (at([0xff, 0xd8, 0xff])) return 'jpeg';
  if (at([0x52, 0x49, 0x46, 0x46]) && at([0x57, 0x45, 0x42, 0x50], 8)) return 'webp';
  if (at([0x42, 0x4d])) return 'bmp';
  if (at([0x49, 0x49, 0x2a, 0x00]) || at([0x4d, 0x4d, 0x00, 0x2a])) return 'tiff';
  if (at([0x47, 0x49, 0x46, 0x38])) return 'gif';
  if (Buffer.from(bytes.subarray(30, 54)).toString('latin1') === 'mimetypeimage/openraster') {
    return 'ora';
  }
  return null;
}

/** A 24-bit 4×3 BMP, bottom-up rows padded to 4 bytes. */
function bmpFixture(): Buffer {
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
function tiffFixture(): Buffer {
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
const GIF_FIXTURE = Buffer.from('R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==', 'base64');

/** Encodes an 8×6 test image in the browser (PNG / JPEG / WebP). */
async function canvasFixture(page: Page, mime: string): Promise<Buffer> {
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

const MIME: Record<Kind, string> = {
  png: 'image/png',
  jpeg: 'image/jpeg',
  webp: 'image/webp',
  bmp: 'image/bmp',
  tiff: 'image/tiff',
  gif: 'image/gif',
};

async function fixture(page: Page, kind: Kind): Promise<Buffer> {
  if (kind === 'bmp') return bmpFixture();
  if (kind === 'tiff') return tiffFixture();
  if (kind === 'gif') return GIF_FIXTURE;
  return canvasFixture(page, MIME[kind]);
}

/** Opens `name` through the start screen's Open… (input fallback). */
async function openFile(page: Page, name: string, kind: Kind | 'ora', bytes?: Buffer): Promise<void> {
  await gotoStart(page);
  const buffer = bytes ?? (kind === 'ora' ? Buffer.alloc(0) : await fixture(page, kind));
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name,
    mimeType: kind === 'ora' ? 'image/openraster' : MIME[kind],
    buffer,
  });
  await waitForEditor(page, 'image');
}

/** Paints a stroke so the document reads unsaved. */
async function paint(page: Page): Promise<void> {
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
}

/** Runs `action` and returns the download it triggers. */
async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ name: string; bytes: Uint8Array }> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return { name: d.suggestedFilename(), bytes: new Uint8Array(await readFile(await d.path())) };
}

const save = (page: Page) => () => menu(page, 'File', 'Save');

const ROUND_TRIPS: { kind: Kind; name: string }[] = [
  { kind: 'png', name: 'screenshot.png' },
  { kind: 'jpeg', name: 'photo.jpg' },
  { kind: 'webp', name: 'pic.webp' },
  { kind: 'bmp', name: 'icon.bmp' },
  { kind: 'tiff', name: 'scan.tif' },
];

for (const { kind, name } of ROUND_TRIPS) {
  test(`Save writes an opened ${kind} back under its own name and format`, async ({ page }) => {
    await openFile(page, name, kind);
    // AC-11: the title is the opened file, not an `.ora` name.
    await expect(page.getByTitle(name, { exact: true })).toBeVisible();
    await paint(page);
    await expect(page.getByTitle(`${name} — unsaved changes`)).toBeVisible();

    const written = await download(page, async () => {
      await menu(page, 'File', 'Save');
      if (kind === 'jpeg') {
        const dialog = page.getByRole('dialog', { name: 'JPEG quality' });
        await dialog.getByRole('button', { name: 'Save' }).click();
      }
    });
    expect(written.name).toBe(name);
    expect(formatOf(written.bytes)).toBe(kind);
    await expect(page.getByTitle(name, { exact: true })).toBeVisible();
    // AC-08: WebP (and every lossless format) saves without a dialog.
    await expect(page.getByRole('dialog')).toHaveCount(0);
  });
}

test('JPEG asks its quality once per document', async ({ page }) => {
  await openFile(page, 'photo.jpeg', 'jpeg');
  await paint(page);
  const dialog = page.getByRole('dialog', { name: 'JPEG quality' });

  // Cancel writes nothing, remembers nothing and keeps the document unsaved.
  await page.keyboard.press('Control+s');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTitle('photo.jpeg — unsaved changes')).toBeVisible();

  const first = await download(page, async () => {
    await page.keyboard.press('Control+s');
    await dialog.getByRole('button', { name: 'Save' }).click();
  });
  expect(first.name).toBe('photo.jpeg');
  expect(formatOf(first.bytes)).toBe('jpeg');

  await paint(page);
  const second = await download(page, save(page));
  expect(second.name).toBe('photo.jpeg');
  expect(formatOf(second.bytes)).toBe('jpeg');
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTitle('photo.jpeg', { exact: true })).toBeVisible();
});

test('Save writes through the opened file handle in place', async ({ page }) => {
  await gotoStart(page);
  const png = await fixture(page, 'png');
  // A fake File System Access handle for the opened file: Open hands it to
  // the editor, Save writes through it without a picker or a download.
  await page.evaluate((bytes) => {
    const w = window as unknown as {
      written: number[][];
      showOpenFilePicker: () => Promise<unknown[]>;
    };
    w.written = [];
    const handle = {
      kind: 'file',
      name: 'shot.png',
      getFile: async () => new File([new Uint8Array(bytes)], 'shot.png', { type: 'image/png' }),
      createWritable: async () => ({
        write: async (data: Uint8Array) => void w.written.push(Array.from(data)),
        close: async () => undefined,
      }),
    };
    w.showOpenFilePicker = async () => [handle];
  }, Array.from(png));
  await page.getByRole('button', { name: 'Open…' }).click();
  await waitForEditor(page, 'image');
  await paint(page);

  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await page.keyboard.press('Control+s');
  await expect(page.getByTitle('shot.png', { exact: true })).toBeVisible();
  const written = await page.evaluate(() => (window as unknown as { written: number[][] }).written);
  expect(written).toHaveLength(1);
  expect(formatOf(new Uint8Array(written[0] ?? []))).toBe('png');
  expect(downloads).toBe(0);
});

test('a document opened from .ora still saves as OpenRaster', async ({ page }) => {
  // Make an .ora through a new document's Save, then reopen it.
  await gotoStart(page);
  await page.getByRole('button', { name: 'New image' }).click();
  const card = page.getByRole('form', { name: 'New image' });
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await paint(page);
  const created = await download(page, save(page));
  expect(created.name).toBe('image.ora');
  expect(formatOf(created.bytes)).toBe('ora');

  await openFile(page, 'project.ora', 'ora', Buffer.from(created.bytes));
  await expect(page.getByTitle('project.ora', { exact: true })).toBeVisible();
  await paint(page);
  const saved = await download(page, save(page));
  expect(saved.name).toBe('project.ora');
  expect(formatOf(saved.bytes)).toBe('ora');
});

const flattenPrompt = (page: Page) => page.getByRole('dialog', { name: 'Flatten image?' });
const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });

/** Opens `screenshot.png` and adds a second layer. */
async function openLayered(page: Page): Promise<void> {
  await openFile(page, 'screenshot.png', 'png');
  await paint(page);
  await page.getByTitle('Add layer').click();
  await expect(layerRows(page)).toHaveCount(2);
}

test('a layered document asks to flatten; Flatten writes the composite and keeps the layers', async ({
  page,
}) => {
  await openLayered(page);
  await menu(page, 'File', 'Save');
  const prompt = flattenPrompt(page);
  await expect(prompt).toBeVisible();
  await expect(prompt.getByRole('button', { name: 'Flatten' })).toBeVisible();
  await expect(prompt.getByRole('button', { name: 'Save as .ora…' })).toBeVisible();
  await expect(prompt.getByRole('button', { name: 'Cancel' })).toBeVisible();

  const written = await download(page, () => prompt.getByRole('button', { name: 'Flatten' }).click());
  expect(written.name).toBe('screenshot.png');
  expect(formatOf(written.bytes)).toBe('png');
  await expect(prompt).toHaveCount(0);
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByTitle('screenshot.png', { exact: true })).toBeVisible();
});

test('a single layer whose opacity would be lost asks to flatten', async ({ page }) => {
  await openFile(page, 'screenshot.png', 'png');
  await page.getByLabel('Layer opacity').fill('40');
  await menu(page, 'File', 'Save');
  await expect(flattenPrompt(page)).toBeVisible();
});

test('a single hidden layer asks to flatten', async ({ page }) => {
  await openFile(page, 'screenshot.png', 'png');
  await layerRows(page).click();
  await menu(page, 'File', 'Save');
  await expect(flattenPrompt(page)).toBeVisible();
});

test('Save as .ora… in the flatten prompt saves the layers as OpenRaster', async ({ page }) => {
  await openLayered(page);
  await menu(page, 'File', 'Save');
  const written = await download(page, () =>
    flattenPrompt(page).getByRole('button', { name: 'Save as .ora…' }).click(),
  );
  expect(written.name).toBe('screenshot.ora');
  expect(formatOf(written.bytes)).toBe('ora');
  await expect(page.getByTitle('screenshot.ora', { exact: true })).toBeVisible();
});

test('Cancel in the flatten prompt writes nothing and keeps the document unsaved', async ({
  page,
}) => {
  await openLayered(page);
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await menu(page, 'File', 'Save');
  await flattenPrompt(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(flattenPrompt(page)).toHaveCount(0);
  await expect(page.getByTitle('screenshot.png — unsaved changes')).toBeVisible();
  expect(downloads).toBe(0);
});

test('"Don\'t ask again" flattens later Saves of the document without asking', async ({
  page,
}) => {
  await openLayered(page);
  await menu(page, 'File', 'Save');
  const prompt = flattenPrompt(page);
  await prompt.getByLabel("Don't ask again for this document").check();
  await download(page, () => prompt.getByRole('button', { name: 'Flatten' }).click());

  await paint(page);
  const again = await download(page, save(page));
  expect(again.name).toBe('screenshot.png');
  expect(formatOf(again.bytes)).toBe('png');
  await expect(prompt).toHaveCount(0);
});

const saveAsDialog = (page: Page) => page.getByRole('dialog', { name: 'Save As' });

test('Save As offers every writable format and writes the chosen one', async ({ page }) => {
  await openFile(page, 'screenshot.png', 'png');
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  const format = dialog.getByLabel('Format');
  await expect(format.locator('option')).toHaveText([
    'OpenRaster (.ora)',
    'PNG',
    'JPEG',
    'WebP (lossless)',
    'BMP',
    'TIFF',
  ]);
  // One layer: the document's own format is preselected.
  await expect(format).toHaveValue('png');
  await format.selectOption('webp');
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(written.name).toBe('screenshot.webp');
  expect(formatOf(written.bytes)).toBe('webp');
  await expect(page.getByTitle('screenshot.webp', { exact: true })).toBeVisible();
});

test('Save As preselects .ora for a layered document', async ({ page }) => {
  await openLayered(page);
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  await expect(dialog.getByLabel('Format')).toHaveValue('ora');
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(written.name).toBe('screenshot.ora');
  expect(formatOf(written.bytes)).toBe('ora');
});

test('Save of a GIF opens Save As with a notice that GIF is not written back', async ({ page }) => {
  await openFile(page, 'anim.gif', 'gif');
  await expect(page.getByTitle('anim.gif', { exact: true })).toBeVisible();
  await paint(page);
  await menu(page, 'File', 'Save');
  const dialog = saveAsDialog(page);
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('GIF files cannot be written back');
  await expect(dialog.getByLabel('Format')).toHaveValue('png');
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(written.name).toBe('anim.png');
  expect(formatOf(written.bytes)).toBe('png');
  await expect(page.getByTitle('anim.png', { exact: true })).toBeVisible();
});

/** Saves the open layered document flattened through the prompt. */
async function saveFlattened(page: Page): Promise<{ name: string; bytes: Uint8Array }> {
  await menu(page, 'File', 'Save');
  return download(page, () => flattenPrompt(page).getByRole('button', { name: 'Flatten' }).click());
}

/** Reloads the page, creates a new image and recovers the snapshot the
 *  earlier page load left. */
async function recoverAfterReload(page: Page): Promise<void> {
  // Let the IndexedDB write settle before leaving the page.
  await page.waitForTimeout(500);
  await page.reload();
  await gotoStart(page);
  await page.getByRole('button', { name: 'New image' }).click();
  await page.getByRole('form', { name: 'New image' }).getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  const recovery = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(recovery).toBeVisible();
  await recovery.getByRole('button', { name: 'Recover' }).first().click();
  await expect(recovery).toHaveCount(0);
}

test('a flattening Save keeps an autosave snapshot that recovers every layer', async ({ page }) => {
  await openLayered(page);
  await saveFlattened(page);
  await expect(page.getByTitle('screenshot.png', { exact: true })).toBeVisible();

  await recoverAfterReload(page);
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByTitle('screenshot.png — unsaved changes')).toBeVisible();
});

test('a recovered document keeps saving in its detected format, not its extension', async ({
  page,
}) => {
  // PNG bytes under a `.jpg` name: the content decides, Save writes PNG.
  await openFile(page, 'photo.jpg', 'png');
  await paint(page);
  await page.getByTitle('Add layer').click();
  expect(formatOf((await saveFlattened(page)).bytes)).toBe('png');

  await recoverAfterReload(page);
  const again = await saveFlattened(page);
  expect(again.name).toBe('photo.jpg');
  expect(formatOf(again.bytes)).toBe('png');
});
