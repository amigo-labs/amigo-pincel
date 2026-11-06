import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, gotoStart, imageCanvas, menu, waitForEditor } from './helpers';

// Image-mode File ▸ Export and the Resize / Scale size limits
// (docs/specs/image-files.md AC-05, AC-07, AC-16). The File System Access
// pickers are removed so every export falls back to a download.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

/** Leading bytes of each exported format. */
function formatOf(bytes: Uint8Array): 'png' | 'jpeg' | 'webp' | null {
  const at = (sig: number[], offset = 0) => sig.every((b, i) => bytes[offset + i] === b);
  if (at([0x89, 0x50, 0x4e, 0x47])) return 'png';
  if (at([0xff, 0xd8, 0xff])) return 'jpeg';
  if (at([0x52, 0x49, 0x46, 0x46]) && at([0x57, 0x45, 0x42, 0x50], 8)) return 'webp';
  return null;
}

/** Runs `action` and returns the download it triggers. */
async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ name: string; bytes: Uint8Array }> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return { name: d.suggestedFilename(), bytes: new Uint8Array(await readFile(await d.path())) };
}

/** Opens a 64 × 48 noisy PNG named `name` through the start screen's Open…. */
async function openPng(page: Page, name: string): Promise<void> {
  await gotoStart(page);
  const bytes = await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(64, 48);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    // Detail the JPEG quality can visibly trade away.
    for (let y = 0; y < 48; y += 2) {
      for (let x = 0; x < 64; x += 2) {
        ctx.fillStyle = `rgb(${(x * 37) % 256} ${(y * 53) % 256} ${(x * y) % 256})`;
        ctx.fillRect(x, y, 2, 2);
      }
    }
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  });
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name, mimeType: 'image/png', buffer: Buffer.from(bytes) });
  await waitForEditor(page, 'image');
}

test('PNG and WebP export without a dialog as image.* and keep the unsaved state', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 40, height: 30 });
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  const unsaved = page.getByTitle('image.ora — unsaved changes', { exact: true });
  await expect(unsaved).toBeVisible();

  const png = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(png.name).toBe('image.png');
  expect(formatOf(png.bytes)).toBe('png');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(unsaved).toBeVisible();

  const webp = await download(page, () => menu(page, 'File', 'Export', 'WebP (lossless)'));
  expect(webp.name).toBe('image.webp');
  expect(formatOf(webp.bytes)).toBe('webp');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(unsaved).toBeVisible();
});

test('JPEG export asks for the quality first and is named after the opened file', async ({
  page,
}) => {
  await openPng(page, 'beach.trip.png');
  const saved = page.getByTitle('beach.trip.png', { exact: true });
  await expect(saved).toBeVisible();
  let downloads = 0;
  page.on('download', () => (downloads += 1));

  await menu(page, 'File', 'Export', 'JPEG…');
  const dialog = page.getByRole('dialog', { name: 'Export JPEG' });
  await expect(dialog).toBeVisible();
  const quality = dialog.getByLabel('Quality');
  await expect(quality).toHaveAttribute('min', '1');
  await expect(quality).toHaveAttribute('max', '100');
  await expect(quality).toHaveValue('90');
  // Nothing is written while the dialog asks.
  expect(downloads).toBe(0);

  const q90 = await download(page, () => dialog.getByRole('button', { name: 'Export' }).click());
  expect(q90.name).toBe('beach.trip.jpg');
  expect(formatOf(q90.bytes)).toBe('jpeg');
  await expect(dialog).toHaveCount(0);
  // Exporting does not touch the document's saved state.
  await expect(saved).toBeVisible();

  // The chosen quality reaches the encoder.
  await menu(page, 'File', 'Export', 'JPEG…');
  await quality.fill('1');
  await expect(dialog.getByText('1', { exact: true })).toBeVisible();
  const q1 = await download(page, () => dialog.getByRole('button', { name: 'Export' }).click());
  expect(q1.name).toBe('beach.trip.jpg');
  expect(q1.bytes.length).toBeLessThan(q90.bytes.length);
  await expect(saved).toBeVisible();
});

// Allocating a 64 M px canvas takes a while in the unoptimized dev wasm.
test('Resize Canvas and Scale Image reduce an oversized request to the limits', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await createDocument(page, 'image', { width: 100, height: 50 });
  await expect(page.getByText('100 × 50 · sRGB')).toBeVisible();

  // An edge beyond 16384 px is cut to 16384 px.
  await menu(page, 'Image', 'Resize Canvas…');
  const resize = page.getByRole('dialog', { name: 'Resize Canvas' });
  await resize.getByRole('radio', { name: 'By absolute size' }).check();
  await resize.getByLabel('Maintain aspect ratio').uncheck();
  await resize.getByLabel('Width').fill('20000');
  await resize.getByLabel('Height').fill('50');
  await resize.getByRole('button', { name: 'Resize' }).click();
  await expect(page.getByText('16384 × 50 · sRGB')).toBeVisible({ timeout: 60_000 });

  // An area beyond 8192 × 8192 px scales both edges down together:
  // 16384 × 8192 · √½ → 11585 × 5792.
  await menu(page, 'Image', 'Scale Image…');
  const scale = page.getByRole('dialog', { name: 'Scale Image' });
  await scale.getByRole('radio', { name: 'By absolute size' }).check();
  await scale.getByLabel('Maintain aspect ratio').uncheck();
  await scale.getByLabel('Width').fill('16384');
  await scale.getByLabel('Height').fill('8192');
  await scale.getByLabel('Resampling').selectOption('nearest');
  await scale.getByRole('button', { name: 'Scale' }).click();
  await expect(page.getByText('11585 × 5792 · sRGB')).toBeVisible({ timeout: 60_000 });
});
