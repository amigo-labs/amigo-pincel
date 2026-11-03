import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  gotoStart,
  menu,
  pixelCanvas,
  storedPref,
  waitForEditor,
} from './helpers';

// Pixel grid and custom grid (docs/specs/pixel-grid.md). Both are drawn on
// the stage-sized grid canvas over the sprite; the specs read it test-side.

// Force the download fallback: the File System Access picker would open a
// native dialog the test cannot drive.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const viewMenu = (page: Page) => page.getByRole('menu', { name: 'View', exact: true });

/** The `aria-checked` state of a View-menu checkbox entry (menu closed again). */
async function viewChecked(page: Page, label: string): Promise<boolean> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  const item = viewMenu(page).getByRole('menuitemcheckbox', { name: label });
  const checked = (await item.getAttribute('aria-checked')) === 'true';
  await page.keyboard.press('Escape');
  await expect(viewMenu(page)).toBeHidden();
  return checked;
}

/** Clicks a View-menu checkbox entry. */
async function viewToggle(page: Page, label: string): Promise<void> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await viewMenu(page).getByRole('menuitemcheckbox', { name: label }).click();
}

/** Sets the integer display zoom: 1:1, then doubles. */
async function setZoom(page: Page, zoom: 1 | 2 | 4 | 8 | 16): Promise<void> {
  await page.keyboard.press('1');
  for (let z = 1; z < zoom; z *= 2) await page.keyboard.press('+');
  await expect(page.getByRole('button', { name: 'Fit to window' })).toHaveText(`${zoom}×`);
}

/** The canvas element's pixels as a PNG, with the pointer off the stage. */
async function canvasShot(page: Page): Promise<Buffer> {
  await page.mouse.move(0, 0);
  // Let the grid repaint on the next animation frame.
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  return pixelCanvas(page).screenshot();
}

/**
 * RGBA on the grid canvas at sprite position (`sx`, `sy`) (fractions allowed),
 * mapped through the sprite canvas's on-screen box and the device pixel ratio.
 */
async function gridPixel(page: Page, sx: number, sy: number): Promise<number[]> {
  return page.getByTestId('pixel-grid').evaluate(
    (el, [x, y]) => {
      const grid = el as HTMLCanvasElement;
      const sprite = document.querySelector('[aria-label="Pincel canvas"]') as HTMLCanvasElement;
      const g = grid.getBoundingClientRect();
      const c = sprite.getBoundingClientRect();
      const scale = c.width / sprite.width;
      const dpr = grid.width / g.width;
      const px = Math.round((c.left - g.left + x * scale) * dpr);
      const py = Math.round((c.top - g.top + y * scale) * dpr);
      const ctx = grid.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      return Array.from(ctx.getImageData(px, py, 1, 1).data);
    },
    [sx, sy] as const,
  );
}

test('fresh profile: Show Pixel Grid is on, Show Grid off, and each menu entry flips (AC-01, AC-04, AC-05, AC-16)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(true);
  expect(await viewChecked(page, 'Show Grid')).toBe(false);

  await viewToggle(page, 'Show Pixel Grid');
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(false);
  await viewToggle(page, 'Show Grid');
  expect(await viewChecked(page, 'Show Grid')).toBe(true);
  await viewToggle(page, 'Show Grid');
  expect(await viewChecked(page, 'Show Grid')).toBe(false);
});

test('the pixel grid shows from zoom 8 and hides below it (AC-02, AC-03)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });

  await setZoom(page, 8);
  const on8 = await canvasShot(page);
  await viewToggle(page, 'Show Pixel Grid');
  const off8 = await canvasShot(page);
  expect(on8.equals(off8)).toBe(false);
  // Lines between sprite pixels, none inside one.
  await viewToggle(page, 'Show Pixel Grid');
  await expect.poll(async () => (await gridPixel(page, 3, 2.5))[3]).toBeGreaterThan(0);
  expect((await gridPixel(page, 3.5, 2.5))[3]).toBe(0);

  await setZoom(page, 4);
  const on4 = await canvasShot(page);
  await viewToggle(page, 'Show Pixel Grid');
  const off4 = await canvasShot(page);
  expect(on4.equals(off4)).toBe(true);
  // The entry stays checked while the zoom hides the grid.
  await viewToggle(page, 'Show Pixel Grid');
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(true);
});

test('the custom grid draws at offset + k × cell size in its colour and opacity, at any zoom (AC-05, AC-06)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 40, height: 40 });
  await setZoom(page, 4);
  await viewToggle(page, 'Show Pixel Grid');
  await viewToggle(page, 'Show Grid');

  // Default 16×16 at 0, 0: lines at 0, 16, 32 on both axes, in the default
  // blue at 60 % opacity.
  const blue = [0, 0, 255, 153];
  for (const x of [0, 16, 32]) {
    await expect.poll(() => gridPixel(page, x, 5.5)).toEqual(blue);
    expect(await gridPixel(page, 5.5, x)).toEqual(blue);
  }
  for (const x of [8, 15, 17, 24]) {
    expect((await gridPixel(page, x, 5.5))[3]).toBe(0);
    expect((await gridPixel(page, 5.5, x))[3]).toBe(0);
  }

  // Also at zoom 1, where the pixel grid never shows.
  await setZoom(page, 1);
  await expect.poll(() => gridPixel(page, 16, 5.5)).toEqual(blue);

  await viewToggle(page, 'Show Grid');
  await expect.poll(async () => (await gridPixel(page, 16, 5.5))[3]).toBe(0);
});

test("Ctrl+' toggles Show Grid and Ctrl+Shift+' Show Pixel Grid (AC-12)", async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press("ControlOrMeta+'");
  expect(await viewChecked(page, 'Show Grid')).toBe(true);
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(true);
  await page.keyboard.press("ControlOrMeta+Shift+'");
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(false);
  expect(await viewChecked(page, 'Show Grid')).toBe(true);
});

test('the show states survive a reload and are not written into the file (AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const save = async () => {
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      menu(page, 'File', 'Save (download)'),
    ]);
    return readFile(await download.path());
  };
  const before = await save();

  await viewToggle(page, 'Show Grid');
  await viewToggle(page, 'Show Pixel Grid');
  await expect
    .poll(() => storedPref(page, 'pixelGrid'))
    .toMatchObject({ showGrid: true, showPixelGrid: false });
  const after = await save();
  expect(after.equals(before)).toBe(true);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await viewChecked(page, 'Show Grid')).toBe(true);
  expect(await viewChecked(page, 'Show Pixel Grid')).toBe(false);
});

const settings = (page: Page) => page.getByRole('dialog', { name: 'Grid Settings' });

/** Opens View ▸ Grid Settings… and returns its four geometry fields' values. */
async function openSettings(page: Page): Promise<string[]> {
  await menu(page, 'View', 'Grid Settings…');
  const d = settings(page);
  await expect(d).toBeVisible();
  return Promise.all(
    ['Cell width', 'Cell height', 'Offset X', 'Offset Y'].map((l) => d.getByLabel(l).inputValue()),
  );
}

/** Fills the open Grid Settings dialog; `undefined` leaves a field alone. */
async function fillSettings(
  page: Page,
  v: { w?: number; h?: number; x?: number; y?: number; color?: string; opacity?: number },
): Promise<void> {
  const d = settings(page);
  const fields: [string, number | string | undefined][] = [
    ['Cell width', v.w],
    ['Cell height', v.h],
    ['Offset X', v.x],
    ['Offset Y', v.y],
    ['Line color', v.color],
    ['Opacity (%)', v.opacity],
  ];
  for (const [label, value] of fields) {
    if (value !== undefined) await d.getByLabel(label).fill(String(value));
  }
}

/** Saves the document through the download fallback and returns the bytes. */
async function saveBytes(page: Page): Promise<Buffer> {
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    menu(page, 'File', 'Save (download)'),
  ]);
  return readFile(await download.path());
}

test('Grid Settings starts at 16×16 / 0, 0 and redraws the grid with the new values (AC-07, AC-08)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 40, height: 40 });
  await setZoom(page, 4);
  await viewToggle(page, 'Show Pixel Grid');
  await viewToggle(page, 'Show Grid');
  expect(await openSettings(page)).toEqual(['16', '16', '0', '0']);
  await fillSettings(page, { w: 10, h: 12, x: 4, y: 2, color: '#ff0000', opacity: 100 });
  await settings(page).getByRole('button', { name: 'OK' }).click();
  await expect(settings(page)).toBeHidden();

  const red = [255, 0, 0, 255];
  for (const x of [4, 14, 24, 34]) await expect.poll(() => gridPixel(page, x, 5.5)).toEqual(red);
  for (const y of [2, 14, 26, 38]) expect(await gridPixel(page, 5.5, y)).toEqual(red);
  expect((await gridPixel(page, 16, 5.5))[3]).toBe(0);
  expect((await gridPixel(page, 5.5, 16))[3]).toBe(0);
  expect(await openSettings(page)).toEqual(['10', '12', '4', '2']);
});

test('undo restores the previous cell size and offset in one step, redo re-applies them (AC-10)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 40, height: 40 });
  await viewToggle(page, 'Show Grid');
  await openSettings(page);
  await fillSettings(page, { w: 8, h: 5, x: 1, y: 3 });
  await settings(page).getByRole('button', { name: 'OK' }).click();
  await expect(settings(page)).toBeHidden();
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();

  await page.keyboard.press('ControlOrMeta+z');
  await expect(page.getByText('undo 0 / redo 1')).toBeVisible();
  expect(await openSettings(page)).toEqual(['16', '16', '0', '0']);
  await settings(page).getByRole('button', { name: 'Cancel' }).click();
  await page.keyboard.press('ControlOrMeta+Shift+z');
  expect(await openSettings(page)).toEqual(['8', '5', '1', '3']);
});

test('out-of-range cell size or offset is not applied (AC-13)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openSettings(page);
  const d = settings(page);
  for (const bad of [{ w: 0 }, { h: 65536 }, { x: 32768 }, { y: -32769 }, { w: 2.5 }]) {
    await fillSettings(page, { w: 16, h: 16, x: 0, y: 0, ...bad });
    await d.getByRole('button', { name: 'OK' }).click();
    await expect(d.getByRole('alert')).toContainText('1–65535');
    await expect(d).toBeVisible();
  }
  await d.getByRole('button', { name: 'Cancel' }).click();
  expect(await openSettings(page)).toEqual(['16', '16', '0', '0']);
  // Nothing reached the undo history.
  await expect(page.getByText('undo 0 / redo 0')).toBeVisible();
});

test('a changed grid survives saving as .aseprite and reopening (AC-09)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openSettings(page);
  await fillSettings(page, { w: 12, h: 8, x: 3, y: -5 });
  await settings(page).getByRole('button', { name: 'OK' }).click();
  const bytes = await saveBytes(page);

  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'grid.aseprite',
    mimeType: 'application/octet-stream',
    buffer: bytes,
  });
  await waitForEditor(page, 'pixel');
  expect(await openSettings(page)).toEqual(['12', '8', '3', '-5']);
});

test('line colour and opacity persist per user and stay out of the file (AC-15)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const before = await saveBytes(page);
  await openSettings(page);
  await fillSettings(page, { color: '#00ff00', opacity: 40 });
  await settings(page).getByRole('button', { name: 'OK' }).click();
  await expect
    .poll(() => storedPref(page, 'pixelGrid'))
    .toMatchObject({ color: '#00ff00', opacity: 40 });
  expect((await saveBytes(page)).equals(before)).toBe(true);

  await page.reload();
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await setZoom(page, 4);
  await viewToggle(page, 'Show Pixel Grid');
  await viewToggle(page, 'Show Grid');
  await expect.poll(() => gridPixel(page, 16, 5.5)).toEqual([0, 255, 0, 102]);
  await menu(page, 'View', 'Grid Settings…');
  await expect(settings(page).getByLabel('Line color')).toHaveValue('#00ff00');
  await expect(settings(page).getByLabel('Opacity (%)')).toHaveValue('40');
});

test('on a tilemap layer, Use tile size sets the cell size to the tile size at offset 0, 0 (AC-17)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openSettings(page);
  await expect(settings(page).getByRole('button', { name: /^Use tile size/ })).toHaveCount(0);
  await settings(page).getByRole('button', { name: 'Cancel' }).click();

  const tilesets = page.getByRole('region', { name: 'Tilesets' });
  await tilesets.getByRole('button', { name: 'Tilesets', exact: true }).click();
  await tilesets.getByRole('button', { name: 'Add tileset' }).click();
  await tilesets.getByLabel('Tile W').fill('8');
  await tilesets.getByLabel('Tile H').fill('4');
  await tilesets.getByRole('button', { name: 'Add', exact: true }).click();
  const addLayer = tilesets.getByRole('button', { name: /^Add tilemap layer using / });
  const tileset = ((await addLayer.getAttribute('aria-label')) ?? '').replace(
    'Add tilemap layer using ',
    '',
  );
  await addLayer.click();
  // Make the new tilemap layer the active one.
  const tilemapRow = page
    .getByRole('list', { name: 'Layer list' })
    .getByTitle(`${tileset} Layer`, { exact: true });
  await tilemapRow.click();
  await expect(tilemapRow).toHaveAttribute('aria-pressed', 'true');

  await openSettings(page);
  await fillSettings(page, { x: 3, y: 3 });
  const d = settings(page);
  await d.getByRole('button', { name: 'Use tile size (8×4)' }).click();
  await expect(d.getByLabel('Cell width')).toHaveValue('8');
  await expect(d.getByLabel('Cell height')).toHaveValue('4');
  await expect(d.getByLabel('Offset X')).toHaveValue('0');
  await expect(d.getByLabel('Offset Y')).toHaveValue('0');
  await d.getByRole('button', { name: 'OK' }).click();
  expect(await openSettings(page)).toEqual(['8', '4', '0', '0']);
});
