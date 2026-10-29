import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, menu, pixelCanvas } from './helpers';

// Pixel-mode canvas updates and the overlay layer
// (docs/specs/pixel-compose.md AC-27, AC-28). Pixels are compared through
// element screenshots; the app itself never reads canvas pixels.

// Force the download fallback for PNG export (as in export.spec.ts).
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
  });
});

/** Screenshot of the canvas stack, with the pointer parked off the canvas. */
async function shot(page: Page): Promise<Buffer> {
  await page.mouse.move(0, 0);
  return pixelCanvas(page).screenshot();
}

/** Polls until the canvas screenshot equals (or differs from) `ref`. */
async function expectCanvas(page: Page, ref: Buffer, same: boolean): Promise<void> {
  await expect.poll(async () => (await shot(page)).equals(ref)).toBe(same);
}

/** The base layer alone: the overlay canvas hidden for one screenshot. */
async function baseShot(page: Page): Promise<Buffer> {
  const overlay = pixelCanvas(page).locator('xpath=following-sibling::canvas[1]');
  await overlay.evaluate((el) => (el.style.visibility = 'hidden'));
  const s = await pixelCanvas(page).screenshot();
  await overlay.evaluate((el) => (el.style.visibility = ''));
  return s;
}

/** File ▸ Export ▸ PNG (current frame); the downloaded bytes. */
async function exportFrame(page: Page): Promise<Buffer> {
  const download = page.waitForEvent('download');
  await menu(page, 'File', 'Export', 'PNG (current frame)');
  return readFile(await (await download).path());
}

test('pixel mode: the canvas follows paint, undo / redo, frames and layer visibility', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const blank = await shot(page);

  await page.getByTitle('Pencil (B)').click();
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await pixelCanvas(page).click();
  await expectCanvas(page, blank, false);
  const painted = await shot(page);

  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const redo = page.getByRole('button', { name: 'Redo', exact: true });
  await undo.click();
  await expectCanvas(page, blank, true);
  await redo.click();
  await expectCanvas(page, painted, true);

  // A new frame has no cel; switching back shows the stroke again.
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
  await expectCanvas(page, blank, true);
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await expectCanvas(page, painted, true);

  // Hiding the layer empties the canvas; showing it restores the stroke.
  await page.getByRole('button', { name: /^Hide / }).click();
  await expectCanvas(page, blank, true);
  await page.getByRole('button', { name: /^Show / }).click();
  await expectCanvas(page, painted, true);
});

test('pixel mode: a drag preview and the selection marquee stay off the sprite', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const blank = await shot(page);
  const blankExport = await exportFrame(page);
  const canvas = pixelCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');

  // Mid-drag: the marquee preview shows, the base layer is untouched.
  await page.getByRole('button', { name: 'Marquee: Rectangle Select' }).click();
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.7, { steps: 4 });
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  expect((await baseShot(page)).equals(blank)).toBe(true);
  await page.mouse.up();

  // The committed selection's marquee: above the sprite, not in its pixels.
  await expectCanvas(page, blank, false);
  expect((await baseShot(page)).equals(blank)).toBe(true);
  expect((await exportFrame(page)).equals(blankExport)).toBe(true);
});

test('pixel mode: the tile-stamp hover and the active slice stay off the sprite', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const canvas = pixelCanvas(page);
  const blank = await shot(page);
  const blankExport = await exportFrame(page);

  // A tileset, a tilemap layer on it and a picked tile arm the Tile Stamp
  // (an empty tilemap layer adds no pixels).
  const tilesets = page.getByRole('region', { name: 'Tilesets' });
  await tilesets.getByRole('button', { name: 'Tilesets', exact: true }).click();
  await tilesets.getByRole('button', { name: 'Add tileset' }).click();
  await tilesets.getByLabel('Tile W').fill('4');
  await tilesets.getByLabel('Tile H').fill('4');
  await tilesets.getByRole('button', { name: 'Add', exact: true }).click();
  await tilesets.getByRole('button', { name: /^Add tile to / }).click();
  await tilesets.getByRole('button', { name: /^Add tilemap layer using / }).click();

  const tile = tilesets.getByRole('button', { name: /^Select tile \d+ of / }).last();
  if ((await tile.getAttribute('aria-pressed')) !== 'true') await tile.click();
  const stamp = page.getByRole('button', { name: 'Tile Stamp' });
  if ((await stamp.getAttribute('aria-pressed')) !== 'true') await stamp.click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.4);
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  expect((await baseShot(page)).equals(blank)).toBe(true);
  expect((await exportFrame(page)).equals(blankExport)).toBe(true);

  // A slice drawn with the Slice tool and activated: accents only.
  await page.getByRole('button', { name: 'Slice', exact: true }).click();
  await drag(page, canvas, [0.2, 0.2], [0.6, 0.6]);
  const slices = page.getByRole('region', { name: 'Slices' });
  await slices.getByRole('button', { name: 'Slices', exact: true }).click();
  const activate = slices.getByRole('button', { name: /^Activate slice / }).first();
  if ((await activate.getAttribute('aria-pressed')) !== 'true') await activate.click();
  await expectCanvas(page, blank, false);
  expect((await baseShot(page)).equals(blank)).toBe(true);
  expect((await exportFrame(page)).equals(blankExport)).toBe(true);
});
