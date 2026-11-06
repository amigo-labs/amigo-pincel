import { expect, test, type Page } from '@playwright/test';
import { createDocument, putClipboardImage } from './helpers';
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
  selectionMarks,
  setFontSize,
  textBox,
  typeAt,
} from './text-helpers';

// Caret, selection and clipboard inside an Image-mode live text
// (docs/specs/image-live-text.md AC-04 – AC-06, AC-14, AC-23).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

test.beforeEach(async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await chooseText(page);
  await setFontSize(page, 24);
});

/** Expects the live text to read `expected`: replacing it with `expected`
 *  typed afresh leaves the layer exactly as it was. */
async function expectText(page: Page, expected: string): Promise<void> {
  const shown = await canvasShot(page);
  await page.keyboard.press('Control+a');
  await page.keyboard.type(expected);
  expect((await canvasShot(page)).equals(shown)).toBe(true);
}

/** The caret's x in document pixels. */
async function caretX(page: Page): Promise<number> {
  const r = await docRect(page, caret(page));
  return (r.x0 + r.x1) / 2;
}

test('arrows, Home and End move the caret; Shift extends the selection', async ({ page }) => {
  await typeAt(page, [20, 20], 'ABC');
  await page.keyboard.press('Home');
  await page.keyboard.type('x');
  await page.keyboard.press('End');
  await page.keyboard.type('y');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.type('z');
  await expectText(page, 'xABCzy');

  // Up / Down keep the caret's x on the next line.
  await page.keyboard.press('Control+a');
  await page.keyboard.type('HH\nHH');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.type('z');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.type('w');
  await expectText(page, 'HHz\nHwH');

  // Shift extends from the anchor; typing replaces the selection.
  await page.keyboard.press('Control+a');
  await page.keyboard.type('ABCD');
  await page.keyboard.press('Home');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await expect(selectionMarks(page)).toHaveCount(1);
  await page.keyboard.type('x');
  await expect(selectionMarks(page)).toHaveCount(0);
  await expectText(page, 'xCD');
});

test('Backspace and Delete remove the selected characters', async ({ page }) => {
  await typeAt(page, [20, 20], 'ABCDE');
  // "AB" selected: Backspace removes both, not the character before.
  await page.keyboard.press('Home');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Backspace');
  await expect(selectionMarks(page)).toHaveCount(0);
  await expectText(page, 'CDE');
  // "DE" selected backwards from the end: Delete removes both.
  await page.keyboard.press('End');
  await page.keyboard.press('Shift+ArrowLeft');
  await page.keyboard.press('Shift+ArrowLeft');
  await page.keyboard.press('Delete');
  await expectText(page, 'C');
});

test('Ctrl+A selects the characters, not the canvas', async ({ page }) => {
  await typeAt(page, [20, 20], 'AB');
  await page.keyboard.press('Enter');
  await page.keyboard.type('CD');
  await page.keyboard.press('Control+a');
  // A canvas Select All would have committed the text.
  await expect(selectionMarks(page)).toHaveCount(2);
  await expect(textBox(page)).toBeVisible();
  await page.keyboard.type('q');
  await expectText(page, 'q');
});

for (const variant of ['left', 'center', 'right', 'bold', 'italic'] as const) {
  test(`the caret sits on the layout's boundary (${variant})`, async ({ page }) => {
    await page.getByRole('checkbox', { name: 'Anti-alias' }).uncheck();
    if (variant === 'bold') await page.getByRole('checkbox', { name: 'Bold' }).check();
    else if (variant === 'italic') await page.getByRole('checkbox', { name: 'Italic' }).check();
    else await page.getByRole('combobox', { name: 'Align' }).selectOption(variant);
    await typeAt(page, [40, 20], 'HHHHHH');
    await page.keyboard.press('Enter');
    await page.keyboard.type('HHHH');
    const box = await docRect(page, textBox(page));
    const mid = (box.y0 + box.y1) / 2;
    // H is symmetric: the middle boundary of a line of Hs is the middle of
    // its ink (on the baseline rows, where italic does not shear it).
    const middle = async (from: number, to: number): Promise<number> => {
      const all = await ink(page, Math.floor(from), Math.ceil(to));
      if (!all) throw new Error('no ink');
      const base = await ink(page, all.y1 - 2, all.y1);
      if (!base) throw new Error('no baseline ink');
      return (base.x0 + base.x1) / 2;
    };
    // Bold smears each glyph to the right by a pixel or two.
    const slack = variant === 'bold' ? 2 : 1;
    // Second line, caret at its middle.
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    expect(Math.abs((await caretX(page)) - (await middle(mid, box.y1)))).toBeLessThanOrEqual(slack);
    const c = await docRect(page, caret(page));
    expect(c.y0).toBeGreaterThanOrEqual(mid - 1);
    // First line, caret at its middle.
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('Home');
    for (let i = 0; i < 3; i += 1) await page.keyboard.press('ArrowRight');
    expect(Math.abs((await caretX(page)) - (await middle(box.y0, mid)))).toBeLessThanOrEqual(slack);
    const top = await docRect(page, caret(page));
    expect(Math.abs(top.y0 - box.y0)).toBeLessThanOrEqual(1);
  });
}

test('a press inside the box places the caret; a drag selects', async ({ page }) => {
  await page.getByRole('checkbox', { name: 'Anti-alias' }).uncheck();
  // An empty box keeps a press inside it.
  await clickAt(page, [40, 20]);
  const empty = await docRect(page, textBox(page));
  await clickAt(page, [empty.x0 + 2, empty.y0 + 4]);
  const same = await docRect(page, textBox(page));
  expect(same.x0).toBeCloseTo(empty.x0, 1);
  await expect(liveInput(page)).toBeFocused();

  await page.keyboard.type('HHHH');
  const line = await ink(page);
  if (!line) throw new Error('no ink');
  const quarter = (line.x1 - line.x0) / 4;
  const y = (line.y0 + line.y1) / 2;
  // Press near the first boundary: the caret goes there.
  await clickAt(page, [line.x0 + quarter + 1, y]);
  await expect(textBox(page)).toHaveCount(1);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('z');
  await expectText(page, 'HzHHH');

  // Drag from the first to the third boundary of "HHHH": two selected.
  await page.keyboard.press('Control+a');
  await page.keyboard.type('HHHH');
  const [x0, y0] = await at(page, [line.x0 + quarter, y]);
  const [x1] = await at(page, [line.x0 + 3 * quarter, y]);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move(x1, y0, { steps: 6 });
  await page.mouse.up();
  await expect(selectionMarks(page)).toHaveCount(1);
  await expect(textBox(page)).toHaveCount(1);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('z');
  await expectText(page, 'HzH');
});

test('the hit area reaches 4 CSS px around the box', async ({ page }) => {
  await typeAt(page, [40, 40], 'HHHH');
  const b = await textBox(page).boundingBox();
  if (!b) throw new Error('no box');
  // The drawn border sits 1 CSS px outside the box.
  const left = b.x + 1;
  const midY = b.y + b.height / 2;
  const before = await docRect(page, textBox(page));
  // 3 CSS px left of the box: the caret goes to the start, nothing commits.
  await page.mouse.click(left - 3, midY);
  await expect(textBox(page)).toHaveCount(1);
  await expect(liveInput(page)).toBeFocused();
  const same = await docRect(page, textBox(page));
  expect(same.x0).toBeCloseTo(before.x0, 1);
  expect(same.y0).toBeCloseTo(before.y0, 1);
  await page.keyboard.type('z');
  await expectText(page, 'zHHHH');
  // 3 CSS px above the box, too.
  await page.mouse.click(left + 2, b.y + 1 - 3);
  await expect(textBox(page)).toHaveCount(1);
  expect((await docRect(page, textBox(page))).x0).toBeCloseTo(before.x0, 1);

  // 12 CSS px left of it: the press commits and opens a new box there.
  await page.mouse.click(left - 12, midY);
  await expect(textBox(page)).toHaveCount(1);
  const fresh = await docRect(page, textBox(page));
  expect(fresh.x0).toBeLessThan(before.x0 - 1);
  await page.keyboard.press('Escape');
  await expect(textBox(page)).toHaveCount(0);
  expect(await ink(page)).not.toBeNull();
});

test('Ctrl+C, Ctrl+X and Ctrl+V copy, cut and paste plain text', async ({ page }) => {
  await page.evaluate(() => navigator.clipboard.writeText('-'));
  await typeAt(page, [20, 20], 'Hello');
  // Nothing selected: copy and cut do nothing.
  await page.keyboard.press('Control+c');
  await page.keyboard.press('Control+x');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('-');

  await page.keyboard.press('Shift+Home');
  await page.keyboard.press('Control+c');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('Hello');
  await page.keyboard.press('Control+x');
  await expect(textBox(page)).toBeVisible();
  await expect.poll(async () => ink(page)).toBeNull();

  // Paste brings the text back, with its line breaks.
  await page.evaluate(() => navigator.clipboard.writeText('He\r\nllo'));
  await page.keyboard.press('Control+v');
  await expectText(page, 'He\nllo');

  // A clipboard without text pastes nothing and no image layer.
  const before = await canvasShot(page);
  await putClipboardImage(page, 8, 8, '#ff0000');
  await page.keyboard.press('Control+v');
  await expect(textBox(page)).toBeVisible();
  expect((await canvasShot(page)).equals(before)).toBe(true);
  await expect(page.getByRole('button', { name: 'Layer 2', exact: true })).toHaveCount(0);
});
