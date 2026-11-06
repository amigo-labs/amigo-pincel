import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu } from './helpers';

// image-resize-autocrop: the shared size input of Scale Image / Resize
// Canvas, the Supersampling filter and Image ▸ Auto Crop. The arithmetic
// is pinned without a page in size-input.spec.ts.

function status(page: Page, w: number, h: number): Locator {
  return page.getByText(`${w} × ${h} px · sRGB`, { exact: true });
}

async function open(page: Page, item: 'Scale Image…' | 'Resize Canvas…'): Promise<Locator> {
  await menu(page, 'Image', item);
  const dialog = page.getByRole('dialog', {
    name: item === 'Scale Image…' ? 'Scale Image' : 'Resize Canvas',
  });
  await expect(dialog).toBeVisible();
  return dialog;
}

test('both dialogs open fresh with the size input (AC-01, AC-09)', async ({ page }) => {
  await createDocument(page, 'image', { width: 40, height: 30 });
  for (const item of ['Scale Image…', 'Resize Canvas…'] as const) {
    let dialog = await open(page, item);
    await expect(dialog.getByRole('radio', { name: 'By percentage' })).toBeChecked();
    await expect(dialog.getByRole('radio', { name: 'By absolute size' })).not.toBeChecked();
    await expect(dialog.getByLabel('Percentage', { exact: true })).toHaveValue('100');
    await expect(dialog.getByLabel('Width')).toHaveValue('40');
    await expect(dialog.getByLabel('Height')).toHaveValue('30');
    await expect(dialog.getByLabel('Unit')).toHaveValue('px');
    const units = await dialog.getByLabel('Unit').locator('option').allTextContents();
    expect(units).toEqual(['Pixels', 'Percent', 'Inches', 'Centimeters']);
    await expect(dialog.getByLabel('Maintain aspect ratio')).toBeChecked();
    // The percentage scales both edges: the aspect lock does not apply.
    await expect(dialog.getByLabel('Maintain aspect ratio')).toBeDisabled();
    await expect(dialog.getByLabel('Resolution')).toHaveValue('96');
    await expect(dialog.getByTestId('size-result')).toHaveText('40 × 30 px');
    await dialog.getByLabel('Percentage', { exact: true }).fill('50');
    await expect(dialog.getByTestId('size-result')).toHaveText('20 × 15 px');
    await dialog.getByRole('radio', { name: 'By absolute size' }).check();
    await expect(dialog.getByLabel('Maintain aspect ratio')).toBeEnabled();
    await expect(dialog.getByTestId('size-result')).toHaveText('40 × 30 px');
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    // No value of the last opening is kept.
    dialog = await open(page, item);
    await expect(dialog.getByLabel('Percentage', { exact: true })).toHaveValue('100');
    await expect(dialog.getByRole('radio', { name: 'By percentage' })).toBeChecked();
    if (item === 'Scale Image…') {
      const filters = dialog.getByLabel('Resampling');
      await expect(filters).toHaveValue('supersampling');
      expect(await filters.locator('option').allTextContents()).toEqual([
        'Supersampling',
        'Nearest',
        'Bilinear',
        'Bicubic',
      ]);
    }
    await dialog.getByRole('button', { name: 'Cancel' }).click();
  }
});

test('a percentage and a print size apply exactly (AC-02, AC-03, AC-05, AC-12)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 96, height: 48 });
  let dialog = await open(page, 'Scale Image…');
  await dialog.getByLabel('Percentage', { exact: true }).fill('25');
  await dialog.getByRole('button', { name: 'Scale' }).click();
  await expect(status(page, 24, 12)).toBeVisible();
  await page.keyboard.press('ControlOrMeta+z');
  await expect(status(page, 96, 48)).toBeVisible();

  dialog = await open(page, 'Resize Canvas…');
  await dialog.getByRole('radio', { name: 'By absolute size' }).check();
  await dialog.getByLabel('Unit').selectOption('in');
  await expect(dialog.getByLabel('Width')).toHaveValue('1.00');
  await expect(dialog.getByLabel('Height')).toHaveValue('0.50');
  // A resolution change keeps the inches and moves the pixels.
  await dialog.getByLabel('Resolution').fill('192');
  await expect(dialog.getByLabel('Width')).toHaveValue('1.00');
  await expect(dialog.getByTestId('size-result')).toHaveText('192 × 96 px');
  await dialog.getByRole('button', { name: 'Resize' }).click();
  await expect(status(page, 192, 96)).toBeVisible();
});

test('an invalid used field blocks Apply and Enter (AC-07)', async ({ page }) => {
  await createDocument(page, 'image', { width: 40, height: 30 });
  const dialog = await open(page, 'Scale Image…');
  const apply = dialog.getByRole('button', { name: 'Scale' });
  await dialog.getByLabel('Percentage', { exact: true }).fill('0');
  await expect(dialog.getByLabel('Percentage', { exact: true })).toHaveAttribute('aria-invalid', 'true');
  await expect(dialog.getByTestId('size-result')).toHaveText('—');
  await expect(apply).toBeDisabled();
  await dialog.getByLabel('Percentage', { exact: true }).press('Enter');
  await expect(dialog).toBeVisible();
  // The absolute size does not read the percentage.
  await dialog.getByRole('radio', { name: 'By absolute size' }).check();
  await expect(apply).toBeEnabled();
  await dialog.getByLabel('Maintain aspect ratio').uncheck();
  await dialog.getByLabel('Width').fill('12.5');
  await expect(dialog.getByLabel('Width')).toHaveAttribute('aria-invalid', 'true');
  await expect(apply).toBeDisabled();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(status(page, 40, 30)).toBeVisible();
});

test('German shows a decimal comma and accepts a point (AC-22, AC-21)', async ({ page }) => {
  await createDocument(page, 'image', { width: 96, height: 48 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await menu(page, 'Bild', 'Bild skalieren…');
  const dialog = page.getByRole('dialog', { name: 'Bild skalieren' });
  await dialog.getByRole('radio', { name: 'Absolute Größe' }).check();
  await dialog.getByLabel('Einheit').selectOption('in');
  await expect(dialog.getByLabel('Breite')).toHaveValue('1,00');
  await dialog.getByLabel('Breite').fill('2.5');
  await expect(dialog.getByLabel('Höhe')).toHaveValue('1,25');
  await expect(dialog.getByTestId('size-result')).toHaveText('240 × 120 px');
  await expect(dialog.getByLabel('Seitenverhältnis beibehalten')).toBeChecked();
  await expect(dialog.getByLabel('Neuberechnung')).toHaveValue('supersampling');
});

test('Auto Crop trims the border in one undo step (AC-13 – AC-16)', async ({ page }) => {
  await createDocument(page, 'image', { width: 40, height: 30 });
  // Nothing to crop on a uniform image: a notice, no undo step.
  await menu(page, 'Image', 'Auto Crop');
  await expect(page.getByText('Nothing to crop')).toBeVisible();
  await expect(status(page, 40, 30)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();

  // A stroke in the middle, then a selection: Auto Crop stays enabled
  // and ignores the selection.
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.48, 0.5], [0.52, 0.5]);
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.05, 0.05], [0.2, 0.2]);
  await menu(page, 'Image', 'Auto Crop');
  const size = page.getByText(/^\d+ × \d+ px · sRGB$/);
  await expect(size).not.toHaveText('40 × 30 px · sRGB');
  const [w, h] = ((await size.textContent()) ?? '').split(/ × | px · /).map(Number);
  expect(w).toBeLessThan(40);
  expect(h).toBeLessThan(30);
  await page.keyboard.press('ControlOrMeta+z');
  await expect(status(page, 40, 30)).toBeVisible();
});
