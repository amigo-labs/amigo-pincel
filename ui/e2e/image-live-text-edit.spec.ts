import { expect, test, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, setColor } from './helpers';
import {
  H,
  W,
  canvasShot,
  caret,
  chooseText,
  docRect,
  ink,
  liveInput,
  moveHandle,
  redoButton,
  selectionMarks,
  setFontSize,
  textBox,
  typeAt,
  undoAll,
  undoButton,
  zoomOf,
} from './text-helpers';

// Moving, restyling, panning around and undoing an Image-mode live text
// (docs/specs/image-live-text.md AC-07, AC-08, AC-12, AC-13, AC-24).

test.beforeEach(async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await chooseText(page);
  await setFontSize(page, 24);
});

/** Page coordinates of the move handle's centre. */
async function handleAt(page: Page): Promise<[number, number]> {
  const b = await moveHandle(page).boundingBox();
  if (!b) throw new Error('no move handle');
  return [b.x + b.width / 2, b.y + b.height / 2];
}

test('the move handle drags the text by whole pixels as one step', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hi');
  await page.keyboard.press('Shift+ArrowLeft');
  const box = await docRect(page, textBox(page));
  const before = await ink(page);
  const [hx, hy] = await handleAt(page);
  const z = await zoomOf(page);
  const [tx, ty] = [hx + 30.3 * z, hy + 10.3 * z];
  await page.mouse.move(hx, hy);
  await page.mouse.down();
  await page.mouse.move(tx, ty, { steps: 8 });
  await page.mouse.up();

  const moved = await docRect(page, textBox(page));
  expect(moved.x0 - box.x0).toBeCloseTo(30, 0);
  expect(moved.y0 - box.y0).toBeCloseTo(10, 0);
  const after = await ink(page);
  expect((after?.x0 ?? 0) - (before?.x0 ?? 0)).toBe(30);
  expect((after?.y0 ?? 0) - (before?.y0 ?? 0)).toBe(10);
  // Content, caret and selection stay; the text stays editable.
  await expect(selectionMarks(page)).toHaveCount(1);
  await expect(liveInput(page)).toBeFocused();

  // The drag is one step: Undo puts the text back and keeps it live.
  await page.keyboard.press('Control+z');
  await expect(textBox(page)).toBeVisible();
  const back = await docRect(page, textBox(page));
  expect(back.x0).toBeCloseTo(box.x0, 1);
  expect(back.y0).toBeCloseTo(box.y0, 1);

  // Escape during a drag undoes just that drag.
  const [gx, gy] = await handleAt(page);
  await page.mouse.move(gx, gy);
  await page.mouse.down();
  await page.mouse.move(gx + 60, gy + 30, { steps: 6 });
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await expect(textBox(page)).toBeVisible();
  const kept = await docRect(page, textBox(page));
  expect(kept.x0).toBeCloseTo(box.x0, 1);
  expect(kept.y0).toBeCloseTo(box.y0, 1);
  // The dropped drag leaves nothing to redo.
  await expect(redoButton(page)).toBeDisabled();
  // Steps: begin "H", the burst "i".
  expect(await undoAll(page)).toBe(2);
});

test('options and the color apply to the live text, one step each', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hello');
  await page.keyboard.press('Shift+ArrowLeft');
  await page.keyboard.press('Shift+ArrowLeft');
  const plain = await ink(page);

  await page.getByRole('checkbox', { name: 'Bold' }).check();
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await expect(selectionMarks(page)).toHaveCount(1);
  const bold = await ink(page);
  expect((bold?.x1 ?? 0) - (bold?.x0 ?? 0)).toBeGreaterThan((plain?.x1 ?? 0) - (plain?.x0 ?? 0));
  // A second Bold change right after merges into the same step.
  await page.getByRole('checkbox', { name: 'Bold' }).uncheck();
  await page.getByRole('checkbox', { name: 'Bold' }).check();

  await page.getByRole('checkbox', { name: 'Underline' }).check();
  await expect(liveInput(page)).toBeFocused();
  const under = await ink(page);
  expect(under?.y1 ?? 0).toBeGreaterThan(bold?.y1 ?? 0);

  await page.getByRole('combobox', { name: 'Align' }).selectOption('center');
  await expect(liveInput(page)).toBeFocused();

  // A slider drag without letting go is one step.
  const slider = page.getByRole('slider', { name: 'Font size', exact: true });
  const s = await slider.boundingBox();
  if (!s) throw new Error('no slider');
  await page.mouse.move(s.x + s.width * 0.1, s.y + s.height / 2);
  await page.mouse.down();
  await page.mouse.move(s.x + s.width * 0.15, s.y + s.height / 2, { steps: 5 });
  await page.mouse.up();
  await expect(liveInput(page)).toBeFocused();

  await setColor(page, 'fg', '#ff0000');
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  // The selection was kept through all of it: typing replaces it.
  await expect(selectionMarks(page)).toHaveCount(1);
  await page.keyboard.type('p');
  await expect(selectionMarks(page)).toHaveCount(0);

  // begin, burst, bold, underline, align, size, color, "p".
  expect(await undoAll(page)).toBe(8);
});

test('italic, strikethrough and anti-alias redraw the live text, one step each', async ({
  page,
}) => {
  await typeAt(page, [20, 20], 'Hello');
  let shot = await canvasShot(page);
  for (const [name, on] of [
    ['Italic', true],
    ['Strikethrough', true],
    ['Anti-alias', false],
  ] as const) {
    const box = page.getByRole('checkbox', { name, exact: true });
    if (on) await box.check();
    else await box.uncheck();
    await expect(textBox(page), name).toBeVisible();
    await expect(liveInput(page), name).toBeFocused();
    await expect
      .poll(async () => (await canvasShot(page)).equals(shot), { message: name })
      .toBe(false);
    shot = await canvasShot(page);
  }
  // begin, burst, italic, strikethrough, anti-alias.
  expect(await undoAll(page)).toBe(5);
});

test('panning, zooming and the options bar leave the text open', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hi');
  const canvas = imageCanvas(page);
  const b = await canvas.boundingBox();
  if (!b) throw new Error('no canvas');
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.wheel(0, -200);
  await expect(textBox(page)).toBeVisible();
  await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
  await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
  await expect(textBox(page)).toBeVisible();
  // Middle-button drag pans.
  await page.mouse.move(b.x + 10, b.y + 10);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(b.x + 40, b.y + 30, { steps: 4 });
  await page.mouse.up({ button: 'middle' });
  await expect(textBox(page)).toBeVisible();
  await page.getByRole('checkbox', { name: 'Italic' }).check();
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('!');
  // begin, burst, italic, "!".
  expect(await undoAll(page)).toBe(4);
});

test('Undo and Redo step through the live text and reopen it', async ({ page }) => {
  await typeAt(page, [20, 20], 'AB');
  await page.keyboard.press('Shift+ArrowLeft');
  await expect(selectionMarks(page)).toHaveCount(1);
  // Undo from inside the text: the burst goes, the caret to the end.
  await page.keyboard.press('Control+z');
  await expect(textBox(page)).toBeVisible();
  await expect(selectionMarks(page)).toHaveCount(0);
  const box = await docRect(page, textBox(page));
  const end = await docRect(page, caret(page));
  expect(Math.abs((end.x0 + end.x1) / 2 - box.x1)).toBeLessThanOrEqual(1.5);
  // Undo from the menu removes the beginning step and closes the box.
  await menu(page, 'Edit', 'Undo');
  await expect(textBox(page)).toHaveCount(0);
  expect(await ink(page)).toBeNull();
  // Redo from the toolbar opens it again, focused, caret at the end.
  await redoButton(page).click();
  await expect(textBox(page)).toBeVisible();
  await expect(moveHandle(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await redoButton(page).click();
  await expect(redoButton(page)).toBeDisabled();
  const again = await docRect(page, textBox(page));
  const tail = await docRect(page, caret(page));
  expect(Math.abs((tail.x0 + tail.x1) / 2 - again.x1)).toBeLessThanOrEqual(1.5);
  await liveInput(page).focus();
  await page.keyboard.type('C');
  await expect(undoButton(page)).toBeEnabled();
});

test('a command from outside the text closes the box', async ({ page }) => {
  await typeAt(page, [20, 20], 'Hi');
  await page.getByRole('button', { name: 'Add layer', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
  await expect(liveInput(page)).toHaveCount(0);
  expect(await ink(page)).not.toBeNull();
  // Keys go to the editor again: B picks the Brush.
  await page.keyboard.press('b');
  await expect(page.getByRole('button', { name: 'Brush', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // Undo of the layer and the commit, from outside: no live text returns.
  await undoButton(page).click();
  await undoButton(page).click();
  await expect(textBox(page)).toHaveCount(0);
});
