import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, setColor } from './helpers';

// Image-mode spiral gradient kinds (docs/specs/image-palette-gradient-spiral.md).

const KINDS = [
  ['linear', 'Linear'],
  ['reflected', 'Reflected'],
  ['radial', 'Radial'],
  ['conical', 'Conical'],
  ['diamond', 'Diamond'],
  ['spiral_cw', 'Spiral (clockwise)'],
  ['spiral_ccw', 'Spiral (counter-clockwise)'],
] as const;

/** The [value, label] pairs of a select, in order. */
async function options(page: Page, label: string): Promise<string[][]> {
  return page
    .getByLabel(label)
    .locator('option')
    .evaluateAll((els) =>
      els.map((o) => [(o as HTMLOptionElement).value, o.textContent?.trim() ?? '']),
    );
}

/**
 * The RGB shown at the centre of document pixel (x, y) of a 32 × 32 image,
 * read from a screenshot of the canvas (the app never reads canvas pixels;
 * the test reads the screen).
 */
async function shownAt(page: Page, x: number, y: number): Promise<number[]> {
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const px = Math.floor(bitmap.width * fx);
      const py = Math.floor(bitmap.height * fy);
      return Array.from(ctx.getImageData(px, py, 1, 1).data.slice(0, 3));
    },
    { b64: png.toString('base64'), fx: (x + 0.5) / 32, fy: (y + 0.5) / 32 },
  );
}

/** Every channel of the pixel is `grey` ±1. */
async function expectGrey(page: Page, x: number, y: number, grey: number): Promise<void> {
  await expect
    .poll(async () => (await shownAt(page, x, y)).every((c) => Math.abs(c - grey) <= 1), {
      message: `pixel (${x}, ${y}) should be grey ${grey}`,
    })
    .toBe(true);
}

test('the Gradient options offer both spirals after the five kinds (AC-01)', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('n');
  expect(await options(page, 'Ramp')).toEqual(KINDS.map(([v, l]) => [v, l]));
});

test('a spiral drag paints the spec example and is one undo step (AC-03, AC-08)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await setColor(page, 'fg', '#000000');
  await setColor(page, 'bg', '#ffffff');
  await page.keyboard.press('n');
  const ramp = page.getByLabel('Ramp');

  // From the centre of (8, 8) to the centre of (24, 8).
  const from: [number, number] = [8.5 / 32, 8.5 / 32];
  const to: [number, number] = [24.5 / 32, 8.5 / 32];
  for (const [kind, below, above] of [
    ['spiral_cw', 64, 191],
    ['spiral_ccw', 191, 64],
  ] as const) {
    await expect(undo).toBeDisabled();
    await ramp.selectOption(kind);
    await drag(page, imageCanvas(page), from, to);
    await expect(undo).toBeEnabled();
    // Enter commits the live gradient, so its handles and their guide line
    // no longer cover row 8.
    await page.keyboard.press('Enter');
    await expect(page.locator('[data-live-handle]')).toHaveCount(0);
    await expectGrey(page, 16, 8, 128);
    await expectGrey(page, 8, 16, below);
    await expectGrey(page, 8, 0, above);
    // One Undo takes the whole drag back.
    await undo.click();
    await expect(undo).toBeDisabled();
  }
});

test.describe('German', () => {
  test('the spiral kinds read in German (AC-14)', async ({ page }) => {
    await createDocument(page, 'image', { width: 64, height: 64 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Language' }).click();
    await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
    await page.keyboard.press('n');
    const kinds = await options(page, 'Verlaufsart');
    expect(kinds.slice(5)).toEqual([
      ['spiral_cw', 'Spirale (im Uhrzeigersinn)'],
      ['spiral_ccw', 'Spirale (gegen den Uhrzeigersinn)'],
    ]);
  });
});
