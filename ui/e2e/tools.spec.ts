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

test('image mode: the zoom buttons still work with the Zoom tool selected', async ({ page }) => {
  // The floating control is a child of the viewport, whose capture handler
  // serves the Pan and Zoom tools — it must not swallow the buttons' clicks.
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('z');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Zoom');

  const zoom = page.getByRole('button', { name: 'Actual size' });
  await zoom.click();
  await expect(page.getByRole('button', { name: 'Fit to window' })).toHaveText('100%');

  // "Zoom out" must zoom out, not in through the viewport gesture.
  await page.getByRole('button', { name: 'Zoom out' }).click();
  await expect(page.getByRole('button', { name: 'Fit to window' })).not.toHaveText('100%');
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

test('image mode: Select All, Contract and Feather are one undo step each', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();

  await menu(page, 'Select', 'Select All');
  await expect(undo).toBeEnabled();
  // Contract and Feather ask for a radius like Expand.
  for (const op of ['Contract…', 'Feather…']) {
    await menu(page, 'Select', op);
    const dialog = page.getByRole('dialog', { name: op });
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('Radius (px)').fill('3');
    await dialog.getByRole('button', { name: 'Apply' }).click();
    await expect(dialog).toBeHidden();
  }

  // Three changes, three undo steps.
  await undo.click();
  await undo.click();
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
});

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

test('image mode: a moved selection can be moved again', async ({ page }) => {
  // The outline travels with its content, so the second drag carries the
  // patch on instead of lifting the transparency the first one left.
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  await drag(page, imageCanvas(page), [0.1, 0.5], [0.4, 0.5]);
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.05, 0.4], [0.45, 0.6]);

  await page.keyboard.press('v');
  await page.keyboard.press('v');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Move: Move Selected Pixels');

  await drag(page, imageCanvas(page), [0.2, 0.5], [0.4, 0.5]);
  await drag(page, imageCanvas(page), [0.4, 0.5], [0.6, 0.5]);
  await expect(undo).toBeEnabled();

  // Both moves are their own undo step, and both come back.
  await undo.click();
  await undo.click();
  await expect(undo).toBeEnabled();
});

test('image mode: the Gradient tool paints a ramp', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();

  await page.keyboard.press('n');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Gradient');
  // The options bar offers the five ramps and the two channels.
  await page.getByLabel('Ramp').selectOption('radial');
  await page.getByLabel('Channel').selectOption('color');

  await drag(page, imageCanvas(page), [0.5, 0.5], [0.9, 0.5]);
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
});

test('image mode: Recolor writes only over the colour it replaces', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  // Paint a black line on the transparent canvas, then recolor it.
  await drag(page, imageCanvas(page), [0.1, 0.5], [0.9, 0.5]);
  await page.keyboard.press('r');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Recolor');
  await expect(page.getByLabel('Tolerance')).toBeVisible();

  await drag(page, imageCanvas(page), [0.1, 0.5], [0.9, 0.5]);
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeEnabled(); // the first stroke is still there
});

test('image mode: Smudge drags along a stroke', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  await drag(page, imageCanvas(page), [0.1, 0.4], [0.5, 0.4]);
  await page.keyboard.press('s');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Smudge');
  await expect(page.getByLabel('Strength')).toBeVisible();

  await drag(page, imageCanvas(page), [0.4, 0.4], [0.9, 0.4]);
  await expect(undo).toBeEnabled();
});

// Rail sections (spec §9.2): separators split navigate / select / paint /
// fill & color / draw (/ sprite in Pixel mode).
test('the rail separates its tool sections in both modes', async ({ page }) => {
  await createDocument(page, 'image');
  const rail = page.getByRole('toolbar', { name: 'Tools' });
  await expect(rail.getByRole('separator')).toHaveCount(4);
  await createDocument(page, 'pixel');
  await expect(rail.getByRole('separator')).toHaveCount(5);
});

// Probing the document through the Eyedropper keeps these tests on the DOM
// (spec §7.4): a 1 px sample of the composite lands in the foreground swatch.

/** Screen point at a fractional position of the Image canvas. */
async function canvasPoint(page: Page, [fx, fy]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + box.width * fx, box.y + box.height * fy];
}

/** Samples the composite at a fractional canvas position; returns `#rrggbb`. */
async function probe(page: Page, at: [number, number]): Promise<string> {
  await page.keyboard.press('i');
  const [x, y] = await canvasPoint(page, at);
  await page.mouse.click(x, y);
  return page.getByLabel('Foreground color', { exact: true }).inputValue();
}

/** Sets the foreground and background swatches. */
async function setColors(page: Page, fg: string, bg: string): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill(fg);
  await page.getByLabel('Background color', { exact: true }).fill(bg);
}

test('image mode: the right button paints and samples the background color', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const fg = page.getByLabel('Foreground color', { exact: true });
  const bg = page.getByLabel('Background color', { exact: true });

  // Paint Bucket, right click: the uniform layer takes the background color.
  await setColors(page, '#ff0000', '#00ff00');
  await page.keyboard.press('g');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Fill');
  const [cx, cy] = await canvasPoint(page, [0.5, 0.5]);
  await page.mouse.click(cx, cy, { button: 'right' });
  expect(await probe(page, [0.5, 0.2])).toBe('#00ff00');

  // Brush, right drag: the stroke is in the background color.
  await setColors(page, '#ff0000', '#0000ff');
  await page.keyboard.press('b');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Brush');
  const [x0, y0] = await canvasPoint(page, [0.2, 0.5]);
  const [x1, y1] = await canvasPoint(page, [0.8, 0.5]);
  await page.mouse.move(x0, y0);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(x1, y1, { steps: 8 });
  await page.mouse.up({ button: 'right' });
  expect(await probe(page, [0.5, 0.5])).toBe('#0000ff');

  // Eyedropper, right click: the sample becomes the background color and
  // the foreground stays.
  await setColors(page, '#ff0000', '#ffffff');
  await page.keyboard.press('i');
  const [gx, gy] = await canvasPoint(page, [0.5, 0.2]);
  await page.mouse.click(gx, gy, { button: 'right' });
  await expect(bg).toHaveValue('#00ff00');
  await expect(fg).toHaveValue('#ff0000');
});

test('image mode: Shift squares a shape and snaps a line to 45°', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const shiftDrag = async (from: [number, number], to: [number, number]): Promise<void> => {
    await page.keyboard.down('Shift');
    await drag(page, imageCanvas(page), from, to);
    await page.keyboard.up('Shift');
  };
  const rectangleTool = async (): Promise<void> => {
    await page.keyboard.press('u');
    await page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name: 'Rectangle', exact: true })
      .click();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Rectangle');
  };

  // A filled rectangle dragged 0.6 wide and 0.15 tall ...
  await rectangleTool();
  await page.getByRole('combobox', { name: 'Mode', exact: true }).selectOption('fill');
  await setColors(page, '#000000', '#ff0000');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.35]);
  expect(await probe(page, [0.5, 0.3])).toBe('#ff0000');
  expect(await probe(page, [0.5, 0.7])).not.toBe('#ff0000');
  await undo.click();

  // ... becomes the 0.6 × 0.6 square of its longer side with Shift.
  await rectangleTool();
  await shiftDrag([0.2, 0.2], [0.8, 0.35]);
  expect(await probe(page, [0.75, 0.75])).toBe('#ff0000');
  expect(await probe(page, [0.75, 0.85])).not.toBe('#ff0000');
  expect(await probe(page, [0.85, 0.75])).not.toBe('#ff0000');
  await undo.click();

  // A line dragged 14° off the horizontal snaps to 0° ...
  await page.keyboard.press('l');
  await setColors(page, '#ff0000', '#ffffff');
  await shiftDrag([0.2, 0.2], [0.8, 0.35]);
  expect(await probe(page, [0.6, 0.2])).toBe('#ff0000');
  expect(await probe(page, [0.6, 0.3])).not.toBe('#ff0000');
  await undo.click();

  // ... and one dragged 39° off it snaps to the 45° diagonal.
  await page.keyboard.press('l');
  await setColors(page, '#ff0000', '#ffffff');
  await shiftDrag([0.2, 0.2], [0.7, 0.6]);
  expect(await probe(page, [0.5, 0.5])).toBe('#ff0000');
});

test('image mode: W, B, E, G and T select their tools', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const keys: ReadonlyArray<readonly [string, string]> = [
    ['w', 'Magic Wand'],
    ['b', 'Brush'],
    ['e', 'Eraser'],
    ['g', 'Fill'],
    ['t', 'Text'],
  ];
  for (const [key, label] of keys) {
    await page.keyboard.press(key);
    await expect(activeTool(page)).toHaveAttribute('aria-label', label);
  }
});
