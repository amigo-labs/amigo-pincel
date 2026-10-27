import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas } from './helpers';

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
