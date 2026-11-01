import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, pixelCanvas, waitForEditor } from './helpers';

// File ▸ Export ▸ SVG… in both modes (docs/specs/vectorize-dialog.md).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

// Exports go through the download fallback so Playwright can read them.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const dialog = (page: Page) => page.getByRole('dialog', { name: 'Export as SVG' });
const summary = (page: Page) => dialog(page).getByTestId('svg-summary');
const modeSelect = (page: Page) => dialog(page).getByLabel('Mode');

async function openDialog(page: Page): Promise<void> {
  await menu(page, 'File', 'Export', 'SVG…');
  await expect(dialog(page)).toBeVisible();
}

async function exportDownload(page: Page): Promise<{ name: string; text: string }> {
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    dialog(page).getByRole('button', { name: 'Export' }).click(),
  ]);
  return {
    name: download.suggestedFilename(),
    text: await readFile(await download.path(), 'utf8'),
  };
}

async function paintPixel(page: Page): Promise<void> {
  await page.getByTitle('Pencil (B)').click();
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await pixelCanvas(page).click();
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
}

test('both modes list SVG… last under File ▸ Export', async ({ page }) => {
  for (const mode of ['pixel', 'image'] as const) {
    await createDocument(page, mode, { width: 16, height: 16 });
    await menu(page, 'File', 'Export');
    const items = page.getByRole('menu').last().getByRole('menuitem');
    await expect(items.last()).toHaveText(/SVG…/);
    await expect(items.last()).toBeEnabled();
    await page.keyboard.press('Escape');
  }
});

test('File ▸ Export is disabled until the document is loaded', async ({ page }) => {
  for (const mode of ['pixel', 'image'] as const) {
    // Hold the core's wasm: the editor mounts its menus, but no document
    // arrives until the request is released.
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    await page.route('**/*.wasm', async (route) => {
      await held;
      await route.continue();
    });
    await gotoStart(page);
    const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
    await page.getByRole('button', { name }).click();
    await page.getByRole('form', { name }).getByRole('button', { name: 'Create' }).click();
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    const file = page.getByRole('menu', { name: 'File', exact: true });
    await expect(file.getByRole('menuitem', { name: 'Export', exact: true })).toBeDisabled();
    await page.keyboard.press('Escape');
    release();
    await waitForEditor(page, mode);
    await page.unroute('**/*.wasm');
    await menu(page, 'File', 'Export');
    await expect(
      page
        .getByRole('menu', { name: 'Export', exact: true })
        .getByRole('menuitem', { name: 'SVG…' }),
    ).toBeEnabled();
    await page.keyboard.press('Escape');
  }
});

test('pixel: the dialog opens pixel-exact with defaults and exports the frame', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paintPixel(page);
  await openDialog(page);
  await expect(modeSelect(page)).toHaveValue('exact');
  await expect(dialog(page).getByLabel('Despeckle')).toHaveCount(0);
  await expect(summary(page)).toHaveText(/^1 layer · 1 outline · \d+ B$/);
  await expect(dialog(page).getByRole('img', { name: 'SVG preview' })).toBeVisible();

  const { name, text } = await exportDownload(page);
  expect(name).toBe('pincel.svg');
  expect(text).toMatch(/^<svg xmlns="http:\/\/www.w3.org\/2000\/svg" viewBox="0 0 16 16"/);
  expect(text).toContain('shape-rendering="crispEdges"');
  expect(text).toContain('fill="#ff0000"');
  expect(text.match(/<path /g)).toHaveLength(1);
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.getByText(/exported SVG · \d+ bytes/)).toBeVisible();
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
});

test("image: the dialog opens in colour mode, shows each mode's options and updates the summary", async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await openDialog(page);
  const d = dialog(page);
  await expect(modeSelect(page)).toHaveValue('color');
  await expect(d.getByLabel('Number of colours')).toHaveValue('6');
  await expect(d.getByLabel('Despeckle')).not.toBeChecked();
  await expect(d.getByLabel('Minimum area (px²)')).toHaveValue('2');
  await expect(d.getByLabel('Corner threshold')).toHaveValue('1');
  await expect(d.getByLabel('Optimise curves')).toBeChecked();
  await expect(d.getByLabel('Curve tolerance')).toHaveValue('0.2');
  await expect(summary(page)).toHaveText(/layers? · \d+ outlines?/);
  for (const hidden of ['Automatic threshold', 'Threshold', 'Invert', 'Fill colour']) {
    await expect(d.getByLabel(hidden, { exact: true })).toHaveCount(0);
  }

  await modeSelect(page).selectOption('mono');
  await expect(d.getByLabel('Automatic threshold')).toBeChecked();
  await expect(d.getByLabel('Threshold', { exact: true })).toBeDisabled();
  await expect(d.getByLabel('Invert')).not.toBeChecked();
  await expect(d.getByLabel('Fill colour')).toHaveValue('#000000');
  await expect(d.getByLabel('Number of colours')).toHaveCount(0);
  for (const shown of [
    'Despeckle',
    'Minimum area (px²)',
    'Corner threshold',
    'Optimise curves',
    'Curve tolerance',
  ]) {
    await expect(d.getByLabel(shown)).toBeVisible();
  }
  // A white canvas has nothing darker than the threshold; invert fills it all.
  // Preview and summary follow an option change within a second (AC-04).
  await expect(summary(page)).toHaveText(/^0 layers · 0 outlines/);
  const preview = d.getByRole('img', { name: 'SVG preview' });
  const before = await preview.getAttribute('src');
  await d.getByLabel('Invert').check();
  await expect(summary(page)).toHaveText(/^1 layer · 1 outline ·/, { timeout: 1000 });
  await expect(preview).not.toHaveAttribute('src', before ?? '', { timeout: 1000 });

  await modeSelect(page).selectOption('exact');
  for (const hidden of [
    'Number of colours',
    'Automatic threshold',
    'Threshold',
    'Invert',
    'Fill colour',
    'Despeckle',
    'Minimum area (px²)',
    'Corner threshold',
    'Optimise curves',
    'Curve tolerance',
  ]) {
    await expect(d.getByLabel(hidden, { exact: true })).toHaveCount(0);
  }

  const { name, text } = await exportDownload(page);
  expect(name).toBe('image.svg');
  expect(text).toContain('viewBox="0 0 64 64"');
});

test('Cancel and Escape close the dialog without saving or editing', async ({ page }) => {
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paintPixel(page);
  // Opening the dialog and tracing leaves the canvas as it was (AC-04).
  const canvasBefore = await pixelCanvas(page).screenshot();
  await openDialog(page);
  await expect(summary(page)).toHaveText(/^1 layer/);
  await dialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog(page)).toHaveCount(0);
  await openDialog(page);
  await page.keyboard.press('Escape');
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
  expect(await pixelCanvas(page).screenshot()).toEqual(canvasBefore);

  await createDocument(page, 'image', { width: 16, height: 16 });
  await openDialog(page);
  await dialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog(page)).toHaveCount(0);
  await openDialog(page);
  await page.keyboard.press('Escape');
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  expect(downloads).toBe(0);
});

test('pixel-exact refuses more than 256 colours until another mode is chosen', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(20, 20);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    for (let y = 0; y < 20; y++)
      for (let x = 0; x < 20; x++) {
        ctx.fillStyle = `rgb(${x * 12}, ${y * 12}, 77)`;
        ctx.fillRect(x, y, 1, 1);
      }
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
  });
  await menu(page, 'Edit', 'Paste');
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
  await openDialog(page);
  const exportButton = dialog(page).getByRole('button', { name: 'Export' });
  await expect(dialog(page).getByRole('alert')).toHaveText(
    'Pixel-exact supports at most 256 colours; this image has 400.',
  );
  await expect(exportButton).toBeDisabled();
  await modeSelect(page).selectOption('color');
  await expect(dialog(page).getByRole('alert')).toHaveCount(0);
  await expect(exportButton).toBeEnabled();
});

test('a hidden layer is not exported', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paintPixel(page);
  await page.getByRole('button', { name: /^Hide / }).click();
  await openDialog(page);
  await expect(summary(page)).toHaveText(/^0 layers · 0 outlines/);
  const { text } = await exportDownload(page);
  expect(text).not.toContain('<path');
});

test('onion skin and the selection outline are not exported', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paintPixel(page);
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
  await timeline.getByLabel('Onion skin').check();
  await page.keyboard.press('ControlOrMeta+a');
  await openDialog(page);
  // Frame 2 is empty: the red pixel shows only as onion skin from frame 1.
  await expect(summary(page)).toHaveText(/^0 layers · 0 outlines/);
  const { text } = await exportDownload(page);
  expect(text).not.toContain('<path');
});

test('cancelling the save picker saves nothing and shows no error', async ({ page }) => {
  await page.addInitScript(() => {
    (window as { showSaveFilePicker?: unknown }).showSaveFilePicker = () =>
      Promise.reject(new DOMException('cancelled', 'AbortError'));
  });
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paintPixel(page);
  await openDialog(page);
  await dialog(page).getByRole('button', { name: 'Export' }).click();
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.getByText(/exported SVG|failed/)).toHaveCount(0);
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
});

test('menu entry, dialog and summary follow the German language', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await menu(page, 'Datei', 'Exportieren', 'SVG…');
  const d = page.getByRole('dialog', { name: 'Als SVG exportieren' });
  await expect(d).toBeVisible();
  await expect(d.getByLabel('Modus')).toHaveValue('color');
  await expect(d.getByLabel('Anzahl Farben')).toBeVisible();
  await expect(d.getByTestId('svg-summary')).toHaveText(/Ebenen? · \d+ Umrisse? ·/);
  await expect(d.getByRole('button', { name: 'Exportieren' })).toBeVisible();
  await expect(d.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
});

test('the dialog takes focus, keeps Tab inside and returns focus on close', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await openDialog(page);
  const d = dialog(page);
  await expect(modeSelect(page)).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(d.getByRole('button', { name: 'Export' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(modeSelect(page)).toBeFocused();
  for (let i = 0; i < 20; i++) {
    await page.keyboard.press('Tab');
    expect(await d.evaluate((el) => el.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(d).toHaveCount(0);
  expect(
    await page.evaluate(() => document.activeElement?.closest('[role="dialog"]') ?? null),
  ).toBeNull();
});

test.describe('tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } });

  test('the dialog covers the open panel drawer', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    const drawer = page.getByRole('complementary', { name: 'Panels' });
    const addLayer = drawer.getByRole('button', { name: 'Add layer' });
    await expect(addLayer).toBeVisible();
    await openDialog(page);
    // The drawer stays rendered but sits under the modal backdrop.
    await expect(addLayer.click({ trial: true, timeout: 1000 })).rejects.toThrow();
  });
});
