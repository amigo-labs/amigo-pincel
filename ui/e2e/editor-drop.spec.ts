import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  drag,
  dragFiles,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  rasterFile,
  solidPng,
  storedPref,
  waitForEditor,
  type DraggedFile,
} from './helpers';

// Dropping files onto an open editor (docs/specs/file-access.md AC-20 –
// AC-28): add them as layers or open the first as a document, with a
// remembered choice. Drags are dispatched as DOM events carrying `File`s.

type Mode = 'pixel' | 'image';
const MODES = ['pixel', 'image'] as const;

const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];

/** A 32 × 24 sprite (the writer's compatibility fixture). */
const sprite = () =>
  readFile(
    join(
      import.meta.dirname,
      '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
    ),
  );

/** The editor's drawing surface. */
const canvasOf = (page: Page, mode: Mode): Locator =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** Drags `files` over the editor and drops them there. */
async function dropOn(page: Page, mode: Mode, files: DraggedFile[]): Promise<void> {
  await dragFiles(canvasOf(page, mode), 'dragover', files);
  await dragFiles(canvasOf(page, mode), 'drop', files);
}

/** The rows of the layers panel. */
const layerRows = (page: Page, mode: Mode) =>
  mode === 'pixel'
    ? page
        .getByRole('list', { name: 'Layer list' })
        .getByRole('button', { name: /^Drag to reorder / })
    : page.getByRole('button', { name: /^Hide / });

/** The status bar's document size. */
const sizeShown = (page: Page, mode: Mode, [w, h]: [number, number]) =>
  page.getByText(mode === 'pixel' ? `${w}×${h}` : `${w} × ${h} · sRGB`, { exact: true });

const overlay = (page: Page) => page.getByText('Drop to add or open', { exact: true });
const dropDialog = (page: Page) => page.getByRole('dialog', { name: 'Dropped images' });
const button = (scope: Locator, name: string) => scope.getByRole('button', { name, exact: true });
const dontAsk = (page: Page) => dropDialog(page).getByRole('checkbox', { name: "Don't ask again" });

/** The shell's notice that contains `text`. */
const notice = (page: Page, text: string | RegExp) =>
  page.getByRole('alert').filter({ hasText: text });

/** The composite of the visible layers, through Edit ▸ Copy Merged. */
async function merged(page: Page) {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy Merged');
  return copiedImage(page);
}

/** One edit that makes the document dirty. */
async function edit(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
}

/** The File ▸ Dropped Images entry `label`. */
async function droppedImagesItem(page: Page, label: string): Promise<Locator> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
  await page
    .getByRole('menu', { name: 'File', exact: true })
    .getByRole('menuitem', { name: 'Dropped Images', exact: true })
    .click();
  return page
    .getByRole('menu', { name: 'Dropped Images', exact: true })
    .getByRole('menuitemcheckbox', { name: label, exact: true });
}

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

for (const mode of MODES) {
  test(`AC-20, AC-27: ${mode}: a file drag shows the overlay; a raster drop asks, Cancel changes nothing`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const png: DraggedFile[] = [{ name: 'a.png', bytes: await solidPng(page, 4, 4, '#ff0000') }];
    const canvas = canvasOf(page, mode);

    await dragFiles(canvas, 'dragover', png);
    await expect(overlay(page)).toBeVisible();
    await dragFiles(canvas, 'dragleave', png);
    await expect(overlay(page)).toHaveCount(0);
    // Dragged text is no file: no drop state.
    await dragFiles(canvas, 'dragover', 'text');
    await expect(overlay(page)).toHaveCount(0);

    await dropOn(page, mode, png);
    await expect(overlay(page)).toHaveCount(0);
    const dialog = dropDialog(page);
    await expect(dialog).toBeVisible();
    await expect(button(dialog, 'Add as layer')).toBeEnabled();
    await expect(button(dialog, 'Open as document')).toBeEnabled();
    await expect(dontAsk(page)).not.toBeChecked();
    await dontAsk(page).check();
    await button(dialog, 'Cancel').click();
    await expect(dialog).toHaveCount(0);
    await expect(layerRows(page, mode)).toHaveCount(1);
    await expect(sizeShown(page, mode, [16, 16])).toBeVisible();
    expect(await storedPref(page, 'droppedImages')).toBeUndefined();

    // Cancel remembered nothing: the next drop asks again.
    await dropOn(page, mode, png);
    await expect(dialog).toBeVisible();
    await expect(dontAsk(page)).not.toBeChecked();
    await button(dialog, 'Cancel').click();

    // While a modal dialog is open, a drag shows nothing and a drop is ignored.
    await menu(page, 'Layer', 'Rename…');
    const rename = page.getByRole('dialog').filter({ has: page.getByRole('textbox') });
    await expect(rename).toBeVisible();
    await dragFiles(rename, 'dragover', png);
    await expect(overlay(page)).toHaveCount(0);
    await dragFiles(rename, 'drop', png);
    await expect(dialog).toHaveCount(0);
    await button(rename, 'Cancel').click();
    await expect(layerRows(page, mode)).toHaveCount(1);
  });

  test(`AC-21: ${mode}: Add as layer stacks the files above the active layer, one undo step each`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const files: DraggedFile[] = [
      { name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') },
      { name: 'big.png', bytes: await solidPng(page, 32, 32, '#00ff00') },
      { name: 'blue.png', bytes: await solidPng(page, 4, 4, '#0000ff') },
    ];
    await dropOn(page, mode, files);
    await button(dropDialog(page), 'Add as layer').click();
    // The file larger than the canvas asks like a paste; its Cancel skips only it.
    const larger = page.getByRole('dialog', { name: 'Paste larger image' });
    await expect(larger).toBeVisible();
    await button(larger, 'Cancel').click();
    await expect(layerRows(page, mode)).toHaveCount(3);
    await expect(sizeShown(page, mode, [16, 16])).toBeVisible();

    // The last file lies on top; one Undo per file.
    expect(pixelOf(await merged(page), 0, 0)).toEqual(BLUE);
    await menu(page, 'Edit', 'Undo');
    await expect(layerRows(page, mode)).toHaveCount(2);
    expect(pixelOf(await merged(page), 0, 0)).toEqual(RED);
    await menu(page, 'Edit', 'Undo');
    await expect(layerRows(page, mode)).toHaveCount(1);
  });

  test(`AC-23: ${mode}: Open as document opens the first file after the unsaved-changes guard`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const files: DraggedFile[] = [
      { name: 'photo.png', bytes: await rasterFile(page, 'png') },
      { name: 'blue.png', bytes: await solidPng(page, 4, 4, '#0000ff') },
    ];
    await edit(page, mode);

    await dropOn(page, mode, files);
    await button(dropDialog(page), 'Open as document').click();
    const guard = page.getByRole('alertdialog', { name: 'Unsaved changes' });
    await expect(guard).toBeVisible();
    await button(guard, 'Cancel').click();
    await expect(guard).toHaveCount(0);
    await expect(sizeShown(page, mode, [16, 16])).toBeVisible();
    await expect(layerRows(page, mode)).toHaveCount(1);
    await expect(page.getByRole('alert')).toHaveCount(0);

    await dropOn(page, mode, files);
    await button(dropDialog(page), 'Open as document').click();
    await button(guard, 'Discard changes').click();
    await waitForEditor(page, 'image');
    await expect(page.getByTitle('photo.png', { exact: true })).toBeVisible();
    await expect(sizeShown(page, 'image', [8, 6])).toBeVisible();
    await expect(notice(page, 'Only photo.png was opened')).toBeVisible();
  });

  test(`AC-24: ${mode}: a project file in a drop opens without the dialog`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await dropOn(page, mode, [
      { name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') },
      { name: 'hero.aseprite', bytes: await sprite() },
      { name: 'blue.png', bytes: await solidPng(page, 4, 4, '#0000ff') },
    ]);
    await waitForEditor(page, 'pixel');
    await expect(page.getByTitle('hero.aseprite', { exact: true })).toBeVisible();
    await expect(sizeShown(page, 'pixel', [32, 24])).toBeVisible();
    await expect(dropDialog(page)).toHaveCount(0);
    await expect(notice(page, 'Only hero.aseprite was opened')).toBeVisible();
  });

  test(`AC-26: ${mode}: unsupported and broken files get a notice, the others proceed`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await dropOn(page, mode, [
      { name: 'notes.txt', bytes: Buffer.from('not an image') },
      { name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') },
      { name: 'empty.png', bytes: [] },
    ]);
    await expect(notice(page, 'notes.txt: not a supported file')).toBeVisible();
    await button(dropDialog(page), 'Add as layer').click();
    await expect(notice(page, /empty\.png/)).toBeVisible();
    await expect(notice(page, 'notes.txt: not a supported file')).toBeVisible();
    // The good file was added and stays.
    await expect(layerRows(page, mode)).toHaveCount(2);
    expect(pixelOf(await merged(page), 0, 0)).toEqual(RED);
  });
}

test('AC-22: pixel: a drop with a non-PNG file cannot be added as layers', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await dropOn(page, 'pixel', [
    { name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') },
    { name: 'photo.jpg', bytes: await rasterFile(page, 'jpeg') },
  ]);
  const dialog = dropDialog(page);
  await expect(button(dialog, 'Add as layer')).toBeDisabled();
  await expect(dialog.getByText('Pixel mode adds PNG files only')).toBeVisible();
  await expect(button(dialog, 'Open as document')).toBeEnabled();
  await button(dialog, 'Cancel').click();

  // Image mode adds every raster format.
  await createDocument(page, 'image', { width: 16, height: 16 });
  await dropOn(page, 'image', [
    { name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') },
    { name: 'photo.jpg', bytes: await rasterFile(page, 'jpeg') },
  ]);
  await expect(button(dialog, 'Add as layer')).toBeEnabled();
  await expect(dialog.getByText('Pixel mode adds PNG files only')).toHaveCount(0);
  await button(dialog, 'Add as layer').click();
  await expect(layerRows(page, 'image')).toHaveCount(3);
});

test("AC-25: Don't ask again remembers the choice; File ▸ Dropped Images changes it", async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const red: DraggedFile[] = [{ name: 'red.png', bytes: await solidPng(page, 4, 4, '#ff0000') }];
  expect(await (await droppedImagesItem(page, 'Ask')).getAttribute('aria-checked')).toBe('true');
  await page.keyboard.press('Escape');

  await dropOn(page, 'pixel', red);
  await dontAsk(page).check();
  await button(dropDialog(page), 'Add as layer').click();
  await expect(layerRows(page, 'pixel')).toHaveCount(2);
  await expect.poll(() => storedPref(page, 'droppedImages')).toBe('layer');

  // Later drops add without asking, also after a restart.
  await dropOn(page, 'pixel', red);
  await expect(layerRows(page, 'pixel')).toHaveCount(3);
  await expect(dropDialog(page)).toHaveCount(0);
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await dropOn(page, 'pixel', red);
  await expect(layerRows(page, 'pixel')).toHaveCount(2);
  await expect(dropDialog(page)).toHaveCount(0);
  const layer = await droppedImagesItem(page, 'Add as Layer');
  await expect(layer).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Escape');

  // The menu brings the dialog back…
  await (await droppedImagesItem(page, 'Ask')).click();
  await expect.poll(() => storedPref(page, 'droppedImages')).toBe('ask');
  await dropOn(page, 'pixel', red);
  await expect(dropDialog(page)).toBeVisible();
  await button(dropDialog(page), 'Cancel').click();

  // …and sets "Open as Document", here from Image mode.
  await createDocument(page, 'image', { width: 16, height: 16 });
  const item = await droppedImagesItem(page, 'Open as Document');
  await expect(item).toHaveAttribute('aria-checked', 'false');
  await item.click();
  await expect.poll(() => storedPref(page, 'droppedImages')).toBe('document');
  await dropOn(page, 'image', [{ name: 'photo.png', bytes: await rasterFile(page, 'png') }]);
  await expect(page.getByTitle('photo.png', { exact: true })).toBeVisible();
  await waitForEditor(page, 'image');
  await expect(sizeShown(page, 'image', [8, 6])).toBeVisible();
  await expect(dropDialog(page)).toHaveCount(0);
});

test('AC-28: a remembered Add as Layer asks when the drop cannot be added', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await (await droppedImagesItem(page, 'Add as Layer')).click();
  await expect.poll(() => storedPref(page, 'droppedImages')).toBe('layer');
  await dropOn(page, 'pixel', [{ name: 'photo.jpg', bytes: await rasterFile(page, 'jpeg') }]);
  const dialog = dropDialog(page);
  await expect(dialog).toBeVisible();
  await expect(button(dialog, 'Add as layer')).toBeDisabled();
  await button(dialog, 'Cancel').click();
  await expect(layerRows(page, 'pixel')).toHaveCount(1);
});
