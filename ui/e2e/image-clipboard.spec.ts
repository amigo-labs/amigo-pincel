import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  putClipboardImage,
  waitForEditor,
  type ClipImage,
} from './helpers';

// Image mode: Edit ▸ Cut / Copy / Copy Merged / Paste, and copy / paste
// between the two modes (spec §5.4, docs/specs/copy-cut-paste.md).
// Clipboard images are decoded test-side; the app never reads canvas pixels.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];
const CLEAR = [0, 0, 0, 0];
const SIZE = 32;

const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

/** Screen point of the document position `(x, y)` (pixel corners at integers). */
async function point(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

/** Rectangle Select of the document rect `(x, y, w, h)`. */
async function select(page: Page, x: number, y: number, w: number, h: number): Promise<void> {
  await page.keyboard.press('m');
  await page.mouse.move(...(await point(page, x, y)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, x + w, y + h)), { steps: 4 });
  await page.mouse.up();
}

/** Paint Bucket of `color` at document pixel `(x, y)`. */
async function fill(page: Page, color: string, x: number, y: number): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill(color);
  await page.keyboard.press('g');
  await page.mouse.click(...(await point(page, x + 0.5, y + 0.5)));
}

async function copyVia(page: Page, action: () => Promise<void>): Promise<ClipImage> {
  await clearClipboard(page);
  await action();
  return copiedImage(page);
}

/** The active layer as a whole, read through Deselect + Copy. */
async function activeLayer(page: Page): Promise<ClipImage> {
  await page.keyboard.press('ControlOrMeta+d');
  return copyVia(page, () => menu(page, 'Edit', 'Copy'));
}

test('image: the Edit menu has Cut, Copy, Copy Merged and Paste with their shortcuts', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
  const edit = page.getByRole('menu', { name: 'Edit', exact: true });
  await expect(edit.getByRole('menuitem', { name: 'Cut', exact: true })).toContainText('Ctrl+X');
  await expect(edit.getByRole('menuitem', { name: 'Copy', exact: true })).toContainText('Ctrl+C');
  await expect(edit.getByRole('menuitem', { name: 'Copy Merged', exact: true })).toBeVisible();
  await expect(edit.getByRole('menuitem', { name: 'Paste', exact: true })).toContainText('Ctrl+V');
});

test.describe('on macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36',
  });

  test('both modes show ⌘X, ⌘C and ⌘V', async ({ page }) => {
    for (const mode of ['image', 'pixel'] as const) {
      await createDocument(page, mode, { width: SIZE, height: SIZE });
      await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
      const edit = page.getByRole('menu', { name: 'Edit', exact: true });
      await expect(edit.getByRole('menuitem', { name: 'Cut', exact: true })).toContainText('⌘X');
      await expect(edit.getByRole('menuitem', { name: 'Copy', exact: true })).toContainText('⌘C');
      await expect(edit.getByRole('menuitem', { name: 'Paste', exact: true })).toContainText('⌘V');
      await page.keyboard.press('Escape');
    }
  });
});

test('image: Copy puts the selected box on the clipboard and changes nothing', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  await select(page, 8, 4, 6, 3);
  const img = await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  expect([img.width, img.height]).toEqual([6, 3]);
  expect(pixelOf(img, 0, 0)).toEqual(RED);
  expect(pixelOf(img, 5, 2)).toEqual(RED);
  // The document is unchanged: one layer, still red inside and outside.
  await expect(layerRows(page)).toHaveCount(1);
  const layer = await activeLayer(page);
  expect(pixelOf(layer, 8, 4)).toEqual(RED);
  expect(pixelOf(layer, 0, 0)).toEqual(RED);
});

test('image: a Rectangle Select snaps each corner to the nearest grid line', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  // 0.4 snaps to 0 and 10.6 to 11: an 11 × 11 box, not round(10.2) = 10.
  await page.keyboard.press('m');
  await page.mouse.move(...(await point(page, 0.4, 0.4)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, 10.6, 10.6)), { steps: 4 });
  await page.mouse.up();
  const img = await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  expect([img.width, img.height]).toEqual([11, 11]);
});

test('image: Copy keeps only the selected pixels of a shaped selection', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  await page.keyboard.press('m');
  await page
    .getByRole('radiogroup', { name: 'Marquee variant' })
    .getByRole('radio', { name: 'Ellipse Select', exact: true })
    .click();
  await page.mouse.move(...(await point(page, 4, 4)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, 20, 20)), { steps: 4 });
  await page.mouse.up();
  const img = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect(img.width).toBeGreaterThan(10);
  expect(pixelOf(img, 0, 0)[3]).toBe(0);
  const c = Math.floor(img.width / 2);
  expect(pixelOf(img, c, c)).toEqual(RED);
});

test('image: a feathered selection scales the layer alpha by its coverage', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  // A half-transparent layer (alpha 128) over the whole canvas.
  await putClipboardImage(page, SIZE, SIZE, 'rgba(255, 0, 0, 0.5)');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  await select(page, 8, 8, 16, 16);
  await menu(page, 'Select', 'Feather…');
  const dialog = page.getByRole('dialog', { name: 'Feather…' });
  await dialog.getByLabel('Radius (px)').fill('3');
  await dialog.getByRole('button', { name: 'Apply' }).click();
  const img = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  const alphas = [];
  for (let i = 3; i < img.data.length; i += 4) alphas.push(img.data[i] ?? 0);
  // Layer alpha (128) times coverage (≤ 1): never above the layer's alpha —
  // coverage alone would reach ~255 inside — and fading out at the edge.
  expect(Math.max(...alphas)).toBeGreaterThan(100);
  expect(Math.max(...alphas)).toBeLessThanOrEqual(128);
  expect(alphas.some((a) => a > 0 && a < 64)).toBe(true);
});

test('image: Copy without a selection takes the whole layer; Copy Merged the composite', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  await page.getByRole('button', { name: 'Add layer' }).click();
  await expect(layerRows(page)).toHaveCount(2);
  await select(page, 10, 8, 4, 4);
  await fill(page, '#0000ff', 11, 9);
  await page.keyboard.press('ControlOrMeta+d');

  const whole = await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  expect([whole.width, whole.height]).toEqual([SIZE, SIZE]);
  expect(pixelOf(whole, 0, 0)).toEqual(CLEAR);
  expect(pixelOf(whole, 11, 9)).toEqual(BLUE);

  await select(page, 8, 8, 6, 4);
  const own = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  const merged = await copyVia(page, () => menu(page, 'Edit', 'Copy Merged'));
  expect([merged.width, merged.height]).toEqual([own.width, own.height]);
  expect(pixelOf(own, 0, 0)).toEqual(CLEAR);
  expect(pixelOf(merged, 0, 0)).toEqual(RED);
  expect(pixelOf(merged, 3, 1)).toEqual(BLUE);
});

test('image: Cut copies and clears the selection, without one the whole layer; Undo restores', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  await select(page, 4, 4, 4, 4);
  const cut = await copyVia(page, () => page.keyboard.press('ControlOrMeta+x'));
  expect([cut.width, cut.height]).toEqual([4, 4]);
  expect(pixelOf(cut, 0, 0)).toEqual(RED);
  // A Copy of the same selection reads back what the layer holds there.
  const hole = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect(hole.data.every((v) => v === 0)).toBe(true);
  await undoButton(page).click();
  const back = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect(pixelOf(back, 1, 1)).toEqual(RED);

  // Without a selection: the whole layer.
  await page.keyboard.press('ControlOrMeta+d');
  const all = await copyVia(page, () => menu(page, 'Edit', 'Cut'));
  expect([all.width, all.height]).toEqual([SIZE, SIZE]);
  expect(pixelOf(all, 9, 9)).toEqual(RED);
  const empty = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect([empty.width, empty.height]).toEqual([SIZE, SIZE]);
  expect(empty.data.every((v) => v === 0)).toBe(true);
  await undoButton(page).click();
  const layer = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect(pixelOf(layer, 9, 9)).toEqual(RED);
  expect(pixelOf(layer, 0, 0)).toEqual(RED);
});

test('image: Pincel’s own copy pastes in place on a new layer, one undo step', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await select(page, 12, 10, 4, 4);
  await fill(page, '#ff0000', 13, 11);
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  // One Undo removes the pasted layer; Redo brings it back.
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(1);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect(layerRows(page)).toHaveCount(2);
  const pasted = await activeLayer(page);
  expect(pixelOf(pasted, 12, 10)).toEqual(RED);
  expect(pixelOf(pasted, 15, 13)).toEqual(RED);
  expect(pixelOf(pasted, 11, 10)).toEqual(CLEAR);
  expect(pixelOf(pasted, 0, 0)).toEqual(CLEAR);
});

test('image: another image lands at the viewport top-left; Cancel on a larger one changes nothing', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 200, height: 150 });
  const zoomIn = page.getByRole('button', { name: 'Zoom in', exact: true });
  for (let i = 0; i < 8; i += 1) await zoomIn.click();
  const tl = await imageCanvas(page).evaluate((el) => {
    const viewport = el.parentElement?.parentElement;
    if (!viewport) throw new Error('no viewport');
    const c = el.getBoundingClientRect();
    const v = viewport.getBoundingClientRect();
    const scale = c.width / 200;
    return { x: Math.floor((v.left - c.left) / scale), y: Math.floor((v.top - c.top) / scale) };
  });
  expect(tl.x).toBeGreaterThan(0);
  expect(tl.y).toBeGreaterThan(0);
  await putClipboardImage(page, 2, 2, '#0000ff');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  const layer = await activeLayer(page);
  expect(pixelOf(layer, tl.x, tl.y)).toEqual(BLUE);
  expect(pixelOf(layer, tl.x - 1, tl.y)).toEqual(CLEAR);
  expect(pixelOf(layer, tl.x, tl.y - 1)).toEqual(CLEAR);

  await putClipboardImage(page, 300, 20, '#0000ff');
  await menu(page, 'Edit', 'Paste');
  const dialog = page.getByRole('dialog', { name: 'Paste larger image' });
  await expect(dialog.getByRole('button', { name: 'Expand canvas' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Keep canvas size' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
  await expect(layerRows(page)).toHaveCount(2);
  await expect(page.getByText('200 × 150 · sRGB')).toBeVisible();
});

test('image: Ctrl+C / X / V in a text field act on the field only', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await fill(page, '#ff0000', 1, 1);
  await select(page, 4, 4, 4, 4);
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  // A different selection: a Copy reaching the editor would record it.
  await page.keyboard.press('ControlOrMeta+a');

  const hex = page.getByLabel('Foreground color hex value');
  await hex.focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await page.keyboard.press('ControlOrMeta+x');
  await expect(hex).toHaveValue('');
  await page.keyboard.press('ControlOrMeta+v');
  await expect(hex).toHaveValue('#ff0000');
  await expect(layerRows(page)).toHaveCount(1);
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('#ff0000');
  // The in-app copy is still the 4×4 box from (4, 4), pasted back in place.
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  const pasted = await activeLayer(page);
  expect(pixelOf(pasted, 4, 4)).toEqual(RED);
  expect(pixelOf(pasted, 0, 0)).toEqual(CLEAR);
});

/** Opens a new document of `mode` from File ▸ New…, discarding the open one. */
async function newDocument(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  const unsaved = page.getByRole('alertdialog', { name: 'Unsaved changes' });
  await unsaved.or(dialog).first().waitFor();
  if (await unsaved.isVisible()) await unsaved.getByRole('button', { name: 'Discard changes' }).click();
  await dialog.getByRole('radio', { name: mode === 'pixel' ? /Pixel sprite/ : /^Image/ }).click();
  await dialog.getByLabel('Width').fill(String(SIZE));
  await dialog.getByLabel('Height').fill(String(SIZE));
  await dialog.getByRole('button', { name: 'Create' }).click();
  if (await unsaved.isVisible()) await unsaved.getByRole('button', { name: 'Discard changes' }).click();
  await waitForEditor(page, mode);
}

test('a copy pastes into the other mode as a new layer at the viewport top-left', async ({
  page,
}) => {
  // Pixel → Image: copied from (2, 2), pasted at (0, 0) of the fitted view.
  await createDocument(page, 'pixel', { width: SIZE, height: SIZE });
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const at = (x: number, y: number): [number, number] => [
    box.x + ((x + 0.5) * box.width) / SIZE,
    box.y + ((y + 0.5) * box.height) / SIZE,
  ];
  await page.getByTitle('Pencil (B)').click();
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await page.mouse.click(...at(2, 2));
  await page.keyboard.press('m');
  await page.mouse.move(...at(2, 2));
  await page.mouse.down();
  await page.mouse.move(...at(4, 4), { steps: 4 });
  await page.mouse.up();
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));

  await newDocument(page, 'image');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  let layer = await activeLayer(page);
  expect(pixelOf(layer, 0, 0)).toEqual(RED);
  expect(pixelOf(layer, 2, 2)).toEqual(CLEAR);

  // Image → Pixel: copied from (5, 5), pasted at (0, 0).
  await select(page, 5, 5, 2, 2);
  await fill(page, '#0000ff', 5, 5);
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  await newDocument(page, 'pixel');
  await menu(page, 'Edit', 'Paste');
  await expect(
    page.getByRole('list', { name: 'Layer list' }).getByRole('button', { name: /^Drag to reorder / }),
  ).toHaveCount(2);
  await page.keyboard.press('ControlOrMeta+d');
  layer = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect(pixelOf(layer, 0, 0)).toEqual(BLUE);
  expect(pixelOf(layer, 1, 1)).toEqual(BLUE);
  expect(pixelOf(layer, 5, 5)).toEqual(CLEAR);
});
