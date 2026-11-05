import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  setColor,
  type ClipImage,
} from './helpers';
import { effectById, toParams, defaultValues } from '../src/lib/effects/catalog';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';

// docs/specs/outline-effect.md: Effects ▸ Object ▸ Outline Object… in both
// modes, the colour parameter kind of the effect panel, live preview, one
// undo step. Pixel math (AC-02 – AC-06, AC-12, AC-13) is covered in
// crates/pincel-effects/tests/outline_effect.rs, the cores' selection and
// refusal paths (AC-07, AC-10) in pincel-wasm / pincel-image-wasm.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const N = 5;
const GREEN = [0, 255, 0, 255];
const CLEAR = [0, 0, 0, 0];

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });
const hexField = (dialog: Locator) => dialog.getByLabel('Color hex', { exact: true });

/** Centre of sprite / document pixel `(x, y)` of an `N`² canvas. */
async function at(canvas: Locator, x: number, y: number): Promise<[number, number]> {
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + ((x + 0.5) * box.width) / N, box.y + ((y + 0.5) * box.height) / N];
}

/** The active layer as a whole, read through Copy (these tests never
 *  select, and Image mode's Deselect would be an undo step of its own). */
async function activeLayer(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

/** An `N`² document of `mode` with one green pixel in the centre, then
 *  `color` as the foreground colour. */
async function dot(page: Page, mode: 'pixel' | 'image', color: string): Promise<Locator> {
  await createDocument(page, mode, { width: N, height: N });
  const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
  await setColor(page, 'fg', '#00ff00');
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  // Image mode starts on its Brush (hard round); 1 px paints one pixel.
  else await page.getByRole('slider', { name: /^Size / }).fill('1');
  await page.mouse.click(...(await at(canvas, 2, 2)));
  await setColor(page, 'fg', color);
  return canvas;
}

async function openOutline(page: Page): Promise<Locator> {
  await menu(page, 'Effects', 'Object', 'Outline Object…');
  const dialog = page.getByRole('dialog', { name: 'Outline Object' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** RGBA of a `#rrggbb` colour at full alpha. */
const rgba = (hex: string): number[] => [
  parseInt(hex.slice(1, 3), 16),
  parseInt(hex.slice(3, 5), 16),
  parseInt(hex.slice(5, 7), 16),
  255,
];

function expectOutline(layer: ClipImage, color: number[]): void {
  for (let y = 0; y < N; y += 1) {
    for (let x = 0; x < N; x += 1) {
      const ring = x >= 1 && x <= 3 && y >= 1 && y <= 3;
      const want = x === 2 && y === 2 ? GREEN : ring ? color : CLEAR;
      expect(pixelOf(layer, x, y), `(${x}, ${y})`).toEqual(want);
    }
  }
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Outline Object opens with its defaults (AC-01)`, async ({ page }) => {
    await dot(page, mode, '#3366cc');
    const dialog = await openOutline(page);
    const place = dialog.getByLabel('Place', { exact: true });
    await expect(place).toHaveValue('0');
    await expect(place.locator('option')).toHaveText(['Outside', 'Inside', 'Both']);
    const shape = dialog.getByLabel('Shape', { exact: true });
    await expect(shape).toHaveValue('1');
    await expect(shape.locator('option')).toHaveText(['Circle', 'Square']);
    const thickness = dialog.getByRole('slider', { name: 'Thickness', exact: true });
    await expect(thickness).toHaveAttribute('min', '1');
    await expect(thickness).toHaveAttribute('max', '8');
    await expect(thickness).toHaveValue('1');
    // The colour starts at the current foreground colour.
    await expect(hexField(dialog)).toHaveValue('#3366ccff');
    await expect(dialog.getByLabel('Color', { exact: true })).toHaveValue('#3366cc');
  });

  test(`${mode} mode: the outline previews live, cancels, applies in one undo step (AC-08, AC-09)`, async ({
    page,
  }) => {
    const canvas = await dot(page, mode, '#ff0000');
    const before = await activeLayer(page);
    const shot = await canvas.screenshot();

    let dialog = await openOutline(page);
    // Live preview: the canvas shows the outline while the panel is open.
    await expect.poll(async () => (await canvas.screenshot()).equals(shot)).toBe(false);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => (await canvas.screenshot()).equals(shot)).toBe(true);
    expect((await activeLayer(page)).data).toEqual(before.data);

    dialog = await openOutline(page);
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);
    const after = await activeLayer(page);
    expectOutline(after, rgba('#ff0000'));

    await undoButton(page).click();
    expect((await activeLayer(page)).data).toEqual(before.data);
    await redoButton(page).click();
    expect((await activeLayer(page)).data).toEqual(after.data);
    // The pencil dot is still the step before.
    await undoButton(page).click();
    await undoButton(page).click();
    await expect(undoButton(page)).toBeDisabled();
  });
}

test('pixel mode: the colour takes hex text with alpha and refuses invalid text (AC-06, AC-11)', async ({
  page,
}) => {
  await dot(page, 'pixel', '#ff0000');
  const dialog = await openOutline(page);
  const hex = hexField(dialog);
  const swatch = dialog.getByLabel('Color', { exact: true });

  await hex.fill('#0000ff80');
  await expect(hex).toHaveAttribute('aria-invalid', 'false');
  await expect(swatch).toHaveValue('#0000ff');
  // Invalid text is marked and not used: OK applies the last valid colour.
  for (const bad of ['zzz', '#12345', '0000ff', '#00ff00f', '#gggggg']) {
    await hex.fill(bad);
    await expect(hex, bad).toHaveAttribute('aria-invalid', 'true');
    await expect(swatch).toHaveValue('#0000ff');
  }
  await dialog.getByRole('button', { name: 'OK' }).click();
  expectOutline(await activeLayer(page), [0, 0, 255, 0x80]);
});

test('pixel mode: the native colour input keeps the alpha of the hex text', async ({ page }) => {
  await dot(page, 'pixel', '#ff0000');
  const dialog = await openOutline(page);
  await hexField(dialog).fill('#ff000040');
  await dialog.getByLabel('Color', { exact: true }).fill('#00ffff');
  await expect(hexField(dialog)).toHaveValue('#00ffff40');
  await expect(hexField(dialog)).toHaveAttribute('aria-invalid', 'false');
  // Reset goes back to the foreground colour.
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(hexField(dialog)).toHaveValue('#ff0000ff');
});

test('the catalog sends the colour as one 0xRRGGBBAA slot and names it in both languages', () => {
  const def = effectById('outline');
  if (!def) throw new Error('outline missing from the catalog');
  expect(def.group).toBe('object');
  expect(def.modes).toEqual(['pixel', 'image']);
  const values = defaultValues(def, { foreground: 0x3366ccff, background: 0xffffffff });
  expect(toParams(def, values)).toEqual([0, 1, 1, 0x3366ccff]);
  expect(en['effect.outline']).toBe('Outline Object');
  expect(de['effect.outline']).toBe('Objekt umranden');
  for (const key of [
    'Place',
    'Outside',
    'Inside',
    'Both',
    'Shape',
    'Circle',
    'Square',
    'Thickness',
    'Color',
  ] as const) {
    expect(en[`fx.${key}`], key).toBe(key);
    expect(de[`fx.${key}`], key).toBeTruthy();
  }
});
