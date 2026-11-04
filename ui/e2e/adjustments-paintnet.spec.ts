import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas } from './helpers';

// docs/specs/adjustments-paintnet.md: paint.net's remaining adjustments in
// both modes' Adjustments menu (AC-01), their dialogs (AC-02), one undo step
// each (AC-10) and the German names and labels (AC-12). Pixel math, alpha,
// selection limits and wire errors are covered by the Rust tests
// (crates/pincel-effects/tests/adjustments_paintnet.rs, pincel-wasm, pincel-agent).

const NEW_EN = [
  'Auto-Level',
  'Black and White…',
  'Exposure…',
  'Highlights / Shadows…',
  'Invert Alpha',
  'Sepia…',
  'Temperature and Tint…',
];

const NEW_DE = [
  'Auto-Tonwert',
  'Schwarzweiß…',
  'Belichtung…',
  'Tiefen / Lichter…',
  'Alpha umkehren',
  'Sepia…',
  'Temperatur und Farbton…',
];

/** The entries of the Adjustments menu (`top` in the UI language), in display order. */
async function adjustments(page: Page, top: string): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  const list = page.getByRole('menu', { name: top, exact: true });
  await expect(list).toBeVisible();
  const rows = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  await page.keyboard.press('Escape');
  return rows.map((r) => r.trim());
}

function expectMenu(list: readonly string[], expected: readonly string[], locale: string): void {
  for (const name of expected) expect(list).toContain(name);
  expect(list.some((name) => /Grayscale|Graustufen/.test(name))).toBe(false);
  expect([...list].sort(new Intl.Collator(locale).compare)).toEqual(list);
}

async function openAdjustment(page: Page, entry: string, top = 'Adjustments'): Promise<Locator> {
  await menu(page, top, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

type Slider = readonly [label: string, min: string, max: string, value: string, step: string];

/** The parameters of the spec's table, per dialog. */
const PARAMS: Record<string, readonly Slider[]> = {
  'Exposure…': [['Exposure', '-5', '5', '0', '0.01']],
  'Highlights / Shadows…': [
    ['Shadows', '-100', '100', '0', '1'],
    ['Highlights', '-100', '100', '0', '1'],
  ],
  'Sepia…': [['Intensity', '0', '100', '100', '1']],
  'Temperature and Tint…': [
    ['Temperature', '-100', '100', '0', '1'],
    ['Tint', '-100', '100', '0', '1'],
  ],
};

test('both modes list the paint.net adjustments, sorted, without Grayscale (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  expectMenu(await adjustments(page, 'Adjustments'), NEW_EN, 'en');
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expectMenu(await adjustments(page, 'Adjustments'), NEW_EN, 'en');
});

test('the dialogs show exactly the parameters of the table (AC-02)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  for (const [entry, sliders] of Object.entries(PARAMS)) {
    const dialog = await openAdjustment(page, entry);
    await expect(dialog.locator('input[type="range"]')).toHaveCount(sliders.length);
    for (const [label, min, max, value, step] of sliders) {
      const slider = dialog.getByRole('slider', { name: label, exact: true });
      await expect(slider).toHaveAttribute('min', min);
      await expect(slider).toHaveAttribute('max', max);
      await expect(slider).toHaveAttribute('step', step);
      await expect(slider).toHaveValue(value);
      await expect(dialog.getByLabel(`${label} value`, { exact: true })).toHaveValue(value);
    }
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }
  for (const entry of ['Auto-Level', 'Invert Alpha']) {
    const dialog = await openAdjustment(page, entry);
    await expect(dialog.getByText('This effect has no parameters.')).toBeVisible();
    await expect(dialog.locator('input[type="range"]')).toHaveCount(0);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
  }
});

test('image mode: every new adjustment applies as one undo step (AC-10)', async ({ page }) => {
  await createDocument(page, 'image', { width: 48, height: 32 });
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const entries = NEW_EN.filter((e) => e !== 'Black and White…');
  for (const entry of entries) {
    const dialog = await openAdjustment(page, entry);
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);
  }
  // The stroke plus one step per adjustment.
  for (let i = 0; i < entries.length; i++) {
    await undo.click();
    await expect(undo).toBeEnabled();
  }
  await undo.click();
  await expect(undo).toBeDisabled();
});

test('pixel mode: every new adjustment applies (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  for (const entry of NEW_EN) {
    const dialog = await openAdjustment(page, entry);
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);
    const name = entry.replace('…', '');
    await expect(page.getByText(`${name} applied`)).toBeVisible();
  }
});

test('German names and parameter labels (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  expectMenu(await adjustments(page, 'Korrekturen'), NEW_DE, 'de');
  const labels: Record<string, readonly string[]> = {
    'Belichtung…': ['Belichtung'],
    'Tiefen / Lichter…': ['Tiefen', 'Lichter'],
    'Sepia…': ['Intensität'],
    'Temperatur und Farbton…': ['Temperatur', 'Farbton'],
  };
  for (const [entry, sliders] of Object.entries(labels)) {
    const dialog = await openAdjustment(page, entry, 'Korrekturen');
    await expect(dialog.getByRole('heading', { name: entry.replace('…', '') })).toBeVisible();
    for (const label of sliders) {
      await expect(dialog.getByRole('slider', { name: label, exact: true })).toBeVisible();
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
});
