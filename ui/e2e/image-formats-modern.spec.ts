import { readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  drag,
  dragFiles,
  gotoStart,
  imageCanvas,
  menu,
  rasterFile,
  solidPng,
  waitForEditor,
  type DraggedFile,
} from './helpers';

// JPEG XL in Image mode (docs/specs/image-formats-more-modern.md). The
// files are the core's fixtures, rendered once with the reference tools;
// saves fall back to downloads.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const fixtures = fileURLToPath(
  new URL('../../crates/pincel-image-core/tests/fixtures/modern/', import.meta.url),
);
const fixture = (name: string): Buffer => readFileSync(`${fixtures}${name}`);

/** The JPEG XL codestream / container signatures (AC-04). */
const CODESTREAM = [0xff, 0x0a];
const CONTAINER = [0, 0, 0, 0x0c, 0x4a, 0x58, 0x4c, 0x20, 0x0d, 0x0a, 0x87, 0x0a];
const isJxl = (bytes: Buffer): boolean =>
  [CODESTREAM, CONTAINER].some((sig) => sig.every((b, i) => bytes[i] === b));

/** Opens `name` through the start screen's Open… (input fallback). */
async function openFile(page: Page, name: string, buffer: Buffer, ok = true): Promise<void> {
  await gotoStart(page);
  await chooseFile(page, () => page.getByRole('button', { name: 'Open…' }).click(), name, buffer);
  if (ok) await waitForEditor(page, 'image');
}

/** Answers the file chooser that `open` raises with `name`. */
async function chooseFile(
  page: Page,
  open: () => Promise<void>,
  name: string,
  buffer: Buffer,
): Promise<void> {
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), open()]);
  await chooser.setFiles({ name, mimeType: 'application/octet-stream', buffer });
}

async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ name: string; bytes: Buffer }> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return { name: d.suggestedFilename(), bytes: await readFile(await d.path()) };
}

/** The composite, read through Edit ▸ Copy Merged (never from the canvas). */
async function merged(page: Page): Promise<{ width: number; height: number; data: number[] }> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy Merged');
  return copiedImage(page);
}

const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });
const sizeShown = (page: Page, w: number, h: number) =>
  page.getByText(`${w} × ${h} px · sRGB`, { exact: true });
const saveAsDialog = (page: Page) => page.getByRole('dialog', { name: 'Save As' });

test('a JPEG XL file opens with its pixels and Save writes it back as JPEG XL (AC-03, AC-06)', async ({
  page,
}) => {
  // The reference rendering first: the pixels Open must produce.
  await openFile(page, 'reference.png', fixture('lossless8.png'));
  const want = await merged(page);

  await openFile(page, 'lossless8.jxl', fixture('lossless8.jxl'));
  await expect(page.getByTitle('lossless8.jxl', { exact: true })).toBeVisible();
  await expect(sizeShown(page, 24, 16)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
  expect(await merged(page)).toEqual(want);

  // Save writes the file in place, lossless, without any dialog.
  const written = await download(page, () => menu(page, 'File', 'Save'));
  expect(written.name).toBe('lossless8.jxl');
  expect(isJxl(written.bytes)).toBe(true);
  await expect(saveAsDialog(page)).toHaveCount(0);
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByTitle('lossless8.jxl', { exact: true })).toBeVisible();

  // The ISOBMFF container opens the same image.
  await openFile(page, 'container.jxl', fixture('container.jxl'));
  await expect(sizeShown(page, 24, 16)).toBeVisible();
  expect(await merged(page)).toEqual(want);
});

test('JPEG XL is told by content: a PNG named .jxl opens as PNG, an AVIF fails (AC-04)', async ({
  page,
}) => {
  await gotoStart(page);
  const png = await rasterFile(page, 'png');
  await openFile(page, 'fake.jxl', png);
  await expect(page.getByTitle('fake.jxl', { exact: true })).toBeVisible();
  // A PNG document: Save writes PNG bytes, whatever the name says.
  const written = await download(page, () => menu(page, 'File', 'Save'));
  expect([...written.bytes.subarray(0, 4)]).toEqual([0x89, 0x50, 0x4e, 0x47]);

  await openFile(page, 'still.avif', fixture('still.avif'), false);
  await expect(page.getByText(/still\.avif/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeVisible();
  // Not JPEG XL content under a .jxl name: no document either.
  await openFile(page, 'garbage.jxl', Buffer.from('not an image at all'), false);
  await expect(page.getByText(/garbage\.jxl could not be opened/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeVisible();
});

test('a truncated JPEG XL fails as undecodable, a huge one with the size message (AC-08, AC-12)', async ({
  page,
}) => {
  await openFile(page, 'cut.jxl', fixture('truncated.jxl'), false);
  await expect(page.getByText(/cut\.jxl could not be opened/)).toBeVisible();
  await expect(page.getByText(/invalid canvas size/)).toHaveCount(0);

  await openFile(page, 'huge.jxl', fixture('huge.jxl'), false);
  await expect(
    page.getByText(/huge\.jxl could not be opened: .*invalid canvas size/),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeVisible();

  // An open document stays as it was.
  await openFile(page, 'lossless8.jxl', fixture('lossless8.jxl'));
  const before = await merged(page);
  await chooseFile(page, () => menu(page, 'File', 'Open…'), 'cut.jxl', fixture('truncated.jxl'));
  await expect(page.getByText(/Could not open image: .*JPEG XL/)).toBeVisible();
  await expect(page.getByRole('tab')).toHaveCount(1);
  await expect(page.getByTitle('lossless8.jxl', { exact: true })).toBeVisible();
  expect(await merged(page)).toEqual(before);
});

test('Save As offers JPEG XL with the name <name>.jxl (AC-13)', async ({ page }) => {
  await createDocument(page, 'image', { width: 8, height: 8 });
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  const format = dialog.getByLabel('Format');
  await expect(format.locator('option')).toContainText(['JPEG XL']);
  await format.selectOption('jxl');
  // Lossless: no quality to ask.
  await expect(dialog.getByLabel('Quality')).toHaveCount(0);
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(written.name).toBe('image.jxl');
  expect(isJxl(written.bytes)).toBe(true);
  await expect(page.getByTitle('image.jxl', { exact: true })).toBeVisible();
});

test('a JPEG XL that cannot be written leaves the document unsaved with a notice (AC-14)', async ({
  page,
}) => {
  // The encoder needs 2 px on each side (spec, Open questions).
  await gotoStart(page);
  await openFile(page, 'thin.png', await solidPng(page, 1, 8, '#ff0000'));
  await drag(page, imageCanvas(page), [0.5, 0.2], [0.5, 0.8]);
  await expect(page.getByTitle('thin.png — unsaved changes', { exact: true })).toBeVisible();

  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  await dialog.getByLabel('Format').selectOption('jxl');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText(/Save failed: /)).toBeVisible();
  expect(downloads).toBe(0);
  await expect(page.getByTitle('thin.png — unsaved changes', { exact: true })).toBeVisible();
});

test('a dropped JPEG XL lands as one layer in one undo step with the pixels of Open (AC-10, AC-15)', async ({
  page,
}) => {
  /** The composite after dropping `file` onto a fresh 24 × 16 document. */
  const dropped = async (file: DraggedFile) => {
    await createDocument(page, 'image', { width: 24, height: 16 });
    await dragFiles(imageCanvas(page), 'drop', [file]);
    await page
      .getByRole('dialog')
      .getByRole('button', { name: /Add as layer/ })
      .click();
    await expect(layerRows(page)).toHaveCount(2);
    return merged(page);
  };
  const want = await dropped({ name: 'reference.png', bytes: fixture('lossless8.png') });
  expect(await dropped({ name: 'lossless8.jxl', bytes: fixture('lossless8.jxl') })).toEqual(want);
  await menu(page, 'Edit', 'Undo');
  await expect(layerRows(page)).toHaveCount(1);
});

test('a dropped JPEG XL that fails to open, or an AVIF, is an undecodable drop (AC-10)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 24, height: 16 });
  const before = await layerRows(page).count();
  await dragFiles(imageCanvas(page), 'drop', [
    { name: 'good.jxl', bytes: fixture('lossless8.jxl') },
    { name: 'cut.jxl', bytes: fixture('truncated.jxl') },
    { name: 'huge.jxl', bytes: fixture('huge.jxl') },
    { name: 'still.avif', bytes: fixture('still.avif') },
  ]);
  await page
    .getByRole('dialog')
    .getByRole('button', { name: /Add as layer/ })
    .click();
  // Each failing file gets its own notice (file-access AC-26); the good
  // file is added and stays.
  for (const name of ['cut.jxl', 'huge.jxl', 'still.avif']) {
    await expect(page.getByRole('alert').filter({ hasText: name })).toBeVisible();
  }
  await expect(layerRows(page)).toHaveCount(before + 1);
});
