import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';

// docs/specs/effects-blurs.md: paint.net's ten blurs in Image mode's Blurs
// submenu (AC-01), the new dialogs (AC-02), Radial Blur without its Kind
// choice (AC-12) and the German names and labels (AC-15). Pixel math, wire
// errors and the Pixel core are covered by the Rust tests
// (crates/pincel-effects/tests/effects_blurs.rs, pincel-wasm, pincel-agent);
// the wire vectors by ui/e2e/effect-catalog.spec.ts.

const BLURS_EN = [
  'Bokeh…',
  'Fragment…',
  'Gaussian Blur…',
  'Median…',
  'Motion Blur…',
  'Radial Blur…',
  'Sketch Blur…',
  'Square Blur…',
  'Surface Blur…',
  'Zoom Blur…',
];

/** The row labels of the open menu named `name`, in display order. */
async function rows(page: Page, name: string): Promise<string[]> {
  const list = page.getByRole('menu', { name, exact: true });
  await expect(list).toBeVisible();
  const texts = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return texts.map((r) => r.trim());
}

/** The entries of the Effects menu's Blurs submenu (`top` / `sub` in the UI language). */
async function blurs(page: Page, top: string, sub: string): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  await page
    .getByRole('menu', { name: top, exact: true })
    .getByRole('menuitem', { name: sub, exact: true })
    .click();
  const list = await rows(page, sub);
  await page.keyboard.press('Escape');
  return list;
}

async function open(page: Page, entry: string, top = 'Effects', sub = 'Blurs'): Promise<Locator> {
  await menu(page, top, sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

type Slider = readonly [label: string, min: string, max: string, value: string];

async function expectSliders(dialog: Locator, sliders: readonly Slider[]): Promise<void> {
  await expect(dialog.locator('input[type="range"]')).toHaveCount(sliders.length);
  for (const [label, min, max, value] of sliders) {
    const slider = dialog.getByRole('slider', { name: label, exact: true });
    await expect(slider).toHaveAttribute('min', min);
    await expect(slider).toHaveAttribute('max', max);
    await expect(slider).toHaveAttribute('step', '1');
    await expect(slider).toHaveValue(value);
    await expect(dialog.getByLabel(`${label} value`, { exact: true })).toHaveValue(value);
  }
}

test('image mode lists the ten blurs, sorted; pixel mode none (AC-01)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const list = await blurs(page, 'Effects', 'Blurs');
  expect(list).toEqual(BLURS_EN);
  expect([...list].sort(new Intl.Collator('en').compare)).toEqual(list);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Effects', exact: true }).click();
  const subs = await rows(page, 'Effects');
  expect(subs).not.toContain('Blurs');
  const all: string[] = [];
  for (const sub of subs) {
    await page
      .getByRole('menu', { name: 'Effects', exact: true })
      .getByRole('menuitem', { name: sub, exact: true })
      .click();
    all.push(...(await rows(page, sub)));
  }
  await page.keyboard.press('Escape');
  for (const name of BLURS_EN) expect(all).not.toContain(name);
});

test('the new blur dialogs show exactly the parameters of the table (AC-02)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });

  let dialog = await open(page, 'Bokeh…');
  await expectSliders(dialog, [
    ['Radius', '1', '100', '10'],
    ['Highlights', '0', '100', '25'],
  ]);
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Fragment…');
  await expectSliders(dialog, [
    ['Fragments', '2', '50', '4'],
    ['Distance', '0', '100', '8'],
  ]);
  await expect(dialog.getByRole('slider', { name: 'Rotation dial' })).toHaveAttribute(
    'aria-valuenow',
    '0',
  );
  await expect(dialog.getByLabel('Rotation in degrees')).toHaveValue('0');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Median…');
  await expectSliders(dialog, [
    ['Radius', '1', '200', '10'],
    ['Percentile', '0', '100', '50'],
  ]);
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Sketch Blur…');
  await expectSliders(dialog, [['Radius', '1', '100', '10']]);
  const seed = dialog.getByLabel('Seed value');
  await expect(seed).toHaveValue('1');
  await expect(seed).toHaveAttribute('min', '0');
  await expect(seed).toHaveAttribute('max', '9999');
  await expect(dialog.getByRole('button', { name: 'Reseed' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Surface Blur…');
  await expectSliders(dialog, [
    ['Radius', '1', '100', '6'],
    ['Threshold', '1', '100', '25'],
  ]);
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Zoom Blur…');
  await expectSliders(dialog, [['Amount', '1', '100', '10']]);
  await expect(dialog.getByLabel('Center X in percent')).toHaveValue('50');
  await expect(dialog.getByLabel('Center Y in percent')).toHaveValue('50');
  await expect(dialog.getByRole('button', { name: 'Pick on canvas' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  // Motion Blur gains its Centered switch, on by default.
  dialog = await open(page, 'Motion Blur…');
  await expect(dialog.getByRole('checkbox', { name: 'Centered' })).toBeChecked();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
});

test('Radial Blur shows Amount and Center but no Kind choice (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const dialog = await open(page, 'Radial Blur…');
  await expectSliders(dialog, [['Amount', '1', '100', '10']]);
  await expect(dialog.getByLabel('Center X in percent')).toHaveValue('50');
  await expect(dialog.getByLabel('Center Y in percent')).toHaveValue('50');
  await expect(dialog.getByRole('combobox')).toHaveCount(0);
  await expect(dialog.getByText('Kind', { exact: true })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
});

test('German names and parameter labels (AC-15)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const list = await blurs(page, 'Effekte', 'Weichzeichnen');
  const names = [
    'Bokeh…',
    'Fragmente…',
    'Median…',
    'Skizzenhaftes Weichzeichnen…',
    'Oberflächen-Weichzeichner…',
    'Zoom-Weichzeichnen…',
  ];
  for (const name of names) expect(list).toContain(name);
  expect([...list].sort(new Intl.Collator('de').compare)).toEqual(list);

  const labels: Record<string, readonly string[]> = {
    'Bokeh…': ['Radius', 'Lichter'],
    'Fragmente…': ['Fragmente', 'Distanz'],
    'Median…': ['Radius', 'Perzentil'],
    'Skizzenhaftes Weichzeichnen…': ['Radius'],
    'Oberflächen-Weichzeichner…': ['Radius', 'Schwellenwert'],
    'Zoom-Weichzeichnen…': ['Stärke'],
  };
  for (const [entry, sliders] of Object.entries(labels)) {
    const dialog = await open(page, entry, 'Effekte', 'Weichzeichnen');
    await expect(dialog.getByRole('heading', { name: entry.replace('…', '') })).toBeVisible();
    for (const label of sliders) {
      await expect(dialog.getByRole('slider', { name: label, exact: true })).toBeVisible();
    }
    if (entry === 'Fragmente…') {
      await expect(dialog.getByLabel('Drehung in Grad')).toBeVisible();
    }
    if (entry === 'Skizzenhaftes Weichzeichnen…') {
      await expect(dialog.getByLabel('Startwert Wert')).toBeVisible();
    }
    if (entry === 'Zoom-Weichzeichnen…') {
      await expect(dialog.getByLabel('Mitte X in Prozent')).toBeVisible();
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
  const motion = await open(page, 'Bewegungsunschärfe…', 'Effekte', 'Weichzeichnen');
  await expect(motion.getByRole('checkbox', { name: 'Zentriert' })).toBeChecked();
});
