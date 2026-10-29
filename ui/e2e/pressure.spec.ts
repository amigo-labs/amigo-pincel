import { expect, test } from '@playwright/test';
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
