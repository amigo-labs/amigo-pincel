import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, switchLanguage, waitForEditor } from './helpers';

// docs/specs/new-document-form.md — the start screen's New cards and the New
// dialog share one size form: Width, Height, the dialog's notes, and presets.

const PIXEL = ['16 × 16', '32 × 32', '64 × 64', '128 × 128', '256 × 256'];
const IMAGE = ['800 × 600', '1280 × 720', '1920 × 1080', '1080 × 1080'];

const card = (page: Page, mode: 'pixel' | 'image') =>
  page.getByRole('form', { name: mode === 'pixel' ? 'New pixel sprite' : 'New image' });
const dialog = (page: Page) => page.getByRole('dialog', { name: 'New document' });
const presets = (scope: Locator) => scope.getByRole('group', { name: 'Size presets', exact: true });
const presetNames = (scope: Locator) =>
  presets(scope)
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim()));
const size = (page: Page) => page.locator('[data-editor]:not([inert]) [data-frame="status"] [data-status-size]');

async function openCard(page: Page, mode: 'pixel' | 'image'): Promise<Locator> {
  await gotoStart(page);
  await page.getByRole('button', { name: mode === 'pixel' ? 'New pixel sprite' : 'New image' }).click();
  await expect(card(page, mode)).toBeVisible();
  return card(page, mode);
}

test.use({ viewport: { width: 1280, height: 800 } });

test('the start cards and the dialog show Width, Height and the mode’s presets below them (AC-01, AC-02, AC-06)', async ({
  page,
}) => {
  const pixel = await openCard(page, 'pixel');
  await expect(pixel.getByLabel('Width', { exact: true })).toBeVisible();
  await expect(pixel.getByLabel('Height', { exact: true })).toBeVisible();
  expect(await presetNames(pixel)).toEqual(PIXEL);
  const fields = await pixel.getByLabel('Width', { exact: true }).boundingBox();
  const row = await presets(pixel).boundingBox();
  if (!fields || !row) throw new Error('not laid out');
  expect(row.y).toBeGreaterThan(fields.y);
  await page.getByRole('button', { name: 'New image' }).click();
  expect(await presetNames(card(page, 'image'))).toEqual(IMAGE);

  // The dialog: the same fields and presets, following the mode.
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'File', 'New…');
  const d = dialog(page);
  await expect(d.getByLabel('Width', { exact: true })).toBeVisible();
  await expect(d.getByLabel('Height', { exact: true })).toBeVisible();
  expect(await presetNames(d)).toEqual(PIXEL);
  await d.getByRole('radio', { name: /^Image/ }).click();
  expect(await presetNames(d)).toEqual(IMAGE);
  await d.getByRole('radio', { name: /^Pixel sprite/ }).click();
  expect(await presetNames(d)).toEqual(PIXEL);
});

test('a preset fills both fields, is pressed while they hold it, and Create makes that size (AC-03, AC-04)', async ({
  page,
}) => {
  const c = await openCard(page, 'pixel');
  const width = c.getByLabel('Width', { exact: true });
  const height = c.getByLabel('Height', { exact: true });
  // 64 × 64 is the default: pressed.
  await expect(presets(c).getByRole('button', { name: '64 × 64' })).toHaveAttribute('aria-pressed', 'true');
  await presets(c).getByRole('button', { name: '32 × 32' }).click();
  await expect(width).toHaveValue('32');
  await expect(height).toHaveValue('32');
  await expect(presets(c).getByRole('button', { name: '32 × 32' })).toHaveAttribute('aria-pressed', 'true');
  await expect(presets(c).getByRole('button', { pressed: true })).toHaveCount(1);
  // 32 × 64 is no preset.
  await height.fill('64');
  await expect(presets(c).getByRole('button', { pressed: true })).toHaveCount(0);
  // A preset after an invalid value: the notes go.
  await width.fill('');
  await expect(c.locator('.am-hint')).toHaveCount(1);
  await presets(c).getByRole('button', { name: '128 × 128' }).click();
  await expect(c.locator('.am-hint')).toHaveCount(0);
  await c.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'pixel');
  await expect(size(page)).toHaveText('128 × 128 px');

  // In the dialog, Image mode.
  await menu(page, 'File', 'New…');
  const d = dialog(page);
  await d.getByRole('radio', { name: /^Image/ }).click();
  await presets(d).getByRole('button', { name: '1280 × 720' }).click();
  await expect(d.getByLabel('Width', { exact: true })).toHaveValue('1280');
  await expect(d.getByLabel('Height', { exact: true })).toHaveValue('720');
  await d.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await expect(size(page)).toHaveText('1280 × 720 px');
});

test('the start card shows the dialog’s notes and describes its fields with them (AC-05)', async ({ page }) => {
  const c = await openCard(page, 'pixel');
  const width = c.getByLabel('Width', { exact: true });
  const notes = c.locator('.am-hint');
  await expect(notes).toHaveCount(0);
  await width.fill('');
  await expect(notes).toHaveText(['Empty — 64 will be used']);
  await width.fill('0');
  await expect(notes).toHaveText(['At least 1 — 1 will be used']);
  await width.fill('5000');
  await expect(notes).toHaveText(['At most 4096 — 4096 will be used']);
  await width.fill('12.5');
  await expect(notes).toHaveText(['Whole pixels only — 12 will be used']);
  const described = await width.getAttribute('aria-describedby');
  expect(described).not.toBeNull();
  await expect(page.locator(`#${described}`)).toContainText('Whole pixels only');
  // Height is described by the same notes.
  const height = c.getByLabel('Height', { exact: true });
  expect(await height.getAttribute('aria-describedby')).toBe(described);
  // Not a number: typed, as a number field takes no such value by fill().
  await width.fill('');
  await width.focus();
  await page.keyboard.type('1e');
  await expect(notes).toHaveText(['Not a number — 64 will be used']);
  await width.fill('64');
  await expect(notes).toHaveCount(0);
  expect(await width.getAttribute('aria-describedby')).toBeNull();
  expect(await height.getAttribute('aria-describedby')).toBeNull();
  // Image mode: the area limit.
  await page.getByRole('button', { name: 'New image' }).click();
  const i = card(page, 'image');
  await i.getByLabel('Width', { exact: true }).fill('16384');
  await i.getByLabel('Height', { exact: true }).fill('16384');
  await expect(i.locator('.am-hint')).toContainText(['Too many pixels']);
});

test('German: the presets group is named "Größenvorgaben" (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await switchLanguage(page, 'Deutsch');
  await menu(page, 'Datei', 'Neu…');
  const d = page.getByRole('dialog', { name: 'Neues Dokument' });
  await expect(d.getByRole('group', { name: 'Größenvorgaben', exact: true })).toBeVisible();
});
