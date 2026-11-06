import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  fakeFileHandles,
  fakeFs,
  fakeFsLog,
  gotoStart,
  imageCanvas,
  menu,
  rasterFile,
  waitForEditor,
} from './helpers';

// Opening Photoshop files in Image mode (docs/specs/psd-import.md). The
// fixtures are written by `crates/pincel-image-core/tests/psd.rs`:
// `sample.psd` is 40 × 30 with Base, a pass-through group "Folder" holding
// a Multiply layer "Ink", a text layer "Title" and a Curves adjustment
// layer; `flat.psd` is 40 × 30 without a layer section.

const fixture = (name: string): Promise<Buffer> =>
  readFile(join(import.meta.dirname, '../../crates/pincel-image-core/tests/fixtures', name));

const PSD_MIME = 'image/vnd.adobe.photoshop';

const hideButtons = (page: Page) => page.getByRole('button', { name: /^Hide / });

const row = (page: Page, name: string) =>
  page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });

/** The status-bar notice of an opened PSD. */
const openNotice = (page: Page) => page.getByText(/^Opened with changes:/);

/** Picks `name` in the open picker (`<input type=file>` fallback) that
 *  `open` brings up. */
async function choose(
  page: Page,
  open: () => Promise<void>,
  name: string,
  buffer: Buffer,
  mimeType = PSD_MIME,
): Promise<void> {
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), open()]);
  await chooser.setFiles({ name, mimeType, buffer });
}

const startOpen = (page: Page) => () => page.getByRole('button', { name: 'Open…' }).click();

/** `bytes` with a big-endian 16- or 32-bit value written at `offset`. */
function patched(bytes: Buffer, offset: number, value: number, size: 2 | 4): Buffer {
  const out = Buffer.from(bytes);
  if (size === 2) out.writeUInt16BE(value, offset);
  else out.writeUInt32BE(value, offset);
  return out;
}

test.describe('without File System Access', () => {
  // Force the download / <input type=file> fallbacks: the File System
  // Access pickers would open native dialogs the test cannot drive.
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  test('AC-01, AC-05, AC-06, AC-14: a layered PSD opens with its layers, group and one notice', async ({
    page,
  }) => {
    await gotoStart(page);
    await choose(page, startOpen(page), 'sample.psd', await fixture('sample.psd'));
    await waitForEditor(page, 'image');
    await expect(page.getByText('40 × 30 px · sRGB')).toBeVisible();
    await expect(page.getByTitle('sample.psd', { exact: true })).toBeVisible();
    // Base, Folder, Ink, Title — the Curves adjustment layer is left out.
    await expect(hideButtons(page)).toHaveCount(4);
    await expect(row(page, 'Folder')).toHaveAttribute('data-depth', '0');
    await expect(row(page, 'Folder')).toContainText('group');
    await expect(row(page, 'Ink')).toHaveAttribute('data-depth', '1');
    await expect(row(page, 'Title')).toHaveAttribute('data-depth', '0');
    const notice = openNotice(page);
    await expect(notice).toHaveCount(1);
    await expect(notice).toContainText('adjustment layer “Curves” left out');
    await expect(notice).toContainText('text layer “Title” opened as pixels');
    await expect(notice).toContainText('pass-through groups opened as isolated (“Folder”)');
  });

  test('AC-07: a PSD without layers opens its flattened image with a notice saying why', async ({
    page,
  }) => {
    await gotoStart(page);
    await choose(page, startOpen(page), 'flat.psd', await fixture('flat.psd'));
    await waitForEditor(page, 'image');
    await expect(page.getByText('40 × 30 px · sRGB')).toBeVisible();
    await expect(hideButtons(page)).toHaveCount(1);
    await expect(openNotice(page)).toContainText(
      'the file has no layers; opened its flattened image',
    );
  });

  test('a .psd whose content is a PNG opens by its content', async ({ page }) => {
    await gotoStart(page);
    await choose(page, startOpen(page), 'photo.psd', await rasterFile(page, 'png'));
    await waitForEditor(page, 'image');
    await expect(page.getByText('8 × 6 px · sRGB')).toBeVisible();
    await expect(openNotice(page)).toHaveCount(0);
  });

  test('AC-08, AC-09, AC-10: a PSD that cannot be opened leaves the start screen and the open document as they were', async ({
    page,
  }) => {
    const flat = await fixture('flat.psd');
    const broken: [string, Buffer, string][] = [
      ['deep.psd', patched(flat, 22, 16, 2), '16 bits per channel is not supported'],
      ['large.psb', patched(flat, 4, 2, 2), 'PSB (large document format) is not supported'],
      ['print.psd', patched(flat, 24, 4, 2), 'color mode CMYK is not supported'],
      ['wide.psd', patched(flat, 18, 20000, 4), 'canvas 20000 × 30 exceeds the size limit'],
      [
        'huge.psd',
        patched(patched(flat, 18, 9000, 4), 14, 9000, 4),
        'canvas 9000 × 9000 exceeds the size limit',
      ],
      ['cut.psd', flat.subarray(0, 20), 'the PSD file is truncated'],
      ['short.psd', flat.subarray(0, flat.length - 10), 'the flattened image cannot be read'],
    ];
    await gotoStart(page);
    for (const [name, bytes, message] of broken) {
      await choose(page, startOpen(page), name, bytes);
      await expect(page.getByRole('alert')).toContainText(`Open failed: ${message}`);
      await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
      await expect(imageCanvas(page)).toHaveCount(0);
    }

    // With a document open, File ▸ Open… of a broken PSD keeps it.
    await choose(page, startOpen(page), 'sample.psd', await fixture('sample.psd'));
    await waitForEditor(page, 'image');
    await expect(hideButtons(page)).toHaveCount(4);
    for (const [name, bytes, message] of broken) {
      await choose(page, () => menu(page, 'File', 'Open…'), name, bytes);
      await expect(page.getByRole('alert')).toContainText(`Open failed: ${message}`);
      await expect(page.getByTitle('sample.psd', { exact: true })).toBeVisible();
      await expect(page.getByText('40 × 30 px · sRGB')).toBeVisible();
      await expect(hideButtons(page)).toHaveCount(4);
    }
  });

  test('AC-12: Save of a document opened from a PSD asks for an .ora target', async ({ page }) => {
    await gotoStart(page);
    await choose(page, startOpen(page), 'sample.psd', await fixture('sample.psd'));
    await waitForEditor(page, 'image');
    await menu(page, 'File', 'Save');
    const dialog = page.getByRole('dialog', { name: 'Save As' });
    await expect(dialog).toContainText('PSD files cannot be written back');
    await expect(dialog.getByLabel('Format')).toHaveValue('ora');
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      dialog.getByRole('button', { name: 'Save' }).click(),
    ]);
    expect(download.suggestedFilename()).toBe('sample.ora');
    const bytes = await readFile(await download.path());
    expect(bytes.subarray(30, 54).toString('latin1')).toBe('mimetypeimage/openraster');
    await expect(page.getByTitle('sample.ora', { exact: true })).toBeVisible();
  });
});

test('AC-11: the open picker offers .psd and the start screen names PSD', async ({ page }) => {
  await fakeFileHandles(page, {});
  await gotoStart(page);
  await expect(page.getByText(/PSD open in Image mode\.$/)).toBeVisible();
  await fakeFs(page, { pick: null });
  await page.getByRole('button', { name: 'Open…' }).click();
  await expect.poll(async () => (await fakeFsLog(page)).pickers.length).toBe(1);
  const [picker] = (await fakeFsLog(page)).pickers;
  const accepts = (picker?.types ?? []).flatMap((t) => Object.entries(t.accept));
  expect(accepts).toContainEqual([PSD_MIME, ['.psd']]);
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('AC-11: the start screen names PSD in German', async ({ page }) => {
    await page.goto('./');
    await expect(page.getByText(/PSD öffnen im Bild-Modus\.$/)).toBeVisible();
  });
});
