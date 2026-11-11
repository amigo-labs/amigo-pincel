import { expect, type Download, type Locator, type Page } from '@playwright/test';

// Shared helpers of the Image-mode palette specs
// (docs/specs/image-palette-gradient.md): the Colors panel, its palette
// menu, loading through the `<input type=file>` fallback and saving through
// the download fallback (`noFilePickers`).

/** Removes the File System Access pickers, so Load palette… opens the file
 *  chooser and Save palette downloads (see ora.spec). */
export async function noFilePickers(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
}

/** The shown Image editor's Colors panel. */
export function colorsPanel(page: Page): Locator {
  return page.locator('[data-editor="image"]:not([inert])').getByRole('region', { name: 'Colors' });
}

/** The swatches of Image mode's palette. */
export function swatches(page: Page): Locator {
  return colorsPanel(page).getByRole('group', { name: 'Color palette' }).getByRole('button');
}

/** The accessible names of Image mode's palette swatches, in order. */
export async function swatchNames(page: Page): Promise<string[]> {
  return swatches(page).evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
}

/** The shown editor's status bar. */
export function statusBar(page: Page): Locator {
  return page.locator('[data-editor]:not([inert])').getByRole('contentinfo');
}

/** Opens the Colors panel's palette menu. */
export async function openPaletteMenu(page: Page): Promise<Locator> {
  await colorsPanel(page).getByRole('button', { name: 'Palette options' }).click();
  const menu = page.getByRole('menu', { name: 'Palette options' });
  await expect(menu).toBeVisible();
  return menu;
}

/** Loads `buffer` as `name` through Load palette… and the file chooser. */
export async function loadPalette(page: Page, name: string, buffer: Buffer): Promise<void> {
  await openPaletteMenu(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('menuitem', { name: 'Load palette…' }).click(),
  ]);
  await chooser.setFiles({ name, mimeType: 'application/octet-stream', buffer });
}

/** Clicks the Save palette submenu entry `entry`, without waiting. */
export async function chooseSave(page: Page, entry: string): Promise<void> {
  await openPaletteMenu(page);
  await page.getByRole('menuitem', { name: 'Save palette', exact: true }).click();
  await page
    .getByRole('menu', { name: 'Save palette' })
    .getByRole('menuitem', { name: entry, exact: true })
    .click();
}

/** Saves the palette through the Save palette entry `entry` (download). */
export async function savePalette(page: Page, entry: string): Promise<Download> {
  const [download] = await Promise.all([page.waitForEvent('download'), chooseSave(page, entry)]);
  return download;
}

/** A PNG whose pixels, left to right, are `colors` (`[r, g, b, a]`). */
export async function stripPng(page: Page, colors: number[][]): Promise<Buffer> {
  const bytes = await page.evaluate(async (px) => {
    const canvas = new OffscreenCanvas(px.length, 1);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.putImageData(new ImageData(new Uint8ClampedArray(px.flat()), px.length, 1), 0, 0);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  }, colors);
  return Buffer.from(bytes);
}

/** A `.gpl` file of `count` reds `(i, 0, 0)`, i = 0 … count − 1. */
export function redsGpl(count: number): Buffer {
  const rows = Array.from({ length: count }, (_, i) => `${i} 0 0`).join('\n');
  return Buffer.from(`GIMP Palette\n${rows}\n`);
}

/** The swatch name `#rrggbb` of red `(i, 0, 0)`. */
export const red = (i: number): string => `#${i.toString(16).padStart(2, '0')}0000`;
