import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';

// docs/specs/effects-render.md: paint.net's Render effects in the Render
// submenu, per mode (AC-01), their dialogs with the colours starting at the
// editor's foreground / background (AC-02) and the German names, labels and
// blend-mode options (AC-12). Pixel math, wire errors and the cores'
// target handling are covered by the Rust tests
// (crates/pincel-effects/tests/effects_render.rs, pincel-wasm,
// pincel-image-wasm, pincel-agent); the wire vectors by
// ui/e2e/effect-catalog.spec.ts.

const RENDER_EN = ['Clouds…', 'Julia Fractal…', 'Mandelbrot Fractal…', 'Turbulence…'];
const BLENDS_EN = ['Normal', 'Multiply', 'Screen', 'Overlay'];
const FG = '#3366cc';
const BG = '#ffcc00';

/** The row labels of the open menu named `name`, in display order. */
async function rows(page: Page, name: string): Promise<string[]> {
  const list = page.getByRole('menu', { name, exact: true });
  await expect(list).toBeVisible();
  const texts = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return texts.map((r) => r.trim());
}

/** The entries of the Effects menu's Render submenu (`top` / `sub` in the UI language). */
async function renders(page: Page, top = 'Effects', sub = 'Render'): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  await page
    .getByRole('menu', { name: top, exact: true })
    .getByRole('menuitem', { name: sub, exact: true })
    .click();
  const list = await rows(page, sub);
  await page.keyboard.press('Escape');
  return list;
}

async function open(page: Page, entry: string, top = 'Effects', sub = 'Render'): Promise<Locator> {
  await menu(page, top, sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

async function setColors(page: Page): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill(FG);
  await page.getByLabel('Background color', { exact: true }).fill(BG);
}

type Slider = readonly [label: string, min: string, max: string, value: string];

/** The dialog shows exactly these controls, with these ranges and values. */
async function expectControls(
  dialog: Locator,
  c: {
    sliders: readonly Slider[];
    noise?: boolean;
    angle?: boolean;
    invert?: boolean;
  },
): Promise<void> {
  await expect(dialog.locator('input[type="range"]')).toHaveCount(c.sliders.length);
  for (const [label, min, max, value] of c.sliders) {
    const slider = dialog.getByRole('slider', { name: label, exact: true });
    await expect(slider).toHaveAttribute('min', min);
    await expect(slider).toHaveAttribute('max', max);
    await expect(slider).toHaveAttribute('step', '1');
    await expect(slider).toHaveValue(value);
    await expect(dialog.getByLabel(`${label} value`, { exact: true })).toHaveValue(value);
  }
  // Clouds / Turbulence: seed, two colours, blend mode.
  await expect(dialog.getByRole('button', { name: 'Reseed' })).toHaveCount(c.noise ? 1 : 0);
  await expect(dialog.getByRole('combobox')).toHaveCount(c.noise ? 1 : 0);
  await expect(dialog.locator('input[type="color"]')).toHaveCount(c.noise ? 2 : 0);
  if (c.noise) {
    const seed = dialog.getByLabel('Seed value');
    await expect(seed).toHaveValue('1');
    await expect(seed).toHaveAttribute('min', '0');
    await expect(seed).toHaveAttribute('max', '9999');
    // The colours start at the editor's foreground / background colours.
    await expect(dialog.getByLabel('Color 1 hex', { exact: true })).toHaveValue(`${FG}ff`);
    await expect(dialog.getByLabel('Color 2 hex', { exact: true })).toHaveValue(`${BG}ff`);
    const blend = dialog.getByRole('combobox', { name: 'Blend mode', exact: true });
    await expect(blend).toHaveValue('0');
    expect((await blend.locator('option').allTextContents()).map((o) => o.trim())).toEqual(
      BLENDS_EN,
    );
  }
  await expect(dialog.getByRole('slider', { name: 'Angle dial' })).toHaveCount(c.angle ? 1 : 0);
  if (c.angle) {
    await expect(dialog.getByRole('slider', { name: 'Angle dial' })).toHaveAttribute(
      'aria-valuenow',
      '0',
    );
    await expect(dialog.getByLabel('Angle in degrees')).toHaveValue('0');
  }
  // Besides the panel's own Preview toggle.
  await expect(dialog.getByRole('checkbox')).toHaveCount(c.invert ? 2 : 1);
  if (c.invert) {
    await expect(dialog.getByRole('checkbox', { name: 'Invert colors' })).not.toBeChecked();
  }
}

const CLOUDS = {
  sliders: [
    ['Scale', '2', '1000', '250'],
    ['Roughness', '0', '100', '50'],
  ],
  noise: true,
} as const;
const TURBULENCE = {
  sliders: [
    ['Scale', '2', '1000', '200'],
    ['Octaves', '1', '8', '5'],
  ],
  noise: true,
} as const;

test('image mode lists the four render effects, sorted; pixel mode Clouds and Turbulence (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const list = await renders(page);
  expect(list).toEqual(RENDER_EN);
  expect([...list].sort(new Intl.Collator('en').compare)).toEqual(list);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await renders(page)).toEqual(['Clouds…', 'Turbulence…']);
});

test('the render dialogs show exactly the parameters of the table (AC-02)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await setColors(page);
  const cases: [string, Parameters<typeof expectControls>[1]][] = [
    ['Clouds…', CLOUDS],
    ['Turbulence…', TURBULENCE],
    [
      'Julia Fractal…',
      {
        sliders: [
          ['Factor', '1', '10', '4'],
          ['Zoom', '1', '50', '1'],
          ['Quality', '1', '5', '2'],
        ],
        angle: true,
      },
    ],
    [
      'Mandelbrot Fractal…',
      {
        sliders: [
          ['Factor', '1', '10', '1'],
          ['Zoom', '1', '100', '10'],
          ['Quality', '1', '5', '2'],
        ],
        angle: true,
        invert: true,
      },
    ],
  ];
  for (const [entry, controls] of cases) {
    const dialog = await open(page, entry);
    await expectControls(dialog, controls);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }

  // Pixel mode: the same two noise dialogs, following its own colours.
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColors(page);
  for (const [entry, controls] of [
    ['Clouds…', CLOUDS],
    ['Turbulence…', TURBULENCE],
  ] as const) {
    const dialog = await open(page, entry);
    await expectControls(dialog, controls);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Clouds previews and applies as one undo step`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await expect(undo).toBeDisabled();
    const dialog = await open(page, 'Clouds…');
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(undo).toBeEnabled();
    await undo.click();
    await expect(undo).toBeDisabled();
  });
}

test('German names, parameter labels and blend-mode options (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const list = await renders(page, 'Effekte', 'Rendern');
  const names = ['Wolken…', 'Turbulenz…', 'Julia-Fraktal…', 'Mandelbrot-Fraktal…'];
  expect([...list].sort()).toEqual([...names].sort());
  expect([...list].sort(new Intl.Collator('de').compare)).toEqual(list);

  const blends = ['Normal', 'Multiplizieren', 'Negativ multiplizieren', 'Ineinanderkopieren'];
  const labels: Record<string, readonly string[]> = {
    'Wolken…': ['Skalierung', 'Rauheit'],
    'Turbulenz…': ['Skalierung', 'Oktaven'],
    'Julia-Fraktal…': ['Faktor', 'Zoom', 'Qualität'],
    'Mandelbrot-Fraktal…': ['Faktor', 'Zoom', 'Qualität'],
  };
  for (const [entry, sliders] of Object.entries(labels)) {
    const dialog = await open(page, entry, 'Effekte', 'Rendern');
    await expect(dialog.getByRole('heading', { name: entry.replace('…', '') })).toBeVisible();
    for (const label of sliders) {
      await expect(dialog.getByRole('slider', { name: label, exact: true })).toBeVisible();
    }
    if (entry === 'Wolken…' || entry === 'Turbulenz…') {
      const select = dialog.getByRole('combobox', { name: 'Mischmodus', exact: true });
      expect((await select.locator('option').allTextContents()).map((o) => o.trim())).toEqual(
        blends,
      );
      await expect(dialog.getByLabel('Farbe 1', { exact: true })).toBeVisible();
      await expect(dialog.getByLabel('Farbe 2 als Hex', { exact: true })).toBeVisible();
      await expect(dialog.getByLabel('Startwert Wert')).toBeVisible();
    } else {
      await expect(dialog.getByLabel('Winkel in Grad')).toBeVisible();
    }
    if (entry === 'Mandelbrot-Fraktal…') {
      await expect(dialog.getByRole('checkbox', { name: 'Farben umkehren' })).toBeVisible();
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
});
