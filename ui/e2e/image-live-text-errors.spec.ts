import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu } from './helpers';
import {
  H,
  W,
  chooseText,
  clickAt,
  docRect,
  ink,
  setFontSize,
  textBox,
  typeAt,
} from './text-helpers';

// Selection clipping, refused layers and the font load of the Image-mode
// live text (docs/specs/image-live-text.md AC-17 – AC-19).

const FONT = '**/fonts/LiberationSans-Regular.ttf';

test.beforeEach(async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
});

test('with a selection the live text is drawn only inside it', async ({ page }) => {
  await page.getByRole('button', { name: /Rectangle Select/ }).first().click();
  // Canvas fractions of the document rect (10, 10) – (60, 100).
  await drag(page, imageCanvas(page), [10 / W, 10 / H], [60 / W, 100 / H]);
  await chooseText(page);
  await setFontSize(page, 24);
  await typeAt(page, [20, 30], 'HHHHHHHH');
  await expect(textBox(page)).toBeVisible();
  const box = await docRect(page, textBox(page));
  expect(box.x1).toBeGreaterThan(70);
  const drawn = await ink(page, 31, 80);
  expect(drawn?.x0).toBeGreaterThanOrEqual(20);
  expect(drawn?.x1).toBeLessThanOrEqual(60);
});

test('a locked layer or a group refuses the Text tool with the notice', async ({ page }) => {
  await chooseText(page);
  await page.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
  await clickAt(page, [40, 40]);
  await expect(textBox(page)).toHaveCount(0);
  await expect(page.getByText('Layer “Layer 1” is locked')).toBeVisible();
  await page.getByRole('button', { name: 'Unlock Layer 1', exact: true }).click();

  // A group as the active layer.
  await menu(page, 'Layer', 'New Group');
  await clickAt(page, [40, 40]);
  await expect(textBox(page)).toHaveCount(0);
  await expect(page.getByText(/^Layer “Group.*” is locked$/)).toBeVisible();
});

test('a font that does not load opens no box; the next press retries', async ({ page }) => {
  await page.route(FONT, (route) => route.fulfill({ status: 404, body: '' }));
  await chooseText(page);
  await clickAt(page, [40, 40]);
  await expect(page.getByText('The text font could not be loaded')).toBeVisible();
  await expect(textBox(page)).toHaveCount(0);

  await page.unroute(FONT);
  await clickAt(page, [40, 40]);
  await expect(textBox(page)).toBeVisible();
});

test('while the font loads a second press moves the box; another tool drops it', async ({
  page,
}) => {
  let release = (): void => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(FONT, async (route) => {
    await gate;
    await route.continue();
  });
  await chooseText(page);
  await clickAt(page, [20, 20]);
  await clickAt(page, [100, 60]);
  release();
  await expect(textBox(page)).toBeVisible();
  const box = await docRect(page, textBox(page));
  expect(box.x0).toBeCloseTo(100, -0.5);
  expect(box.y0).toBeCloseTo(60, -0.5);
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
});

test('another tool chosen while the font loads drops the pending box', async ({ page }) => {
  let release = (): void => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(FONT, async (route) => {
    await gate;
    await route.continue();
  });
  await chooseText(page);
  await clickAt(page, [20, 20]);
  await page.getByRole('button', { name: 'Brush', exact: true }).click();
  // Back to Text before the load ends: the dropped press stays dropped.
  await chooseText(page);
  const loaded = page.waitForResponse(FONT);
  release();
  await loaded;
  // The load has finished; no box appeared for the dropped press.
  await page.waitForTimeout(300);
  await expect(textBox(page)).toHaveCount(0);
  await clickAt(page, [60, 60]);
  await expect(textBox(page)).toBeVisible();
});
