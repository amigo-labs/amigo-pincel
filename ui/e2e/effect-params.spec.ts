import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, penStroke, switchLanguage } from './helpers';

// effect-catalog-v2 AC-09 – AC-15: the angle, point, random-seed and
// linked-pair parameter kinds of the effect dialog (Image mode, which has
// every entry that uses them).

async function open(page: Page, sub: string, entry: string): Promise<Locator> {
  await menu(page, 'Effects', sub, entry);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Image document with a stroke, so effects have something to change. */
async function scene(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 48, height: 32 });
  await drag(page, imageCanvas(page), [0.1, 0.3], [0.9, 0.7]);
}

/** Presses the primary mouse button at fractional position `(fx, fy)` of `target`. */
async function pressAt(page: Page, target: Locator, fx: number, fy: number): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('target is not visible');
  await page.mouse.move(box.x + box.width * fx, box.y + box.height * fy);
  await page.mouse.down();
}

test('angle: the dial and the degree field set the same angle (AC-09)', async ({ page }) => {
  await scene(page);
  const dialog = await open(page, 'Blurs', 'Motion Blur…');
  const dial = dialog.getByRole('slider', { name: 'Angle dial' });
  const field = dialog.getByLabel('Angle in degrees');
  await expect(dial).toHaveAttribute('aria-valuenow', '0');
  await expect(field).toHaveValue('0');

  // Pointing below the centre is 90° (the image's y axis runs down).
  await pressAt(page, dial, 0.5, 0.95);
  await page.mouse.up();
  await expect(dial).toHaveAttribute('aria-valuenow', '90');
  await expect(field).toHaveValue('90');
  // Dragging follows the pointer: left of the centre is 180°, above 270°.
  await pressAt(page, dial, 0.05, 0.5);
  await expect(dial).toHaveAttribute('aria-valuenow', '180');
  const box = await dial.boundingBox();
  if (!box) throw new Error('dial is not visible');
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.05, { steps: 3 });
  await page.mouse.up();
  await expect(dial).toHaveAttribute('aria-valuenow', '270');
  // A diagonal rounds to whole degrees.
  await pressAt(page, dial, 0.8, 0.8);
  await page.mouse.up();
  await expect(dial).toHaveAttribute('aria-valuenow', '45');

  // Typing moves the handle; values outside 0–360 wrap around.
  const handleX = await dial.getAttribute('data-handle-x');
  await field.fill('120');
  await expect(dial).toHaveAttribute('aria-valuenow', '120');
  expect(await dial.getAttribute('data-handle-x')).not.toBe(handleX);
  await field.fill('370');
  await field.press('Tab');
  await expect(field).toHaveValue('10');
  await expect(dial).toHaveAttribute('aria-valuenow', '10');
  await field.fill('-10');
  await field.press('Tab');
  await expect(field).toHaveValue('350');
  await expect(dial).toHaveAttribute('aria-valuenow', '350');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  for (const [sub, entry] of [
    ['Stylize', 'Emboss…'],
    ['Stylize', 'Relief…'],
  ] as const) {
    const d = await open(page, sub, entry);
    await expect(d.getByRole('slider', { name: 'Angle dial' })).toHaveAttribute(
      'aria-valuenow',
      '45',
    );
    await d.getByRole('button', { name: 'Cancel' }).click();
  }
});

test('point: X / Y in percent, default centre, the effect follows it (AC-10)', async ({
  page,
}) => {
  await scene(page);
  const dialog = await open(page, 'Blurs', 'Radial Blur…');
  const x = dialog.getByLabel('Center X in percent');
  const y = dialog.getByLabel('Center Y in percent');
  await expect(x).toHaveValue('50');
  await expect(y).toHaveValue('50');
  await dialog.getByLabel('Amount value').fill('60');
  const canvas = imageCanvas(page);
  await page.waitForTimeout(300);
  const centred = await canvas.screenshot({ mask: [dialog] });
  await x.fill('5');
  await y.fill('95');
  await expect.poll(async () => (await canvas.screenshot({ mask: [dialog] })).equals(centred)).toBe(false);
  await x.fill('50');
  await y.fill('50');
  await expect.poll(async () => (await canvas.screenshot({ mask: [dialog] })).equals(centred)).toBe(true);
});

test('point: Pick on canvas sets the point without a tool stroke (AC-11)', async ({ page }) => {
  await createDocument(page, 'image', { width: 40, height: 40 });
  await page.keyboard.press('b');
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();
  const canvas = imageCanvas(page);
  const dialog = await open(page, 'Blurs', 'Radial Blur…');
  const pick = dialog.getByRole('button', { name: 'Pick on canvas' });
  const x = dialog.getByLabel('Center X in percent');
  const y = dialog.getByLabel('Center Y in percent');

  // A press sets the point, a drag moves it, the release ends picking.
  await pick.click();
  await expect(pick).toHaveAttribute('aria-pressed', 'true');
  await pressAt(page, canvas, 0.25, 0.75);
  await expect(x).toHaveValue('25');
  await expect(y).toHaveValue('75');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.4, { steps: 4 });
  await expect(x).toHaveValue('60');
  await expect(y).toHaveValue('40');
  await page.mouse.up();
  await expect(pick).toHaveAttribute('aria-pressed', 'false');
  // No stroke happened under the Pencil.
  await expect(undo).toBeDisabled();

  // Beside the image the point clamps to 0–100 %.
  await pick.click();
  const viewport = await canvas.evaluate((el) => {
    const r = el.parentElement?.parentElement?.getBoundingClientRect();
    return r ? { x: r.x, y: r.y } : null;
  });
  if (!viewport) throw new Error('viewport is not visible');
  await page.mouse.click(viewport.x + 3, viewport.y + 3);
  await expect(x).toHaveValue('0');
  await expect(y).toHaveValue('0');

  // Escape and a second press of the button end picking, dialog stays open.
  await pick.click();
  await page.keyboard.press('Escape');
  await expect(pick).toHaveAttribute('aria-pressed', 'false');
  await expect(dialog).toBeVisible();
  // Panning still pans while picking is armed (Space-drag, middle button).
  await pick.click();
  const before = await canvas.boundingBox();
  await page.keyboard.down('Space');
  await drag(page, canvas, [0.5, 0.5], [0.8, 0.8]);
  await page.keyboard.up('Space');
  const middle = await canvas.boundingBox();
  expect(middle?.x).not.toBe(before?.x);
  await page.mouse.move((middle?.x ?? 0) + 10, (middle?.y ?? 0) + 10);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move((middle?.x ?? 0) + 40, (middle?.y ?? 0) + 30, { steps: 3 });
  await page.mouse.up({ button: 'middle' });
  expect((await canvas.boundingBox())?.x).not.toBe(middle?.x);
  await expect(pick).toHaveAttribute('aria-pressed', 'true');
  await expect(x).toHaveValue('0');

  // Enter while picking applies the current values.
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();

  // A second press of the button ends picking; the point stays, and while
  // the effect is open the canvas takes no tool input (effect-panel AC-03).
  const again = await open(page, 'Blurs', 'Radial Blur…');
  const pickAgain = again.getByRole('button', { name: 'Pick on canvas' });
  await pickAgain.click();
  await pickAgain.click();
  await expect(pickAgain).toHaveAttribute('aria-pressed', 'false');
  await drag(page, canvas, [0.2, 0.2], [0.4, 0.4]);
  await expect(again.getByLabel('Center X in percent')).toHaveValue('50');
  await expect(undo).toBeDisabled();
  await again.getByRole('button', { name: 'Cancel' }).click();
  // Cancel while armed: later presses reach the tool again.
  const third = await open(page, 'Blurs', 'Radial Blur…');
  await third.getByRole('button', { name: 'Pick on canvas' }).click();
  await third.getByRole('button', { name: 'Cancel' }).click();
  await expect(third).toHaveCount(0);
  await drag(page, canvas, [0.6, 0.2], [0.8, 0.4]);
  await expect(undo).toBeEnabled();
});

test('point: a picked point updates the preview (AC-11)', async ({ page }) => {
  await scene(page);
  const dialog = await open(page, 'Blurs', 'Radial Blur…');
  await dialog.getByLabel('Amount value').fill('60');
  const canvas = imageCanvas(page);
  await page.waitForTimeout(300);
  const centred = await canvas.screenshot({ mask: [dialog] });
  await dialog.getByRole('button', { name: 'Pick on canvas' }).click();
  await pressAt(page, canvas, 0.05, 0.95);
  await page.mouse.up();
  await expect(dialog.getByLabel('Center X in percent')).toHaveValue('5');
  await expect.poll(async () => (await canvas.screenshot({ mask: [dialog] })).equals(centred)).toBe(false);
});

test.describe('touch device', () => {
  test.use({ hasTouch: true });

  test('pen and single touch pick the point; two fingers still pinch (AC-11)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 40, height: 40 });
    await page.keyboard.press('b');
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const canvas = imageCanvas(page);
    const dialog = await open(page, 'Blurs', 'Radial Blur…');
    const pick = dialog.getByRole('button', { name: 'Pick on canvas' });
    const x = dialog.getByLabel('Center X in percent');
    const y = dialog.getByLabel('Center Y in percent');

    // A pen stroke along the middle row ends at 90 % / 50 %.
    await pick.click();
    await penStroke(page, canvas, 0.5);
    await expect(x).toHaveValue('90');
    await expect(y).toHaveValue('50');
    await expect(pick).toHaveAttribute('aria-pressed', 'false');

    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    const cdp = await page.context().newCDPSession(page);
    // One finger picks.
    await pick.click();
    const at = (fx: number, fy: number, id: number) => ({
      x: box.x + box.width * fx,
      y: box.y + box.height * fy,
      id,
    });
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [at(0.3, 0.2, 1)] });
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await expect(x).toHaveValue('30');
    await expect(y).toHaveValue('20');
    await expect(pick).toHaveAttribute('aria-pressed', 'false');

    // Two fingers pinch-zoom while picking is armed.
    await pick.click();
    const zoom = page.getByRole('button', { name: 'Fit to window' });
    const before = await zoom.textContent();
    await cdp.send('Input.dispatchTouchEvent', {
      type: 'touchStart',
      touchPoints: [at(0.45, 0.5, 1), at(0.55, 0.5, 2)],
    });
    for (const d of [0.1, 0.2, 0.3, 0.4]) {
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchMove',
        touchPoints: [at(0.45 - d, 0.5, 1), at(0.55 + d, 0.5, 2)],
      });
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await expect.poll(() => zoom.textContent()).not.toBe(before);
    await expect(undo).toBeDisabled();
  });
});

test('seed: Reseed draws a new seed, the same seed gives the same pixels (AC-12)', async ({
  page,
}) => {
  await scene(page);
  const dialog = await open(page, 'Noise', 'Add Noise…');
  const seed = dialog.getByLabel('Seed value');
  await expect(seed).toHaveValue('1');
  const canvas = imageCanvas(page);
  await page.waitForTimeout(300);
  const first = await canvas.screenshot({ mask: [dialog] });
  for (let i = 0; i < 5; i++) {
    const before = await seed.inputValue();
    await dialog.getByRole('button', { name: 'Reseed' }).click();
    const after = Number(await seed.inputValue());
    expect(String(after)).not.toBe(before);
    expect(Number.isInteger(after) && after >= 0 && after <= 9999).toBe(true);
  }
  await expect.poll(async () => (await canvas.screenshot({ mask: [dialog] })).equals(first)).toBe(false);
  await seed.fill('1');
  await expect.poll(async () => (await canvas.screenshot({ mask: [dialog] })).equals(first)).toBe(true);
});

test('linked pair: Square Blur width and height move together while linked (AC-13)', async ({
  page,
}) => {
  await scene(page);
  const dialog = await open(page, 'Blurs', 'Square Blur…');
  const w = dialog.getByLabel('Width value');
  const h = dialog.getByLabel('Height value');
  const link = dialog.getByLabel('Link Width and Height');
  await expect(w).toHaveValue('3');
  await expect(h).toHaveValue('3');
  await expect(link).toBeChecked();
  await w.fill('7');
  await expect(h).toHaveValue('7');
  await h.fill('11');
  await expect(w).toHaveValue('11');
  await link.uncheck();
  await w.fill('5');
  await h.fill('9');
  await expect(w).toHaveValue('5');
  await expect(h).toHaveValue('9');
  // Linking again takes the value edited last.
  await link.check();
  await expect(w).toHaveValue('9');
  await expect(h).toHaveValue('9');
});

test('Reset returns angle, point, seed and link to their defaults (AC-14)', async ({ page }) => {
  await scene(page);
  let dialog = await open(page, 'Blurs', 'Motion Blur…');
  await dialog.getByLabel('Angle in degrees').fill('200');
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByRole('slider', { name: 'Angle dial' })).toHaveAttribute(
    'aria-valuenow',
    '0',
  );
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Blurs', 'Radial Blur…');
  await dialog.getByLabel('Center X in percent').fill('10');
  await dialog.getByLabel('Center Y in percent').fill('20');
  await dialog.getByRole('button', { name: 'Pick on canvas' }).click();
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByLabel('Center X in percent')).toHaveValue('50');
  await expect(dialog.getByLabel('Center Y in percent')).toHaveValue('50');
  await expect(dialog.getByRole('button', { name: 'Pick on canvas' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Noise', 'Add Noise…');
  await dialog.getByRole('button', { name: 'Reseed' }).click();
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByLabel('Seed value')).toHaveValue('1');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Blurs', 'Square Blur…');
  await dialog.getByLabel('Link Width and Height').uncheck();
  await dialog.getByLabel('Width value').fill('15');
  await dialog.getByRole('button', { name: 'Reset' }).click();
  await expect(dialog.getByLabel('Link Width and Height')).toBeChecked();
  await expect(dialog.getByLabel('Width value')).toHaveValue('3');
  await expect(dialog.getByLabel('Height value')).toHaveValue('3');
});

test('the new controls work from the keyboard (AC-15)', async ({ page }) => {
  await scene(page);
  let dialog = await open(page, 'Blurs', 'Motion Blur…');
  const dial = dialog.getByRole('slider', { name: 'Angle dial' });
  await dial.focus();
  await page.keyboard.press('ArrowRight');
  await expect(dial).toHaveAttribute('aria-valuenow', '1');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await expect(dial).toHaveAttribute('aria-valuenow', '359');
  await page.keyboard.press('PageUp');
  await expect(dial).toHaveAttribute('aria-valuenow', '14');
  // Tab moves from the dial to its degree field; arrows step it.
  await page.keyboard.press('Tab');
  await expect(dialog.getByLabel('Angle in degrees')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(dialog.getByLabel('Angle in degrees')).toHaveValue('15');
  await expect(dial).toHaveAttribute('aria-valuenow', '15');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Blurs', 'Radial Blur…');
  const x = dialog.getByLabel('Center X in percent');
  await x.focus();
  await page.keyboard.press('ArrowUp');
  await expect(x).toHaveValue('50.1');
  await page.keyboard.press('Tab');
  await expect(dialog.getByLabel('Center Y in percent')).toBeFocused();
  await page.keyboard.press('Tab');
  const pick = dialog.getByRole('button', { name: 'Pick on canvas' });
  await expect(pick).toBeFocused();
  await page.keyboard.press('Space');
  await expect(pick).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Space');
  await expect(pick).toHaveAttribute('aria-pressed', 'false');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Noise', 'Add Noise…');
  const reseed = dialog.getByRole('button', { name: 'Reseed' });
  await reseed.focus();
  await page.keyboard.press('Space');
  await expect(dialog.getByLabel('Seed value')).not.toHaveValue('1');
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await open(page, 'Blurs', 'Square Blur…');
  const link = dialog.getByLabel('Link Width and Height');
  await link.focus();
  await page.keyboard.press('Space');
  await expect(link).not.toBeChecked();
});

test.describe('German UI', () => {
  test('the new controls are named in German (AC-15)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await switchLanguage(page, 'Deutsch');
    await menu(page, 'Effekte', 'Weichzeichnen', 'Radialer Weichzeichner…');
    let dialog = page.getByRole('dialog');
    await expect(dialog.getByLabel('Mitte X in Prozent')).toHaveValue('50');
    await expect(dialog.getByRole('button', { name: 'Auf der Leinwand wählen' })).toBeVisible();
    await dialog.getByRole('button', { name: 'Abbrechen' }).click();
    await menu(page, 'Effekte', 'Weichzeichnen', 'Bewegungsunschärfe…');
    dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('slider', { name: 'Winkel-Drehregler' })).toBeVisible();
    await expect(dialog.getByLabel('Winkel in Grad')).toBeVisible();
    await dialog.getByRole('button', { name: 'Abbrechen' }).click();
    await menu(page, 'Effekte', 'Rauschen', 'Rauschen hinzufügen…');
    dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('button', { name: 'Neu würfeln' })).toBeVisible();
    await dialog.getByRole('button', { name: 'Abbrechen' }).click();
    await menu(page, 'Effekte', 'Weichzeichnen', 'Quadratisches Weichzeichnen…');
    await expect(page.getByRole('dialog').getByLabel('Breite und Höhe verknüpfen')).toBeChecked();
  });
});
