import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, menu, pixelCanvas, storedPref } from './helpers';

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
