import { expect, test, type Locator, type Page } from '@playwright/test';
import { colorOf, createDocument, pixelCanvas, setColor } from './helpers';

// Pixel-mode tilesets and tilemap layers (docs/specs/tilemaps.md): the
// Tilesets panel, the Tile Stamp tool and the Tile editor. Composed pixels
// are read back with the Eyedropper; tile pixels and the hover grid are
// sampled test-side from the editor, thumbnail and overlay canvases (the
// app never reads them).

type Xy = readonly [number, number];

interface Editor {
  page: Page;
  /** Page coordinates of the centre of sprite pixel (px, py). */
  at: (px: number, py: number) => Xy;
}

/** A fresh `w` × `h` Pixel sprite with the Tilesets panel open. */
async function open(page: Page, w: number, h: number): Promise<Editor> {
  await createDocument(page, 'pixel', { width: w, height: h });
  await panel(page).getByRole('button', { name: 'Tilesets', exact: true }).click();
  await expect(panel(page).getByText('no tilesets yet')).toBeVisible();
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const at = (px: number, py: number): Xy => [
    box.x + ((px + 0.5) * box.width) / w,
    box.y + ((py + 0.5) * box.height) / h,
  ];
  return { page, at };
}

const panel = (page: Page) => page.getByRole('region', { name: 'Tilesets' });
const statusBar = (page: Page) => page.locator('footer', { hasText: /\d+×\d+/ });
const history = (page: Page, undo: number, redo = 0) =>
  expect(statusBar(page)).toContainText(`undo ${undo} / redo ${redo}`);
/** The undo depth the status bar shows. */
async function undoDepth(page: Page): Promise<number> {
  const text = (await statusBar(page).textContent()) ?? '';
  const m = /undo (\d+)/.exec(text);
  if (!m) throw new Error(`no undo depth in "${text}"`);
  return Number(m[1]);
}
const undo = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true }).click();
const redo = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true }).click();
const stampTool = (page: Page) =>
  page.getByRole('toolbar', { name: 'Tools' }).getByRole('button', { name: 'Tile Stamp' });
const activeTool = (page: Page) =>
  page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
const alpha = (page: Page) => page.getByLabel('Foreground alpha');

/** The form fields of the Add tileset form. */
function form(page: Page) {
  const f = panel(page).locator('form');
  return {
    form: f,
    name: f.getByLabel('Name', { exact: true }),
    w: f.getByLabel('Tile W', { exact: true }),
    h: f.getByLabel('Tile H', { exact: true }),
    add: f.getByRole('button', { name: 'Add', exact: true }),
    cancel: f.getByRole('button', { name: 'Cancel', exact: true }),
    error: f.getByRole('alert'),
  };
}

/** Adds a tileset through the form. */
async function addTileset(page: Page, name: string, w: number, h: number): Promise<void> {
  await panel(page).getByRole('button', { name: 'Add tileset' }).click();
  const f = form(page);
  await f.name.fill(name);
  await f.w.fill(String(w));
  await f.h.fill(String(h));
  await f.add.click();
  await expect(f.form).toBeHidden();
}

/** The panel row of the tileset called `name`. */
function tilesetRow(page: Page, name: string): Locator {
  return panel(page)
    .getByRole('listitem')
    .filter({ has: page.getByRole('button', { name: `Add tile to ${name}`, exact: true }) });
}

const plusTile = (page: Page, name: string) =>
  panel(page).getByRole('button', { name: `Add tile to ${name}`, exact: true });
const plusLayer = (page: Page, name: string) =>
  panel(page).getByRole('button', { name: `Add tilemap layer using ${name}`, exact: true });

/** The thumbnail button of tile `id` in tileset `name`. */
const tileButton = (page: Page, name: string, id: number) =>
  panel(page).getByRole('button', {
    name: `Select tile ${id} of ${name} (double-click to edit)`,
    exact: true,
  });

/** The tile ids the panel lists for tileset `name`, in panel order. */
async function listedTiles(page: Page, name: string): Promise<number[]> {
  const labels = await tilesetRow(page, name)
    .getByRole('button', { name: /^Select tile \d+ of / })
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
  return labels.map((l) => Number(/Select tile (\d+)/.exec(l)?.[1]));
}

/** RGBA of pixel (x, y) of a canvas element's backing store, read test-side. */
function canvasPixel(canvas: Locator, x: number, y: number): Promise<number[]> {
  return canvas.evaluate(
    (el, [px, py]) => {
      const ctx = (el as HTMLCanvasElement).getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      return Array.from(ctx.getImageData(px, py, 1, 1).data);
    },
    [x, y] as const,
  );
}

/** Every RGBA byte of a canvas element's backing store, read test-side. */
function canvasBytes(canvas: Locator): Promise<{ w: number; h: number; data: number[] }> {
  return canvas.evaluate((el) => {
    const c = el as HTMLCanvasElement;
    const ctx = c.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    return {
      w: c.width,
      h: c.height,
      data: Array.from(ctx.getImageData(0, 0, c.width, c.height).data),
    };
  });
}

/** The color of sprite pixel (px, py) on the current frame: `#rrggbb/alpha`. */
async function sample(ed: Editor, px: number, py: number): Promise<string> {
  await ed.page.keyboard.press('i');
  await ed.page.mouse.click(...ed.at(px, py));
  return `${await colorOf(ed.page, 'fg')}/${await alpha(ed.page).inputValue()}`;
}

const RED = '#ff0000/255';
const CLEAR = '#000000/0';

/** Opens the Tile editor on tile `id` of `name` and paints pixel (x, y) red. */
async function paintTilePixel(page: Page, name: string, id: number, x: number, y: number) {
  await setColor(page, 'fg', '#ff0000');
  await alpha(page).fill('255');
  await tileButton(page, name, id).dblclick();
  const editor = page.getByRole('dialog', { name: 'Tile editor' });
  const canvas = editor.getByLabel(`tile ${id} pixel editor`);
  const box = await canvas.boundingBox();
  const tile = await canvasBytes(canvas);
  if (!box) throw new Error('tile editor not visible');
  await page.mouse.click(
    box.x + ((x + 0.5) * box.width) / tile.w,
    box.y + ((y + 0.5) * box.height) / tile.h,
  );
  await expect.poll(() => canvasPixel(canvas, x, y)).toEqual([255, 0, 0, 255]);
  await editor.getByRole('button', { name: 'Done' }).click();
  await expect(editor).toBeHidden();
}

test('the Add tileset form adds a tileset with an empty tile 0 and refuses bad input (AC-01, AC-02, AC-03)', async ({
  page,
}) => {
  await open(page, 16, 16);
  const f = form(page);
  const addButton = panel(page).getByRole('button', { name: 'Add tileset' });

  // AC-02: the form opens with `Tileset` and 16 × 16.
  await addButton.click();
  await expect(f.name).toHaveValue('Tileset');
  await expect(f.w).toHaveValue('16');
  await expect(f.h).toHaveValue('16');

  // AC-03: an empty name, or a tile width / height that is not a positive
  // integer, shows an error and adds nothing.
  await f.name.fill('   ');
  await f.add.click();
  await expect(f.error).toHaveText('name is required');
  await f.name.fill('Ground');
  for (const bad of ['0', '-4', '2.5', '']) {
    await f.w.fill(bad);
    await f.add.click();
    await expect(f.error).toHaveText('tile width must be a positive integer');
  }
  await f.w.fill('4');
  for (const bad of ['0', '-1', '1.5', '']) {
    await f.h.fill(bad);
    await f.add.click();
    await expect(f.error).toHaveText('tile height must be a positive integer');
  }
  await expect(panel(page).getByText('no tilesets yet')).toBeVisible();
  await history(page, 0);

  // AC-02 (always): reopening after Cancel shows the defaults again.
  await f.cancel.click();
  await addButton.click();
  await expect(f.name).toHaveValue('Tileset');
  await expect(f.w).toHaveValue('16');
  await expect(f.h).toHaveValue('16');

  // AC-01: a valid form adds the tileset, listed with its id, tile size and
  // tile count — the one stored empty tile 0, which the panel does not show.
  await f.name.fill('Ground');
  await f.w.fill('4');
  await f.h.fill('3');
  await f.add.click();
  await expect(f.form).toBeHidden();
  const row = tilesetRow(page, 'Ground');
  await expect(row).toContainText('Ground');
  await expect(row).toContainText('id 0 · 4×3 · 1 tile');
  expect(await listedTiles(page, 'Ground')).toEqual([]);
  await history(page, 1);

  // AC-02 (always): after an add the form opens with the defaults again.
  await addButton.click();
  await expect(f.name).toHaveValue('Tileset');
  await expect(f.w).toHaveValue('16');
  await expect(f.h).toHaveValue('16');
  await f.cancel.click();

  // AC-01: one undo removes the tileset.
  await undo(page);
  await expect(row).toHaveCount(0);
  await expect(panel(page).getByText('no tilesets yet')).toBeVisible();
  await history(page, 0, 1);
});

test('+ Tile appends transparent tiles from id 1, picks them and lists them as thumbnails (AC-04, AC-05, AC-07, AC-08, AC-09)', async ({
  page,
}) => {
  await open(page, 16, 16);
  await addTileset(page, 'Ground', 4, 3);
  await history(page, 1);

  // AC-09: no pick yet — the Tile Stamp tool is disabled with its hint.
  await expect(stampTool(page)).toBeDisabled();
  await expect(stampTool(page)).toHaveAttribute(
    'title',
    'Tile Stamp — pick a tile in the Tilesets panel first',
  );

  // AC-04, AC-05: "+ Tile" appends tile 1, picks it and activates the Stamp tool.
  await page.keyboard.press('b');
  await expect(activeTool(page)).toHaveAccessibleName('Pencil');
  await plusTile(page, 'Ground').click();
  await expect(tileButton(page, 'Ground', 1)).toHaveAttribute('aria-pressed', 'true');
  await expect(stampTool(page)).toBeEnabled();
  await expect(activeTool(page)).toHaveAccessibleName('Tile Stamp');
  await expect(tilesetRow(page, 'Ground')).toContainText('id 0 · 4×3 · 2 tiles');
  await history(page, 2);

  await plusTile(page, 'Ground').click();
  await plusTile(page, 'Ground').click();
  await history(page, 4);
  // AC-07: tiles from id 1 in id order; tile 0 is neither shown nor pickable.
  expect(await listedTiles(page, 'Ground')).toEqual([1, 2, 3]);
  await expect(tileButton(page, 'Ground', 0)).toHaveCount(0);
  await expect(tileButton(page, 'Ground', 3)).toHaveAttribute('aria-pressed', 'true');
  await expect(tileButton(page, 'Ground', 1)).toHaveAttribute('aria-pressed', 'false');

  // AC-04: each new tile is fully transparent and of the tileset's tile size.
  for (const id of [1, 2, 3]) {
    const thumb = await canvasBytes(tileButton(page, 'Ground', id).locator('canvas'));
    expect([thumb.w, thumb.h]).toEqual([4, 3]);
    expect(thumb.data.every((b) => b === 0)).toBe(true);
  }

  // AC-04: one undo removes the last tile.
  await undo(page);
  expect(await listedTiles(page, 'Ground')).toEqual([1, 2]);
  await expect(tilesetRow(page, 'Ground')).toContainText('3 tiles');
  await history(page, 3, 1);

  // AC-08: clicking a thumbnail picks it (pressed) and activates the Stamp tool.
  await page.keyboard.press('b');
  await expect(activeTool(page)).toHaveAccessibleName('Pencil');
  await tileButton(page, 'Ground', 2).click();
  await expect(tileButton(page, 'Ground', 2)).toHaveAttribute('aria-pressed', 'true');
  await expect(tileButton(page, 'Ground', 1)).toHaveAttribute('aria-pressed', 'false');
  await expect(activeTool(page)).toHaveAccessibleName('Tile Stamp');
  await expect(stampTool(page)).toHaveAttribute('title', 'Tile Stamp — tile 2 of tileset 0');

  // AC-07: a thumbnail shows its tile's current pixels.
  await paintTilePixel(page, 'Ground', 2, 3, 1);
  const thumb = tileButton(page, 'Ground', 2).locator('canvas');
  await expect.poll(() => canvasPixel(thumb, 3, 1)).toEqual([255, 0, 0, 255]);
  expect(await canvasPixel(thumb, 0, 0)).toEqual([0, 0, 0, 0]);
  const other = await canvasBytes(tileButton(page, 'Ground', 1).locator('canvas'));
  expect(other.data.every((b) => b === 0)).toBe(true);
});

test('the Tile Stamp tool places the picked tile into the cell under the pointer (AC-10, AC-11)', async ({
  page,
}) => {
  const ed = await open(page, 8, 8);
  // Two frames, then two tilemap layers on one tileset: both get a grid on
  // both frames.
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
  await addTileset(page, 'Ground', 4, 4);
  await plusTile(page, 'Ground').click();
  await paintTilePixel(page, 'Ground', 1, 1, 2);
  await plusLayer(page, 'Ground').click();
  await plusLayer(page, 'Ground').click();
  await tileButton(page, 'Ground', 1).click();
  await expect(activeTool(page)).toHaveAccessibleName('Tile Stamp');
  const depth = await undoDepth(page);

  // Frame 2 is current. A click in sprite pixel (6, 1) places the tile into
  // column ⌊6 / 4⌋ = 1, row ⌊1 / 4⌋ = 0.
  await page.mouse.click(...ed.at(6, 1));
  await history(page, depth + 1);
  expect(await sample(ed, 4 + 1, 2)).toBe(RED);
  expect(await sample(ed, 1, 2)).toBe(CLEAR);
  expect(await sample(ed, 1, 4 + 2)).toBe(CLEAR);

  // It went into the topmost of the two tilemap layers: hiding it hides the tile.
  await page.getByRole('button', { name: 'Hide Ground Layer', exact: true }).first().click();
  expect(await sample(ed, 4 + 1, 2)).toBe(CLEAR);
  await page.getByRole('button', { name: 'Show Ground Layer', exact: true }).click();
  expect(await sample(ed, 4 + 1, 2)).toBe(RED);

  // Only the current frame changed.
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await expect(timeline.getByText('frame 1/2')).toBeVisible();
  expect(await sample(ed, 4 + 1, 2)).toBe(CLEAR);
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();

  // A drag places only the tile under the press: one more undo step (the
  // hide / show above were undo steps of their own).
  await tileButton(page, 'Ground', 1).click();
  const beforeDrag = await undoDepth(page);
  await page.mouse.move(...ed.at(1, 6));
  await page.mouse.down();
  await page.mouse.move(...ed.at(6, 6), { steps: 6 });
  await page.mouse.move(...ed.at(1, 1), { steps: 6 });
  await page.mouse.up();
  await history(page, beforeDrag + 1);
  expect(await sample(ed, 1, 4 + 2)).toBe(RED);
  expect(await sample(ed, 4 + 1, 4 + 2)).toBe(CLEAR);
  expect(await sample(ed, 1, 2)).toBe(CLEAR);

  // AC-11: one undo restores the cell's previous tile; the earlier
  // placement stays.
  await undo(page);
  await history(page, beforeDrag, 1);
  expect(await sample(ed, 1, 4 + 2)).toBe(CLEAR);
  expect(await sample(ed, 4 + 1, 2)).toBe(RED);
});

test('the Tile Stamp tool draws the tile grid and the hovered cell (AC-12)', async ({ page }) => {
  const ed = await open(page, 32, 32);
  await addTileset(page, 'Ground', 8, 8);
  const overlay = pixelCanvas(page).locator('xpath=following-sibling::canvas[1]');
  const alphaAt = async (x: number, y: number) => (await canvasPixel(overlay, x, y))[3] ?? 0;

  // Without the Stamp tool there is no grid.
  await page.mouse.move(...ed.at(12, 20));
  expect(await alphaAt(8, 4)).toBe(0);

  await plusTile(page, 'Ground').click();
  await expect(activeTool(page)).toHaveAccessibleName('Tile Stamp');
  // The pointer over sprite pixel (12, 20): cell (1, 2) spans 8..16 × 16..24.
  await page.mouse.move(...ed.at(12, 21));
  await page.mouse.move(...ed.at(12, 20));
  // Grid lines at the tile size (x = 8, y = 8), none in between.
  await expect.poll(() => alphaAt(8, 4)).toBeGreaterThan(0);
  expect(await alphaAt(4, 8)).toBeGreaterThan(0);
  expect(await alphaAt(4, 4)).toBe(0);
  expect(await alphaAt(12, 4)).toBe(0);
  const gridOnly = await alphaAt(8, 4);
  // The hovered cell's outline is drawn over the grid; its inside stays clear.
  expect(await alphaAt(8, 20)).toBeGreaterThan(gridOnly);
  expect(await alphaAt(12, 20)).toBe(0);

  // The outline follows the pointer to another cell.
  await page.mouse.move(...ed.at(28, 4));
  await expect.poll(() => alphaAt(8, 20)).toBe(gridOnly);
  expect(await alphaAt(24, 4)).toBeGreaterThan(gridOnly);

  // Off the canvas: the grid stays, the cell outline goes.
  await page.mouse.move(1, 1);
  await expect.poll(() => alphaAt(24, 4)).toBe(gridOnly);
  expect(await alphaAt(8, 4)).toBe(gridOnly);
});

test('stamping without a tilemap layer asks for one and changes nothing (AC-13)', async ({
  page,
}) => {
  const ed = await open(page, 8, 8);
  await addTileset(page, 'Ground', 4, 4);
  await plusTile(page, 'Ground').click();
  await paintTilePixel(page, 'Ground', 1, 1, 1);
  await history(page, 3);
  await page.mouse.click(...ed.at(1, 1));
  await expect(statusBar(page)).toContainText(
    'add a tilemap layer first (+ Layer in the Tilesets panel)',
  );
  await history(page, 3);
  expect(await sample(ed, 1, 1)).toBe(CLEAR);
});

test('the Tile editor paints the pixels of one tile, one undo step each (AC-16, AC-17)', async ({
  page,
}) => {
  await open(page, 16, 16);
  await addTileset(page, 'Ground', 4, 3);
  await plusTile(page, 'Ground').click();
  await history(page, 2);
  await setColor(page, 'fg', '#00ff00');
  await alpha(page).fill('255');

  await tileButton(page, 'Ground', 1).dblclick();
  const editor = page.getByRole('dialog', { name: 'Tile editor' });
  await expect(editor).toBeVisible();
  await expect(editor.getByRole('heading')).toHaveText('Editing tileset 0 · tile 1 · 4×3');
  const canvas = editor.getByLabel('tile 1 pixel editor');
  // 16 screen px per tile pixel.
  const box = await canvas.boundingBox();
  if (!box) throw new Error('tile editor not visible');
  expect(Math.round(box.width)).toBe(4 * 16);
  expect(Math.round(box.height)).toBe(3 * 16);
  const at = (x: number, y: number): Xy => [box.x + (x + 0.5) * 16, box.y + (y + 0.5) * 16];

  // A click paints the pixel under the pointer in the foreground colour.
  await page.mouse.click(...at(1, 0));
  await expect.poll(() => canvasPixel(canvas, 1, 0)).toEqual([0, 255, 0, 255]);
  expect(await canvasPixel(canvas, 0, 0)).toEqual([0, 0, 0, 0]);
  await history(page, 3);

  // A drag paints every pixel it passes, each paint its own undo step.
  const beforeDrag = await undoDepth(page);
  await page.mouse.move(...at(0, 2));
  await page.mouse.down();
  await page.mouse.move(...at(1, 2), { steps: 2 });
  await page.mouse.move(...at(2, 2), { steps: 2 });
  await page.mouse.move(...at(3, 2), { steps: 2 });
  await page.mouse.up();
  for (const x of [0, 1, 2, 3]) {
    await expect.poll(() => canvasPixel(canvas, x, 2)).toEqual([0, 255, 0, 255]);
  }
  expect(await canvasPixel(canvas, 0, 1)).toEqual([0, 0, 0, 0]);
  expect(await undoDepth(page)).toBeGreaterThanOrEqual(beforeDrag + 4);

  // AC-17: undo restores a painted pixel's previous colour — here a red
  // pixel painted over green, then a red pixel over transparent.
  await setColor(page, 'fg', '#ff0000');
  await page.mouse.click(...at(1, 0));
  await page.mouse.click(...at(3, 0));
  await expect.poll(() => canvasPixel(canvas, 3, 0)).toEqual([255, 0, 0, 255]);
  expect(await canvasPixel(canvas, 1, 0)).toEqual([255, 0, 0, 255]);
  await undo(page);
  await expect.poll(() => canvasPixel(canvas, 3, 0)).toEqual([0, 0, 0, 0]);
  expect(await canvasPixel(canvas, 1, 0)).toEqual([255, 0, 0, 255]);
  await undo(page);
  await expect.poll(() => canvasPixel(canvas, 1, 0)).toEqual([0, 255, 0, 255]);
  expect(await canvasPixel(canvas, 2, 2)).toEqual([0, 255, 0, 255]);
  await redo(page);
  await expect.poll(() => canvasPixel(canvas, 1, 0)).toEqual([255, 0, 0, 255]);

  // Done closes the editor.
  await editor.getByRole('button', { name: 'Done' }).click();
  await expect(editor).toBeHidden();
});

test('undoing the picked tile clears the stamp pick (AC-20)', async ({ page }) => {
  await open(page, 8, 8);
  await addTileset(page, 'Ground', 4, 4);
  await plusTile(page, 'Ground').click();
  await expect(stampTool(page)).toBeEnabled();
  await undo(page);
  await expect(tileButton(page, 'Ground', 1)).toHaveCount(0);
  await expect(stampTool(page)).toBeDisabled();
  await expect(stampTool(page)).toHaveAttribute(
    'title',
    'Tile Stamp — pick a tile in the Tilesets panel first',
  );
  // Redo brings the tile back, but not the pick.
  await redo(page);
  await expect(tileButton(page, 'Ground', 1)).toHaveAttribute('aria-pressed', 'false');
  await expect(stampTool(page)).toBeDisabled();
  await tileButton(page, 'Ground', 1).click();
  await expect(stampTool(page)).toBeEnabled();
});

test('stamping on a frame added after the tilemap layer creates its grid in one undo step (AC-21)', async ({
  page,
}) => {
  const ed = await open(page, 8, 8);
  await addTileset(page, 'Ground', 4, 4);
  await plusTile(page, 'Ground').click();
  await paintTilePixel(page, 'Ground', 1, 1, 2);
  await plusLayer(page, 'Ground').click();
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
  await history(page, 5);

  await tileButton(page, 'Ground', 1).click();
  await page.mouse.click(...ed.at(5, 5));
  await history(page, 6);
  await expect(statusBar(page)).not.toContainText('failed');
  expect(await sample(ed, 4 + 1, 4 + 2)).toBe(RED);

  // One undo removes the placement (and the grid it created).
  await undo(page);
  await history(page, 5, 1);
  expect(await sample(ed, 4 + 1, 4 + 2)).toBe(CLEAR);
  await redo(page);
  expect(await sample(ed, 4 + 1, 4 + 2)).toBe(RED);
});

test('the Tilesets panel speaks the UI language', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const tilesets = page.getByRole('region', { name: 'Tilesets' });
  await tilesets.getByRole('button', { name: 'Tilesets', exact: true }).click();
  await tilesets.getByRole('button', { name: 'Tileset hinzufügen' }).click();
  await tilesets.getByLabel('Kachel B', { exact: true }).fill('4');
  await tilesets.getByLabel('Kachel H', { exact: true }).fill('4');
  await tilesets.getByRole('button', { name: 'Hinzufügen', exact: true }).click();
  const row = tilesets.getByRole('listitem');
  await expect(row).toContainText('ID 0 · 4×4 · 1 Kachel');
  await expect(row.getByRole('button', { name: /Kachel zu .* hinzufügen/ })).toHaveText('+ Kachel');
  await expect(row.getByRole('button', { name: /Tilemap-Ebene/ })).toHaveText('+ Ebene');
  await row.getByRole('button', { name: /Kachel zu .* hinzufügen/ }).click();
  await expect(row.locator('canvas[title="Kachel 1"]')).toHaveCount(1);
});
