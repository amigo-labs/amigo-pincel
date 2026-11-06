import { expect, test, type Page } from '@playwright/test';
import { colorOf, createDocument, menu, pixelCanvas, setColor } from './helpers';

// Symmetry drawing and tiled mode (docs/specs/symmetry-tiled-mode.md).
// Pixels are read back with the Eyedropper (the app never reads canvas
// pixels); the tiled copies are checked through the DOM and screenshots.

const N = 16;
type At = (px: number, py: number) => readonly [number, number];

/** A fresh `w`×`h` sprite; returns the sprite-pixel → page-point mapper. */
async function setup(page: Page, w = N, h = N): Promise<At> {
  await createDocument(page, 'pixel', { width: w, height: h });
  return mapper(page, w, h);
}

/** Sprite pixel → page point for the canvas as it is laid out now. */
async function mapper(page: Page, w: number, h: number): Promise<At> {
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return (px, py) =>
    [box.x + ((px + 0.5) * box.width) / w, box.y + ((py + 0.5) * box.height) / h] as const;
}

const symmetry = (page: Page) => page.getByLabel('Symmetry', { exact: true });
const axisX = (page: Page) => page.getByTestId('symmetry-axis-x');
const axisY = (page: Page) => page.getByTestId('symmetry-axis-y');
const handleX = (page: Page) => page.getByRole('button', { name: 'Vertical symmetry axis' });
const handleY = (page: Page) => page.getByRole('button', { name: 'Horizontal symmetry axis' });
const copies = (page: Page) => page.getByTestId('tile-copy');

async function tool(page: Page, title: RegExp): Promise<void> {
  await page.getByTitle(title).first().click();
}

/** Sample sprite pixel (x, y) with the Eyedropper; `#000000` is transparent. */
async function pick(page: Page, at: At, x: number, y: number): Promise<string> {
  await setColor(page, 'fg', '#000000');
  await tool(page, /^Eyedropper/);
  await page.mouse.click(...at(x, y));
  return colorOf(page, 'fg');
}

/** Press at the first point, move through the rest one sample each, release. */
async function stroke(page: Page, at: At, pts: [number, number][]): Promise<void> {
  const [first, ...rest] = pts;
  if (!first) return;
  await page.mouse.move(...at(...first));
  await page.mouse.down();
  for (const p of rest) await page.mouse.move(...at(...p), { steps: 1 });
  await page.mouse.up();
}

/** Pencil with opaque `#ff0000` (an Eyedropper pick of a transparent pixel zeroes the alpha). */
async function redPencil(page: Page): Promise<void> {
  await tool(page, /^Pencil/);
  await opaque(page, '#ff0000');
}

/** Foreground `color` at full alpha. */
async function opaque(page: Page, color: string): Promise<void> {
  await setColor(page, 'fg', color);
  await page.getByLabel('Foreground alpha').fill('255');
}

/** Every pixel of `pts` reads `want`. */
async function expectColors(
  page: Page,
  at: At,
  pts: [number, number][],
  want: string,
): Promise<void> {
  for (const [x, y] of pts) expect(await pick(page, at, x, y), `pixel (${x}, ${y})`).toBe(want);
}

/** Opens View ▸ Tiled Mode and picks `choice`. */
async function tiled(page: Page, choice: 'None' | 'X' | 'Y' | 'Both'): Promise<void> {
  await menu(page, 'View', 'Tiled Mode');
  await tiledMenu(page).getByRole('menuitemcheckbox', { name: choice, exact: true }).click();
}

const tiledMenu = (page: Page) => page.getByRole('menu', { name: 'Tiled Mode', exact: true });

test.describe('symmetry', () => {
  test('pencil and eraser offer none, left-right, top-bottom and both', async ({ page }) => {
    await setup(page);
    for (const name of [/^Pencil/, /^Eraser/]) {
      await tool(page, name);
      await expect(symmetry(page)).toBeVisible();
      await expect(symmetry(page).locator('option')).toHaveText([
        'None',
        'Left–right',
        'Top–bottom',
        'Both',
      ]);
      await expect(symmetry(page)).toHaveValue('none');
    }
  });

  test('a left-right stroke paints the mirror image across the centre axis', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await symmetry(page).selectOption('x');
    await stroke(page, at, [
      [1, 2],
      [3, 2],
    ]);
    // Width 16: column x mirrors to 15 − x.
    await expectColors(
      page,
      at,
      [
        [1, 2],
        [2, 2],
        [3, 2],
        [14, 2],
        [13, 2],
        [12, 2],
      ],
      '#ff0000',
    );
    await expectColors(
      page,
      at,
      [
        [4, 2],
        [11, 2],
      ],
      '#000000',
    );
  });

  test('top-bottom and both mirror across the horizontal axis too', async ({ page }) => {
    const at = await setup(page, N, 10);
    await redPencil(page);
    await symmetry(page).selectOption('y');
    await page.mouse.click(...at(2, 1));
    await expectColors(
      page,
      at,
      [
        [2, 1],
        [2, 8],
      ],
      '#ff0000',
    );
    await expectColors(
      page,
      at,
      [
        [13, 1],
        [13, 8],
      ],
      '#000000',
    );
    await redPencil(page);
    await symmetry(page).selectOption('xy');
    await page.mouse.click(...at(5, 3));
    await expectColors(
      page,
      at,
      [
        [5, 3],
        [10, 3],
        [5, 6],
        [10, 6],
      ],
      '#ff0000',
    );
  });

  test('an odd width puts the axis through the middle pixel', async ({ page }) => {
    const at = await setup(page, 15, 4);
    await redPencil(page);
    await symmetry(page).selectOption('x');
    await page.mouse.click(...at(7, 1));
    await page.mouse.click(...at(2, 2));
    await expectColors(
      page,
      at,
      [
        [7, 1],
        [2, 2],
        [12, 2],
      ],
      '#ff0000',
    );
    await expectColors(
      page,
      at,
      [
        [6, 1],
        [8, 1],
      ],
      '#000000',
    );
  });

  test('a symmetric stroke is one undo step', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await symmetry(page).selectOption('xy');
    await stroke(page, at, [
      [1, 1],
      [4, 3],
    ]);
    await page.keyboard.press('ControlOrMeta+z');
    await expectColors(
      page,
      at,
      [
        [1, 1],
        [14, 1],
        [1, 14],
        [14, 14],
        [4, 3],
        [11, 12],
      ],
      '#000000',
    );
  });

  test('the eraser mirrors too, and line and bucket do not', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await stroke(page, at, [
      [0, 5],
      [15, 5],
    ]);
    await tool(page, /^Eraser/);
    await symmetry(page).selectOption('x');
    await page.mouse.click(...at(2, 5));
    await expectColors(
      page,
      at,
      [
        [2, 5],
        [13, 5],
      ],
      '#000000',
    );
    await expectColors(
      page,
      at,
      [
        [3, 5],
        [12, 5],
      ],
      '#ff0000',
    );
    // Line: only the drawn pixels.
    await tool(page, /^Line/);
    await opaque(page, '#00ff00');
    await stroke(page, at, [
      [1, 9],
      [3, 9],
    ]);
    await expectColors(
      page,
      at,
      [
        [1, 9],
        [3, 9],
      ],
      '#00ff00',
    );
    await expectColors(
      page,
      at,
      [
        [14, 9],
        [12, 9],
      ],
      '#000000',
    );
    // Shapes: only the drawn rectangle.
    await tool(page, /^Shapes/);
    await opaque(page, '#00ff00');
    await stroke(page, at, [
      [1, 1],
      [3, 2],
    ]);
    await expectColors(
      page,
      at,
      [
        [1, 1],
        [3, 2],
      ],
      '#00ff00',
    );
    await expectColors(
      page,
      at,
      [
        [14, 1],
        [12, 2],
      ],
      '#000000',
    );
    // Bucket: fills the clicked region only — a closed box on the left.
    await redPencil(page);
    await symmetry(page).selectOption('none');
    await stroke(page, at, [
      [0, 11],
      [3, 11],
      [3, 14],
      [0, 14],
      [0, 11],
    ]);
    await symmetry(page).selectOption('x');
    await tool(page, /^Paint Bucket|^Fill/);
    await opaque(page, '#0000ff');
    await page.mouse.click(...at(1, 12));
    await expectColors(page, at, [[1, 12]], '#0000ff');
    await expectColors(page, at, [[14, 12]], '#000000');
  });

  test('axis lines show with pencil and eraser, handles with any tool', async ({ page }) => {
    await setup(page);
    await redPencil(page);
    await expect(axisX(page)).toHaveCount(0);
    await expect(handleX(page)).toHaveCount(0);
    await symmetry(page).selectOption('x');
    await expect(axisX(page)).toBeVisible();
    await expect(axisY(page)).toHaveCount(0);
    await expect(handleX(page)).toBeVisible();
    await expect(handleY(page)).toHaveCount(0);
    await symmetry(page).selectOption('xy');
    await expect(axisY(page)).toBeVisible();
    await tool(page, /^Line/);
    await expect(axisX(page)).toHaveCount(0);
    await expect(handleX(page)).toBeVisible();
    await expect(handleY(page)).toBeVisible();
    await tool(page, /^Eraser/);
    await expect(axisX(page)).toBeVisible();
    await symmetry(page).selectOption('none');
    await expect(axisX(page)).toHaveCount(0);
    await expect(handleX(page)).toHaveCount(0);
  });

  test('dragging a handle moves the axis; double-click resets it', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await symmetry(page).selectOption('x');
    const box = await handleX(page).boundingBox();
    if (!box) throw new Error('no handle');
    // Drag from the centre (x = 8 px) to the edge between columns 11 and 12.
    const [x12] = at(12, 0);
    const cell = at(1, 0)[0] - at(0, 0)[0];
    await tool(page, /^Line/);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(x12 - cell / 2, box.y + box.height / 2, { steps: 4 });
    await page.mouse.up();
    await expect(handleX(page)).toHaveAttribute('data-axis', '24');
    await redPencil(page);
    await page.mouse.click(...at(10, 4));
    // A = 24: column 10 mirrors to 13.
    await expectColors(
      page,
      at,
      [
        [10, 4],
        [13, 4],
      ],
      '#ff0000',
    );
    // A mirror image beyond the canvas is skipped.
    await redPencil(page);
    await page.mouse.click(...at(2, 6));
    await expectColors(page, at, [[2, 6]], '#ff0000');
    await handleX(page).dblclick();
    await expect(handleX(page)).toHaveAttribute('data-axis', '16');
    await redPencil(page);
    await page.mouse.click(...at(10, 8));
    await expectColors(
      page,
      at,
      [
        [10, 8],
        [5, 8],
      ],
      '#ff0000',
    );
  });

  test('a press inside the canvas never moves an axis', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await symmetry(page).selectOption('xy');
    await tool(page, /^Line/);
    await stroke(page, at, [
      [8, 0],
      [12, 0],
    ]);
    await expect(handleX(page)).toHaveAttribute('data-axis', '16');
    await expect(handleY(page)).toHaveAttribute('data-axis', '16');
  });

  test('an unmoved axis follows a resize and another document resets a moved one', async ({
    page,
  }) => {
    await setup(page);
    await redPencil(page);
    await symmetry(page).selectOption('xy');
    const resize = async (w: number, h: number) => {
      await menu(page, 'Image', 'Resize Canvas…');
      const dialog = page.getByRole('dialog', { name: 'Resize canvas' });
      await dialog.getByLabel('Width').fill(String(w));
      await dialog.getByLabel('Height').fill(String(h));
      await dialog.getByRole('button', { name: 'Resize' }).click();
      await expect(dialog).toBeHidden();
    };
    // Unmoved: follows the centre, through column 7 at width 15.
    await resize(15, 16);
    await expect(handleX(page)).toHaveAttribute('data-axis', '15');
    await expect(handleY(page)).toHaveAttribute('data-axis', '16');
    // Moved to the right edge, then shrunk: clamped to the new edge.
    const box = await handleX(page).boundingBox();
    const canvas = await pixelCanvas(page).boundingBox();
    if (!box || !canvas) throw new Error('not laid out');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(canvas.x + canvas.width + 40, box.y + box.height / 2, { steps: 4 });
    await page.mouse.up();
    await expect(handleX(page)).toHaveAttribute('data-axis', '30');
    await resize(10, 16);
    await expect(handleX(page)).toHaveAttribute('data-axis', '20');
    await resize(14, 16);
    await expect(handleX(page)).toHaveAttribute('data-axis', '20');
    // Another document: both axes back at the centre.
    await createDocument(page, 'pixel', { width: 12, height: 6 });
    await redPencil(page);
    await symmetry(page).selectOption('xy');
    await expect(handleX(page)).toHaveAttribute('data-axis', '12');
    await expect(handleY(page)).toHaveAttribute('data-axis', '6');
  });
});

test.describe('tiled mode', () => {
  test('View ▸ Tiled Mode offers None, X, Y and Both, None by default', async ({ page }) => {
    await setup(page);
    await menu(page, 'View', 'Tiled Mode');
    const sub = tiledMenu(page);
    const names = await sub
      .getByRole('menuitemcheckbox')
      .evaluateAll((els) => els.map((e) => e.getAttribute('aria-label') ?? e.textContent?.trim()));
    expect(names.map((n) => n?.replace(/^\W+/, ''))).toEqual(['None', 'X', 'Y', 'Both']);
    await expect(sub.getByRole('menuitemcheckbox', { name: 'None' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await sub.getByRole('menuitemcheckbox', { name: 'Y' }).click();
    await menu(page, 'View', 'Tiled Mode');
    await expect(sub.getByRole('menuitemcheckbox', { name: 'Y' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  test('the frame repeats at the neighbouring offsets', async ({ page }) => {
    const at = await setup(page, 8, 8);
    await expect(copies(page)).toHaveCount(0);
    const offsets = async () =>
      (
        await copies(page).evaluateAll((els) => els.map((e) => e.getAttribute('data-offset')))
      ).sort();
    await tiled(page, 'X');
    expect(await offsets()).toEqual(['-1,0', '1,0']);
    await tiled(page, 'Y');
    expect(await offsets()).toEqual(['0,-1', '0,1']);
    await tiled(page, 'Both');
    expect(await offsets()).toEqual(['-1,-1', '-1,0', '-1,1', '0,-1', '0,1', '1,-1', '1,0', '1,1']);
    // Each copy sits one canvas away and shows the painted pixel.
    const canvasBox = await pixelCanvas(page).boundingBox();
    const right = copies(page).and(page.locator('[data-offset="1,0"]'));
    const copyBox = await right.boundingBox();
    if (!canvasBox || !copyBox) throw new Error('not laid out');
    expect(Math.abs(copyBox.x - canvasBox.x - canvasBox.width)).toBeLessThan(3);
    expect(Math.abs(copyBox.y - canvasBox.y)).toBeLessThan(3);
    expect(Math.abs(copyBox.width - canvasBox.width)).toBeLessThan(3);
    // A stroke on the canvas shows up in the copy.
    const blank = await right.screenshot();
    await redPencil(page);
    await page.mouse.click(...at(1, 1));
    await expect.poll(async () => (await right.screenshot()).equals(blank)).toBe(false);
    await tiled(page, 'None');
    await expect(copies(page)).toHaveCount(0);
  });

  test('a stroke past the right edge continues at the left edge', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await tiled(page, 'X');
    await stroke(page, at, [
      [13, 4],
      [18, 4],
    ]);
    await expectColors(
      page,
      at,
      [
        [13, 4],
        [14, 4],
        [15, 4],
        [0, 4],
        [1, 4],
        [2, 4],
      ],
      '#ff0000',
    );
    await expectColors(page, at, [[3, 4]], '#000000');
    // Undo restores both sides of the seam in one step.
    await page.keyboard.press('ControlOrMeta+z');
    await expectColors(
      page,
      at,
      [
        [14, 4],
        [1, 4],
      ],
      '#000000',
    );
  });

  test('pressing on a repeated copy paints the corresponding canvas pixel', async ({ page }) => {
    await setup(page);
    await redPencil(page);
    await tiled(page, 'Both');
    // Zoom out so the copies around the canvas are on screen.
    await page.keyboard.press('-');
    await page.keyboard.press('-');
    const at = await mapper(page, N, N);
    await page.mouse.click(...at(16 + 3, 5));
    await page.mouse.click(...at(4, -16 + 6));
    await page.mouse.click(...at(-16 + 9, 16 + 2));
    await expectColors(
      page,
      at,
      [
        [3, 5],
        [4, 6],
        [9, 2],
      ],
      '#ff0000',
    );
  });

  test('an axis tiled mode does not cover clips the stroke', async ({ page }) => {
    const at = await setup(page);
    await redPencil(page);
    await tiled(page, 'Y');
    await stroke(page, at, [
      [13, 4],
      [18, 4],
    ]);
    await expectColors(page, at, [[15, 4]], '#ff0000');
    await expectColors(
      page,
      at,
      [
        [0, 4],
        [1, 4],
      ],
      '#000000',
    );
  });

  test('line, shapes and bucket clip at the edges in tiled mode', async ({ page }) => {
    const at = await setup(page);
    await tiled(page, 'X');
    await tool(page, /^Line/);
    await opaque(page, '#00ff00');
    await stroke(page, at, [
      [12, 7],
      [19, 7],
    ]);
    await expectColors(
      page,
      at,
      [
        [12, 7],
        [15, 7],
      ],
      '#00ff00',
    );
    await expectColors(
      page,
      at,
      [
        [0, 7],
        [2, 7],
      ],
      '#000000',
    );
    await tool(page, /^Shapes/);
    await opaque(page, '#00ff00');
    await stroke(page, at, [
      [13, 1],
      [18, 3],
    ]);
    await expectColors(
      page,
      at,
      [
        [13, 1],
        [15, 1],
      ],
      '#00ff00',
    );
    await expectColors(
      page,
      at,
      [
        [0, 1],
        [2, 3],
      ],
      '#000000',
    );
    // A bucket press on a repeated copy does nothing.
    await tool(page, /^Fill/);
    await opaque(page, '#0000ff');
    await page.mouse.click(...at(16 + 4, 12));
    await expectColors(page, at, [[4, 12]], '#000000');
  });

  test('tiled mode never changes the document and a reload starts with None', async ({ page }) => {
    const at = await setup(page, 8, 8);
    await redPencil(page);
    await page.mouse.click(...at(2, 3));
    await page.mouse.move(0, 0);
    const before = await pixelCanvas(page).screenshot();
    await tiled(page, 'Both');
    await expect(copies(page)).toHaveCount(8);
    await page.mouse.move(0, 0);
    // The composited frame is unchanged, and so is the undo history.
    expect((await pixelCanvas(page).screenshot()).equals(before)).toBe(true);
    await page.keyboard.press('ControlOrMeta+z');
    await expectColors(page, at, [[2, 3]], '#000000');
    await page.reload();
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    await expect(copies(page)).toHaveCount(0);
    await menu(page, 'View', 'Tiled Mode');
    await expect(tiledMenu(page).getByRole('menuitemcheckbox', { name: 'None' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });
});
