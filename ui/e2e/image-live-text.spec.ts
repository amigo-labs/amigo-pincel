import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu } from './helpers';
import {
  H,
  W,
  at,
  canvasShot,
  caret,
  chooseText,
  clickAt,
  docRect,
  ink,
  liveInput,
  moveHandle,
  redoButton,
  setFontSize,
  textBox,
  typeAt,
  undoAll,
  undoButton,
  zoomOf,
} from './text-helpers';

// Image-mode live text, opening, typing, committing and cancelling
// (docs/specs/image-live-text.md AC-01 – AC-03, AC-09, AC-11, AC-15,
// AC-16, AC-21, AC-23).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

test.beforeEach(async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await chooseText(page);
  await setFontSize(page, 24);
});

/** A row of the layers panel. */
function layerRow(page: Page, name: string): Locator {
  return page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });
}

/**
 * The right edge of the dark (ink) pixels in Layer 1's thumbnail, as a
 * fraction of the thumbnail's width; 0 when there is none. Reads a
 * screenshot of the thumbnail inside its border.
 */
async function thumbnailInkRight(page: Page): Promise<number> {
  const png = await layerRow(page, 'Layer 1').locator('canvas').screenshot();
  return page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    const inner = bitmap.width - 2;
    let right = 0;
    // 2 px in: the border and its rounded corners are dark too.
    for (let y = 2; y < bitmap.height - 2; y += 1) {
      for (let x = 2; x < bitmap.width - 2; x += 1) {
        const i = (y * bitmap.width + x) * 4;
        const sum = (data[i] ?? 255) + (data[i + 1] ?? 255) + (data[i + 2] ?? 255);
        if (sum < 300) right = Math.max(right, x);
      }
    }
    return right / inner;
  }, png.toString('base64'));
}

test('a Text press opens an empty box; the first character begins the text', async ({ page }) => {
  const z = await zoomOf(page);
  const [px, py] = await at(page, [40, 30]);
  await page.mouse.click(px, py);
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await expect(undoButton(page)).toBeDisabled();
  const box = await textBox(page).boundingBox();
  expect(Math.abs((box?.x ?? 0) - px)).toBeLessThanOrEqual(z + 1);
  expect(Math.abs((box?.y ?? 0) - py)).toBeLessThanOrEqual(z + 1);
  // The empty box shows a caret at its left edge.
  await expect(caret(page)).toBeVisible();
  const c0 = await docRect(page, caret(page));
  const b0 = await docRect(page, textBox(page));
  expect(Math.abs((c0.x0 + c0.x1) / 2 - b0.x0)).toBeLessThanOrEqual(1.5);
  expect(c0.y0).toBeGreaterThanOrEqual(b0.y0 - 0.5);

  // Options on an empty box change only the tool option: no step.
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  await expect(undoButton(page)).toBeDisabled();
  await page.getByRole('checkbox', { name: 'Bold' }).uncheck();

  // Escape just closes the empty box.
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();

  // Center and Right alignment: the box still opens with its top-left at
  // the press.
  await page.getByRole('combobox', { name: 'Align' }).selectOption('center');
  await page.mouse.click(px, py);
  const center = await textBox(page).boundingBox();
  expect(Math.abs((center?.x ?? 0) - px)).toBeLessThanOrEqual(z + 1);
  expect(Math.abs((center?.y ?? 0) - py)).toBeLessThanOrEqual(z + 1);
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
  await page.getByRole('combobox', { name: 'Align' }).selectOption('right');
  await page.mouse.click(px, py);
  const right = await textBox(page).boundingBox();
  expect(Math.abs((right?.x ?? 0) - px)).toBeLessThanOrEqual(z + 1);
  expect(Math.abs((right?.y ?? 0) - py)).toBeLessThanOrEqual(z + 1);

  // The first character begins the live text as one undo step.
  await page.keyboard.type('H');
  await expect(undoButton(page)).toBeEnabled();
  expect(await ink(page)).not.toBeNull();
  await undoButton(page).click();
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
  expect(await ink(page)).toBeNull();
});

test('an empty box: a paste begins the text; Undo and the commit triggers just close it', async ({
  page,
}) => {
  // A brush dot as the step before.
  await page.getByRole('button', { name: 'Brush', exact: true }).click();
  await clickAt(page, [150, 100]);
  await chooseText(page);

  // Undo closes the empty box and undoes the step before as usual.
  await clickAt(page, [20, 20]);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.press('Control+z');
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
  await expect(redoButton(page)).toBeEnabled();

  // Commit triggers close the empty box and add no step of their own.
  const triggers: [string, () => Promise<void>][] = [
    ['Finish', () => page.getByRole('button', { name: 'Finish', exact: true }).click()],
    ['Edit ▸ Copy', () => menu(page, 'Edit', 'Copy')],
    ['Layer ▸ New Layer', () => menu(page, 'Layer', 'New Layer')],
    [
      'an active-layer change',
      () => page.getByRole('button', { name: 'Layer 1', exact: true }).click(),
    ],
  ];
  for (const [name, run] of triggers) {
    await clickAt(page, [20, 20]);
    await expect(textBox(page), name).toBeVisible();
    await run();
    await expect(textBox(page), name).toHaveCount(0);
  }
  // Only New Layer is a step; nothing of the boxes is.
  expect(await undoAll(page)).toBe(1);

  // A paste into an empty box begins the live text as one step.
  await page.evaluate(() => navigator.clipboard.writeText('Hi'));
  await clickAt(page, [20, 20]);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.press('Control+v');
  await expect(textBox(page)).toBeVisible();
  expect(await ink(page)).not.toBeNull();
  await expect(moveHandle(page)).toBeVisible();
  expect(await undoAll(page)).toBe(1);
  await expect(textBox(page)).toHaveCount(0);
  expect(await ink(page)).toBeNull();
});

test('typing draws the text into the layer as the commit leaves it', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hellx');
  await page.keyboard.press('Backspace');
  await page.keyboard.type('o');
  await expect(textBox(page)).toBeVisible();
  const live = await canvasShot(page);
  expect(await ink(page)).not.toBeNull();

  await page.keyboard.press('Control+Enter');
  await expect(textBox(page)).toHaveCount(0);
  expect((await canvasShot(page)).equals(live)).toBe(true);
  // The commit is one undo step.
  expect(await undoAll(page)).toBe(1);
  expect(await ink(page)).toBeNull();

  // The same text typed in one go paints the same pixels.
  await typeAt(page, [20, 20], 'Hello');
  expect((await canvasShot(page)).equals(live)).toBe(true);
});

test('every edit redraws the text over the layer as it was before', async ({ page }) => {
  // Paint under the text's place first.
  await page.getByRole('button', { name: 'Brush', exact: true }).click();
  await drag(page, imageCanvas(page), [10 / W, 32 / H], [120 / W, 32 / H]);
  await chooseText(page);
  await typeAt(page, [20, 20], 'Hellxxx');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.type('o');
  const edited = await canvasShot(page);
  // The same text over the same stroke, typed in one go.
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
  await typeAt(page, [20, 20], 'Hello');
  expect((await canvasShot(page)).equals(edited)).toBe(true);
});

test('a typing burst is one step; a caret jump starts the next', async ({ page }) => {
  await typeAt(page, [20, 20], 'ab');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.type('c');
  // Delete every character: the text stays open with its steps.
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Delete');
  await expect(textBox(page)).toBeVisible();
  expect(await ink(page)).toBeNull();

  // Steps: begin "a", the burst "b", the burst after the jump.
  await undoButton(page).click();
  await expect(textBox(page)).toBeVisible();
  expect(await ink(page)).not.toBeNull();
  await undoButton(page).click();
  await expect(textBox(page)).toBeVisible();
  await undoButton(page).click();
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
});

test('the layer thumbnail shows the live text as it is typed', async ({ page }) => {
  await setFontSize(page, 48);
  await expect.poll(() => thumbnailInkRight(page)).toBe(0);
  await typeAt(page, [10, 20], 'HH');
  const two = await ink(page);
  await expect
    .poll(async () => Math.abs((await thumbnailInkRight(page)) - (two?.x1 ?? 0) / W))
    .toBeLessThan(0.08);
  // More characters of the same burst: no new undo step, a new thumbnail.
  await page.keyboard.type('HHH');
  const five = await ink(page);
  expect((five?.x1 ?? 0) - (two?.x1 ?? 0)).toBeGreaterThan(60);
  await expect
    .poll(async () => Math.abs((await thumbnailInkRight(page)) - (five?.x1 ?? 0) / W))
    .toBeLessThan(0.08);
  await expect(textBox(page)).toBeVisible();
  // Steps: begin "H", the burst "HHHH".
  expect(await undoAll(page)).toBe(2);
});

test('a press inside the text is a caret jump: the next typing is its own step', async ({
  page,
}) => {
  await typeAt(page, [20, 20], 'ab');
  const box = await docRect(page, textBox(page));
  // A press at the end of the text, inside its box.
  await clickAt(page, [box.x1 - 1, (box.y0 + box.y1) / 2]);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('c');
  await expect(textBox(page)).toBeVisible();
  const abc = await ink(page);
  expect((abc?.x1 ?? 0) - (box.x1 - 2)).toBeGreaterThan(0);
  // Undo takes "c" alone: the text is "ab" again and still open.
  await undoButton(page).click();
  await expect(textBox(page)).toBeVisible();
  await expect.poll(async () => (await ink(page))?.x1).toBeLessThan(abc?.x1 ?? 0);
  // Steps left: begin "a", the burst "b".
  expect(await undoAll(page)).toBe(2);
});

test('Enter breaks the line; the box keeps its top-left and fits the lines', async ({ page }) => {
  await typeAt(page, [20, 20], 'H');
  const one = await docRect(page, textBox(page));
  await page.keyboard.type('HHH');
  const four = await docRect(page, textBox(page));
  expect(four.x0).toBeCloseTo(one.x0, 0);
  expect(four.y0).toBeCloseTo(one.y0, 0);
  expect(four.x1 - four.x0).toBeGreaterThan(one.x1 - one.x0);
  const lineInk = await ink(page);

  await page.keyboard.press('Enter');
  await page.keyboard.type('H');
  const two = await docRect(page, textBox(page));
  expect(two.x0).toBeCloseTo(one.x0, 0);
  expect(two.y0).toBeCloseTo(one.y0, 0);
  expect(two.x1 - two.x0).toBeCloseTo(four.x1 - four.x0, 0);
  expect(two.y1 - two.y0).toBeGreaterThan((four.y1 - four.y0) * 1.5);
  const both = await ink(page);
  expect((both?.y1 ?? 0) - (both?.y0 ?? 0)).toBeGreaterThan(
    ((lineInk?.y1 ?? 0) - (lineInk?.y0 ?? 0)) * 1.5,
  );
  // The box is as wide as the widest line's advance.
  const w = (lineInk?.x1 ?? 0) - (lineInk?.x0 ?? 0);
  expect(two.x1 - two.x0).toBeGreaterThanOrEqual(w);
  expect(two.x1 - two.x0).toBeLessThan(w + 12);

  // A size change keeps the top-left too.
  await setFontSize(page, 32);
  const big = await docRect(page, textBox(page));
  expect(big.x0).toBeCloseTo(one.x0, 0);
  expect(big.y0).toBeCloseTo(one.y0, 0);
  expect(big.x1 - big.x0).toBeGreaterThan(two.x1 - two.x0);
});

for (const align of ['left', 'center', 'right'] as const) {
  test(`${align}-aligned lines line up inside the box`, async ({ page }) => {
    await page.getByRole('combobox', { name: 'Align' }).selectOption(align);
    await typeAt(page, [30, 20], 'HHHH');
    await page.keyboard.press('Enter');
    await page.keyboard.type('HH');
    const box = await docRect(page, textBox(page));
    const mid = (box.y0 + box.y1) / 2;
    const top = await ink(page, Math.floor(box.y0), Math.floor(mid));
    const bottom = await ink(page, Math.ceil(mid), Math.ceil(box.y1));
    if (!top || !bottom) throw new Error('no ink');
    const edge = (r: { x0: number; x1: number }): number =>
      align === 'left' ? r.x0 : align === 'right' ? r.x1 : (r.x0 + r.x1) / 2;
    expect(Math.abs(edge(top) - edge(bottom))).toBeLessThanOrEqual(1);
    // The widest line fills the box.
    expect(top.x0).toBeGreaterThanOrEqual(box.x0 - 1);
    expect(top.x1).toBeLessThanOrEqual(box.x1 + 1);
  });
}

test('Ctrl+Enter, Finish, another tool and a press outside commit the text', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hi');
  await page.keyboard.press('Control+Enter');
  await expect(textBox(page)).toHaveCount(0);

  await typeAt(page, [20, 60], 'Hi');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);

  await typeAt(page, [100, 20], 'Hi');
  await page.getByRole('button', { name: 'Brush', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);

  await chooseText(page);
  await typeAt(page, [100, 60], 'Hi');
  // A press outside the box commits and opens a new, empty box there.
  await clickAt(page, [150, 100]);
  await expect(textBox(page)).toHaveCount(1);
  await expect(liveInput(page)).toBeFocused();
  const fresh = await docRect(page, textBox(page));
  expect(fresh.x0).toBeCloseTo(150, -0.5);
  expect(fresh.y0).toBeCloseTo(100, -0.5);
  await page.keyboard.press('Escape');

  // Four commits, one step each; the ink is all there.
  expect(await ink(page, 0, 40)).not.toBeNull();
  expect(await ink(page, 50, 90)).not.toBeNull();
  expect(await undoAll(page)).toBe(4);
});

test('a document command or an active-layer change commits the text first', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hi');
  await menu(page, 'Layer', 'New Layer');
  await expect(textBox(page)).toHaveCount(0);
  // The text went onto the layer it was opened on, as one step.
  await typeAt(page, [20, 60], 'Hi');
  // Back to the first layer in the layers panel.
  await page.getByRole('button', { name: 'Layer 1', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);

  // A chord with the caret in the text (Invert Selection) commits and runs.
  await typeAt(page, [100, 20], 'Hi');
  await page.keyboard.press('Control+Shift+I');
  await expect(textBox(page)).toHaveCount(0);
  expect(await ink(page, 0, 50)).not.toBeNull();
  // Steps: commit, New Layer, commit, commit, Invert Selection.
  expect(await undoAll(page)).toBe(5);
});

test('an Alt chord in the text commits it, then runs', async ({ page }) => {
  await menu(page, 'Layer', 'New Layer');
  await expect(layerRow(page, 'Layer 2')).toHaveClass(/layer-active/);
  await typeAt(page, [20, 20], 'Hi');
  // Alt+Down selects the layer below (AC-23): the text commits first.
  await page.keyboard.press('Alt+ArrowDown');
  await expect(textBox(page)).toHaveCount(0);
  await expect(layerRow(page, 'Layer 1')).toHaveClass(/layer-active/);
  expect(await ink(page)).not.toBeNull();
  // Steps: New Layer, the commit.
  expect(await undoAll(page)).toBe(2);
});

test('a text of only whitespace or wholly off the canvas leaves no step', async ({ page }) => {
  await typeAt(page, [20, 20], '   ');
  await page.keyboard.press('Enter');
  await page.keyboard.type(' ');
  await page.keyboard.press('Control+Enter');
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();

  // Beside the canvas: editable while live, gone without a step on commit.
  await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
  await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
  await typeAt(page, [W + 15, 20], 'Hi');
  await expect(textBox(page)).toBeVisible();
  await page.keyboard.press('Control+Enter');
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();

  // Partly outside: stays editable, the layer holds the part on the canvas.
  await typeAt(page, [W - 12, 20], 'HHHH');
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  const part = await ink(page);
  expect(part?.x1).toBe(W);
  await page.keyboard.press('Control+Enter');
  expect(await undoAll(page)).toBe(1);
});

test('Escape cancels the live text and keeps the step before it', async ({ page }) => {
  // A brush dot as the step before.
  await page.getByRole('button', { name: 'Brush', exact: true }).click();
  await clickAt(page, [150, 100]);
  const before = await canvasShot(page);
  await chooseText(page);
  await typeAt(page, [20, 20], 'Hi');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.type('o');
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
  expect((await canvasShot(page)).equals(before)).toBe(true);
  await expect(redoButton(page)).toBeDisabled();
  expect(await undoAll(page)).toBe(1);
});

test('in the text, tool keys and Space type; Tab stays; Undo and Redo keys step', async ({
  page,
}) => {
  const canvas = await imageCanvas(page).boundingBox();
  await typeAt(page, [20, 20], 'b');
  await page.keyboard.type(' v1');
  await expect(textBox(page)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Text', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // Space typed and did not pan: the view stayed where it was.
  expect(await imageCanvas(page).boundingBox()).toEqual(canvas);
  const typed = await canvasShot(page);
  // Tab keeps the focus and types nothing.
  await page.keyboard.press('Tab');
  await expect(liveInput(page)).toBeFocused();
  expect((await canvasShot(page)).equals(typed)).toBe(true);
  // Ctrl+Z undoes the burst " v1": the text stays open with "b".
  await page.keyboard.press('Control+z');
  await expect(textBox(page)).toBeVisible();
  expect((await canvasShot(page)).equals(typed)).toBe(false);
  // Ctrl+Y and Ctrl+Shift+Z redo it.
  await page.keyboard.press('Control+y');
  await expect.poll(async () => (await canvasShot(page)).equals(typed)).toBe(true);
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+Shift+z');
  await expect.poll(async () => (await canvasShot(page)).equals(typed)).toBe(true);
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  // The text is "b v1" with its space: typed afresh it paints the same.
  await page.keyboard.press('Control+a');
  await page.keyboard.type('b v1');
  expect((await canvasShot(page)).equals(typed)).toBe(true);
  // ... and not "bv1": the Space key typed a space.
  await page.keyboard.press('Control+a');
  await page.keyboard.type('bv1');
  expect((await canvasShot(page)).equals(typed)).toBe(false);
  await expect(imageCanvas(page)).toHaveAttribute('data-ready', 'true');
});
