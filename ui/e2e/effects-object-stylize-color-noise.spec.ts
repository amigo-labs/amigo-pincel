import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  menu,
  pixelCanvas,
  pixelOf,
  setColor,
  type ClipImage,
} from './helpers';

// docs/specs/effects-object-stylize-color-noise.md: Object ▸ Drop Shadow,
// Stylize ▸ Outline, Color ▸ Quantize and Noise ▸ Reduce Noise in their
// submenus per mode (AC-01), their dialogs with per-mode defaults (AC-02)
// and the German names and labels (AC-12). Pixel math, wire errors and the
// Pixel core are covered by the Rust tests
// (crates/pincel-effects/tests/effects_object_stylize_color_noise.rs,
// pincel-wasm, pincel-agent); the wire vectors by effect-catalog.spec.ts.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** The row labels of the open menu named `name`, in display order. */
async function rows(page: Page, name: string): Promise<string[]> {
  const list = page.getByRole('menu', { name, exact: true });
  await expect(list).toBeVisible();
  const texts = await list.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return texts.map((r) => r.trim());
}

/** Every submenu of the Effects menu (`top` in the UI language) with its entries. */
async function effectsTree(page: Page, top = 'Effects'): Promise<Record<string, string[]>> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  const tree: Record<string, string[]> = {};
  for (const sub of await rows(page, top)) {
    await page
      .getByRole('menu', { name: top, exact: true })
      .getByRole('menuitem', { name: sub, exact: true })
      .click();
    tree[sub] = await rows(page, sub);
  }
  await page.keyboard.press('Escape');
  return tree;
}

function expectSorted(list: readonly string[], locale: string): void {
  expect([...list].sort(new Intl.Collator(locale).compare)).toEqual(list);
}

async function open(page: Page, sub: string, entry: string, top = 'Effects'): Promise<Locator> {
  await menu(page, top, sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

type Slider = readonly [label: string, min: string, max: string, value: string];
type Choice = readonly [label: string, options: readonly string[], value: string];

/** The dialog shows exactly these controls, with these ranges and values. */
async function expectControls(
  dialog: Locator,
  c: { sliders: readonly Slider[]; choices?: readonly Choice[]; color?: string },
): Promise<void> {
  await expect(dialog.locator('input[type="range"]')).toHaveCount(c.sliders.length);
  for (const [label, min, max, value] of c.sliders) {
    const slider = dialog.getByRole('slider', { name: label, exact: true });
    await expect(slider).toHaveAttribute('min', min);
    await expect(slider).toHaveAttribute('max', max);
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
  await expect(dialog.locator('input[type="color"]')).toHaveCount(c.color === undefined ? 0 : 1);
  if (c.color !== undefined) {
    await expect(dialog.getByLabel('Color hex', { exact: true })).toHaveValue(c.color);
    await expect(dialog.getByLabel('Color', { exact: true })).toHaveValue(c.color.slice(0, 7));
  }
  // No seed, point or angle control in any of the four.
  await expect(dialog.getByRole('button', { name: 'Reseed' })).toHaveCount(0);
  await expect(dialog.getByRole('button', { name: 'Pick on canvas' })).toHaveCount(0);
}

const SHADOW = (blur: string): Parameters<typeof expectControls>[1] => ({
  sliders: [
    ['Offset X', '-100', '100', '4'],
    ['Offset Y', '-100', '100', '4'],
    ['Blur radius', '0', '50', blur],
    ['Opacity', '0', '100', '50'],
  ],
  color: '#000000ff',
});

const QUANTIZE: Parameters<typeof expectControls>[1] = {
  sliders: [
    ['Colors', '2', '256', '16'],
    ['Dithering', '0', '100', '0'],
  ],
  choices: [['Algorithm', ['Median Cut', 'Octree'], '0']],
};

test('the four entries sit in their submenus, Outline and Reduce Noise in Image mode only (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  let tree = await effectsTree(page);
  expect(tree.Object).toContain('Drop Shadow…');
  expect(tree.Stylize).toContain('Outline…');
  expect(tree.Color).toContain('Quantize…');
  expect(tree.Noise).toContain('Reduce Noise…');
  for (const list of Object.values(tree)) expectSorted(list, 'en');

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  tree = await effectsTree(page);
  expect(tree.Object).toContain('Drop Shadow…');
  expect(tree.Color).toContain('Quantize…');
  const all = Object.values(tree).flat();
  expect(all).not.toContain('Outline…');
  expect(all).not.toContain('Reduce Noise…');
  for (const list of Object.values(tree)) expectSorted(list, 'en');
});

test('the dialogs show exactly the parameters of the table with their defaults (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const cases: [string, string, Parameters<typeof expectControls>[1]][] = [
    ['Object', 'Drop Shadow…', SHADOW('4')],
    [
      'Stylize',
      'Outline…',
      {
        sliders: [
          ['Thickness', '1', '200', '3'],
          ['Intensity', '0', '100', '50'],
        ],
      },
    ],
    ['Color', 'Quantize…', QUANTIZE],
    [
      'Noise',
      'Reduce Noise…',
      {
        sliders: [
          ['Radius', '1', '200', '6'],
          ['Strength', '0', '100', '40'],
        ],
      },
    ],
  ];
  for (const [sub, entry, controls] of cases) {
    const dialog = await open(page, sub, entry);
    await expect(dialog.getByRole('heading', { name: entry.replace('…', '') })).toBeVisible();
    await expectControls(dialog, controls);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
  }

  // Pixel mode: Drop Shadow's Blur radius starts at 0, also after Reset.
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  let dialog = await open(page, 'Object', 'Drop Shadow…');
  await expectControls(dialog, SHADOW('0'));
  await dialog.getByLabel('Blur radius value', { exact: true }).fill('7');
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByRole('slider', { name: 'Blur radius', exact: true })).toHaveValue('0');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  dialog = await open(page, 'Color', 'Quantize…');
  await expectControls(dialog, QUANTIZE);
});

/** The active layer as a whole, read through Copy. */
async function activeLayer(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

test('pixel mode: Drop Shadow with its defaults casts a hard half-opaque black shadow', async ({
  page,
}) => {
  const n = 8;
  await createDocument(page, 'pixel', { width: n, height: n });
  await setColor(page, 'fg', '#00ff00');
  await page.getByTitle('Pencil (B)').click();
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.click(box.x + (1.5 * box.width) / n, box.y + (1.5 * box.height) / n);
  const dialog = await open(page, 'Object', 'Drop Shadow…');
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);
  const layer = await activeLayer(page);
  for (let y = 0; y < n; y += 1) {
    for (let x = 0; x < n; x += 1) {
      const want =
        x === 1 && y === 1 ? [0, 255, 0, 255] : x === 5 && y === 5 ? [0, 0, 0, 128] : [0, 0, 0, 0];
      expect(pixelOf(layer, x, y), `(${x}, ${y})`).toEqual(want);
    }
  }
});

test('German names and parameter labels (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const tree = await effectsTree(page, 'Effekte');
  expect(tree.Objekt).toContain('Schlagschatten…');
  expect(tree.Stilisieren).toContain('Kontur…');
  expect(tree.Farbe).toContain('Quantisieren…');
  expect(tree.Rauschen).toContain('Rauschen entfernen…');
  for (const list of Object.values(tree)) expectSorted(list, 'de');

  const cases: [string, string, readonly string[], readonly Choice[]][] = [
    [
      'Objekt',
      'Schlagschatten…',
      ['Versatz X', 'Versatz Y', 'Weichzeichnungsradius', 'Deckkraft'],
      [],
    ],
    ['Stilisieren', 'Kontur…', ['Dicke', 'Intensität'], []],
    [
      'Farbe',
      'Quantisieren…',
      ['Farben', 'Dithering'],
      [['Algorithmus', ['Median-Schnitt', 'Octree'], '0']],
    ],
    ['Rauschen', 'Rauschen entfernen…', ['Radius', 'Stärke'], []],
  ];
  for (const [sub, entry, sliders, choices] of cases) {
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
    if (entry === 'Schlagschatten…') {
      await expect(dialog.getByLabel('Farbe als Hex', { exact: true })).toHaveValue('#000000ff');
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
});
