import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  menu,
  pixelCanvas,
  pixelOf,
  putClipboardImage,
  readClipboardImage,
  type ClipImage,
} from './helpers';

// Pixel mode: Edit ▸ Cut / Copy / Copy Merged / Paste through the system
// clipboard as PNG (spec §5.4, docs/specs/copy-cut-paste.md). Clipboard
// images are decoded test-side; the app never reads canvas pixels.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];
const CLEAR = [0, 0, 0, 0];

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const layerRows = (page: Page) =>
  page.getByRole('list', { name: 'Layer list' }).getByRole('button', { name: /^Drag to reorder / });
const depths = (page: Page, u: number, r = 0) => page.getByText(`undo ${u} / redo ${r}`);

/** Screen point at the centre of sprite pixel `(x, y)` of a `size`² sprite. */
async function at(page: Page, x: number, y: number, size: number): Promise<[number, number]> {
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + ((x + 0.5) * box.width) / size, box.y + ((y + 0.5) * box.height) / size];
}

/** Pencil dots of `color` at sprite pixels `pts`. */
async function paint(page: Page, color: string, pts: [number, number][], size = 16): Promise<void> {
  await page.getByTitle('Pencil (B)').click();
  await page.getByLabel('Foreground color', { exact: true }).fill(color);
  for (const [x, y] of pts) await page.mouse.click(...(await at(page, x, y, size)));
}

/** Marquee from pixel `(x0, y0)` to `(x1, y1)`, both inclusive. */
async function select(page: Page, x0: number, y0: number, x1: number, y1: number): Promise<void> {
  await page.keyboard.press('m');
  await page.mouse.move(...(await at(page, x0, y0, 16)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, x1, y1, 16)), { steps: 4 });
  await page.mouse.up();
}

/** Runs `action` (a Copy / Cut) and returns the image it put on the clipboard. */
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

test('pixel: the Edit menu has Cut, Copy, Copy Merged and Paste with their shortcuts', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
  const edit = page.getByRole('menu', { name: 'Edit', exact: true });
  await expect(edit.getByRole('menuitem', { name: 'Cut', exact: true })).toContainText('Ctrl+X');
  await expect(edit.getByRole('menuitem', { name: 'Copy', exact: true })).toContainText('Ctrl+C');
  await expect(edit.getByRole('menuitem', { name: 'Copy Merged', exact: true })).toBeVisible();
  await expect(edit.getByRole('menuitem', { name: 'Paste', exact: true })).toContainText('Ctrl+V');
});

test('pixel: Copy puts the selected box of the active layer on the clipboard and changes nothing', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [
    [2, 2],
    [4, 4],
    [8, 8],
  ]);
  await page.keyboard.press('ControlOrMeta+d');
  await expect(depths(page, 3)).toBeVisible();
  await select(page, 2, 2, 4, 4);
  const img = await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  expect([img.width, img.height]).toEqual([3, 3]);
  expect(pixelOf(img, 0, 0)).toEqual(RED);
  expect(pixelOf(img, 2, 2)).toEqual(RED);
  expect(pixelOf(img, 1, 1)).toEqual(CLEAR);
  // The document is unchanged: no undo step, one layer, pixels in place.
  await expect(depths(page, 3)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
  const layer = await activeLayer(page);
  expect(pixelOf(layer, 2, 2)).toEqual(RED);
});

test('pixel: Copy without a selection takes the whole active layer at canvas size', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [[15, 15]]);
  await page.keyboard.press('ControlOrMeta+d');
  const img = await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  expect([img.width, img.height]).toEqual([16, 16]);
  expect(pixelOf(img, 15, 15)).toEqual(RED);
  expect(pixelOf(img, 0, 0)).toEqual(CLEAR);
});

test('pixel: Copy Merged takes the same region from the visible composite', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [[2, 2]]);
  await page.getByRole('button', { name: 'Add layer' }).click();
  await expect(layerRows(page)).toHaveCount(2);
  await paint(page, '#0000ff', [[3, 2]]);
  await select(page, 2, 2, 3, 2);
  const own = await copyVia(page, () => menu(page, 'Edit', 'Copy'));
  expect([own.width, own.height]).toEqual([2, 1]);
  expect(pixelOf(own, 0, 0)).toEqual(CLEAR);
  expect(pixelOf(own, 1, 0)).toEqual(BLUE);
  const merged = await copyVia(page, () => menu(page, 'Edit', 'Copy Merged'));
  expect([merged.width, merged.height]).toEqual([2, 1]);
  expect(pixelOf(merged, 0, 0)).toEqual(RED);
  expect(pixelOf(merged, 1, 0)).toEqual(BLUE);
});

test('pixel: Cut copies the selection, clears it, and one Undo restores it', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [
    [2, 2],
    [4, 4],
  ]);
  await page.keyboard.press('ControlOrMeta+d');
  await expect(depths(page, 2)).toBeVisible();
  await select(page, 2, 2, 3, 3);
  const cut = await copyVia(page, () => page.keyboard.press('ControlOrMeta+x'));
  expect([cut.width, cut.height]).toEqual([2, 2]);
  expect(pixelOf(cut, 0, 0)).toEqual(RED);
  await expect(depths(page, 3)).toBeVisible();
  const after = await activeLayer(page);
  expect(pixelOf(after, 2, 2)).toEqual(CLEAR);
  expect(pixelOf(after, 4, 4)).toEqual(RED);
  await undoButton(page).click();
  await expect(depths(page, 2, 1)).toBeVisible();
  expect(pixelOf(await activeLayer(page), 2, 2)).toEqual(RED);
});

test('pixel: Cut without a selection takes and clears the whole layer; one Undo restores it', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [
    [0, 0],
    [15, 15],
  ]);
  await page.keyboard.press('ControlOrMeta+d');
  const cut = await copyVia(page, () => menu(page, 'Edit', 'Cut'));
  expect([cut.width, cut.height]).toEqual([16, 16]);
  expect(pixelOf(cut, 15, 15)).toEqual(RED);
  const cleared = await activeLayer(page);
  expect(pixelOf(cleared, 0, 0)).toEqual(CLEAR);
  expect(pixelOf(cleared, 15, 15)).toEqual(CLEAR);
  await undoButton(page).click();
  const restored = await activeLayer(page);
  expect(pixelOf(restored, 0, 0)).toEqual(RED);
  expect(pixelOf(restored, 15, 15)).toEqual(RED);
});

test('pixel: Pincel’s own copy pastes in place on a new layer above the active one, one undo step', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [[5, 6]]);
  await select(page, 4, 5, 6, 7);
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  // The new layer sits above the old one and is active.
  await expect(layerRows(page).first()).toHaveAccessibleName('Drag to reorder Layer 2');
  const pasted = await activeLayer(page);
  expect(pixelOf(pasted, 5, 6)).toEqual(RED);
  expect(pixelOf(pasted, 0, 0)).toEqual(CLEAR);
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(1);

  // Ctrl+V: the browser's paste event carries the same image, re-encoded.
  await page.evaluate(async () => {
    const item = (await navigator.clipboard.read()).find((i) => i.types.includes('image/png'));
    if (!item) throw new Error('no image');
    const bitmap = await createImageBitmap(await item.getType('image/png'));
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    canvas.getContext('2d')?.drawImage(bitmap, 0, 0);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    const data = new DataTransfer();
    data.items.add(new File([blob], 'clip.png', { type: 'image/png' }));
    window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data }));
  });
  await expect(layerRows(page)).toHaveCount(2);
  expect(pixelOf(await activeLayer(page), 5, 6)).toEqual(RED);
});

/** Canvas pixel at the top-left of the Pixel viewport, from DOM layout. */
async function viewportTopLeft(page: Page, size: number): Promise<{ x: number; y: number }> {
  return pixelCanvas(page).evaluate((el, n) => {
    const stage = el.parentElement?.parentElement;
    if (!stage) throw new Error('no stage');
    const c = el.getBoundingClientRect();
    const s = stage.getBoundingClientRect();
    const scale = c.width / n;
    return { x: Math.floor((s.left - c.left) / scale), y: Math.floor((s.top - c.top) / scale) };
  }, size);
}

/** Whether pixel `(x, y)` of a decoded image is opaque blue. */
function isBlueAt(img: ClipImage, x: number, y: number): boolean {
  return pixelOf(img, x, y).join() === BLUE.join();
}

test('pixel: another image lands at the viewport top-left, clamped into the canvas', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 64, height: 64 });
  // Fitted: the viewport's top-left lies left of / above the canvas → (0, 0).
  await putClipboardImage(page, 2, 2, '#0000ff');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  let layer = await activeLayer(page);
  expect(isBlueAt(layer, 0, 0)).toBe(true);
  expect(isBlueAt(layer, 2, 2)).toBe(false);

  // Zoomed in: the image's top-left is the canvas pixel at the viewport's.
  const zoomIn = page.getByRole('button', { name: 'Zoom in', exact: true });
  for (let i = 0; i < 3; i += 1) await zoomIn.click();
  const tl = await viewportTopLeft(page, 64);
  expect(tl.x).toBeGreaterThan(4);
  expect(tl.y).toBeGreaterThan(4);
  await putClipboardImage(page, 2, 2, '#0000ff');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(3);
  layer = await activeLayer(page);
  expect(isBlueAt(layer, tl.x, tl.y)).toBe(true);
  expect(isBlueAt(layer, tl.x - 1, tl.y)).toBe(false);
  expect(isBlueAt(layer, tl.x, tl.y - 1)).toBe(false);

  // An image that fits but would run past the edge is shifted inside.
  await putClipboardImage(page, 60, 60, '#0000ff');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(4);
  layer = await activeLayer(page);
  expect(isBlueAt(layer, 4, 4)).toBe(true);
  expect(isBlueAt(layer, 63, 63)).toBe(true);
  expect(isBlueAt(layer, 3, 4)).toBe(false);
});

test('pixel: a larger image asks — Cancel, Keep canvas size, Expand canvas in one undo step', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await putClipboardImage(page, 20, 10, '#ff0000');
  const dialog = page.getByRole('dialog', { name: 'Paste larger image' });
  const size = (w: number, h: number) => page.getByText(`${w}×${h}`, { exact: true });

  await menu(page, 'Edit', 'Paste');
  await expect(dialog).toContainText(
    'The image being pasted (20 × 10) is larger than the canvas. Expand the canvas to fit it?',
  );
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
  await expect(layerRows(page)).toHaveCount(1);
  await expect(depths(page, 0)).toBeVisible();

  await menu(page, 'Edit', 'Paste');
  await dialog.getByRole('button', { name: 'Keep canvas size' }).click();
  await expect(size(16, 16)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(2);
  const kept = await activeLayer(page);
  expect([kept.width, kept.height]).toEqual([16, 16]);
  expect(pixelOf(kept, 15, 9)).toEqual(RED);
  expect(pixelOf(kept, 15, 10)).toEqual(CLEAR);
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(1);

  await putClipboardImage(page, 20, 10, '#ff0000');
  await menu(page, 'Edit', 'Paste');
  await dialog.getByRole('button', { name: 'Expand canvas' }).click();
  await expect(size(20, 16)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(2);
  const grown = await activeLayer(page);
  expect(pixelOf(grown, 19, 9)).toEqual(RED);
  // One Undo removes the layer, its pixels and the expansion.
  await undoButton(page).click();
  await expect(size(16, 16)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
});

test('pixel: Import PNG as layer is one undo step', async ({ page }) => {
  // Force the <input type=file> fallback: the File System Access picker
  // would open a native dialog the test cannot drive.
  await page.addInitScript(() => {
    delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
  });
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const png = await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(4, 4);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.fillStyle = '#ff0000';
    ctx.fillRect(0, 0, 4, 4);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  });
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    menu(page, 'File', 'Import PNG as Layer…'),
  ]);
  await chooser.setFiles({ name: 'red.png', mimeType: 'image/png', buffer: Buffer.from(png) });
  await expect(layerRows(page)).toHaveCount(2);
  await expect(depths(page, 1)).toBeVisible();
  expect(pixelOf(await activeLayer(page), 3, 3)).toEqual(RED);
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(1);
  await expect(depths(page, 0, 1)).toBeVisible();
});

test('pixel: Ctrl+C / X / V in a text field act on the field only', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [
    [3, 3],
    [8, 8],
  ]);
  await select(page, 3, 3, 4, 4);
  await copyVia(page, () => page.keyboard.press('ControlOrMeta+c'));
  // A different selection: a Copy reaching the editor would record it.
  await page.keyboard.press('ControlOrMeta+a');
  await expect(depths(page, 2)).toBeVisible();

  const hex = page.getByLabel('Foreground color hex value');
  await hex.focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await page.keyboard.press('ControlOrMeta+x');
  await expect(hex).toHaveValue('');
  await page.keyboard.press('ControlOrMeta+v');
  await expect(hex).toHaveValue('#ff0000');
  // The text went to the field; the document did not change ...
  await expect(depths(page, 2)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  expect(clip).toBe('#ff0000');
  // ... and the in-app copy is still the 2×2 box from (3, 3): a Paste with
  // only text on the clipboard falls back to it, in place.
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  const pasted = await activeLayer(page);
  expect(pixelOf(pasted, 3, 3)).toEqual(RED);
  expect(pixelOf(pasted, 8, 8)).toEqual(CLEAR);
});

test('pixel: Copy on a group takes its rendered pixels; Cut on it is refused', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, '#ff0000', [[2, 2]]);
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect(layerRows(page)).toHaveCount(2);
  // Move the layer into the group: drop it on the middle of the group row.
  const grip = page.getByRole('button', { name: 'Drag to reorder Layer 1', exact: true });
  const groupRow = page
    .getByRole('list', { name: 'Layer list' })
    .locator('li', { has: page.getByRole('button', { name: 'Drag to reorder Group 1', exact: true }) });
  const a = await grip.boundingBox();
  const b = await groupRow.boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 6 });
  await page.mouse.up();
  await page.getByTitle('Group 1', { exact: true }).click();

  const group = await activeLayer(page);
  expect([group.width, group.height]).toEqual([16, 16]);
  expect(pixelOf(group, 2, 2)).toEqual(RED);

  await putClipboardImage(page, 3, 3, '#00ff00');
  const depthText = await page.getByText(/^undo \d+ \/ redo \d+$/).textContent();
  await page.keyboard.press('ControlOrMeta+x');
  await expect(page.getByText('Cut works on image layers only')).toBeVisible();
  const clip = await readClipboardImage(page);
  expect(clip && [clip.width, clip.height]).toEqual([3, 3]);
  await expect(page.getByText(depthText ?? '', { exact: true })).toBeVisible();
  expect(pixelOf(await activeLayer(page), 2, 2)).toEqual(RED);
});

test('pixel: a refused clipboard write keeps the copy in Pincel for Paste, at its origin', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  // A stale image the system clipboard keeps when Pincel's write fails.
  await putClipboardImage(page, 3, 3, '#0000ff');
  await page.evaluate(() => {
    navigator.clipboard.write = () => Promise.reject(new DOMException('denied', 'NotAllowedError'));
  });
  await paint(page, '#ff0000', [
    [6, 6],
    [7, 7],
  ]);
  await select(page, 6, 6, 7, 7);
  await page.keyboard.press('ControlOrMeta+x');
  await page.keyboard.press('ControlOrMeta+d');
  await menu(page, 'Edit', 'Paste');
  await expect(layerRows(page)).toHaveCount(2);
  const fg = page.getByLabel('Foreground color', { exact: true });
  const probe = async (x: number, y: number): Promise<string> => {
    await fg.fill('#123456');
    await page.keyboard.press('i');
    await page.mouse.click(...(await at(page, x, y, 16)));
    return fg.inputValue();
  };
  // The cut pixels are back where they came from, not the stale blue image.
  expect(await probe(6, 6)).toBe('#ff0000');
  expect(await probe(7, 7)).toBe('#ff0000');
  expect(await probe(0, 0)).not.toBe('#0000ff');
});

test('pixel: a paste the core cannot use shows a notice and changes nothing', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.items.add(new File([new Uint8Array([1, 2, 3, 4])], 'bad.png', { type: 'image/png' }));
    window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data }));
  });
  await expect(page.getByText(/^Could not paste: /)).toBeVisible();
  await expect(layerRows(page)).toHaveCount(1);
  await expect(depths(page, 0)).toBeVisible();
});
