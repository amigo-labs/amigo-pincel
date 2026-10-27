import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu } from './helpers';

/** The tool-rail button that is currently pressed. */
function activeTool(page: Page) {
  return page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
}

// Image-mode tool rail (spec §9.2 / §9.4): Line has its own key, the
// shapes are variants of one button, and the viewport tools are reachable
// without a keyboard.

test('image mode: L selects Line and the shapes are its own group', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });

  await page.keyboard.press('l');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Line');

  await page.keyboard.press('u');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Rectangle');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name: 'Ellipse' })
    .click();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Ellipse');

  // Repeated presses cycle the group.
  await page.keyboard.press('u');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Polygon');
});

test('image mode: the Line tool draws and undoes', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();

  await page.keyboard.press('l');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
});

test('image mode: Pan and Zoom are rail tools', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('h');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pan');
  await page.keyboard.press('z');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Zoom');

  // A click with the Zoom tool steps the view in; the control shows it.
  const zoomBefore = await page.getByRole('button', { name: 'Fit' }).textContent();
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(page.getByRole('button', { name: 'Fit' })).not.toHaveText(zoomBefore ?? '');
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Select ▸ Expand asks for a radius`, async ({ page }) => {
    await createDocument(page, mode, { width: 64, height: 64 });
    await page.keyboard.press('Control+a');
    await menu(page, 'Select', 'Expand…');
    const dialog = page.getByRole('dialog', { name: 'Expand…' });
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('Radius (px)').fill('4');
    await dialog.getByRole('button', { name: 'Apply' }).click();
    await expect(dialog).toBeHidden();
  });
}

test('image mode: the move tools drag pixels, the outline, or the layer', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  // Paint something, then select a corner of it.
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.1, 0.3], [0.5, 0.7]);

  await page.keyboard.press('v');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Move: Move Layer');
  await page.keyboard.press('v');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Move: Move Selected Pixels');

  const steps = await undo.isEnabled();
  expect(steps).toBe(true);
  await drag(page, imageCanvas(page), [0.3, 0.5], [0.6, 0.5]);
  // The move is one undo step of its own.
  await undo.click();

  await page.keyboard.press('v');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Move: Move Selection');
  await drag(page, imageCanvas(page), [0.3, 0.5], [0.6, 0.5]);
  await expect(undo).toBeEnabled();
});
