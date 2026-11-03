import { readFile } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, pixelCanvas } from './helpers';

// Pixel-mode slices (docs/specs/slices.md): the Slice tool, the Slices panel
// and the active slice's overlay. Slice state is read from the panel's rows;
// the overlay is sampled test-side from its canvas (the app never reads it).

const N = 16;
type At = (px: number, py: number) => readonly [number, number];

/** A fresh N×N sprite with the Slices panel open; the sprite-pixel → page mapper. */
async function setup(page: Page): Promise<At> {
  await createDocument(page, 'pixel', { width: N, height: N });
  await slicesPanel(page).getByRole('button', { name: 'Slices', exact: true }).click();
  await expect(slicesPanel(page).getByText('no slices yet')).toBeVisible();
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return (px, py) =>
    [box.x + ((px + 0.5) * box.width) / N, box.y + ((py + 0.5) * box.height) / N] as const;
}

const slicesPanel = (page: Page) => page.getByRole('region', { name: 'Slices' });
const sliceTool = (page: Page) =>
  page.getByRole('toolbar', { name: 'Tools' }).getByRole('button', { name: 'Slice', exact: true });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });

/** The panel row of the slice called `name`. */
function row(page: Page, name: string): Locator {
  return slicesPanel(page)
    .getByRole('listitem')
    .filter({ has: page.getByRole('button', { name: `Activate slice ${name}`, exact: true }) });
}

/** The row's activate swatch: `aria-pressed` marks the active slice. */
const activator = (page: Page, name: string) =>
  row(page, name).getByRole('button', { name: `Activate slice ${name}`, exact: true });

/** Asserts the row's X, Y, W, H fields. */
async function expectBounds(r: Locator, [x, y, w, h]: number[]): Promise<void> {
  for (const [label, v] of [
    ['X', x],
    ['Y', y],
    ['W', w],
    ['H', h],
  ] as const) {
    await expect(r.getByLabel(label, { exact: true })).toHaveValue(String(v));
  }
}

/** Types `value` into the row field `label` and commits it (change on blur). */
async function commitField(r: Locator, label: string, value: string): Promise<void> {
  const field = r.getByLabel(label, { exact: true });
  await field.fill(value);
  await field.blur();
}

/** Drags with the active tool from sprite pixel `a` to `b`, then parks the pointer. */
async function dragPixels(page: Page, at: At, a: [number, number], b: [number, number]) {
  await page.mouse.move(...at(...a));
  await page.mouse.down();
  await page.mouse.move(...at(...b), { steps: 4 });
  await page.mouse.up();
  await page.mouse.move(0, 0);
}

const overlay = (page: Page) => pixelCanvas(page).locator('xpath=following-sibling::canvas[1]');

/** RGBA of sprite pixel (x, y) on the overlay canvas (sprite-sized backing store). */
function overlayPixel(page: Page, x: number, y: number): Promise<number[]> {
  return overlay(page).evaluate(
    (el, [px, py]) => {
      const ctx = (el as HTMLCanvasElement).getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      return Array.from(ctx.getImageData(px, py, 1, 1).data);
    },
    [x, y] as const,
  );
}

/** Number of opaque overlay pixels. */
function overlayInk(page: Page): Promise<number> {
  return overlay(page).evaluate((el) => {
    const c = el as HTMLCanvasElement;
    const ctx = c.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    const { data } = ctx.getImageData(0, 0, c.width, c.height);
    let n = 0;
    for (let i = 3; i < data.length; i += 4) if ((data[i] ?? 0) > 0) n += 1;
    return n;
  });
}

const alphaAt = async (page: Page, x: number, y: number) => (await overlayPixel(page, x, y))[3];
const WHITE = [255, 255, 255, 255];
const BLACK = [0, 0, 0, 255];

test('a Slice-tool drag creates the active slice "Slice n" in the foreground color', async ({
  page,
}) => {
  const at = await setup(page);
  await page.getByLabel('Foreground color hex value').fill('#22cc88');
  await sliceTool(page).click();
  // Bottom-right to top-left: the bounds start at the smaller corner.
  await dragPixels(page, at, [6, 9], [2, 3]);
  const first = row(page, 'Slice 1');
  await expectBounds(first, [2, 3, 5, 7]);
  await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'true');
  await expect(activator(page, 'Slice 1')).toHaveCSS('background-color', 'rgb(34, 204, 136)');

  // A second slice once none is active: n counts the slices after adding.
  await activator(page, 'Slice 1').click();
  await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'false');
  await dragPixels(page, at, [8, 8], [12, 10]);
  await expectBounds(row(page, 'Slice 2'), [8, 8, 5, 3]);
  await expect(activator(page, 'Slice 2')).toHaveAttribute('aria-pressed', 'true');

  // One undo removes the creation.
  await undoButton(page).click();
  await expect(row(page, 'Slice 2')).toHaveCount(0);
  await expect(row(page, 'Slice 1')).toHaveCount(1);
});

test('a Slice-tool click creates a 1 × 1 slice at that pixel', async ({ page }) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await page.mouse.click(...at(4, 5));
  await page.mouse.move(0, 0);
  await expectBounds(row(page, 'Slice 1'), [4, 5, 1, 1]);
});

test('redrawing the active slice moves its bounds, 9-patch and pivot along', async ({ page }) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [2, 2], [9, 9]);
  const r = row(page, 'Slice 1');
  await r.getByLabel('9-patch').check();
  await r.getByLabel('Pivot').check();
  await commitField(r, 'pX', '3');
  await expect(r.getByLabel('pX', { exact: true })).toHaveValue('3');

  await dragPixels(page, at, [4, 5], [12, 14]);
  await expectBounds(r, [4, 5, 9, 10]);
  // Center and pivot keep their coordinates relative to the bounds.
  for (const [label, v] of [
    ['cX', '1'],
    ['cY', '1'],
    ['cW', '6'],
    ['cH', '6'],
    ['pX', '3'],
    ['pY', '0'],
  ] as const) {
    await expect(r.getByLabel(label, { exact: true })).toHaveValue(v);
  }
  // …so on the canvas they moved with the slice: center outline from
  // (5, 6), pivot crosshair at (7, 5).
  await expect.poll(() => alphaAt(page, 5, 6)).toBe(255);
  await expect.poll(() => overlayPixel(page, 7, 5)).toEqual(BLACK);

  await undoButton(page).click();
  await expectBounds(r, [2, 2, 8, 8]);
});

test('the Add slice form creates a whole-canvas slice', async ({ page }) => {
  await setup(page);
  const panel = slicesPanel(page);
  await panel.getByRole('button', { name: 'Add slice' }).click();
  const name = panel.getByLabel('Name', { exact: true });
  const color = panel.getByLabel('Color', { exact: true });
  await expect(name).toHaveValue('Slice');
  await expect(color).toHaveValue('#3b82f6');

  // An empty or blank name is refused.
  for (const blank of ['', '   ']) {
    await name.fill(blank);
    await panel.getByRole('button', { name: 'Add', exact: true }).click();
    await expect(panel.getByRole('alert')).toHaveText('name is required');
    await expect(panel.getByRole('listitem')).toHaveCount(0);
  }

  await name.fill('Hitbox');
  await color.fill('#ff8800');
  await panel.getByRole('button', { name: 'Add', exact: true }).click();
  await expectBounds(row(page, 'Hitbox'), [0, 0, N, N]);
  await expect(activator(page, 'Hitbox')).toHaveAttribute('aria-pressed', 'true');
  await expect(activator(page, 'Hitbox')).toHaveCSS('background-color', 'rgb(255, 136, 0)');

  await undoButton(page).click();
  await expect(row(page, 'Hitbox')).toHaveCount(0);
});

test('clicking a row activates the slice and the Slice tool, clicking again deactivates', async ({
  page,
}) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [1, 1], [4, 4]);
  await activator(page, 'Slice 1').click();
  await dragPixels(page, at, [8, 8], [12, 12]);
  await expect(activator(page, 'Slice 2')).toHaveAttribute('aria-pressed', 'true');

  await page.getByTitle('Pencil (B)').click();
  await expect(sliceTool(page)).toHaveAttribute('aria-pressed', 'false');
  await row(page, 'Slice 1').getByRole('button', { name: 'Slice 1', exact: true }).click();
  await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'true');
  await expect(activator(page, 'Slice 2')).toHaveAttribute('aria-pressed', 'false');
  await expect(sliceTool(page)).toHaveAttribute('aria-pressed', 'true');

  await row(page, 'Slice 1').getByRole('button', { name: 'Slice 1', exact: true }).click();
  await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'false');
  await expect(activator(page, 'Slice 2')).toHaveAttribute('aria-pressed', 'false');
});

test.describe('overlay', () => {
  // Force the download fallback for PNG export (as in compose.spec.ts).
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
    });
  });

  /** File ▸ Export ▸ PNG (current frame); the downloaded bytes. */
  async function exportFrame(page: Page): Promise<Buffer> {
    const download = page.waitForEvent('download');
    await menu(page, 'File', 'Export', 'PNG (current frame)');
    return readFile(await (await download).path());
  }

  test('the active slice shows ants, its 9-patch center and pivot, off the sprite', async ({
    page,
  }) => {
    const at = await setup(page);
    const blank = await exportFrame(page);
    await page.getByLabel('Foreground color hex value').fill('#ff0000');
    await sliceTool(page).click();
    await dragPixels(page, at, [2, 3], [11, 10]);
    const r = row(page, 'Slice 1');
    await expectBounds(r, [2, 3, 10, 8]);

    // Marching ants on the bounds' corners, nothing inside.
    for (const [x, y] of [
      [2, 3],
      [11, 3],
      [2, 10],
      [11, 10],
    ] as const) {
      await expect.poll(() => alphaAt(page, x, y)).toBe(255);
    }
    expect(await alphaAt(page, 6, 6)).toBe(0);

    // 9-patch center (1, 1, 8, 6) relative → outline from (3, 4) to (10, 9),
    // in the slice's color.
    await r.getByLabel('9-patch').check();
    for (const [x, y] of [
      [3, 4],
      [10, 4],
      [3, 9],
      [10, 9],
    ] as const) {
      await expect.poll(() => overlayPixel(page, x, y)).toEqual([255, 0, 0, 255]);
    }
    expect(await alphaAt(page, 1, 1)).toBe(0);

    // Pivot (4, 3) relative → crosshair centered on (6, 6).
    await r.getByLabel('Pivot').check();
    await commitField(r, 'pX', '4');
    await commitField(r, 'pY', '3');
    await expect.poll(() => overlayPixel(page, 6, 6)).toEqual(BLACK);
    expect(await overlayPixel(page, 5, 6)).toEqual(WHITE);
    expect(await overlayPixel(page, 7, 6)).toEqual(WHITE);
    expect(await overlayPixel(page, 6, 5)).toEqual(WHITE);
    expect(await overlayPixel(page, 6, 7)).toEqual(WHITE);
    expect(await overlayPixel(page, 4, 6)).toEqual(BLACK);

    // None of it is in the sprite.
    expect((await exportFrame(page)).equals(blank)).toBe(true);
  });

  test('undo of the active slice clears its overlay and highlight', async ({ page }) => {
    const at = await setup(page);
    await sliceTool(page).click();
    await dragPixels(page, at, [2, 2], [8, 8]);
    await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'true');
    await expect.poll(() => overlayInk(page)).toBeGreaterThan(0);

    await undoButton(page).click();
    await expect(row(page, 'Slice 1')).toHaveCount(0);
    await expect.poll(() => overlayInk(page)).toBe(0);
    // The slice comes back on redo, but no longer as the active one.
    await redoButton(page).click();
    await expect(activator(page, 'Slice 1')).toHaveAttribute('aria-pressed', 'false');
    await expect.poll(() => overlayInk(page)).toBe(0);
  });
});

test('row X / Y / W / H commits are one undo step each, sizes below 1 are refused', async ({
  page,
}) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [2, 2], [9, 9]);
  const r = row(page, 'Slice 1');
  await expectBounds(r, [2, 2, 8, 8]);

  await commitField(r, 'X', '5');
  await expectBounds(r, [5, 2, 8, 8]);
  // A field shows what was typed either way; the ants show the slice moved.
  await expect.poll(() => alphaAt(page, 2, 5)).toBe(0);
  await expect.poll(() => alphaAt(page, 5, 5)).toBe(255);
  await commitField(r, 'Y', '1');
  await commitField(r, 'W', '3');
  await commitField(r, 'H', '4');
  await expectBounds(r, [5, 1, 3, 4]);
  // The ants follow: right edge now at x = 7.
  await expect.poll(() => alphaAt(page, 7, 4)).toBe(255);

  await undoButton(page).click();
  await expectBounds(r, [5, 1, 3, 8]);
  await undoButton(page).click();
  await expectBounds(r, [5, 1, 8, 8]);
  await undoButton(page).click();
  await expectBounds(r, [5, 2, 8, 8]);

  // W / H below 1 (and the 9-patch center's) are refused with a message
  // that names the field as labelled.
  for (const [label, value] of [
    ['W', '0'],
    ['H', '-3'],
  ] as const) {
    await commitField(r, label, value);
    await expect(r.getByRole('alert')).toHaveText(`${label} must be a positive integer`);
    // The refused entry snaps back to the stored size.
    await expect(r.getByLabel(label, { exact: true })).toHaveValue('8');
  }
  await r.getByLabel('9-patch').check();
  await commitField(r, 'cW', '0');
  await expect(r.getByRole('alert')).toHaveText('cW must be a positive integer');
  // The slice kept its size: the ants still end at (12, 9), the center
  // outline still spans six pixels.
  await expect.poll(() => alphaAt(page, 12, 9)).toBe(255);
  await expect.poll(() => alphaAt(page, 11, 8)).toBe(255);
  // Undo takes back the 9-patch tick, then the X edit: nothing else landed.
  await undoButton(page).click();
  await expect(r.getByLabel('9-patch')).not.toBeChecked();
  await undoButton(page).click();
  await expect(r.getByLabel('X', { exact: true })).toHaveValue('2');
  await expect.poll(() => alphaAt(page, 9, 9)).toBe(255);
  await expect.poll(() => alphaAt(page, 12, 9)).toBe(0);
});

test('ticking 9-patch insets a center by 1 px, relative to the bounds', async ({ page }) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [4, 4], [11, 9]);
  const r = row(page, 'Slice 1');
  await expectBounds(r, [4, 4, 8, 6]);
  const box = r.getByLabel('9-patch');
  await box.check();
  for (const [label, v] of [
    ['cX', '1'],
    ['cY', '1'],
    ['cW', '6'],
    ['cH', '4'],
  ] as const) {
    await expect(r.getByLabel(label, { exact: true })).toHaveValue(v);
  }
  // Edited relative to the bounds: cX 2 puts the outline's left edge at x = 6.
  await commitField(r, 'cX', '2');
  await expect.poll(() => alphaAt(page, 6, 7)).toBe(255);
  expect(await alphaAt(page, 5, 7)).toBe(0);

  await undoButton(page).click();
  await expect(r.getByLabel('cX', { exact: true })).toHaveValue('1');
  await undoButton(page).click();
  await expect(box).not.toBeChecked();
  await expect(r.getByLabel('cX', { exact: true })).toHaveCount(0);

  await box.check();
  await box.uncheck();
  await expect(r.getByLabel('cX', { exact: true })).toHaveCount(0);

  // A 1 × 1 slice gets a 1 × 1 center.
  await activator(page, 'Slice 1').click();
  await page.mouse.click(...at(14, 14));
  await page.mouse.move(0, 0);
  const small = row(page, 'Slice 2');
  await small.getByLabel('9-patch').check();
  for (const [label, v] of [
    ['cX', '1'],
    ['cY', '1'],
    ['cW', '1'],
    ['cH', '1'],
  ] as const) {
    await expect(small.getByLabel(label, { exact: true })).toHaveValue(v);
  }
});

test('ticking Pivot sets it at the top-left, relative to the bounds', async ({ page }) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [5, 6], [10, 11]);
  const r = row(page, 'Slice 1');
  const box = r.getByLabel('Pivot');
  await box.check();
  await expect(r.getByLabel('pX', { exact: true })).toHaveValue('0');
  await expect(r.getByLabel('pY', { exact: true })).toHaveValue('0');
  await expect.poll(() => overlayPixel(page, 5, 6)).toEqual(BLACK);

  // Negative values are allowed: (-2, 1) relative → (3, 7) on the canvas.
  await commitField(r, 'pX', '-2');
  await commitField(r, 'pY', '1');
  await expect(r.getByLabel('pX', { exact: true })).toHaveValue('-2');
  await expect.poll(() => overlayPixel(page, 3, 7)).toEqual(BLACK);

  await undoButton(page).click();
  await undoButton(page).click();
  await expect(r.getByLabel('pX', { exact: true })).toHaveValue('0');
  await undoButton(page).click();
  await expect(box).not.toBeChecked();
  await expect(r.getByLabel('pX', { exact: true })).toHaveCount(0);

  await box.check();
  await box.uncheck();
  await expect(r.getByLabel('pX', { exact: true })).toHaveCount(0);
});

test('the slice rows follow the UI language', async ({ page }) => {
  const at = await setup(page);
  await sliceTool(page).click();
  await dragPixels(page, at, [1, 1], [4, 4]);
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();

  const r = slicesPanel(page).getByRole('listitem');
  await expect(r.getByText('ID 0 · 1 Key · Frame 0', { exact: true })).toBeVisible();
  const bounds = r.getByRole('group', { name: 'Begrenzung von Slice Slice 1' });
  for (const label of ['X', 'Y', 'B', 'H']) {
    await expect(bounds.getByLabel(label, { exact: true })).toBeVisible();
  }
  await commitField(r, 'B', '0');
  await expect(r.getByRole('alert')).toHaveText('B muss eine positive ganze Zahl sein');
  await r.getByLabel('9-Patch').check();
  const center = r.getByRole('group', { name: '9-Patch-Mittelrechteck' });
  for (const label of ['cX', 'cY', 'cB', 'cH']) {
    await expect(center.getByLabel(label, { exact: true })).toBeVisible();
  }
});
