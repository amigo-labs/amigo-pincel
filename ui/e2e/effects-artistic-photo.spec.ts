import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';

// docs/specs/effects-artistic-photo.md: paint.net's Artistic and Photo
// effects in Image mode's submenus (AC-01), their dialogs (AC-02) and the
// German names and labels (AC-13). Pixel math and wire errors are covered by
// the Rust tests (crates/pincel-effects/tests/effects_artistic_photo.rs,
// pincel-wasm, pincel-agent); the wire vectors by
// ui/e2e/effect-catalog.spec.ts.

const ARTISTIC_EN = ['Ink Sketch…', 'Oil Painting…', 'Pencil Sketch…'];
const PHOTO_EN = [
  'Glow…',
  'Red Eye Removal…',
  'Sharpen',
  'Soften Portrait…',
  'Straighten…',
  'Unsharp Mask…',
  'Vignette…',
];

/** The row labels of the open menu named `name`, in display order. */
async function rows(page: Page, name: string): Promise<string[]> {
  const list = page.getByRole('menu', { name, exact: true });
  await expect(list).toBeVisible();
  const texts = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return texts.map((r) => r.trim());
}

/** The entries of one Effects submenu (`top` / `sub` in the UI language). */
async function entries(page: Page, sub: string, top = 'Effects'): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  await page
    .getByRole('menu', { name: top, exact: true })
    .getByRole('menuitem', { name: sub, exact: true })
    .click();
  const list = await rows(page, sub);
  await page.keyboard.press('Escape');
  return list;
}

/** The submenu names of the Effects menu. */
async function submenus(page: Page): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Effects', exact: true }).click();
  const list = await rows(page, 'Effects');
  await page.keyboard.press('Escape');
  return list;
}

async function open(page: Page, sub: string, entry: string, top = 'Effects'): Promise<Locator> {
  await menu(page, top, sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

type Slider = readonly [label: string, min: string, max: string, value: string, step?: string];
type Choice = readonly [label: string, options: readonly string[], value: string];
type Controls = { sliders: readonly Slider[]; choices?: readonly Choice[]; point?: boolean };

/** The dialog shows exactly these controls, with these ranges and values. */
async function expectControls(dialog: Locator, c: Controls): Promise<void> {
  await expect(dialog.locator('input[type="range"]')).toHaveCount(c.sliders.length);
  for (const [label, min, max, value, step = '1'] of c.sliders) {
    const slider = dialog.getByRole('slider', { name: label, exact: true });
    await expect(slider).toHaveAttribute('min', min);
    await expect(slider).toHaveAttribute('max', max);
    await expect(slider).toHaveAttribute('step', step);
    await expect(slider).toHaveValue(value);
    await expect(dialog.getByLabel(`${label} value`, { exact: true })).toHaveValue(value);
  }
  const choices = c.choices ?? [];
  await expect(dialog.getByRole('combobox')).toHaveCount(choices.length);
  for (const [label, options, value] of choices) {
    const select = dialog.getByRole('combobox', { name: label, exact: true });
    await expect(select).toHaveValue(value);
    expect((await select.locator('option').allTextContents()).map((o) => o.trim())).toEqual([
      ...options,
    ]);
  }
  await expect(dialog.getByRole('button', { name: 'Reseed' })).toHaveCount(0);
  await expect(dialog.getByRole('slider', { name: 'Angle dial' })).toHaveCount(0);
  await expect(dialog.getByRole('button', { name: 'Pick on canvas' })).toHaveCount(c.point ? 1 : 0);
  if (c.point) {
    await expect(dialog.getByLabel('Center X in percent')).toHaveValue('50');
    await expect(dialog.getByLabel('Center Y in percent')).toHaveValue('50');
  }
}

test('image mode lists the Artistic and Photo entries, sorted; pixel mode neither submenu (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const collator = new Intl.Collator('en');
  const artistic = await entries(page, 'Artistic');
  expect(artistic).toEqual(ARTISTIC_EN);
  expect([...artistic].sort(collator.compare)).toEqual(artistic);
  const photo = await entries(page, 'Photo');
  expect(photo).toEqual(PHOTO_EN);
  expect([...photo].sort(collator.compare)).toEqual(photo);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const subs = await submenus(page);
  expect(subs.length).toBeGreaterThan(0);
  expect(subs).not.toContain('Artistic');
  expect(subs).not.toContain('Photo');
});

test('the Artistic and Photo dialogs show exactly the parameters of the table (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const cases: [string, string, Controls][] = [
    [
      'Artistic',
      'Ink Sketch…',
      {
        sliders: [
          ['Ink outline', '0', '99', '50'],
          ['Coloring', '0', '100', '50'],
        ],
      },
    ],
    [
      'Artistic',
      'Oil Painting…',
      {
        sliders: [
          ['Brush size', '1', '8', '3'],
          ['Coarseness', '3', '255', '50'],
        ],
      },
    ],
    [
      'Artistic',
      'Pencil Sketch…',
      {
        sliders: [
          ['Pencil tip size', '1', '20', '2'],
          ['Color range', '-20', '20', '0'],
        ],
      },
    ],
    [
      'Photo',
      'Glow…',
      {
        sliders: [
          ['Radius', '1', '20', '6'],
          ['Brightness', '-100', '100', '10'],
          ['Contrast', '-100', '100', '10'],
        ],
      },
    ],
    [
      'Photo',
      'Red Eye Removal…',
      {
        sliders: [
          ['Tolerance', '0', '100', '70'],
          ['Saturation', '0', '100', '90'],
        ],
      },
    ],
    [
      'Photo',
      'Soften Portrait…',
      {
        sliders: [
          ['Softness', '0', '10', '5'],
          ['Lighting', '-20', '20', '0'],
          ['Warmth', '0', '20', '10'],
        ],
      },
    ],
    [
      'Photo',
      'Straighten…',
      {
        sliders: [['Angle', '-45', '45', '0', '0.1']],
        choices: [['Fill', ['Zoom to fill', 'Transparent corners'], '0']],
      },
    ],
    [
      'Photo',
      'Vignette…',
      {
        sliders: [
          ['Radius %', '10', '400', '100'],
          ['Density', '0', '100', '100'],
        ],
        choices: [['Color', ['Black', 'White'], '0']],
        point: true,
      },
    ],
  ];
  for (const [sub, entry, controls] of cases) {
    const dialog = await open(page, sub, entry);
    await expectControls(dialog, controls);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }
});

test('German names and parameter labels (AC-13)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();

  const collator = new Intl.Collator('de');
  const artistic = await entries(page, 'Künstlerisch', 'Effekte');
  const artisticDe = ['Bleistiftskizze…', 'Ölgemälde…', 'Tuschezeichnung…'];
  expect([...artistic].sort()).toEqual([...artisticDe].sort());
  expect([...artistic].sort(collator.compare)).toEqual(artistic);
  const photo = await entries(page, 'Foto', 'Effekte');
  const photoDe = [
    'Gerade ausrichten…',
    'Glühen…',
    'Porträt weichzeichnen…',
    'Rote Augen entfernen…',
    'Scharfzeichnen',
    'Unscharf maskieren…',
    'Vignette…',
  ];
  expect([...photo].sort()).toEqual([...photoDe].sort());
  expect([...photo].sort(collator.compare)).toEqual(photo);

  const labels: [string, string, readonly string[], readonly Choice[]][] = [
    ['Künstlerisch', 'Tuschezeichnung…', ['Tuschekontur', 'Färbung'], []],
    ['Künstlerisch', 'Ölgemälde…', ['Pinselgröße', 'Grobheit'], []],
    ['Künstlerisch', 'Bleistiftskizze…', ['Stiftspitze', 'Farbbereich'], []],
    ['Foto', 'Glühen…', ['Radius', 'Helligkeit', 'Kontrast'], []],
    ['Foto', 'Rote Augen entfernen…', ['Toleranz', 'Sättigung'], []],
    ['Foto', 'Porträt weichzeichnen…', ['Weichheit', 'Beleuchtung', 'Wärme'], []],
    [
      'Foto',
      'Gerade ausrichten…',
      ['Winkel'],
      [['Füllung', ['Zoomen zum Füllen', 'Transparente Ecken'], '0']],
    ],
    ['Foto', 'Vignette…', ['Radius %', 'Dichte'], [['Farbe', ['Schwarz', 'Weiß'], '0']]],
  ];
  for (const [sub, entry, sliders, choices] of labels) {
    const dialog = await open(page, sub, entry, 'Effekte');
    await expect(dialog.getByRole('heading', { name: entry.replace('…', '') })).toBeVisible();
    for (const label of sliders) {
      await expect(dialog.getByRole('slider', { name: label, exact: true })).toBeVisible();
    }
    for (const [label, options, value] of choices) {
      const select = dialog.getByRole('combobox', { name: label, exact: true });
      await expect(select).toHaveValue(value);
      expect((await select.locator('option').allTextContents()).map((o) => o.trim())).toEqual([
        ...options,
      ]);
    }
    if (entry === 'Vignette…') {
      await expect(dialog.getByLabel('Mitte X in Prozent')).toBeVisible();
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
});
