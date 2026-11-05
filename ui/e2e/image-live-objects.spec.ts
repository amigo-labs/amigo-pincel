import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas } from './helpers';

// Image-mode live objects in the editor (docs/specs/image-live-objects-ui.md):
// a Line, Shapes or Gradient drag leaves the object live with handles until
// it is committed or cancelled.

/** The handles drawn over the live object. */
function handles(page: Page): Locator {
  return page.locator('[data-live-handle]');
}

function undoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Undo', exact: true });
}

function redoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Redo', exact: true });
}

/** Page coordinates of a fractional canvas position. */
async function at(page: Page, [fx, fy]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + box.width * fx, box.y + box.height * fy];
}

/**
 * The color shown at a fractional canvas position, read from a screenshot
 * of the canvas (the app never reads canvas pixels; the test reads the
 * screen). Sample away from handles: the overlay is in the screenshot.
 */
async function shown(page: Page, [fx, fy]: [number, number]): Promise<string> {
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const x = Math.min(bitmap.width - 1, Math.floor(bitmap.width * fx));
      const y = Math.min(bitmap.height - 1, Math.floor(bitmap.height * fy));
      const [r = 0, g = 0, b = 0] = ctx.getImageData(x, y, 1, 1).data;
      return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
    },
    { b64: png.toString('base64'), fx, fy },
  );
}

/** Sets the foreground and background swatches. */
async function setColors(page: Page, fg: string, bg: string): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill(fg);
  await page.getByLabel('Background color', { exact: true }).fill(bg);
}

/** Picks the Rectangle shape, filled with the background color. */
async function filledRectangle(page: Page): Promise<void> {
  await page.keyboard.press('u');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name: 'Rectangle', exact: true })
    .click();
  await page.getByRole('combobox', { name: 'Mode', exact: true }).selectOption('fill');
}

test('a shape, line or gradient drag leaves the object live with handles', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await setColors(page, '#000000', '#ff0000');
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  await expect(handles(page)).toHaveCount(8);
  await expect(undoButton(page)).toBeEnabled();
  expect(await shown(page, [0.4, 0.45])).toBe('#ff0000');

  // Enter commits: the pixels stay, the handles go, one step remains.
  await page.keyboard.press('Enter');
  await expect(handles(page)).toHaveCount(0);
  expect(await shown(page, [0.4, 0.45])).toBe('#ff0000');
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
  expect(await shown(page, [0.4, 0.45])).not.toBe('#ff0000');

  // A line has two endpoints and a move grip, a gradient two endpoints.
  await page.keyboard.press('l');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.7, 0.4]);
  await expect(handles(page)).toHaveCount(3);
  await page.keyboard.press('Enter');
  await page.keyboard.press('n');
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.7, 0.5]);
  await expect(handles(page)).toHaveCount(2);
});

test('a refused or degenerate drag leaves nothing live', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await filledRectangle(page);
  // A press without a drag is a zero-size box: nothing is live, no step.
  const [x, y] = await at(page, [0.5, 0.5]);
  await page.mouse.click(x, y);
  await expect(handles(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();

  // A locked layer refuses the drag with the notice.
  await page.getByRole('button', { name: /^Lock/ }).first().click();
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  await expect(handles(page)).toHaveCount(0);
  await expect(page.getByText(/is locked/)).toBeVisible();
});

test('Escape cancels the live object and the document is clean again', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await setColors(page, '#000000', '#ff0000');
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  await expect(page).toHaveTitle(/^● /);
  await page.keyboard.press('Escape');
  await expect(handles(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
  await expect(redoButton(page)).toBeDisabled();
  expect(await shown(page, [0.4, 0.45])).not.toBe('#ff0000');
  await expect(page).not.toHaveTitle(/^● /);

  // With a selection, Escape cancels the object and keeps the selection.
  await page.keyboard.press('Control+a');
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  await page.keyboard.press('Escape');
  await expect(handles(page)).toHaveCount(0);
  await undoButton(page).click(); // the Select All step, no Deselect step
  await expect(undoButton(page)).toBeDisabled();
});

test('Undo and Redo walk the live object with its handles', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  await expect(handles(page)).toHaveCount(8);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(0);
  await redoButton(page).click();
  await expect(handles(page)).toHaveCount(8);
});

test('another tool, a new drag or a press beside the object commits it', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await setColors(page, '#000000', '#ff0000');
  await filledRectangle(page);

  // Another tool commits: one step for the object.
  await drag(page, imageCanvas(page), [0.1, 0.1], [0.3, 0.3]);
  await page.keyboard.press('b');
  await expect(handles(page)).toHaveCount(0);
  expect(await shown(page, [0.2, 0.25])).toBe('#ff0000');
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();

  // A drag beside a live object commits it and draws a new live one.
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.1, 0.1], [0.3, 0.3]);
  await drag(page, imageCanvas(page), [0.6, 0.6], [0.9, 0.9]);
  await expect(handles(page)).toHaveCount(8);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(0);
  expect(await shown(page, [0.2, 0.25])).toBe('#ff0000');
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();

  // A press beside it without a drag only commits — on the canvas or on
  // the stage beside it.
  await drag(page, imageCanvas(page), [0.1, 0.1], [0.3, 0.3]);
  const [x, y] = await at(page, [0.8, 0.8]);
  await page.mouse.click(x, y);
  await expect(handles(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeEnabled();
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
  await drag(page, imageCanvas(page), [0.1, 0.1], [0.3, 0.3]);
  const [sx, sy] = await at(page, [1.1, 0.5]);
  await page.mouse.click(sx, sy);
  await expect(handles(page)).toHaveCount(0);
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
});
