import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, penStroke } from './helpers';

// Pen pressure in Image mode (spec §17.4): a light stylus stroke paints a
// thinner line than a firm one; mouse strokes ignore pressure. Tilt turns
// a flat nib to the direction the stylus leans in.

test('image mode: a light pen stroke is thinner than a firm one', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const canvas = imageCanvas(page);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  await penStroke(page, canvas, 1);
  await expect(undo).toBeEnabled();
  const firm = await canvas.screenshot();
  await undo.click();

  await penStroke(page, canvas, 0.1);
  await expect(undo).toBeEnabled();
  const light = await canvas.screenshot();
  await undo.click();

  expect(light.equals(firm)).toBe(false);

  // With size dynamics off, pressure no longer changes the stroke.
  await page.getByRole('checkbox', { name: 'Size' }).uncheck();
  await penStroke(page, canvas, 0.1);
  const unscaled = await canvas.screenshot();
  expect(unscaled.equals(firm)).toBe(true);
});

test('image mode: tilt turns a flat nib only while the option is on', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const canvas = imageCanvas(page);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await page.locator('select').filter({ has: page.locator('option[value="flat"]') }).selectOption('flat');
  await page.getByRole('checkbox', { name: 'Size' }).uncheck();

  const stroke = async (tilt: { tiltX: number; tiltY: number }) => {
    await penStroke(page, canvas, 1, tilt);
    await expect(undo).toBeEnabled();
    const shot = await canvas.screenshot();
    await undo.click();
    return shot;
  };

  // Leaning along the stroke vs across it: a thin vs a broad line.
  const along = await stroke({ tiltX: 45, tiltY: 0 });
  const across = await stroke({ tiltX: 0, tiltY: 45 });
  expect(along.equals(across)).toBe(false);

  // Off: the nib keeps its fixed angle, whatever the pen does.
  await page.getByRole('checkbox', { name: 'Tilt turns the nib' }).uncheck();
  const fixedAlong = await stroke({ tiltX: 45, tiltY: 0 });
  const fixedAcross = await stroke({ tiltX: 0, tiltY: 45 });
  expect(fixedAlong.equals(fixedAcross)).toBe(true);
});

/**
 * The same path as `penStroke`, drawn with one real (CDP) finger that
 * reports touch force `force`.
 */
async function touchStroke(page: Page, target: Locator, force: number): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  const cdp = await page.context().newCDPSession(page);
  const at = (fx: number) => [
    { x: box.x + box.width * fx, y: box.y + box.height * 0.5, id: 1, force },
  ];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: at(0.1) });
  for (const fx of [0.3, 0.5, 0.7, 0.9]) {
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: at(fx) });
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await cdp.detach();
}

test.describe('touch device', () => {
  test.use({ hasTouch: true });

  test('image mode: mouse and touch strokes paint at full pressure', async ({ page }) => {
    await createDocument(page, 'image', { width: 64, height: 48 });
    const canvas = imageCanvas(page);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    // Both dynamics on: any pressure below 1 would show in size and alpha.
    await expect(page.getByRole('checkbox', { name: 'Size', exact: true })).toBeChecked();
    await page.getByRole('checkbox', { name: 'Opacity', exact: true }).check();

    const shot = async (draw: () => Promise<void>) => {
      await draw();
      await expect(undo).toBeEnabled();
      const png = await canvas.screenshot();
      await undo.click();
      await expect(undo).toBeDisabled();
      return png;
    };
    // The first pen or touch press reveals the on-screen modifier toggles
    // over the canvas; draw once before taking reference shots.
    await shot(() => penStroke(page, canvas, 1));

    const firm = await shot(() => penStroke(page, canvas, 1));
    const half = await shot(() => penStroke(page, canvas, 0.5));
    const mouse = await shot(() => penStroke(page, canvas, 0.2, undefined, 'mouse'));
    const finger = await shot(() => touchStroke(page, canvas, 0.2));

    expect(half.equals(firm)).toBe(false);
    expect(mouse.equals(firm)).toBe(true);
    expect(finger.equals(firm)).toBe(true);
  });
});

test('image mode: brush tools show the pressure and tilt options with their defaults', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const options = page.getByRole('toolbar', { name: 'Tool options' });
  const shape = options.locator('select').filter({ has: page.locator('option[value="flat"]') });
  const tilt = options.getByRole('checkbox', { name: 'Tilt turns the nib' });

  // Brush, Eraser, Recolor, Smudge.
  for (const key of ['b', 'e', 'r', 's']) {
    await page.keyboard.press(key);
    await expect(options.getByText('Pen pressure')).toBeVisible();
    await expect(options.getByRole('checkbox', { name: 'Size', exact: true })).toBeChecked();
    await expect(options.getByRole('checkbox', { name: 'Opacity', exact: true })).not.toBeChecked();
    // "Tilt turns the nib" only with a flat tip.
    await expect(tilt).toHaveCount(0);
    await shape.selectOption('flat');
    await expect(tilt).toBeChecked();
    await shape.selectOption('hard_round');
    await expect(tilt).toHaveCount(0);
  }
});
