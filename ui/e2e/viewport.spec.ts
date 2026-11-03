import { readFile } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  waitForEditor,
} from './helpers';

// Viewport input in both modes (docs/specs/pen-input.md AC-07 – AC-13):
// wheel and keyboard zoom, fit on create / open / recover, refit on resize
// and Space / middle-button pan. The view is read from the canvas's
// on-screen box and the zoom readout, never from canvas pixels.

type Mode = 'pixel' | 'image';
type Pt = [number, number];
const DOC = { width: 64, height: 48 };

// Force the download / <input type=file> fallbacks for Save and Open….
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** The element the canvas is centred in: Pixel's stage, Image's viewport. */
const stageOf = (page: Page, mode: Mode) => canvasOf(page, mode).locator('xpath=../..');

async function boxOf(target: Locator) {
  const box = await target.boundingBox();
  if (!box) throw new Error('element is not visible');
  return box;
}

/** The zoom readout as a number (`8×` or `125%`). */
async function zoomOf(page: Page): Promise<number> {
  const text = await page.getByRole('button', { name: 'Fit to window' }).textContent();
  return Number(text?.replace(/[×%]/, ''));
}

/** Document coordinate under the client point `p`. */
async function docAt(canvas: Locator, size: typeof DOC, [x, y]: Pt): Promise<Pt> {
  const box = await boxOf(canvas);
  return [((x - box.x) / box.width) * size.width, ((y - box.y) / box.height) * size.height];
}

/** Waits for the zoom readout to leave `from`, then returns the new value. */
async function zoomChangedFrom(page: Page, from: number): Promise<number> {
  await expect.poll(() => zoomOf(page)).not.toBe(from);
  return zoomOf(page);
}

/**
 * Asserts the document is shown whole and centred at the fit zoom: Pixel at
 * the largest integer zoom leaving 24 CSS px on every side, Image at the
 * largest zoom leaving 16 CSS px.
 */
async function expectFitted(page: Page, mode: Mode, size: typeof DOC): Promise<void> {
  const canvas = canvasOf(page, mode);
  const stage = await boxOf(stageOf(page, mode));
  await expect
    .poll(async () => {
      const box = await boxOf(canvas);
      const cx = box.x + box.width / 2 - (stage.x + stage.width / 2);
      const cy = box.y + box.height / 2 - (stage.y + stage.height / 2);
      return Math.abs(cx) < 1 && Math.abs(cy) < 1;
    })
    .toBe(true);
  const zoom = await zoomOf(page);
  if (mode === 'pixel') {
    expect(Number.isInteger(zoom)).toBe(true);
    const fits = (z: number) =>
      size.width * z + 48 <= stage.width && size.height * z + 48 <= stage.height;
    expect(fits(zoom)).toBe(true);
    if (zoom < 64) expect(fits(zoom + 1)).toBe(false);
  } else {
    const box = await boxOf(canvas);
    const marginX = (stage.width - box.width) / 2;
    const marginY = (stage.height - box.height) / 2;
    expect(Math.min(marginX, marginY)).toBeGreaterThan(15.5);
    if (zoom < 6400) expect(Math.min(marginX, marginY)).toBeLessThan(16.5);
    expect(zoom).toBeCloseTo((box.width / size.width) * 100, -1);
  }
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: keys zoom in, out, to fit and to 1:1 outside text fields`, async ({
    page,
  }) => {
    await createDocument(page, mode, DOC);
    const fit = await zoomOf(page);
    const one = mode === 'pixel' ? 1 : 100;

    await page.keyboard.press('+');
    const in1 = await zoomChangedFrom(page, fit);
    expect(in1).toBeGreaterThan(fit);
    await page.keyboard.press('=');
    const in2 = await zoomChangedFrom(page, in1);
    expect(in2).toBeGreaterThan(in1);
    await page.keyboard.press('-');
    const out1 = await zoomChangedFrom(page, in2);
    expect(out1).toBeLessThan(in2);
    await page.keyboard.press('_');
    const out2 = await zoomChangedFrom(page, out1);
    expect(out2).toBeLessThan(out1);
    await page.keyboard.press('1');
    await expect.poll(() => zoomOf(page)).toBe(one);
    await page.keyboard.press('0');
    await expect.poll(() => zoomOf(page)).toBe(fit);

    // The same keys typed into a text field stay there.
    await page.getByRole('textbox', { name: 'Foreground color hex value' }).focus();
    for (const key of ['+', '=', '-', '_', '1']) {
      await page.keyboard.press(key);
      expect(await zoomOf(page)).toBe(fit);
    }
  });

  test(`${mode} mode: a new document is fitted and centred`, async ({ page }) => {
    await createDocument(page, mode, { width: 40, height: 20 });
    await expectFitted(page, mode, { width: 40, height: 20 });
    // Zoomed away, then a second, differently shaped new document fits too.
    await page.keyboard.press('1');
    await menu(page, 'File', 'New…');
    const dialog = page.getByRole('dialog', { name: 'New document' });
    await dialog.getByLabel('Width').fill('24');
    await dialog.getByLabel('Height').fill('30');
    await dialog.getByRole('button', { name: 'Create' }).click();
    await expect(dialog).toBeHidden();
    await waitForEditor(page, mode);
    await expectFitted(page, mode, { width: 24, height: 30 });
  });

  test(`${mode} mode: an opened document is fitted and centred`, async ({ page }) => {
    await createDocument(page, mode, { width: 40, height: 20 });
    await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      menu(page, 'File', mode === 'pixel' ? 'Save (download)' : 'Save'),
    ]);
    const bytes = await readFile(await download.path());

    await gotoStart(page);
    const [chooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.getByRole('button', { name: 'Open…' }).click(),
    ]);
    await chooser.setFiles({
      name: download.suggestedFilename(),
      mimeType: 'application/octet-stream',
      buffer: bytes,
    });
    await waitForEditor(page, mode);
    await expectFitted(page, mode, { width: 40, height: 20 });
  });

  test(`${mode} mode: a recovered document is fitted and centred`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 40, height: 20 });
    if (mode === 'pixel') await page.keyboard.press('b');
    await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
    await page.clock.fastForward(31_000);
    await page.waitForTimeout(500);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
    await dialog.getByRole('button', { name: 'Recover' }).first().click();
    await expect(dialog).toHaveCount(0);
    await expectFitted(page, mode, { width: 40, height: 20 });
  });

  test(`${mode} mode: Space-drag pans with any tool and paints nothing`, async ({ page }) => {
    await createDocument(page, mode, DOC);
    const canvas = canvasOf(page, mode);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    // Brush, fill and shapes: each would change pixels on a plain drag.
    for (const key of ['b', 'g', 'u']) {
      await page.keyboard.press(key);
      const before = await boxOf(canvas);
      await page.keyboard.down('Space');
      await drag(page, canvas, [0.3, 0.3], [0.6, 0.5]);
      await page.keyboard.up('Space');
      const dx = before.width * 0.3;
      const dy = before.height * 0.2;
      await expect.poll(async () => (await boxOf(canvas)).x - before.x).toBeCloseTo(dx, 0);
      expect((await boxOf(canvas)).y - before.y).toBeCloseTo(dy, 0);
      await expect(undo).toBeDisabled();
    }
  });
}

test('pixel mode: the wheel steps integer zooms about the cursor, 1× to 64×', async ({
  page,
}) => {
  await createDocument(page, 'pixel', DOC);
  const canvas = pixelCanvas(page);
  const box = await boxOf(canvas);
  const cursor: Pt = [box.x + box.width * 0.3, box.y + box.height * 0.7];
  await page.mouse.move(...cursor);

  let zoom = await zoomOf(page);
  for (const deltaY of [-100, -100, 100, 100, 100]) {
    const before = await docAt(canvas, DOC, cursor);
    await page.mouse.wheel(0, deltaY);
    const next = await zoomChangedFrom(page, zoom);
    expect(Number.isInteger(next)).toBe(true);
    if (deltaY < 0) expect(next).toBeGreaterThan(zoom);
    else expect(next).toBeLessThan(zoom);
    await expect
      .poll(async () => {
        const after = await docAt(canvas, DOC, cursor);
        return Math.max(Math.abs(after[0] - before[0]), Math.abs(after[1] - before[1]));
      })
      .toBeLessThan(0.25);
    zoom = next;
  }

  // A purely horizontal wheel gesture does not zoom.
  await page.mouse.wheel(100, 0);
  await page.waitForTimeout(200);
  expect(await zoomOf(page)).toBe(zoom);

  // The zoom stops at 64× and 1×.
  for (let i = 0; i < 25; i += 1) await page.mouse.wheel(0, -100);
  await expect.poll(() => zoomOf(page)).toBe(64);
  for (let i = 0; i < 25; i += 1) await page.mouse.wheel(0, 100);
  await expect.poll(() => zoomOf(page)).toBe(1);
});

test('image mode: the wheel zooms smoothly about the cursor, 2 % to 6400 %', async ({
  page,
}) => {
  await createDocument(page, 'image', DOC);
  const canvas = imageCanvas(page);
  const box = await boxOf(canvas);
  const cursor: Pt = [box.x + box.width * 0.3, box.y + box.height * 0.7];
  await page.mouse.move(...cursor);

  let zoom = await zoomOf(page);
  for (const deltaY of [-100, 100, 100]) {
    const before = await docAt(canvas, DOC, cursor);
    await page.mouse.wheel(0, deltaY);
    const next = await zoomChangedFrom(page, zoom);
    if (deltaY < 0) expect(next).toBeGreaterThan(zoom);
    else expect(next).toBeLessThan(zoom);
    await expect
      .poll(async () => {
        const after = await docAt(canvas, DOC, cursor);
        return Math.max(Math.abs(after[0] - before[0]), Math.abs(after[1] - before[1]));
      })
      .toBeLessThan(0.05);
    zoom = next;
  }

  // Smooth: a small wheel turn changes the zoom by a small step.
  await page.mouse.wheel(0, -10);
  const nudged = await zoomChangedFrom(page, zoom);
  expect(nudged / zoom).toBeGreaterThan(1);
  expect(nudged / zoom).toBeLessThan(1.05);

  // The zoom stops at 6400 % and 2 %.
  for (let i = 0; i < 5; i += 1) await page.mouse.wheel(0, -2000);
  await expect.poll(() => zoomOf(page)).toBe(6400);
  for (let i = 0; i < 5; i += 1) await page.mouse.wheel(0, 2000);
  await expect.poll(() => zoomOf(page)).toBe(2);
});

test('image mode: fit mode refits on resize until an explicit zoom or pan', async ({ page }) => {
  await createDocument(page, 'image', DOC);
  await expectFitted(page, 'image', DOC);
  const fit = await zoomOf(page);

  // In fit mode a smaller viewport refits the document.
  const size = page.viewportSize();
  if (!size) throw new Error('no viewport size');
  await page.setViewportSize({ width: size.width - 200, height: size.height - 120 });
  const smaller = await zoomChangedFrom(page, fit);
  await expectFitted(page, 'image', DOC);

  // An explicit zoom leaves fit mode: resizing keeps the zoom.
  await page.getByRole('button', { name: 'Zoom in' }).click();
  const zoomed = await zoomChangedFrom(page, smaller);
  await page.setViewportSize(size);
  await page.waitForTimeout(300);
  expect(await zoomOf(page)).toBe(zoomed);

  // So does a pan, after fitting again.
  await page.keyboard.press('0');
  await expectFitted(page, 'image', DOC);
  const refit = await zoomOf(page);
  await page.keyboard.down('Space');
  await drag(page, imageCanvas(page), [0.5, 0.5], [0.6, 0.6]);
  await page.keyboard.up('Space');
  await page.setViewportSize({ width: size.width - 200, height: size.height - 120 });
  await page.waitForTimeout(300);
  expect(await zoomOf(page)).toBe(refit);
});

test('image mode: a middle-button drag pans and paints nothing', async ({ page }) => {
  await createDocument(page, 'image', DOC);
  const canvas = imageCanvas(page);
  const before = await boxOf(canvas);
  const at = (fx: number, fy: number): Pt => [
    before.x + before.width * fx,
    before.y + before.height * fy,
  ];
  await page.mouse.move(...at(0.3, 0.3));
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(...at(0.5, 0.4), { steps: 4 });
  await page.mouse.move(...at(0.6, 0.5), { steps: 4 });
  await page.mouse.up({ button: 'middle' });
  await expect
    .poll(async () => (await boxOf(canvas)).x - before.x)
    .toBeCloseTo(before.width * 0.3, 0);
  expect((await boxOf(canvas)).y - before.y).toBeCloseTo(before.height * 0.2, 0);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});
