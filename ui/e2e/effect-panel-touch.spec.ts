import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, pixelCanvas } from './helpers';

// Two-finger pinch keeps zooming while an effect panel is open, and starts
// no stroke (effect-panel AC-03). Same CDP touch gesture as pinch.spec.ts.

test.use({ hasTouch: true });

/** Spread two real (CDP) touch points apart over `target`'s centre. */
async function pinchOut(page: Page, target: Locator): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('pinch target is not visible');
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  const cdp = await page.context().newCDPSession(page);
  const points = (spread: number) => [
    { x: cx - spread, y: cy, id: 1 },
    { x: cx + spread, y: cy, id: 2 },
  ];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: points(10) });
  for (const spread of [20, 40, 60, 80]) {
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: points(spread) });
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await cdp.detach();
}

const MODES = {
  pixel: { canvas: pixelCanvas, readout: (page: Page) => page.getByText(/^\d+×$/), unit: '×' },
  image: {
    canvas: imageCanvas,
    readout: (page: Page) => page.getByRole('button', { name: 'Fit to window' }),
    unit: '%',
  },
} as const;

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a two-finger pinch zooms while an effect is open and paints nothing (AC-03)`, async ({
    page,
  }) => {
    const { canvas, readout, unit } = MODES[mode];
    await createDocument(page, mode, { width: 48, height: 32 });
    // The Pencil is armed: a stroke would show as an undo step.
    await page.keyboard.press('b');
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await expect(undo).toBeDisabled();
    const zoom = async () => Number((await readout(page).textContent())?.replace(unit, ''));

    await menu(page, 'Adjustments', 'Invert Colors');
    const panel = page.getByRole('dialog', { name: 'Invert Colors' });
    await expect(panel).toBeVisible();
    const before = await zoom();
    await pinchOut(page, canvas(page));
    await expect.poll(zoom).toBeGreaterThan(before);
    await expect(panel).toBeVisible();

    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(panel).toHaveCount(0);
    await expect(undo).toBeDisabled();
  });
}
