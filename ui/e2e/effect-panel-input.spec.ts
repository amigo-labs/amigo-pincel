import { readFile } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  copiedImage,
  clearClipboard,
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  waitForEditor,
} from './helpers';

// What the editor does while an effect panel is open (effect-panel AC-03,
// AC-16, AC-17): other panels are inert and the canvas only zooms and pans;
// a failing effect keeps the panel; the preview never reaches the document.

type Mode = 'pixel' | 'image';

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

// Save falls back to a download, Open to a file input.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const invertPanel = (page: Page) => page.getByRole('dialog', { name: 'Invert Colors' });

/** A 6 × 6 px screenshot around the canvas centre. */
async function centre(page: Page, mode: Mode): Promise<Buffer> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  return page.screenshot({
    clip: { x: box.x + box.width / 2 - 3, y: box.y + box.height / 2 - 3, width: 6, height: 6 },
  });
}

/** Fills the active layer with the foreground colour (opaque black). */
async function fill(page: Page, mode: Mode): Promise<void> {
  await page.keyboard.press('g');
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
}

async function openInvert(page: Page): Promise<Locator> {
  await menu(page, 'Adjustments', 'Invert Colors');
  const panel = invertPanel(page);
  await expect(panel).toBeVisible();
  return panel;
}

/** Runs `action` and returns the bytes of the download it triggers. */
async function download(page: Page, action: () => Promise<void>): Promise<Uint8Array> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return new Uint8Array(await readFile(await d.path()));
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: other panels are inert and the canvas only zooms and pans (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const layers = page.getByRole('list', { name: 'Layer list' });
    await page.getByRole('button', { name: 'Add layer' }).click();
    await expect(layers.getByRole('listitem')).toHaveCount(2);
    await fill(page, mode);
    const filled = await centre(page, mode);
    const lower = layers
      .getByRole('listitem')
      .nth(1)
      .locator('button[aria-pressed]:not(.row-icon)');
    await expect(lower).toHaveAttribute('aria-pressed', 'false');

    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);
    const preview = await centre(page, mode);

    // Other panels: visible, but no focus and no clicks.
    await expect(layers).toBeVisible();
    await lower.focus();
    await expect(lower).not.toBeFocused();
    await lower.click({ force: true });
    await expect(lower).toHaveAttribute('aria-pressed', 'false');
    const swap = page.getByRole('button', { name: 'Swap foreground and background colors' });
    await swap.focus();
    await expect(swap).not.toBeFocused();
    if (mode === 'pixel') {
      // The timeline docked under the canvas is inert too.
      const addFrame = page.getByRole('button', { name: 'Add frame' });
      await expect(addFrame).toBeVisible();
      await addFrame.focus();
      await expect(addFrame).not.toBeFocused();
    }

    // Drawing tools don't reach the canvas; Undo stays blocked.
    await page
      .getByRole('toolbar', { name: 'Tools' })
      .getByRole('button', { name: 'Eraser' })
      .click();
    await drag(page, canvasOf(page, mode), [0.3, 0.5], [0.7, 0.5]);
    await page.keyboard.press('Control+z');

    // Zoom: wheel and the zoom control; the preview stays shown.
    const canvas = canvasOf(page, mode);
    const before = await canvas.boundingBox();
    if (!before) throw new Error('canvas is not visible');
    await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
    await page.mouse.wheel(0, -300);
    await expect
      .poll(async () => (await canvas.boundingBox())?.width ?? 0)
      .toBeGreaterThan(before.width);
    const zoomed = (await canvas.boundingBox())?.width ?? 0;
    await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
    await expect.poll(async () => (await canvas.boundingBox())?.width ?? 0).toBeLessThan(zoomed);
    expect((await centre(page, mode)).equals(preview)).toBe(true);

    // Pan: middle-button drag, then a drag with Space held.
    const panned = async (move: () => Promise<void>) => {
      const from = await canvas.boundingBox();
      if (!from) throw new Error('canvas is not visible');
      await move();
      await expect
        .poll(async () => Math.round((await canvas.boundingBox())?.x ?? 0))
        .toBe(Math.round(from.x + 40));
    };
    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    const [cx, cy] = [box.x + box.width / 2, box.y + box.height / 2];
    await panned(async () => {
      await page.mouse.move(cx, cy);
      await page.mouse.down({ button: 'middle' });
      await page.mouse.move(cx + 20, cy, { steps: 3 });
      await page.mouse.move(cx + 40, cy, { steps: 3 });
      await page.mouse.up({ button: 'middle' });
    });
    await panned(async () => {
      await page.mouse.move(cx + 40, cy);
      await page.keyboard.down('Space');
      await page.mouse.down();
      await page.mouse.move(cx + 60, cy, { steps: 3 });
      await page.mouse.move(cx + 80, cy, { steps: 3 });
      await page.mouse.up();
      await page.keyboard.up('Space');
    });
    expect((await centre(page, mode)).equals(preview)).toBe(true);

    // Cancel: no erase stroke and no undo happened.
    await invertPanel(page).getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    await expect(lower).toHaveAttribute('aria-pressed', 'false');
    await lower.click();
    await expect(lower).toHaveAttribute('aria-pressed', 'true');
  });

  test(`${mode}: a failing effect keeps the panel and its values (AC-16)`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await fill(page, mode);
    const filled = await centre(page, mode);
    // A locked layer makes the effect fail.
    await page
      .getByRole('button', { name: /^Lock / })
      .first()
      .click();
    await expect(page.getByRole('button', { name: /^Unlock / }).first()).toBeVisible();

    await menu(page, 'Adjustments', 'Levels…');
    const panel = page.getByRole('dialog', { name: 'Levels' });
    const field = panel.getByLabel('Input black value', { exact: true });
    await field.fill('40');
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(page.getByText(/Levels failed: .*locked/)).toBeVisible();
    await expect(panel).toBeVisible();
    await expect(field).toHaveValue('40');

    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    if (mode === 'pixel') {
      // Pixel mode also reports the failed preview.
      await menu(page, 'Adjustments', 'Levels…');
      await expect(page.getByText(/Preview failed: .*locked/)).toBeVisible();
    }
  });

  test(`${mode}: a save while an effect is open writes the document without the preview (AC-17)`, async ({
    page,
  }) => {
    if (mode === 'pixel') {
      await createDocument(page, 'pixel', { width: 16, height: 16 });
    } else {
      // An opened PNG saves back as PNG, byte-stable across saves.
      await gotoStart(page);
      const png = await page.evaluate(async () => {
        const canvas = new OffscreenCanvas(16, 16);
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('no 2d context');
        ctx.fillStyle = '#336699';
        ctx.fillRect(0, 0, 16, 16);
        const blob = await canvas.convertToBlob({ type: 'image/png' });
        return Array.from(new Uint8Array(await blob.arrayBuffer()));
      });
      const [chooser] = await Promise.all([
        page.waitForEvent('filechooser'),
        page.getByRole('button', { name: 'Open…' }).click(),
      ]);
      await chooser.setFiles({ name: 'tile.png', mimeType: 'image/png', buffer: Buffer.from(png) });
      await waitForEditor(page, 'image');
    }
    // Without File System Access, Pixel mode names its Save a download.
    const saveLabel = mode === 'pixel' ? 'Save (download)' : 'Save';
    await fill(page, mode);
    const filled = await centre(page, mode);

    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);
    const withPreview = await download(page, () => menu(page, 'File', saveLabel));
    await expect(invertPanel(page)).toBeVisible();
    await invertPanel(page).getByRole('button', { name: 'Cancel' }).click();
    const without = await download(page, () => menu(page, 'File', saveLabel));
    expect(withPreview.length).toBeGreaterThan(0);
    expect(Buffer.from(withPreview).equals(Buffer.from(without))).toBe(true);
  });

  test(`${mode}: the unsaved-changes prompt cancels the open effect first (AC-17)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await fill(page, mode);
    const filled = await centre(page, mode);
    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);

    await menu(page, 'File', 'New…');
    const prompt = page.getByRole('alertdialog');
    await expect(prompt).toBeVisible();
    await expect(invertPanel(page)).toHaveCount(0);
    await prompt.getByRole('button', { name: 'Cancel' }).click();
    await expect(prompt).toHaveCount(0);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    await expect(invertPanel(page)).toHaveCount(0);
  });
}

test('pixel: opening a file while an effect is open cancels the effect (AC-17)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openInvert(page);
  const png = await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(8, 8);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.fillStyle = '#ff0000';
    ctx.fillRect(0, 0, 8, 8);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  });
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    menu(page, 'File', 'Open…'),
  ]);
  await expect(invertPanel(page)).toHaveCount(0);
  await chooser.setFiles({ name: 'red.png', mimeType: 'image/png', buffer: Buffer.from(png) });
  await expect(page.getByText(/opened red\.png/)).toBeVisible();
  await expect(invertPanel(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
});

test('image: an autosave while an effect is open stores the document without the preview (AC-17)', async ({
  page,
}) => {
  await page.clock.install();
  await createDocument(page, 'image', { width: 16, height: 16 });
  await fill(page, 'image');
  await openInvert(page);
  await page.clock.fastForward(31_000);
  // Let the IndexedDB write settle before leaving the page.
  await page.waitForTimeout(500);

  await page.reload();
  await createDocument(page, 'image', { width: 8, height: 8 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await dialog.getByRole('button', { name: 'Recover' }).first().click();
  await expect(dialog).toHaveCount(0);
  await waitForEditor(page, 'image');
  await clearClipboard(page);
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+c');
  const img = await copiedImage(page);
  expect(img.width).toBe(16);
  // The fill (opaque black), not its inverted preview.
  expect(pixelOf(img, 8, 8)).toEqual([0, 0, 0, 255]);
});
