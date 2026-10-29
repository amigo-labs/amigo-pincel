import { expect, test, type Page } from '@playwright/test';
import { createDocument, pixelCanvas } from './helpers';

// Pixel-mode Pencil / Eraser (docs/specs/pixel-pencil.md): brush size and
// shape, Shift constraint, pixel-perfect, shared and persisted options.

const N = 16;
type At = (px: number, py: number) => readonly [number, number];

/** A fresh N×N sprite; returns the sprite-pixel → page-point mapper. */
async function setup(page: Page): Promise<At> {
  await createDocument(page, 'pixel', { width: N, height: N });
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return (px, py) =>
    [box.x + ((px + 0.5) * box.width) / N, box.y + ((py + 0.5) * box.height) / N] as const;
}

const fg = (page: Page) => page.getByLabel('Foreground color', { exact: true });
const sizeField = (page: Page) => page.getByLabel('Brush size', { exact: true });
const sizeSlider = (page: Page) => page.getByLabel('Brush size slider');
const shapeSelect = (page: Page) => page.getByLabel('Brush shape');
const pixelPerfect = (page: Page) => page.getByLabel('Pixel-perfect');

async function pencil(page: Page): Promise<void> {
  await page.getByTitle('Pencil (B)').click();
}

async function eraser(page: Page): Promise<void> {
  await page.getByTitle('Eraser (E)').click();
}

async function setSize(page: Page, value: string): Promise<void> {
  await sizeField(page).fill(value);
  await sizeField(page).blur();
}

/** Sample sprite pixel (x, y) with the Eyedropper; `#000000` is transparent. */
async function pick(page: Page, at: At, x: number, y: number): Promise<string> {
  await page.getByTitle(/Eyedropper/).click();
  await page.mouse.click(...at(x, y));
  return fg(page).inputValue();
}

/** Press at the first point, move through the rest one sample each, release. */
async function stroke(page: Page, at: At, pts: [number, number][]): Promise<void> {
  const [first, ...rest] = pts;
  if (!first) return;
  await page.mouse.move(...at(...first));
  await page.mouse.down();
  for (const p of rest) await page.mouse.move(...at(...p), { steps: 1 });
  await page.mouse.up();
}

test('pixel mode: a fast pencil stroke stays connected', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  // One jump from (1, 1) to (14, 14): no pointer samples in between.
  await stroke(page, at, [
    [1, 1],
    [14, 14],
  ]);
  // Sample a pixel in the middle of the diagonal with the eyedropper.
  await fg(page).fill('#000000');
  expect(await pick(page, at, 7, 7)).toBe('#00ff00');
});

test('pencil and eraser show size, shape and pixel-perfect options', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await expect(sizeField(page)).toHaveValue('1');
  await expect(sizeField(page)).toHaveAttribute('min', '1');
  await expect(sizeField(page)).toHaveAttribute('max', '8');
  await expect(sizeSlider(page)).toHaveAttribute('max', '8');
  await expect(shapeSelect(page)).toHaveValue('square');
  // A fresh session starts with pixel-perfect off.
  await expect(pixelPerfect(page)).not.toBeChecked();
  await expect(pixelPerfect(page)).toBeEnabled();

  await eraser(page);
  await expect(sizeField(page)).toBeVisible();
  await expect(shapeSelect(page)).toBeVisible();
  await expect(pixelPerfect(page)).toBeVisible();
});

test('pencil and eraser share one size / shape / pixel-perfect setting', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await pixelPerfect(page).check();
  await setSize(page, '3');
  await shapeSelect(page).selectOption('round');
  await eraser(page);
  await expect(sizeField(page)).toHaveValue('3');
  await expect(shapeSelect(page)).toHaveValue('round');
  await expect(pixelPerfect(page)).toBeChecked();

  await setSize(page, '1');
  await shapeSelect(page).selectOption('square');
  await pixelPerfect(page).uncheck();
  await pencil(page);
  await expect(sizeField(page)).toHaveValue('1');
  await expect(shapeSelect(page)).toHaveValue('square');
  await expect(pixelPerfect(page)).not.toBeChecked();
});

test('pixel-perfect is disabled above size 1', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await pixelPerfect(page).check();
  await setSize(page, '3');
  await expect(pixelPerfect(page)).toBeDisabled();
  await setSize(page, '1');
  await expect(pixelPerfect(page)).toBeEnabled();
});

test('a typed size outside 1-8 clamps to the nearest bound', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await setSize(page, '12');
  await expect(sizeField(page)).toHaveValue('8');
  await setSize(page, '0');
  await expect(sizeField(page)).toHaveValue('1');
});

test('[ and ] step the brush size within 1-8', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await page.keyboard.press(']');
  await expect(sizeField(page)).toHaveValue('2');
  for (let i = 0; i < 9; i += 1) await page.keyboard.press(']');
  await expect(sizeField(page)).toHaveValue('8');
  await eraser(page);
  await page.keyboard.press('[');
  await expect(sizeField(page)).toHaveValue('7');
  for (let i = 0; i < 9; i += 1) await page.keyboard.press('[');
  await expect(sizeField(page)).toHaveValue('1');
});

test('a size-3 pencil click paints a 3x3 block', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  await setSize(page, '3');
  await page.mouse.click(...at(5, 5));
  expect(await pick(page, at, 4, 4)).toBe('#00ff00');
  expect(await pick(page, at, 6, 6)).toBe('#00ff00');
  expect(await pick(page, at, 3, 5)).toBe('#000000');
  expect(await pick(page, at, 7, 5)).toBe('#000000');
});

test('a pixel-perfect staircase leaves its corners empty', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  await pixelPerfect(page).check();
  await stroke(page, at, [
    [2, 2],
    [3, 2],
    [3, 3],
    [4, 3],
    [4, 4],
  ]);
  expect(await pick(page, at, 3, 2)).toBe('#000000');
  expect(await pick(page, at, 4, 3)).toBe('#000000');
  expect(await pick(page, at, 3, 3)).toBe('#00ff00');
  expect(await pick(page, at, 4, 4)).toBe('#00ff00');
});

test('a Shift pencil drag snaps to the nearest 45 degree direction', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  await page.mouse.move(...at(2, 3));
  await page.mouse.down();
  await page.keyboard.down('Shift');
  await page.mouse.move(...at(12, 5), { steps: 1 });
  await page.keyboard.up('Shift');
  await page.mouse.up();
  await page.mouse.move(...at(2, 8));
  await page.mouse.down();
  await page.keyboard.down('Shift');
  await page.mouse.move(...at(8, 13), { steps: 1 });
  await page.keyboard.up('Shift');
  await page.mouse.up();
  // Nearly horizontal: the first stroke stays on row 3.
  expect(await pick(page, at, 12, 3)).toBe('#00ff00');
  expect(await pick(page, at, 12, 5)).toBe('#000000');
  // Closer to 45° than to 90°: the second is a diagonal of length 6.
  expect(await pick(page, at, 8, 14)).toBe('#00ff00');
  expect(await pick(page, at, 8, 13)).toBe('#000000');
});

/** Drag from `from` with Shift held through `pts`, then release both. */
async function shiftDrag(
  page: Page,
  at: At,
  from: [number, number],
  pts: [number, number][],
): Promise<void> {
  await page.mouse.move(...at(...from));
  await page.mouse.down();
  await page.keyboard.down('Shift');
  for (const p of pts) await page.mouse.move(...at(...p), { steps: 1 });
  await page.keyboard.up('Shift');
  await page.mouse.up();
}

test('a Shift drag keeps the direction its first movement chose', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  // Starts horizontal, then swings towards the diagonal: stays on row 3.
  await shiftDrag(page, at, [2, 3], [
    [8, 4],
    [12, 10],
  ]);
  // Starts vertical (90°) and drifts sideways: stays on column 5.
  await shiftDrag(page, at, [5, 6], [
    [6, 9],
    [8, 12],
  ]);
  // The Eraser constrains the same way.
  await stroke(page, at, [
    [1, 14],
    [14, 14],
  ]);
  await eraser(page);
  await shiftDrag(page, at, [3, 14], [
    [7, 15],
    [11, 11],
  ]);

  expect(await pick(page, at, 12, 3)).toBe('#00ff00');
  expect(await pick(page, at, 10, 8)).toBe('#000000');
  expect(await pick(page, at, 5, 12)).toBe('#00ff00');
  expect(await pick(page, at, 7, 11)).toBe('#000000');
  expect(await pick(page, at, 11, 14)).toBe('#000000');
  expect(await pick(page, at, 10, 12)).toBe('#000000');
  expect(await pick(page, at, 12, 14)).toBe('#00ff00');
  expect(await pick(page, at, 2, 14)).toBe('#00ff00');
});

test('Shift pressed mid-drag anchors at the last painted point', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await fg(page).fill('#00ff00');
  await page.mouse.move(...at(2, 3));
  await page.mouse.down();
  await page.mouse.move(...at(6, 6), { steps: 1 });
  // Shift goes down after a freehand segment: the constrained segment
  // starts at (6, 6). From the press point (2, 3) it would end at (12, 3).
  await page.keyboard.down('Shift');
  await page.mouse.move(...at(12, 7), { steps: 1 });
  // Shift released mid-drag: the stroke continues freehand from (12, 6).
  await page.keyboard.up('Shift');
  await page.mouse.move(...at(12, 12), { steps: 1 });
  await page.mouse.up();

  expect(await pick(page, at, 10, 6)).toBe('#00ff00');
  expect(await pick(page, at, 12, 6)).toBe('#00ff00');
  expect(await pick(page, at, 12, 3)).toBe('#000000');
  // Freehand after release: straight down from (12, 6) to (12, 12).
  expect(await pick(page, at, 12, 10)).toBe('#00ff00');
  expect(await pick(page, at, 13, 10)).toBe('#000000');
});

test('a size-3 right click paints the background color', async ({ page }) => {
  const at = await setup(page);
  await pencil(page);
  await page.getByLabel('Background color', { exact: true }).fill('#0000ff');
  await setSize(page, '3');
  await page.mouse.click(...at(5, 5), { button: 'right' });
  expect(await pick(page, at, 4, 4)).toBe('#0000ff');
  expect(await pick(page, at, 6, 6)).toBe('#0000ff');
  expect(await pick(page, at, 7, 5)).toBe('#000000');
});

test('brush settings survive a reload', async ({ page }) => {
  await setup(page);
  await pencil(page);
  await pixelPerfect(page).check();
  await setSize(page, '5');
  await shapeSelect(page).selectOption('round');
  // Wait until the pref reached IndexedDB before navigating away.
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          new Promise<unknown>((resolve) => {
            const open = indexedDB.open('pincel');
            open.onsuccess = () => {
              const get = open.result.transaction('prefs').objectStore('prefs').get('pixelBrush');
              get.onsuccess = () => resolve((get.result as { value?: unknown } | undefined)?.value);
              get.onerror = () => resolve(null);
            };
            open.onerror = () => resolve(null);
          }),
      ),
    )
    .toEqual({ size: 5, shape: 'round', pixelPerfect: true });
  await page.reload();
  await setup(page);
  await pencil(page);
  await expect(sizeField(page)).toHaveValue('5');
  await expect(shapeSelect(page)).toHaveValue('round');
  await expect(pixelPerfect(page)).toBeChecked();
});

test.describe('touch device', () => {
  test.use({ hasTouch: true });

  test('the latched Shift toggle constrains a pencil drag', async ({ page }) => {
    await setup(page);
    // Any touch on the stage reveals the toggles; fit the view again after.
    await page.getByRole('button', { name: 'Actual size' }).tap();
    const shift = page.getByRole('button', { name: 'Shift' });
    await shift.click();
    await expect(shift).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('0');
    const box = await pixelCanvas(page).boundingBox();
    if (!box) throw new Error('canvas not visible');
    const at: At = (px, py) =>
      [box.x + ((px + 0.5) * box.width) / N, box.y + ((py + 0.5) * box.height) / N] as const;
    await pencil(page);
    await fg(page).fill('#00ff00');
    await stroke(page, at, [
      [2, 3],
      [12, 5],
    ]);
    await shift.click();
    expect(await pick(page, at, 12, 3)).toBe('#00ff00');
    expect(await pick(page, at, 12, 5)).toBe('#000000');
  });
});
