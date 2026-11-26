import { expect, test, type Page } from '@playwright/test';
import { createDocument, imageCanvas, setColor, byText } from './helpers';

// Image mode: the live preview of a Move Layer drag
// (docs/specs/image-move-layer-preview.md). The scene has three layers of
// opaque, hard-edged content at 100 % opacity, so the preview the browser
// draws matches the core's composite pixel for pixel (AC-02).

const SIZE = 32;

type DiagnosticsWindow = Window & {
  __pincelDiagnostics?: { calls?: Record<string, number> };
};

/** How often the core call `name` ran. */
const calls = (page: Page, name: string): Promise<number> =>
  page.evaluate((n) => (window as DiagnosticsWindow).__pincelDiagnostics?.calls?.[n] ?? 0, name);

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

/** Screen point of the document position `(x, y)`. */
async function point(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

/** Rectangle Select of `(x, y, w, h)`, filled with `hex` (Backspace), then
 *  deselected. */
async function fillRect(
  page: Page,
  x: number,
  y: number,
  w: number,
  h: number,
  hex: string,
): Promise<void> {
  await pick(page, 'Marquee', 'Rectangle Select', 'm');
  await page.mouse.move(...(await point(page, x, y)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, x + w, y + h)), { steps: 4 });
  await page.mouse.up();
  await setColor(page, 'fg', hex);
  await page.keyboard.press('Backspace');
  await page.keyboard.press('ControlOrMeta+d');
}

/** The panel row of layer `name`. */
function layerRow(page: Page, name: string) {
  return page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });
}

/** Makes layer `name` active. */
async function activate(page: Page, name: string): Promise<void> {
  await layerRow(page, name)
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click();
  await expect(layerRow(page, name)).toHaveClass(/layer-active/);
}

/** A 32×32 document: Layer 1 blue, Layer 2 a red square at (4, 4, 8, 8),
 *  Layer 3 a green bar at (16, 0, 4, 32); Layer 2 active, Move Layer on. */
async function scene(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#0000ff');
  await page.keyboard.press('g');
  await page.mouse.click(...(await point(page, 1.5, 1.5)));
  await page.keyboard.press('Shift+N');
  await fillRect(page, 4, 4, 8, 8, '#ff0000');
  await page.keyboard.press('Shift+N');
  await fillRect(page, 16, 0, 4, 32, '#00ff00');
  await activate(page, 'Layer 2');
  await pickMove(page, 'Move Layer');
}

/** Picks the variant `name` of tool group `group`; `key` cycles them. */
async function pick(page: Page, group: string, name: string, key: string): Promise<void> {
  const button = page.getByRole('button', { name: `${group}: ${name}` });
  const on = async (): Promise<boolean> =>
    (await button.count()) > 0 && (await button.getAttribute('aria-pressed')) === 'true';
  for (let i = 0; i < 6 && !(await on()); i++) await page.keyboard.press(key);
  await expect(button).toHaveAttribute('aria-pressed', 'true');
}

/** Picks the Move variant `name`. */
const pickMove = (page: Page, name: string): Promise<void> => pick(page, 'Move', name, 'v');

const shot = (page: Page): Promise<Buffer> => imageCanvas(page).screenshot();

/** Presses at document `(x, y)` and moves to `(x + dx, y + dy)` in steps,
 *  the pointer still held; waits for the frame to show the drag. */
async function holdDrag(page: Page, x: number, y: number, dx: number, dy: number): Promise<void> {
  await page.mouse.move(...(await point(page, x + 0.5, y + 0.5)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, x + 0.5 + dx, y + 0.5 + dy)), { steps: 6 });
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
}

test('the layer follows the drag; the release shows the same picture as one step (AC-01 – AC-04)', async ({
  page,
}) => {
  await scene(page);
  const before = await shot(page);
  const moves = await calls(page, 'image.moveLayer');

  // Drag the red square right, under the green bar above it.
  await holdDrag(page, 8, 8, 10, 2);
  const during = await shot(page);
  expect(during.equals(before), 'the preview shows the layer moved').toBe(false);
  expect(await calls(page, 'image.moveLayer'), 'the document is untouched').toBe(moves);

  await page.mouse.up();
  await expect.poll(() => calls(page, 'image.moveLayer')).toBe(moves + 1);
  const after = await shot(page);
  expect(after.equals(during), 'the preview equals the result').toBe(true);
  expect(await calls(page, 'image.movePreview'), 'handed over once per drag').toBe(1);

  // One undo step brings the picture back.
  await undoButton(page).click();
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
});

test('the layer above stays on top of the moved layer, whatever its blend mode (AC-03)', async ({
  page,
}) => {
  await scene(page);
  await page.getByLabel('Layer blend mode').selectOption({ label: 'Multiply' });
  const bar = async (): Promise<Buffer> => {
    const [x, y] = await point(page, 16, 0);
    const [x1, y1] = await point(page, 20, 32);
    return page.screenshot({ clip: { x: x + 1, y: y + 1, width: x1 - x - 2, height: y1 - y - 2 } });
  };
  const before = await bar();
  // The red square slides under the green bar: the bar looks the same.
  await holdDrag(page, 8, 8, 10, 0);
  expect((await bar()).equals(before)).toBe(true);
  await page.keyboard.press('Escape');
  await page.mouse.up();
});

test('a drag that comes back to its start changes nothing (AC-05)', async ({ page }) => {
  await scene(page);
  const before = await shot(page);
  const moves = await calls(page, 'image.moveLayer');
  await holdDrag(page, 8, 8, -20, 0);
  await page.mouse.move(...(await point(page, 8.5, 8.5)), { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
  expect(await calls(page, 'image.moveLayer')).toBe(moves);
});

for (const [how, end] of [
  ['Escape', (page: Page) => page.keyboard.press('Escape')],
  ['Undo', (page: Page) => page.keyboard.press('ControlOrMeta+z')],
  ['a tool switch', (page: Page) => page.keyboard.press('b')],
  [
    'the window losing focus',
    (page: Page) => page.evaluate(() => window.dispatchEvent(new Event('blur'))),
  ],
] as const) {
  test(`${how} during the drag drops it unapplied (AC-06)`, async ({ page }) => {
    await scene(page);
    const before = await shot(page);
    const moves = await calls(page, 'image.moveLayer');
    await holdDrag(page, 8, 8, 6, 6);
    await end(page);
    await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
    await page.mouse.up();
    // Undo only cancelled the drag: the last step (the deselect) stays,
    // so the picture is still the one before the drag.
    await page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    expect((await shot(page)).equals(before)).toBe(true);
    expect(await calls(page, 'image.moveLayer')).toBe(moves);
  });
}

test('Move Selected Pixels without a selection previews the whole layer (AC-07)', async ({
  page,
}) => {
  await scene(page);
  await pickMove(page, 'Move Selected Pixels');
  const before = await shot(page);
  await holdDrag(page, 8, 8, 6, 3);
  const during = await shot(page);
  expect(during.equals(before)).toBe(false);
  await page.mouse.up();
  await expect.poll(async () => (await shot(page)).equals(during)).toBe(true);
});

test('a group follows the drag and moves as one step (AC-08)', async ({ page }) => {
  await scene(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  const grip = page.getByRole('button', { name: 'Drag to reorder Layer 2', exact: true });
  const row = layerRow(page, 'Group 1');
  const a = await grip.boundingBox();
  const b = await row.boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height * 0.5, { steps: 6 });
  await page.mouse.up();
  await activate(page, 'Group 1');
  await pickMove(page, 'Move Layer');
  const before = await shot(page);
  const moves = await calls(page, 'image.moveLayer');
  await holdDrag(page, 8, 8, 5, 5);
  const during = await shot(page);
  expect(during.equals(before)).toBe(false);
  await page.mouse.up();
  await expect.poll(() => calls(page, 'image.moveLayer')).toBe(moves + 1);
  expect((await shot(page)).equals(during)).toBe(true);
});

test('a locked layer shows no preview and the notice (AC-09)', async ({ page }) => {
  await scene(page);
  await page.getByRole('button', { name: 'Lock Layer 2', exact: true }).click();
  const before = await shot(page);
  await holdDrag(page, 8, 8, 6, 6);
  await expect(byText(page, 'Layer “Layer 2” is locked', { exact: true })).toBeVisible();
  expect((await shot(page)).equals(before)).toBe(true);
  expect(await calls(page, 'image.movePreview')).toBe(0);
  await page.mouse.up();
});
