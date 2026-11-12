import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, switchLanguage } from './helpers';

// docs/specs/effects-distort.md: paint.net's nine distortions in the Distort
// submenu, per mode (AC-01), their dialogs (AC-02) and the German names,
// labels and edge-behaviour options (AC-15). Pixel math, wire errors and the
// Pixel core are covered by the Rust tests
// (crates/pincel-effects/tests/effects_distort.rs, pincel-wasm, pincel-agent);
// the wire vectors by ui/e2e/effect-catalog.spec.ts.

const DISTORT_EN = [
  'Bulge…',
  'Crystalize…',
  'Dents…',
  'Frosted Glass…',
  'Morphology…',
  'Pixelate…',
  'Polar Inversion…',
  'Tile Reflection…',
  'Twist…',
];

const EDGES_EN = ['Clamp', 'Wrap', 'Reflect', 'Transparent'];

/** The row labels of the open menu named `name`, in display order. */
async function rows(page: Page, name: string): Promise<string[]> {
  const list = page.getByRole('menu', { name, exact: true });
  await expect(list).toBeVisible();
  const texts = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return texts.map((r) => r.trim());
}

/** The entries of the Effects menu's Distort submenu (`top` / `sub` in the UI language). */
async function distortions(page: Page, top = 'Effects', sub = 'Distort'): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  await page
    .getByRole('menu', { name: top, exact: true })
    .getByRole('menuitem', { name: sub, exact: true })
    .click();
  const list = await rows(page, sub);
  await page.keyboard.press('Escape');
  return list;
}

async function open(page: Page, entry: string, top = 'Effects', sub = 'Distort'): Promise<Locator> {
  await menu(page, top, sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

type Slider = readonly [label: string, min: string, max: string, value: string, step?: string];
type Choice = readonly [label: string, options: readonly string[], value: string];

/** The dialog shows exactly these controls, with these ranges and values. */
async function expectControls(
  dialog: Locator,
  c: {
    sliders: readonly Slider[];
    choices?: readonly Choice[];
    seed?: boolean;
    point?: boolean;
    angle?: string;
  },
): Promise<void> {
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
  await expect(dialog.getByRole('button', { name: 'Reseed' })).toHaveCount(c.seed ? 1 : 0);
  if (c.seed) {
    const seed = dialog.getByLabel('Seed value');
    await expect(seed).toHaveValue('1');
    await expect(seed).toHaveAttribute('min', '0');
    await expect(seed).toHaveAttribute('max', '9999');
  }
  await expect(dialog.getByRole('button', { name: 'Pick on canvas' })).toHaveCount(c.point ? 1 : 0);
  if (c.point) {
    await expect(dialog.getByLabel('Center X in percent')).toHaveValue('50');
    await expect(dialog.getByLabel('Center Y in percent')).toHaveValue('50');
  }
  if (c.angle !== undefined) {
    await expect(dialog.getByRole('slider', { name: 'Angle dial' })).toHaveAttribute(
      'aria-valuenow',
      c.angle,
    );
    await expect(dialog.getByLabel('Angle in degrees')).toHaveValue(c.angle);
  }
}

test('image mode lists the nine distortions, sorted; pixel mode Morphology and Pixelate (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const list = await distortions(page);
  expect(list).toEqual(DISTORT_EN);
  expect([...list].sort(new Intl.Collator('en').compare)).toEqual(list);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await distortions(page)).toEqual(['Morphology…', 'Pixelate…']);
});

test('the distortion dialogs show exactly the parameters of the table (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const cases: [string, Parameters<typeof expectControls>[1]][] = [
    [
      'Bulge…',
      {
        sliders: [['Amount', '-200', '100', '45']],
        choices: [['Edge behavior', EDGES_EN, '0']],
        point: true,
      },
    ],
    ['Crystalize…', { sliders: [['Cell size', '2', '250', '8']], seed: true }],
    [
      'Dents…',
      {
        sliders: [
          ['Scale', '1', '200', '25'],
          ['Refraction', '0', '200', '50'],
          ['Roughness', '0', '100', '10'],
        ],
        choices: [['Edge behavior', EDGES_EN, '0']],
        seed: true,
      },
    ],
    ['Frosted Glass…', { sliders: [['Amount', '1', '200', '10']], seed: true }],
    [
      'Morphology…',
      {
        sliders: [['Radius', '1', '100', '1']],
        choices: [
          ['Operation', ['Dilate', 'Erode', 'Open', 'Close'], '0'],
          ['Shape', ['Square', 'Circle'], '0'],
        ],
      },
    ],
    [
      'Pixelate…',
      {
        sliders: [['Cell size', '2', '200', '8']],
        // Image mode's default: Average.
        choices: [['Color', ['Average', 'Center'], '0']],
      },
    ],
    [
      'Polar Inversion…',
      {
        sliders: [['Amount', '-4', '4', '1', '0.01']],
        choices: [['Edge behavior', EDGES_EN, '1']],
        point: true,
      },
    ],
    [
      'Tile Reflection…',
      {
        sliders: [
          ['Tile size', '2', '200', '40'],
          ['Curvature', '-100', '100', '8'],
        ],
        choices: [['Edge behavior', EDGES_EN, '0']],
        angle: '30',
      },
    ],
    [
      'Twist…',
      {
        sliders: [
          ['Amount', '-100', '100', '30'],
          ['Size %', '1', '200', '100'],
        ],
        choices: [['Edge behavior', EDGES_EN, '0']],
        point: true,
      },
    ],
  ];
  for (const [entry, controls] of cases) {
    const dialog = await open(page, entry);
    await expectControls(dialog, controls);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }

  // Pixel mode: the same two dialogs, Pixelate's Colour starting at Centre.
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  let dialog = await open(page, 'Pixelate…');
  await expectControls(dialog, {
    sliders: [['Cell size', '2', '200', '8']],
    choices: [['Color', ['Average', 'Center'], '1']],
  });
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByRole('combobox', { name: 'Color' })).toHaveValue('1');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  dialog = await open(page, 'Morphology…');
  await expectControls(dialog, {
    sliders: [['Radius', '1', '100', '1']],
    choices: [
      ['Operation', ['Dilate', 'Erode', 'Open', 'Close'], '0'],
      ['Shape', ['Square', 'Circle'], '0'],
    ],
  });
});

test('German names, parameter labels and edge-behaviour options (AC-15)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await switchLanguage(page, 'Deutsch');
  const list = await distortions(page, 'Effekte', 'Verzerren');
  const names = [
    'Wölben…',
    'Kristallisieren…',
    'Beulen…',
    'Milchglas…',
    'Morphologie…',
    'Verpixeln…',
    'Polarkoordinaten umkehren…',
    'Kachelspiegelung…',
    'Verdrehen…',
  ];
  expect([...list].sort()).toEqual([...names].sort());
  expect([...list].sort(new Intl.Collator('de').compare)).toEqual(list);

  const edges = ['Rand wiederholen', 'Kacheln', 'Spiegeln', 'Transparent'];
  const labels: Record<string, { sliders: readonly string[]; choices: readonly Choice[] }> = {
    'Wölben…': { sliders: ['Stärke'], choices: [['Randverhalten', edges, '0']] },
    'Kristallisieren…': { sliders: ['Zellgröße'], choices: [] },
    'Beulen…': {
      sliders: ['Skalierung', 'Brechung', 'Rauheit'],
      choices: [['Randverhalten', edges, '0']],
    },
    'Milchglas…': { sliders: ['Stärke'], choices: [] },
    'Morphologie…': {
      sliders: ['Radius'],
      choices: [
        ['Operation', ['Ausdehnen', 'Abtragen', 'Öffnen', 'Schließen'], '0'],
        ['Form', ['Quadrat', 'Kreis'], '0'],
      ],
    },
    'Verpixeln…': { sliders: ['Zellgröße'], choices: [['Farbe', ['Durchschnitt', 'Mitte'], '0']] },
    'Polarkoordinaten umkehren…': {
      sliders: ['Stärke'],
      choices: [['Randverhalten', edges, '1']],
    },
    'Kachelspiegelung…': {
      sliders: ['Kachelgröße', 'Krümmung'],
      choices: [['Randverhalten', edges, '0']],
    },
    'Verdrehen…': { sliders: ['Stärke', 'Größe %'], choices: [['Randverhalten', edges, '0']] },
  };
  for (const [entry, { sliders, choices }] of Object.entries(labels)) {
    const dialog = await open(page, entry, 'Effekte', 'Verzerren');
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
    if (entry === 'Kachelspiegelung…') {
      await expect(dialog.getByLabel('Winkel in Grad')).toBeVisible();
    }
    if (entry === 'Wölben…') {
      await expect(dialog.getByLabel('Mitte X in Prozent')).toBeVisible();
    }
    if (entry === 'Milchglas…') {
      await expect(dialog.getByLabel('Startwert Wert')).toBeVisible();
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
});
