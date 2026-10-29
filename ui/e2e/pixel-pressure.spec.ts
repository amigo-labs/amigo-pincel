import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, penStroke, pixelCanvas } from './helpers';

// Pen pressure in Pixel mode (docs/specs/pixel-pressure.md): pressure
// scales the Pencil / Eraser stamp size behind the "Size" option; mouse,
// touch and pen strokes with the option off paint at full size.

const W = 32;
const H = 16;

/** A fresh W×H sprite with the Pencil at size 8 painting red. */
async function setup(page: Page): Promise<Locator> {
  await createDocument(page, 'pixel', { width: W, height: H });
  await page.getByTitle('Pencil (B)').click();
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  const size = page.getByLabel('Brush size', { exact: true });
  await size.fill('8');
  await size.blur();
  // The first pen contact reveals the on-screen Shift / Alt toggles over
  // the canvas corner; reveal them now so every screenshot shows them.
  const canvas = pixelCanvas(page);
  await penStroke(page, canvas, 1);
  await undoButton(page).click();
  await expect(page.getByRole('button', { name: 'Shift' })).toBeVisible();
  return canvas;
}

function undoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Undo', exact: true });
}
const pressureGroup = (page: Page) => page.getByRole('group', { name: 'Pen pressure' });

/** Screenshot of `canvas` once two consecutive shots agree (no redraw pending). */
async function stableShot(canvas: Locator): Promise<Buffer> {
  let last = await canvas.screenshot();
  await expect
    .poll(async () => {
      const next = await canvas.screenshot();
      const same = next.equals(last);
      last = next;
      return same;
    })
    .toBe(true);
  return last;
}

/**
 * Screenshot of the stroke on `canvas` once it is drawn, then undo it and
 * wait until the canvas shows `blank` again: one stroke is one undo entry.
 * The canvas redraws on the next animation frame, so both waits poll.
 */
async function shotAndUndo(page: Page, canvas: Locator, blank: Buffer): Promise<Buffer> {
  await expect(undoButton(page)).toBeEnabled();
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  const shot = await stableShot(canvas);
  await undoButton(page).click();
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(true);
  return shot;
}

/** Red screen pixels in a PNG screenshot (decoded by the browser). */
async function redPixels(page: Page, png: Buffer): Promise<number> {
  return page.evaluate(async (b64) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = surface.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    let n = 0;
    for (let i = 0; i < data.length; i += 4) {
      if ((data[i] ?? 0) > 200 && (data[i + 1] ?? 0) < 60 && (data[i + 2] ?? 0) < 60) n += 1;
    }
    return n;
  }, png.toString('base64'));
}

test('pencil and eraser show a pen pressure Size option, checked by default', async ({ page }) => {
  await createDocument(page, 'pixel', { width: W, height: H });
  for (const title of ['Pencil (B)', 'Eraser (E)']) {
    await page.getByTitle(title).click();
    const group = pressureGroup(page);
    await expect(group).toBeVisible();
    await expect(group).toHaveAttribute(
      'title',
      'What a stylus’s pressure varies (mouse and touch paint at full pressure)',
    );
    // A fresh session starts with size dynamics on, as in Image mode.
    await expect(group.getByRole('checkbox', { name: 'Size', exact: true })).toBeChecked();
    await expect(page.getByRole('checkbox', { name: 'Opacity' })).toHaveCount(0);
  }
  // The shapes have no pressure option.
  await page.getByTitle(/^Line/).click();
  await expect(pressureGroup(page)).toHaveCount(0);
});

test('pixel mode: a light pen stroke is thinner than a firm one', async ({ page }) => {
  const canvas = await setup(page);
  const blank = await stableShot(canvas);

  await penStroke(page, canvas, 1);
  const firm = await shotAndUndo(page, canvas, blank);

  await penStroke(page, canvas, 0.1);
  const light = await shotAndUndo(page, canvas, blank);

  const [firmRed, lightRed] = [await redPixels(page, firm), await redPixels(page, light)];
  expect(lightRed).toBeGreaterThan(0);
  expect(lightRed).toBeLessThan(firmRed);
});

test('mouse strokes and pen strokes with Size off paint at full size', async ({ page }) => {
  const canvas = await setup(page);
  const blank = await stableShot(canvas);
  await penStroke(page, canvas, 1);
  const firm = await shotAndUndo(page, canvas, blank);

  // A mouse carries no pressure, whatever the option says.
  await penStroke(page, canvas, 0.1, undefined, 'mouse');
  expect((await shotAndUndo(page, canvas, blank)).equals(firm)).toBe(true);

  // With the option off, a light pen stroke paints the full size.
  await pressureGroup(page).getByRole('checkbox', { name: 'Size', exact: true }).uncheck();
  await penStroke(page, canvas, 0.1);
  expect((await shotAndUndo(page, canvas, blank)).equals(firm)).toBe(true);
});

test.describe('touch device', () => {
  test.use({ hasTouch: true });

  test('a light touch stroke paints at full size', async ({ page }) => {
    // `setup` already revealed the on-screen toggles a touch would show.
    const canvas = await setup(page);
    const blank = await stableShot(canvas);

    await penStroke(page, canvas, 1);
    const firm = await shotAndUndo(page, canvas, blank);

    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    const at = (fx: number) => ({
      x: box.x + box.width * fx,
      y: box.y + box.height * 0.5,
      id: 1,
      force: 0.1,
    });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [at(0.1)] });
    for (const fx of [0.3, 0.5, 0.7, 0.9]) {
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [at(fx)] });
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await cdp.detach();
    expect((await shotAndUndo(page, canvas, blank)).equals(firm)).toBe(true);
  });
});
