import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, pixelCanvas } from './helpers';

// Pixel-mode drawing tools (docs/specs/pixel-drawing-tools.md). Pixels are
// read back with the Eyedropper (left click: into the foreground color),
// never from the canvas.

type Xy = readonly [number, number];

interface Editor {
  page: Page;
  canvas: Locator;
  /** Page coordinates of the centre of sprite pixel (px, py). */
  at: (px: number, py: number) => Xy;
  box: { x: number; y: number; width: number; height: number };
}

async function open(page: Page, size = 16): Promise<Editor> {
  await createDocument(page, 'pixel', { width: size, height: size });
  const canvas = pixelCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');
  const at = (px: number, py: number): Xy => [
    box.x + ((px + 0.5) * box.width) / size,
    box.y + ((py + 0.5) * box.height) / size,
  ];
  return { page, canvas, at, box };
}

const fg = (page: Page) => page.getByLabel('Foreground color', { exact: true });
const bg = (page: Page) => page.getByLabel('Background color', { exact: true });
const alpha = (page: Page) => page.getByLabel('Foreground alpha');
const statusBar = (page: Page) => page.locator('footer', { hasText: /\d+×\d+/ });
const activeTool = (page: Page) =>
  page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');

/** Selects a tool by its rail key; `variant` picks a Shapes variant. */
async function pick(page: Page, key: string, variant?: string): Promise<void> {
  await page.keyboard.press(key);
  if (variant) {
    await page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name: variant, exact: true })
      .click();
  }
}

/** Opaque foreground and background colors for the next gesture. */
async function colors(page: Page, fore: string, back: string): Promise<void> {
  await fg(page).fill(fore);
  await bg(page).fill(back);
  await alpha(page).fill('255');
}

/** The color of sprite pixel (px, py): `#rrggbb/alpha`; leaves the Eyedropper active. */
async function sample(ed: Editor, px: number, py: number): Promise<string> {
  await pick(ed.page, 'i');
  await ed.page.mouse.click(...ed.at(px, py));
  return `${await fg(ed.page).inputValue()}/${await alpha(ed.page).inputValue()}`;
}

/** Presses at `from`, moves to each of `to` in one sample, releases. */
async function stroke(
  ed: Editor,
  from: Xy,
  to: Xy[],
  opts: { button?: 'left' | 'right'; shiftAtRelease?: boolean } = {},
): Promise<void> {
  const button = opts.button ?? 'left';
  await ed.page.mouse.move(...from);
  await ed.page.mouse.down({ button });
  for (const p of to) await ed.page.mouse.move(...p, { steps: 1 });
  if (opts.shiftAtRelease) await ed.page.keyboard.down('Shift');
  await ed.page.mouse.up({ button });
  if (opts.shiftAtRelease) await ed.page.keyboard.up('Shift');
}

/** Screenshot of the canvas with the pointer parked off it. */
async function shot(ed: Editor): Promise<Buffer> {
  await ed.page.mouse.move(1, 1);
  return ed.canvas.screenshot();
}

const RED = '#ff0000/255';
const BLUE = '#0000ff/255';
const GREEN = '#00ff00/255';
const CLEAR = '#000000/0';

test('pixel mode: each pencil or eraser stroke is one undo step', async ({ page }) => {
  const ed = await open(page);
  await colors(page, '#ff0000', '#ffffff');
  await pick(page, 'b');
  await stroke(ed, ed.at(1, 2), [ed.at(4, 2), ed.at(8, 2)]);
  await stroke(ed, ed.at(1, 5), [ed.at(4, 5), ed.at(8, 5)]);
  await expect(statusBar(page)).toContainText('undo 2 / redo 0');

  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  for (const x of [1, 4, 8]) expect(await sample(ed, x, 5)).toBe(CLEAR);
  for (const x of [1, 4, 6, 8]) expect(await sample(ed, x, 2)).toBe(RED);

  await pick(page, 'e');
  await stroke(ed, ed.at(1, 2), [ed.at(4, 2), ed.at(8, 2)]);
  await expect(statusBar(page)).toContainText('undo 2 / redo 0');
  for (const x of [1, 5, 8]) expect(await sample(ed, x, 2)).toBe(CLEAR);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  for (const x of [1, 4, 6, 8]) expect(await sample(ed, x, 2)).toBe(RED);
});

test('pixel mode: a stroke across the canvas edge paints only the on-canvas part', async ({
  page,
}) => {
  const ed = await open(page);
  const beyondRight: Xy = [ed.box.x + ed.box.width + 40, ed.at(0, 5)[1]];
  const beyondLeft: Xy = [ed.box.x - 40, ed.at(0, 5)[1]];
  await colors(page, '#00ff00', '#ffffff');
  await pick(page, 'b');
  // One sample from inside to well past the right edge, then back in.
  await stroke(ed, ed.at(9, 5), [beyondRight, ed.at(15, 9)]);
  for (const x of [9, 12, 15]) expect(await sample(ed, x, 5)).toBe(GREEN);
  expect(await sample(ed, 15, 9)).toBe(GREEN);
  await expect(statusBar(page)).not.toContainText('failed');

  // The eraser leaves past the left edge in one sample.
  await pick(page, 'e');
  await stroke(ed, ed.at(13, 5), [beyondLeft]);
  for (const x of [9, 12, 13]) expect(await sample(ed, x, 5)).toBe(CLEAR);
  expect(await sample(ed, 15, 5)).toBe(GREEN);
  await expect(statusBar(page)).not.toContainText('failed');
});

test('pixel mode: the right button paints the background color', async ({ page }) => {
  const ed = await open(page, 32);
  const right = { button: 'right' } as const;

  await colors(page, '#ff0000', '#0000ff');
  await pick(page, 'b');
  await stroke(ed, ed.at(2, 2), [], right);
  expect(await sample(ed, 2, 2)).toBe(BLUE);

  await colors(page, '#ff0000', '#0000ff');
  await pick(page, 'l');
  await stroke(ed, ed.at(4, 2), [ed.at(10, 2)], right);
  expect(await sample(ed, 7, 2)).toBe(BLUE);

  // Shapes: the outline Rectangle's edge, the filled variants' interior.
  for (const [variant, x0, probe] of [
    ['Rectangle', 2, [2, 6]],
    ['Filled Rectangle', 10, [13, 9]],
    ['Filled Ellipse', 18, [21, 9]],
  ] satisfies [string, number, Xy][]) {
    await colors(page, '#ff0000', '#0000ff');
    await pick(page, 'u', variant);
    await stroke(ed, ed.at(x0, 6), [ed.at(x0 + 6, 12)], right);
    expect(await sample(ed, probe[0], probe[1])).toBe(BLUE);
  }

  // The Eyedropper's right button sets the background color.
  await colors(page, '#00ff00', '#ffffff');
  await pick(page, 'b');
  await stroke(ed, ed.at(20, 2), []);
  await colors(page, '#ffffff', '#ffffff');
  await pick(page, 'i');
  await stroke(ed, ed.at(20, 2), [], right);
  await expect(bg(page)).toHaveValue('#00ff00');
  await expect(fg(page)).toHaveValue('#ffffff');

  // Other tools ignore the right button: the eraser leaves the pixel,
  // the Text tool opens no dialog.
  await pick(page, 'e');
  await stroke(ed, ed.at(20, 2), [ed.at(22, 2)], right);
  expect(await sample(ed, 20, 2)).toBe(GREEN);
  await pick(page, 't');
  await stroke(ed, ed.at(26, 26), [], right);
  await page.waitForTimeout(300);
  await expect(page.getByRole('dialog', { name: 'Text' })).toHaveCount(0);

  // The Bucket last: it floods the empty area with the background color.
  await colors(page, '#ff0000', '#0000ff');
  await pick(page, 'g');
  await stroke(ed, ed.at(30, 30), [], right);
  expect(await sample(ed, 30, 30)).toBe(BLUE);
  expect(await sample(ed, 0, 31)).toBe(BLUE);
  expect(await sample(ed, 20, 2)).toBe(GREEN);
});

test('pixel mode: Shift at release snaps a line by the 2:1 rule', async ({ page }) => {
  const ed = await open(page);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const cases: { from: Xy; to: Xy; end: Xy }[] = [
    // |dx| > 2|dy|: horizontal.
    { from: [2, 2], to: [13, 6], end: [13, 2] },
    // |dx| = 2|dy| is not more than twice: 45°, length max(|dx|, |dy|).
    { from: [2, 2], to: [12, 7], end: [12, 12] },
    // |dy| > 2|dx|: vertical.
    { from: [2, 2], to: [5, 13], end: [2, 13] },
    // Up-left: 45° the other way.
    { from: [13, 13], to: [4, 7], end: [4, 4] },
  ];
  for (const { from, to, end } of cases) {
    await colors(page, '#ff0000', '#ffffff');
    await pick(page, 'l');
    await stroke(ed, ed.at(...from), [ed.at(...to)], { shiftAtRelease: true });
    expect(await sample(ed, ...end), `snapped end of ${to}`).toBe(RED);
    expect(await sample(ed, ...to), `pointer ${to} not reached`).toBe(CLEAR);
    await undo.click();
  }

  // Shift let go before the release: the line follows the pointer.
  await colors(page, '#ff0000', '#ffffff');
  await pick(page, 'l');
  await page.mouse.move(...ed.at(2, 2));
  await page.mouse.down();
  await page.keyboard.down('Shift');
  await page.mouse.move(...ed.at(13, 6), { steps: 2 });
  await page.keyboard.up('Shift');
  await page.mouse.up();
  expect(await sample(ed, 13, 6)).toBe(RED);
  expect(await sample(ed, 13, 2)).toBe(CLEAR);
});

test('pixel mode: Shift at release squares every shape to the larger side', async ({ page }) => {
  test.setTimeout(90_000);
  const ed = await open(page);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const status = statusBar(page);
  /** Draws from (2, 2) to `to`, screenshots the result and undoes it. */
  const draw = async (to: Xy, shift: boolean): Promise<Buffer> => {
    await stroke(ed, ed.at(2, 2), [ed.at(...to)], { shiftAtRelease: shift });
    await expect(status).toContainText('undo 1 / redo 0');
    const image = await shot(ed);
    await undo.click();
    await expect(status).toContainText('undo 0 / redo 1');
    return image;
  };
  for (const variant of [
    'Rectangle',
    'Filled Rectangle',
    'Ellipse',
    'Filled Ellipse',
    'Rounded Rectangle',
    'Polygon',
  ]) {
    await colors(page, '#ff0000', '#0000ff');
    await pick(page, 'u', variant);
    const square = await draw([12, 12], false);
    const wide = await draw([12, 6], false);
    expect(wide.equals(square), `${variant}: the free drag is not square`).toBe(false);
    expect((await draw([12, 6], true)).equals(square), `${variant}: wide drag`).toBe(true);
    expect((await draw([5, 12], true)).equals(square), `${variant}: tall drag`).toBe(true);
  }
});

test('pixel mode: shapes stroke with one color and fill with the other', async ({ page }) => {
  const ed = await open(page);
  await colors(page, '#ff0000', '#0000ff');
  await pick(page, 'u', 'Rectangle');
  await page.getByLabel('Shape mode').selectOption('fill_outline');

  // Left button: stroke = foreground, fill = background.
  await stroke(ed, ed.at(1, 1), [ed.at(7, 7)]);
  expect(await sample(ed, 1, 1)).toBe(RED);
  expect(await sample(ed, 7, 4)).toBe(RED);
  expect(await sample(ed, 4, 4)).toBe(BLUE);

  // Right button: swapped.
  await colors(page, '#ff0000', '#0000ff');
  await pick(page, 'u', 'Rectangle');
  await stroke(ed, ed.at(9, 1), [ed.at(15, 7)], { button: 'right' });
  expect(await sample(ed, 9, 1)).toBe(BLUE);
  expect(await sample(ed, 15, 4)).toBe(BLUE);
  expect(await sample(ed, 12, 4)).toBe(RED);
});

test('pixel mode: the Text tool opens its dialog at the clicked pixel', async ({ page }) => {
  const ed = await open(page);
  const dialog = page.getByRole('dialog', { name: 'Text' });
  await pick(page, 't');
  await page.mouse.click(...ed.at(5, 3));
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('at 5, 3');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await page.mouse.click(...ed.at(9, 12));
  await expect(dialog).toContainText('at 9, 12');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
});

test('pixel mode: text previews live and places with its options as one undo step', async ({
  page,
}) => {
  test.setTimeout(90_000);
  const ed = await open(page, 48);
  const dialog = page.getByRole('dialog', { name: 'Text' });
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const status = statusBar(page);
  const blank = await shot(ed);

  /** Opens the dialog at (8, 6), types "Hi" and lets `setup` set options. */
  const type = async (setup: () => Promise<void> = async () => {}) => {
    await pick(page, 't');
    await page.mouse.click(...ed.at(8, 6));
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('Text content').fill('Hi');
    await setup();
  };
  /** Places the typed text, screenshots it, undoes it. */
  const place = async (): Promise<Buffer> => {
    await dialog.getByRole('button', { name: 'Place' }).click();
    await expect(dialog).toBeHidden();
    await expect(status).toContainText('undo 1 / redo 0');
    await expect(status).toContainText('text placed');
    const image = await shot(ed);
    await undo.click();
    await expect(status).toContainText('undo 0 / redo 1');
    return image;
  };

  // The live preview draws the text without touching the document.
  await colors(page, '#ff0000', '#0000ff');
  await type();
  await expect.poll(async () => (await shot(ed)).equals(blank)).toBe(false);
  const preview = await shot(ed);
  await expect(status).toContainText('undo 0 / redo 0');
  await expect(undo).toBeDisabled();
  const plain = await place();
  expect(plain.equals(preview), 'placed text matches its preview').toBe(true);
  expect((await shot(ed)).equals(blank), 'undo removes it').toBe(true);

  // Every option reaches the placed text.
  const variants: [string, () => Promise<void>][] = [
    ['bold', () => dialog.getByLabel('Bold').check()],
    ['italic', () => dialog.getByLabel('Italic').check()],
    ['anti-alias', () => dialog.getByLabel('Anti-alias').check()],
    ['size', () => dialog.getByLabel('Font size').fill('20')],
    [
      'alignment',
      async () => {
        await dialog.getByLabel('Text alignment').selectOption('center');
      },
    ],
  ];
  for (const [name, setup] of variants) {
    await colors(page, '#ff0000', '#0000ff');
    await type(setup);
    expect((await place()).equals(plain), `${name} changes the text`).toBe(false);
  }
  // The foreground color, not the background, paints it.
  await colors(page, '#00ff00', '#0000ff');
  await type();
  expect((await place()).equals(plain), 'foreground color').toBe(false);
  await colors(page, '#ff0000', '#00ff00');
  await type();
  expect((await place()).equals(plain), 'background color is unused').toBe(true);
});

test('pixel mode: a locked layer refuses paint and says so until a press is accepted', async ({
  page,
}) => {
  const ed = await open(page);
  await colors(page, '#00ff00', '#ffffff');
  await pick(page, 'b');
  await stroke(ed, ed.at(4, 4), []);
  const lock = page.getByRole('button', { name: /^Lock / });
  const name = ((await lock.getAttribute('aria-label')) ?? '').replace(/^Lock /, '');
  await lock.click();
  const status = statusBar(page);
  const notice = status.getByText(`“${name}” is locked`);

  await colors(page, '#ff0000', '#ffffff');
  await pick(page, 'b');
  await stroke(ed, ed.at(4, 4), [ed.at(6, 4)]);
  await expect(notice).toBeVisible();
  await stroke(ed, ed.at(4, 6), []);
  await expect(notice).toBeVisible();
  await expect(status).toContainText('undo 2 / redo 0');
  // A press the lock does not refuse (the Eyedropper) clears the notice.
  expect(await sample(ed, 4, 4)).toBe(GREEN);
  await expect(notice).toHaveCount(0);
  expect(await sample(ed, 6, 4)).toBe(CLEAR);
  expect(await sample(ed, 4, 6)).toBe(CLEAR);

  await colors(page, '#ff0000', '#ffffff');
  await pick(page, 'b');
  await stroke(ed, ed.at(4, 4), []);
  await expect(notice).toBeVisible();
  await page.getByRole('button', { name: `Unlock ${name}`, exact: true }).click();
  await expect(notice).toBeVisible();
  await stroke(ed, ed.at(4, 4), []);
  await expect(notice).toHaveCount(0);
  expect(await sample(ed, 4, 4)).toBe(RED);
});

test('pixel mode: single keys select the drawing tools and cycle the shapes', async ({ page }) => {
  await open(page);
  const tool = activeTool(page);
  for (const [key, label] of [
    ['b', 'Pencil'],
    ['e', 'Eraser'],
    ['g', 'Fill'],
    ['i', 'Eyedropper'],
    ['l', 'Line'],
    ['t', 'Text'],
  ] as const) {
    await page.keyboard.press(key);
    await expect(tool).toHaveAttribute('aria-label', label);
  }
  for (const label of [
    'Rectangle',
    'Filled Rectangle',
    'Ellipse',
    'Filled Ellipse',
    'Rounded Rectangle',
    'Polygon',
    'Rectangle',
  ]) {
    await page.keyboard.press('u');
    await expect(tool).toHaveAttribute('aria-label', `Shapes: ${label}`);
  }

  // With Ctrl / Cmd / Alt the key selects nothing.
  for (const chord of ['Control+b', 'Meta+b', 'Alt+b', 'Control+e', 'Alt+l']) {
    await page.keyboard.press(chord);
    await expect(tool, chord).toHaveAttribute('aria-label', 'Shapes: Rectangle');
  }
  // Typing in a text field selects nothing.
  await page.getByLabel('Stroke width').focus();
  await page.keyboard.press('b');
  await page.keyboard.press('e');
  await expect(tool).toHaveAttribute('aria-label', 'Shapes: Rectangle');
});
