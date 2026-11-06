import { readFile } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  RASTER_MIME,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  rasterFile,
  type RasterKind,
  waitForEditor,
} from './helpers';

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

type Kind = RasterKind;

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

const MIME = RASTER_MIME;
const fixture = rasterFile;

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

/** Opens `photo.jpeg`, paints and adds a second layer. */
async function openLayeredJpeg(page: Page): Promise<void> {
  await openFile(page, 'photo.jpeg', 'jpeg');
  await paint(page);
  await page.getByTitle('Add layer').click();
  await expect(layerRows(page)).toHaveCount(2);
}

test('a layered JPEG Save asks flattening and quality in one prompt (AC-07, AC-17)', async ({
  page,
}) => {
  await openLayeredJpeg(page);
  await page.keyboard.press('Control+s');
  const prompt = flattenPrompt(page);
  const quality = prompt.getByLabel('Quality');
  await expect(quality).toHaveValue('90');
  await setRange(quality, 55);
  const first = await download(page, () => prompt.getByRole('button', { name: 'Flatten' }).click());
  expect(first.name).toBe('photo.jpeg');
  expect(formatOf(first.bytes)).toBe('jpeg');
  // No JPEG quality dialog follows the prompt.
  await expect(page.getByRole('dialog')).toHaveCount(0);

  // The quality is remembered: the next flatten prompt asks no quality.
  await paint(page);
  await page.keyboard.press('Control+s');
  await expect(prompt).toBeVisible();
  await expect(prompt.getByLabel('Quality')).toHaveCount(0);
  await download(page, () => prompt.getByRole('button', { name: 'Flatten' }).click());
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('Cancel and Save as .ora… in the combined prompt remember no quality (AC-17)', async ({
  page,
}) => {
  await openLayeredJpeg(page);
  const prompt = flattenPrompt(page);
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await page.keyboard.press('Control+s');
  await setRange(prompt.getByLabel('Quality'), 55);
  await prompt.getByRole('button', { name: 'Cancel' }).click();
  await expect(prompt).toHaveCount(0);
  expect(downloads).toBe(0);

  await page.keyboard.press('Control+s');
  await expect(prompt.getByLabel('Quality')).toHaveValue('90');
  await setRange(prompt.getByLabel('Quality'), 55);
  const ora = await download(page, () => prompt.getByRole('button', { name: 'Save as .ora…' }).click());
  expect(ora.name).toBe('photo.ora');
  expect(formatOf(ora.bytes)).toBe('ora');

  await menu(page, 'File', 'Save As…');
  await saveAsDialog(page).getByLabel('Format').selectOption('jpeg');
  await expect(saveAsDialog(page).getByLabel('Quality')).toHaveValue('90');
});


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

/** Sets a range input the way a drag would (Playwright cannot fill one). */
async function setRange(input: Locator, value: number): Promise<void> {
  await input.evaluate((el, v) => {
    (el as HTMLInputElement).value = String(v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const mergeBox = (page: Page) => saveAsDialog(page).getByLabel('Merge layers in the file');

test('Save As asks format, JPEG quality and merging in one dialog, then only the file (AC-10, AC-15, AC-16)', async ({
  page,
}) => {
  await openLayered(page);
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  const saveButton = dialog.getByRole('button', { name: 'Save' });
  await dialog.getByLabel('Format').selectOption('jpeg');
  const quality = dialog.getByLabel('Quality');
  await expect(quality).toHaveValue('90');
  await expect(mergeBox(page)).not.toBeChecked();
  await expect(saveButton).toBeDisabled();
  await setRange(quality, 40);
  await mergeBox(page).check();
  await expect(saveButton).toBeEnabled();

  const written = await download(page, () => saveButton.click());
  expect(written.name).toBe('screenshot.jpg');
  expect(formatOf(written.bytes)).toBe('jpeg');
  // No flatten prompt and no quality dialog follow the Save As dialog.
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByTitle('screenshot.jpg', { exact: true })).toBeVisible();

  // The quality saved with is remembered for the document.
  await menu(page, 'File', 'Save As…');
  await saveAsDialog(page).getByLabel('Format').selectOption('jpeg');
  await expect(saveAsDialog(page).getByLabel('Quality')).toHaveValue('40');
});

test('Save As remembers the JPEG quality only once the file is written (AC-15)', async ({ page }) => {
  await openFile(page, 'screenshot.png', 'png');
  await paint(page);
  // A save picker the user cancels.
  await page.evaluate(() => {
    (window as unknown as { showSaveFilePicker: () => Promise<never> }).showSaveFilePicker =
      async () => {
        throw new DOMException('The user aborted a request.', 'AbortError');
      };
  });
  let downloads = 0;
  page.on('download', () => (downloads += 1));

  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  await dialog.getByLabel('Format').selectOption('jpeg');
  const quality = dialog.getByLabel('Quality');
  await expect(quality).toHaveAttribute('min', '1');
  await expect(quality).toHaveAttribute('max', '100');
  await setRange(quality, 40);
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTitle('screenshot.png — unsaved changes')).toBeVisible();
  expect(downloads).toBe(0);

  await menu(page, 'File', 'Save As…');
  await saveAsDialog(page).getByLabel('Format').selectOption('jpeg');
  await expect(saveAsDialog(page).getByLabel('Quality')).toHaveValue('90');
});

test('the Save As merge checkbox keeps its state and hides for .ora (AC-16)', async ({ page }) => {
  await openLayered(page);
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  const format = dialog.getByLabel('Format');
  const saveButton = dialog.getByRole('button', { name: 'Save' });
  await format.selectOption('png');
  await expect(dialog.getByLabel('Quality')).toHaveCount(0);
  await expect(saveButton).toBeDisabled();
  await mergeBox(page).check();
  await format.selectOption('ora');
  await expect(mergeBox(page)).toHaveCount(0);
  await expect(saveButton).toBeEnabled();
  await format.selectOption('png');
  await expect(mergeBox(page)).toBeChecked();
  await format.selectOption('jpeg');
  await expect(mergeBox(page)).toBeChecked();
  await expect(saveButton).toBeEnabled();

  // Cancel writes and remembers nothing.
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTitle('screenshot.png — unsaved changes')).toBeVisible();
  expect(downloads).toBe(0);
});

test('Save As needs no merge confirmation when flattening loses nothing (AC-16)', async ({
  page,
}) => {
  await openFile(page, 'screenshot.png', 'png');
  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  await dialog.getByLabel('Format').selectOption('jpeg');
  await expect(dialog.getByLabel('Quality')).toBeVisible();
  await expect(mergeBox(page)).toHaveCount(0);
  const written = await download(page, () => dialog.getByRole('button', { name: 'Save' }).click());
  expect(formatOf(written.bytes)).toBe('jpeg');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('"Don\'t ask again" hides the Save As merge checkbox (AC-16)', async ({ page }) => {
  await openLayered(page);
  await menu(page, 'File', 'Save');
  const prompt = flattenPrompt(page);
  await prompt.getByLabel("Don't ask again for this document").check();
  await download(page, () => prompt.getByRole('button', { name: 'Flatten' }).click());

  await menu(page, 'File', 'Save As…');
  const dialog = saveAsDialog(page);
  await dialog.getByLabel('Format').selectOption('png');
  await expect(mergeBox(page)).toHaveCount(0);
  await expect(dialog.getByRole('button', { name: 'Save' })).toBeEnabled();
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

test('the kept snapshot follows undo and redo back to the saved state', async ({ page }) => {
  await page.clock.install();
  await openLayered(page);
  await saveFlattened(page);
  // Undo the added layer and let autosave snapshot the one-layer state,
  // then redo back to the saved state: the snapshot must catch up.
  await menu(page, 'Edit', 'Undo');
  await expect(layerRows(page)).toHaveCount(1);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  await menu(page, 'Edit', 'Redo');
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByTitle('screenshot.png', { exact: true })).toBeVisible();
  await page.clock.fastForward(31_000);

  await recoverAfterReload(page);
  await expect(layerRows(page)).toHaveCount(2);
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
