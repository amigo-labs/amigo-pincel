import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, imageCanvas, menu, setColor, waitForEditor } from './helpers';
import { emit, installTauriMock, launchDesktop } from './tauri-mock';

// Image-mode File ▸ Print… (docs/specs/image-formats-more-print.md). The
// platform print dialog is a stub that counts calls; `afterprint` is fired
// by the test, as the platform would once its dialog closes.

const printDialog = (page: Page) => page.getByRole('dialog', { name: 'Print' });
const printCalls = (page: Page) =>
  page.evaluate(() => (window as unknown as { printCalls: number }).printCalls);

async function stubPrint(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as unknown as { printCalls: number };
    w.printCalls = 0;
    window.print = () => {
      w.printCalls += 1;
    };
  });
}

/** A red 40 × 20 image. */
async function redImage(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 40, height: 20 });
  await setColor(page, 'fg', '#ff0000');
  await menu(page, 'Edit', 'Fill Selection');
  await expect(imageCanvas(page)).toBeVisible();
}

/** Whether the last keydown of `key` reached the end of dispatch unhandled. */
async function watchDefault(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { lastPrevented: boolean | null };
    w.lastPrevented = null;
    window.addEventListener('keydown', (e) => {
      if (e.key === 'p') w.lastPrevented = e.defaultPrevented;
    });
  });
}
const lastPrevented = (page: Page) =>
  page.evaluate(() => (window as unknown as { lastPrevented: boolean | null }).lastPrevented);

test('Ctrl+P and File ▸ Print… open the Print dialog, never the platform print directly (AC-01, AC-02, AC-08)', async ({
  page,
}) => {
  await stubPrint(page);
  await redImage(page);
  await page.keyboard.press('Control+p');
  const dialog = printDialog(page);
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('radio', { name: 'Fit to page' })).toBeChecked();
  await expect(dialog.getByRole('radio', { name: 'Actual size' })).not.toBeChecked();
  await expect(dialog.getByRole('button', { name: 'Print' })).toBeVisible();
  expect(await printCalls(page)).toBe(0);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);

  await menu(page, 'File', 'Print…');
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await printCalls(page)).toBe(0);
});

test('Print shows only the composite under print media, fitted to the page, then cleans up (AC-03 – AC-05, AC-07)', async ({
  page,
}) => {
  await stubPrint(page);
  await redImage(page);
  await page.keyboard.press('Control+p');
  await printDialog(page).getByRole('button', { name: 'Print' }).click();
  await expect(printDialog(page)).toHaveCount(0);
  await expect.poll(() => printCalls(page)).toBe(1);

  const img = page.locator('body > .pincel-print img');
  // Invisible on screen.
  await expect(img).toBeHidden();
  await page.emulateMedia({ media: 'print' });
  await expect(img).toBeVisible();
  await expect(page.locator('#app')).toBeHidden();
  // The composite at canvas size, red.
  const info = await img.evaluate((el: HTMLImageElement) => {
    const c = document.createElement('canvas');
    c.width = el.naturalWidth;
    c.height = el.naturalHeight;
    const ctx = c.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(el, 0, 0);
    return { w: el.naturalWidth, h: el.naturalHeight, px: Array.from(ctx.getImageData(5, 5, 1, 1).data) };
  });
  expect(info).toEqual({ w: 40, h: 20, px: [255, 0, 0, 255] });
  // Fit to page: the box fills the printable width at the top.
  const box = await img.boundingBox();
  const vp = page.viewportSize();
  if (!box || !vp) throw new Error('no box');
  expect(Math.abs(box.width - vp.width)).toBeLessThanOrEqual(2);
  expect(box.y).toBeLessThanOrEqual(1);

  await page.evaluate(() => window.dispatchEvent(new Event('afterprint')));
  await expect(page.locator('body > .pincel-print')).toHaveCount(0);
  await page.emulateMedia({ media: 'screen' });
  await expect(page.getByTitle(/image\.ora/).first()).toBeVisible();
});

test('Actual size prints one image pixel per CSS pixel (AC-06)', async ({ page }) => {
  await stubPrint(page);
  await redImage(page);
  await page.keyboard.press('Control+p');
  await printDialog(page).getByRole('radio', { name: 'Actual size' }).check();
  await printDialog(page).getByRole('button', { name: 'Print' }).click();
  await expect.poll(() => printCalls(page)).toBe(1);
  await page.emulateMedia({ media: 'print' });
  const box = await page.locator('body > .pincel-print img').boundingBox();
  expect(box && [Math.round(box.width), Math.round(box.height), box.x, box.y]).toEqual([40, 20, 0, 0]);
});

test('without a document or with a dialog open Ctrl+P is left to the browser (AC-09)', async ({
  page,
}) => {
  await stubPrint(page);
  await gotoStart(page);
  await watchDefault(page);
  await page.keyboard.press('Control+p');
  expect(await lastPrevented(page)).toBe(false);
  await expect(printDialog(page)).toHaveCount(0);

  await redImage(page);
  await watchDefault(page);
  await menu(page, 'File', 'Save As…');
  await expect(page.getByRole('dialog', { name: 'Save As' })).toBeVisible();
  await page.keyboard.press('Control+p');
  expect(await lastPrevented(page)).toBe(false);
  await expect(printDialog(page)).toHaveCount(0);
});

test('an image that fails to load opens no platform dialog and says so (AC-11)', async ({ page }) => {
  await stubPrint(page);
  await redImage(page);
  await page.evaluate(() => {
    URL.createObjectURL = () => 'blob:http://invalid/broken';
  });
  await page.keyboard.press('Control+p');
  await printDialog(page).getByRole('button', { name: 'Print' }).click();
  await expect(page.getByText(/The print could not be prepared/)).toBeVisible();
  expect(await printCalls(page)).toBe(0);
  await expect(page.locator('body > .pincel-print')).toHaveCount(0);
});

test('the Print dialog and menu entry are German in the German UI (AC-13)', async ({ page }) => {
  await redImage(page);
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await menu(page, 'Datei', 'Drucken…');
  const dialog = page.getByRole('dialog', { name: 'Drucken' });
  await expect(dialog.getByRole('radio', { name: 'An Seite anpassen' })).toBeVisible();
  await expect(dialog.getByRole('radio', { name: 'Originalgröße' })).toBeVisible();
});

test.describe('desktop', () => {
  test('the native Print command opens the dialog in Image mode only (AC-10)', async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
    await stubPrint(page);
    await launchDesktop(page);
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    const pixelCard = page.getByRole('form', { name: 'New pixel sprite' });
    await pixelCard.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'pixel');
    await emit(page, 'menu', 'menu:print');
    await expect(printDialog(page)).toHaveCount(0);

    await page.goto('./');
    await page.getByRole('button', { name: 'Got it' }).click();
    await page.getByRole('button', { name: 'New image' }).click();
    await page.getByRole('form', { name: 'New image' }).getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
    await emit(page, 'menu', 'menu:print');
    await expect(printDialog(page)).toBeVisible();
  });
});
