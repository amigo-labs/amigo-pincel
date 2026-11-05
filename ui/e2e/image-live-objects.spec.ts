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

/** The page-space centre of handle `id`. */
async function handleAt(page: Page, id: string): Promise<[number, number]> {
  const box = await page.locator(`[data-live-handle="${id}"]`).boundingBox();
  if (!box) throw new Error(`no handle ${id}`);
  return [box.x + box.width / 2, box.y + box.height / 2];
}

/** A mouse drag between two page points, in steps. */
async function mouseDrag(
  page: Page,
  from: [number, number],
  to: [number, number],
  shift = false,
): Promise<void> {
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  if (shift) await page.keyboard.down('Shift');
  await page.mouse.move(to[0], to[1], { steps: 6 });
  await page.mouse.up();
  if (shift) await page.keyboard.up('Shift');
}

/** A live red rectangle over the canvas fractions 0.2 to 0.4. */
async function liveRedBox(page: Page): Promise<void> {
  await setColors(page, '#000000', '#ff0000');
  await filledRectangle(page);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.4, 0.4]);
  await expect(handles(page)).toHaveCount(8);
}

test('handles resize the live box, one undo step per drag', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);

  // A corner follows the pointer during the drag and after it.
  const se = await handleAt(page, 'se');
  const target = await at(page, [0.7, 0.7]);
  await page.mouse.move(se[0], se[1]);
  await page.mouse.down();
  await page.mouse.move(target[0], target[1], { steps: 6 });
  await expect.poll(() => shown(page, [0.6, 0.6])).toBe('#ff0000');
  await page.mouse.up();
  await expect(handles(page)).toHaveCount(8);
  await undoButton(page).click();
  expect(await shown(page, [0.6, 0.6])).not.toBe('#ff0000');
  await expect(handles(page)).toHaveCount(8);
  await redoButton(page).click();
  expect(await shown(page, [0.6, 0.6])).toBe('#ff0000');
  await undoButton(page).click();

  // An edge moves only that edge.
  await mouseDrag(page, await handleAt(page, 'e'), await at(page, [0.6, 0.3]));
  expect(await shown(page, [0.5, 0.3])).toBe('#ff0000');
  expect(await shown(page, [0.3, 0.5])).not.toBe('#ff0000');

  // Dragged past the opposite edge, the box flips instead of collapsing.
  await mouseDrag(page, await handleAt(page, 'e'), await at(page, [0.1, 0.3]));
  expect(await shown(page, [0.15, 0.3])).toBe('#ff0000');
  expect(await shown(page, [0.3, 0.3])).not.toBe('#ff0000');
  await expect(handles(page)).toHaveCount(8);
});

test('Shift squares a corner drag and snaps a line end to 45 degrees', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await mouseDrag(page, await handleAt(page, 'se'), await at(page, [0.8, 0.5]), true);
  // Anchored at the north-west corner (0.2): a 0.6 square.
  expect(await shown(page, [0.75, 0.75])).toBe('#ff0000');
  expect(await shown(page, [0.75, 0.85])).not.toBe('#ff0000');
  await page.keyboard.press('Enter');

  await page.keyboard.press('l');
  await page.getByLabel('Foreground color', { exact: true }).fill('#0000ff');
  await drag(page, imageCanvas(page), [0.1, 0.9], [0.3, 0.9]);
  await mouseDrag(page, await handleAt(page, 'end1'), await at(page, [0.62, 0.55]), true);
  // Ten degrees off the diagonal snaps onto it: (0.1, 0.9) to about (0.6, 0.4).
  expect(await shown(page, [0.35, 0.65])).toBe('#0000ff');
});

test('the body moves a box, the grip moves a line, the stroke does nothing', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await mouseDrag(page, await at(page, [0.3, 0.3]), await at(page, [0.6, 0.6]));
  expect(await shown(page, [0.6, 0.6])).toBe('#ff0000');
  expect(await shown(page, [0.25, 0.25])).not.toBe('#ff0000');
  await expect(handles(page)).toHaveCount(8);
  await undoButton(page).click();
  expect(await shown(page, [0.25, 0.25])).toBe('#ff0000');
  await page.keyboard.press('Enter');

  await page.keyboard.press('l');
  await page.getByLabel('Foreground color', { exact: true }).fill('#0000ff');
  await drag(page, imageCanvas(page), [0.1, 0.8], [0.5, 0.8]);
  const grip = await handleAt(page, 'grip');
  await mouseDrag(page, grip, [grip[0], grip[1] - 100]);
  const end = await handleAt(page, 'end0');
  expect(end[1]).toBeLessThan((await at(page, [0.1, 0.8]))[1] - 80);
  // A drag on the stroke neither moves, commits nor adds a step.
  const [ax, ay] = await handleAt(page, 'end0');
  const [bx, by] = await handleAt(page, 'end1');
  const mid: [number, number] = [(ax + bx) / 2, (ay + by) / 2];
  await mouseDrag(page, mid, [mid[0], mid[1] + 60]);
  await expect(handles(page)).toHaveCount(3);
  expect(await handleAt(page, 'end0')).toEqual([ax, ay]);
  await undoButton(page).click(); // the grip drag
  await undoButton(page).click(); // the line
  await undoButton(page).click(); // the committed box
  await expect(undoButton(page)).toBeDisabled();
});

test('handles keep their size and reach at every zoom, on the stage too', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  const size = async () => (await page.locator('[data-live-handle="se"]').boundingBox())?.width;
  const fitted = await size();
  await page.getByRole('button', { name: 'Actual size' }).click();
  expect(await size()).toBe(fitted);

  // Grabbed 7 CSS px off its centre at 100 %.
  const se = await handleAt(page, 'se');
  await mouseDrag(page, [se[0] + 7, se[1]], [se[0] + 27, se[1] + 20]);
  const moved = await handleAt(page, 'se');
  expect(Math.abs(moved[0] - (se[0] + 20))).toBeLessThan(1.5);

  // Moved half off the canvas, its corner on the stage is still grabbed.
  await page.getByRole('button', { name: 'Fit to window' }).click();
  const nw = await handleAt(page, 'nw');
  const left = await at(page, [-0.1, 0.25]);
  await mouseDrag(page, [nw[0] + 12, nw[1] + 12], [left[0] + 12, nw[1] + 12]);
  const onStage = await handleAt(page, 'nw');
  expect(onStage[0]).toBeLessThan((await at(page, [0, 0]))[0]);
  await mouseDrag(page, onStage, await at(page, [0.05, 0.2]));
  expect((await handleAt(page, 'nw'))[0]).toBeGreaterThan((await at(page, [0, 0]))[0]);
});

test('Escape or Undo during a handle drag drops only that drag', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  for (const key of ['Escape', 'Control+z']) {
    const se = await handleAt(page, 'se');
    const target = await at(page, [0.8, 0.8]);
    await page.mouse.move(se[0], se[1]);
    await page.mouse.down();
    await page.mouse.move(target[0], target[1], { steps: 6 });
    await expect.poll(() => shown(page, [0.7, 0.7])).toBe('#ff0000');
    await page.keyboard.press(key);
    await page.mouse.up();
    expect(await shown(page, [0.7, 0.7])).not.toBe('#ff0000');
    expect(await shown(page, [0.3, 0.3])).toBe('#ff0000');
    await expect(handles(page)).toHaveCount(8);
    await expect(redoButton(page)).toBeDisabled();
    expect(await handleAt(page, 'se')).toEqual(se);
  }
  // With no drag in progress, Escape cancels the object.
  await page.keyboard.press('Escape');
  await expect(handles(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
});

test('the cursor shows what a press on the live object grabs', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  const cursor = () => imageCanvas(page).evaluate((el) => getComputedStyle(el).cursor);
  const hover = async (p: [number, number]) => page.mouse.move(p[0], p[1]);
  await hover(await handleAt(page, 'se'));
  await expect.poll(cursor).toBe('nwse-resize');
  await hover(await handleAt(page, 'ne'));
  await expect.poll(cursor).toBe('nesw-resize');
  await hover(await handleAt(page, 'n'));
  await expect.poll(cursor).toBe('ns-resize');
  await hover(await handleAt(page, 'w'));
  await expect.poll(cursor).toBe('ew-resize');
  await hover(await at(page, [0.3, 0.3]));
  await expect.poll(cursor).toBe('move');
  await hover(await at(page, [0.7, 0.7]));
  await expect.poll(cursor).toBe('crosshair');

  await page.keyboard.press('Enter');
  await page.keyboard.press('l');
  await drag(page, imageCanvas(page), [0.1, 0.8], [0.5, 0.8]);
  await hover(await handleAt(page, 'end1'));
  await expect.poll(cursor).toBe('crosshair');
  await hover(await handleAt(page, 'grip'));
  await expect.poll(cursor).toBe('move');
});

test('a drag on an object committed meanwhile is dropped silently', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (err) => errors.push(err.message));
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  const se = await handleAt(page, 'se');
  const target = await at(page, [0.8, 0.8]);
  await page.mouse.move(se[0], se[1]);
  await page.mouse.down();
  await page.mouse.move(se[0] + 10, se[1] + 10, { steps: 2 });
  // A document command commits the object under the drag.
  await page.keyboard.press('Shift+N');
  await expect(handles(page)).toHaveCount(0);
  await page.mouse.move(target[0], target[1], { steps: 4 });
  await page.mouse.up();
  await expect(handles(page)).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(errors).toEqual([]);
  expect(await shown(page, [0.75, 0.75])).not.toBe('#ff0000');
  await undoButton(page).click(); // the new layer
  await undoButton(page).click(); // the committed box
  await expect(undoButton(page)).toBeDisabled();
});
