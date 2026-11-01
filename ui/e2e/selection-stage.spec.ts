import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  type ClipImage,
} from './helpers';

// Selection gestures that start on the stage around the canvas, and
// deselecting by click and Escape (docs/specs/pixel-selection-move.md
// AC-21 – AC-24, docs/specs/image-editing.md AC-29 – AC-32), plus the
// Pixel selection ACs that had no test yet. The selection is read through
// Copy: the clipboard holds its bounding box, or the whole layer without one.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const RED = [255, 0, 0, 255];
const PX = 16;
const IMG = 32;

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** Screen point of canvas position `(x, y)`: pixel corners at integers. */
async function pt(page: Page, mode: Mode, x: number, y: number): Promise<[number, number]> {
  const size = mode === 'pixel' ? PX : IMG;
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / size, box.y + (y * box.height) / size];
}

/** Screen point of the centre of pixel `(x, y)`. */
const mid = (page: Page, mode: Mode, x: number, y: number) => pt(page, mode, x + 0.5, y + 0.5);

/** A point on the stage `d` CSS pixels up and left of the canvas corner. */
async function stageTopLeft(page: Page, mode: Mode, d = 12): Promise<[number, number]> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x - d, box.y - d];
}

async function dragFrom(page: Page, from: [number, number], to: [number, number]): Promise<void> {
  await page.mouse.move(...from);
  await page.mouse.down();
  await page.mouse.move(...to, { steps: 5 });
  await page.mouse.up();
}

async function copySelection(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  return copiedImage(page);
}

const size = async (page: Page) => {
  const img = await copySelection(page);
  return [img.width, img.height];
};

/** Activates the rail tool `name` with its group key, cycling the group's variants. */
async function useTool(page: Page, key: string, name: string): Promise<void> {
  const button = page.getByRole('button', { name: new RegExp(`(^|: )${name}$`) });
  await page.keyboard.press(key);
  for (let i = 0; i < 4 && (await button.count()) === 0; i += 1) await page.keyboard.press(key);
  await expect(button).toHaveCount(1);
}

/** Rectangle marquee over the pixels `(x0, y0)` – `(x1, y1)`, both inclusive. */
async function marquee(page: Page, mode: Mode, x0: number, y0: number, x1: number, y1: number) {
  await useTool(page, 'm', 'Rectangle Select');
  if (mode === 'pixel') await dragFrom(page, await mid(page, mode, x0, y0), await mid(page, mode, x1, y1));
  else await dragFrom(page, await pt(page, mode, x0, y0), await pt(page, mode, x1 + 1, y1 + 1));
}

async function paintRed(page: Page, mode: Mode, x: number, y: number): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await page.keyboard.press(mode === 'pixel' ? 'b' : 'g');
  await page.mouse.click(...(await mid(page, mode, x, y)));
}

/** A document of `mode`, zoomed out so the stage shows on every side. */
async function openDoc(page: Page, mode: Mode): Promise<void> {
  await createDocument(page, mode, mode === 'pixel' ? { width: PX, height: PX } : { width: IMG, height: IMG });
  const before = await canvasOf(page, mode).boundingBox();
  await page.keyboard.press('-');
  await page.keyboard.press('-');
  await expect
    .poll(async () => (await canvasOf(page, mode).boundingBox())?.width)
    .toBeLessThan((before?.width ?? 0) * 0.9);
}

const pxDepths = (page: Page, u: number, r = 0) => page.getByText(`undo ${u} / redo ${r}`);
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

for (const mode of ['pixel', 'image'] as const) {
  const full: [number, number] = mode === 'pixel' ? [PX, PX] : [IMG, IMG];
  const open = (page: Page) => openDoc(page, mode);

  test(`${mode}: a marquee dragged from the stage past the top-left corner selects from (0, 0)`, async ({
    page,
  }) => {
    await open(page);
    await useTool(page, 'm', 'Rectangle Select');
    const end = mode === 'pixel' ? await mid(page, mode, 4, 3) : await pt(page, mode, 5, 4);
    await dragFrom(page, await stageTopLeft(page, mode), end);
    expect(await size(page)).toEqual([5, 4]);
  });

  test(`${mode}: a lasso traced partly over the stage selects the edge pixel`, async ({ page }) => {
    await open(page);
    await paintRed(page, mode, 0, 0);
    await useTool(page, 'q', 'Lasso');
    const [sx, sy] = await stageTopLeft(page, mode);
    const right = await pt(page, mode, 6, 0);
    const corner = await pt(page, mode, 6, 6);
    const bottom = await pt(page, mode, 0, 6);
    await page.mouse.move(sx, sy);
    await page.mouse.down();
    await page.mouse.move(right[0] ?? sx, sy, { steps: 4 });
    await page.mouse.move(...corner, { steps: 4 });
    await page.mouse.move(sx, bottom[1] ?? sy, { steps: 4 });
    await page.mouse.move(sx, sy, { steps: 4 });
    await page.mouse.up();
    const img = await copySelection(page);
    expect(img.width).toBeLessThan(full[0]);
    expect(pixelOf(img, 0, 0)).toEqual(RED);
  });

  test(`${mode}: polygonal lasso vertices may sit on the stage`, async ({ page }) => {
    await open(page);
    await paintRed(page, mode, 0, 0);
    await useTool(page, 'q', 'Polygonal Lasso');
    const [sx, sy] = await stageTopLeft(page, mode);
    const right = await pt(page, mode, 6, 0);
    const corner = await pt(page, mode, 6, 6);
    await page.mouse.click(sx, sy);
    await page.mouse.click(right[0] ?? sx, sy);
    await page.mouse.click(...corner);
    await page.mouse.dblclick(sx, corner[1] ?? sy);
    const img = await copySelection(page);
    expect(img.width).toBeLessThan(full[0]);
    expect(pixelOf(img, 0, 0)).toEqual(RED);
  });

  test(`${mode}: a marquee click on the stage deselects in replace mode only`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await page.keyboard.down('Shift');
    await page.mouse.click(...(await stageTopLeft(page, mode)));
    await page.keyboard.up('Shift');
    expect(await size(page)).toEqual([4, 4]);
    await page.mouse.click(...(await stageTopLeft(page, mode)));
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: a marquee click on the canvas deselects`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await page.mouse.click(...(await mid(page, mode, 9, 9)));
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: a lasso click without a drag deselects`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await useTool(page, 'q', 'Lasso');
    await page.mouse.click(...(await mid(page, mode, 9, 9)));
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: a Magic Wand click on the stage deselects in replace mode only`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await useTool(page, 'w', 'Magic Wand');
    await page.keyboard.down('Shift');
    await page.mouse.click(...(await stageTopLeft(page, mode)));
    await page.keyboard.up('Shift');
    expect(await size(page)).toEqual([4, 4]);
    await page.mouse.click(...(await stageTopLeft(page, mode)));
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: Escape during a marquee drag keeps the selection; Escape after it deselects`, async ({
    page,
  }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await page.mouse.move(...(await mid(page, mode, 8, 8)));
    await page.mouse.down();
    await page.mouse.move(...(await mid(page, mode, 12, 12)), { steps: 4 });
    await page.keyboard.press('Escape');
    await page.mouse.up();
    expect(await size(page)).toEqual([4, 4]);
    await page.keyboard.press('Escape');
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: a paint press on the stage changes nothing`, async ({ page }) => {
    await open(page);
    await page.keyboard.press('b');
    const from = await stageTopLeft(page, mode);
    await dragFrom(page, from, await mid(page, mode, 3, 3));
    if (mode === 'pixel') await expect(pxDepths(page, 0)).toBeVisible();
    else await expect(undoButton(page)).toBeDisabled();
  });
}

test('image: deselecting by a stage click or Escape is one undo step', async ({ page }) => {
  await openDoc(page, 'image');
  await marquee(page, 'image', 2, 2, 5, 5);
  await page.mouse.click(...(await stageTopLeft(page, 'image')));
  expect(await size(page)).toEqual([IMG, IMG]);
  await undoButton(page).click();
  expect(await size(page)).toEqual([4, 4]);
  await page.keyboard.press('Escape');
  expect(await size(page)).toEqual([IMG, IMG]);
  await undoButton(page).click();
  expect(await size(page)).toEqual([4, 4]);
});

test('image: a stage click with nothing selected adds no undo step', async ({ page }) => {
  await openDoc(page, 'image');
  await useTool(page, 'm', 'Rectangle Select');
  await page.mouse.click(...(await stageTopLeft(page, 'image')));
  await page.keyboard.press('Escape');
  await expect(undoButton(page)).toBeDisabled();
});

test('pixel: the Move tool with nothing selected pans from the stage', async ({ page }) => {
  await openDoc(page, 'pixel');
  await page.keyboard.press('v');
  const before = await pixelCanvas(page).boundingBox();
  const from = await stageTopLeft(page, 'pixel');
  await dragFrom(page, from, [from[0] + 30, from[1] + 20]);
  const after = await pixelCanvas(page).boundingBox();
  expect(after?.x).toBeCloseTo((before?.x ?? 0) + 30, 0);
  expect(after?.y).toBeCloseTo((before?.y ?? 0) + 20, 0);
  await expect(pxDepths(page, 0)).toBeVisible();
});

test('image: the Pan tool pans from the stage', async ({ page }) => {
  await openDoc(page, 'image');
  await page.keyboard.press('h');
  const before = await imageCanvas(page).boundingBox();
  const from = await stageTopLeft(page, 'image');
  await dragFrom(page, from, [from[0] + 30, from[1] + 20]);
  const after = await imageCanvas(page).boundingBox();
  expect(after?.x).toBeCloseTo((before?.x ?? 0) + 30, 0);
  await expect(undoButton(page)).toBeDisabled();
});

// ---- Round 2: the remaining gestures of AC-21 – AC-24 / AC-29 – AC-32 ----

/** A point on the stage left of the canvas, `dx` CSS px out, at canvas row fraction `fy`. */
async function stageLeft(page: Page, mode: Mode, dx: number, fy: number): Promise<[number, number]> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x - dx, box.y + box.height * fy];
}

/** Fires `pointercancel` for the mouse pointer at the element under `at`. */
async function cancelPointer(page: Page, at: [number, number]): Promise<void> {
  await page.evaluate(([x, y]) => {
    const el = document.elementFromPoint(x, y);
    el?.dispatchEvent(
      new PointerEvent('pointercancel', { pointerId: 1, pointerType: 'mouse', bubbles: true }),
    );
  }, at);
}

for (const mode of ['pixel', 'image'] as const) {
  const full: [number, number] = mode === 'pixel' ? [PX, PX] : [IMG, IMG];
  const open = (page: Page) => openDoc(page, mode);

  test(`${mode}: an ellipse dragged from the stage reaches the edge column`, async ({ page }) => {
    await open(page);
    await paintRed(page, mode, 0, 4);
    await useTool(page, 'm', 'Ellipse Select');
    await dragFrom(page, await stageTopLeft(page, mode), await mid(page, mode, 9, 9));
    const img = await copySelection(page);
    expect(img.width).toBeLessThan(full[0]);
    expect(pixelOf(img, 0, 4)).toEqual(RED);
  });

  test(`${mode}: an ellipse click on the canvas deselects`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    await useTool(page, 'm', 'Ellipse Select');
    await page.mouse.click(...(await mid(page, mode, 9, 9)));
    expect(await size(page)).toEqual(full);
  });

  test(`${mode}: lasso, polygon and ellipse entirely on the stage deselect, Shift keeps`, async ({
    page,
  }) => {
    await open(page);
    const a = await stageLeft(page, mode, 40, 0.2);
    const b = await stageLeft(page, mode, 10, 0.2);
    const c = await stageLeft(page, mode, 10, 0.6);
    const d = await stageLeft(page, mode, 40, 0.6);
    const lasso = async () => {
      await useTool(page, 'q', 'Lasso');
      await page.mouse.move(...a);
      await page.mouse.down();
      for (const p of [b, c, d, a]) await page.mouse.move(...p, { steps: 3 });
      await page.mouse.up();
    };
    const polygon = async () => {
      await useTool(page, 'q', 'Polygonal Lasso');
      for (const p of [a, b, c]) await page.mouse.click(...p);
      await page.mouse.dblclick(...d);
    };
    const ellipse = async () => {
      await useTool(page, 'm', 'Ellipse Select');
      await dragFrom(page, a, c);
    };
    for (const gesture of [lasso, polygon, ellipse]) {
      await marquee(page, mode, 2, 2, 5, 5);
      await page.keyboard.down('Shift');
      await gesture();
      await page.keyboard.up('Shift');
      expect(await size(page)).toEqual([4, 4]);
      await gesture();
      expect(await size(page)).toEqual(full);
    }
  });

  test(`${mode}: Escape during a lasso or ellipse drag drops it; moving and releasing then changes nothing`, async ({
    page,
  }) => {
    await open(page);
    const before = await canvasOf(page, mode).screenshot();
    for (const [key, name] of [
      ['q', 'Lasso'],
      ['m', 'Ellipse Select'],
    ] as const) {
      await useTool(page, key, name);
      await page.mouse.move(...(await mid(page, mode, 2, 2)));
      await page.mouse.down();
      await page.mouse.move(...(await mid(page, mode, 9, 3)), { steps: 3 });
      await page.mouse.move(...(await mid(page, mode, 9, 9)), { steps: 3 });
      await page.keyboard.press('Escape');
      await expect
        .poll(async () => Buffer.compare(await canvasOf(page, mode).screenshot(), before))
        .toBe(0);
      await page.mouse.move(...(await mid(page, mode, 3, 9)), { steps: 3 });
      await page.mouse.up();
      expect(await size(page)).toEqual(full);
    }
  });

  test(`${mode}: a cancelled pointer drops a marquee drag`, async ({ page }) => {
    await open(page);
    await marquee(page, mode, 2, 2, 5, 5);
    const p = await mid(page, mode, 9, 9);
    await page.mouse.move(...(await mid(page, mode, 7, 7)));
    await page.mouse.down();
    await page.mouse.move(...p, { steps: 3 });
    await cancelPointer(page, p);
    await page.mouse.up();
    expect(await size(page)).toEqual([4, 4]);
  });

  test(`${mode}: a paint or Move press on the stage keeps the selection`, async ({ page }) => {
    await open(page);
    await paintRed(page, mode, 3, 3);
    await marquee(page, mode, 2, 2, 5, 5);
    const from = await stageTopLeft(page, mode);
    await page.keyboard.press('b');
    await dragFrom(page, from, await mid(page, mode, 3, 3));
    await page.keyboard.press('v');
    await dragFrom(page, from, await mid(page, mode, 6, 6));
    const img = await copySelection(page);
    expect([img.width, img.height]).toEqual([4, 4]);
    expect(pixelOf(img, 1, 1)).toEqual(RED);
    if (mode === 'pixel') await expect(pxDepths(page, 1)).toBeVisible();
    else {
      // One step back is the selection itself: nothing was added on the stage.
      await undoButton(page).click();
      expect(await size(page)).toEqual(full);
    }
  });

  test(`${mode}: Space-drag pans from the stage`, async ({ page }) => {
    await open(page);
    const before = await canvasOf(page, mode).boundingBox();
    const from = await stageTopLeft(page, mode);
    await page.keyboard.down('Space');
    await dragFrom(page, from, [from[0] + 30, from[1] + 20]);
    await page.keyboard.up('Space');
    const after = await canvasOf(page, mode).boundingBox();
    expect(after?.x).toBeCloseTo((before?.x ?? 0) + 30, 0);
  });
}

test('image: Escape discards an open polygonal lasso and keeps the selection', async ({ page }) => {
  await openDoc(page, 'image');
  await marquee(page, 'image', 2, 2, 5, 5);
  await useTool(page, 'q', 'Polygonal Lasso');
  for (const [x, y] of [
    [10, 10],
    [20, 10],
    [20, 20],
  ] as const)
    await page.mouse.click(...(await mid(page, 'image', x, y)));
  await page.keyboard.press('Escape');
  expect(await size(page)).toEqual([4, 4]);
});

// ---- Pixel selection ACs that had no test (pixel-selection-move) --------

test('pixel: a marquee selects A–B inclusive and draws its outline (AC-01)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  const before = await pixelCanvas(page).screenshot();
  await marquee(page, 'pixel', 3, 2, 7, 9);
  expect(await size(page)).toEqual([5, 8]);
  // The marching ants sit on the overlay canvas above the base canvas.
  await expect
    .poll(async () => Buffer.compare(await pixelCanvas(page).screenshot(), before))
    .not.toBe(0);
});

test('pixel: polygonal lasso closes on Enter, Escape discards its vertices (AC-04)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await useTool(page, 'q', 'Polygonal Lasso');
  for (const [x, y] of [
    [1, 1],
    [10, 1],
    [10, 10],
  ] as const)
    await page.mouse.click(...(await mid(page, 'pixel', x, y)));
  await page.keyboard.press('Enter');
  const img = await copySelection(page);
  expect(img.width).toBeGreaterThan(1);
  expect(img.width).toBeLessThan(PX);
  // A new polygon, then Escape: the selection made above stays.
  await page.mouse.click(...(await mid(page, 'pixel', 12, 12)));
  await page.mouse.click(...(await mid(page, 'pixel', 14, 12)));
  await page.keyboard.press('Escape');
  expect(await size(page)).toEqual([img.width, img.height]);
});

test('pixel: Shift adds, Alt subtracts, Shift+Alt intersects (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await marquee(page, 'pixel', 0, 0, 3, 3);
  await page.keyboard.down('Shift');
  await marquee(page, 'pixel', 6, 0, 9, 3);
  await page.keyboard.up('Shift');
  expect(await size(page)).toEqual([10, 4]);
  await page.keyboard.down('Alt');
  await marquee(page, 'pixel', 6, 0, 9, 3);
  await page.keyboard.up('Alt');
  expect(await size(page)).toEqual([4, 4]);
  await page.keyboard.down('Shift');
  await page.keyboard.down('Alt');
  await marquee(page, 'pixel', 2, 2, 8, 8);
  await page.keyboard.up('Alt');
  await page.keyboard.up('Shift');
  expect(await size(page)).toEqual([2, 2]);
});

test('pixel: Ctrl+A, Ctrl+Shift+I and Ctrl+D select all, invert and deselect (AC-12)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await marquee(page, 'pixel', 0, 0, 3, 15);
  await page.keyboard.press('ControlOrMeta+Shift+i');
  expect(await size(page)).toEqual([12, 16]);
  await page.keyboard.press('ControlOrMeta+d');
  await menu(page, 'Select');
  await expect(page.getByRole('menuitem', { name: 'Deselect', exact: true })).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.keyboard.press('ControlOrMeta+a');
  await menu(page, 'Select');
  await expect(page.getByRole('menuitem', { name: 'Deselect', exact: true })).toBeEnabled();
  await page.keyboard.press('Escape');
});

test('pixel: Delete clears the selected pixels as one undo entry (AC-12)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await paintRed(page, 'pixel', 4, 4);
  await expect(pxDepths(page, 1)).toBeVisible();
  await marquee(page, 'pixel', 3, 3, 5, 5);
  await page.keyboard.press('Delete');
  await expect(pxDepths(page, 2)).toBeVisible();
});

test('pixel: a Move drag released where it started changes nothing (AC-17)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await paintRed(page, 'pixel', 4, 4);
  await marquee(page, 'pixel', 3, 3, 5, 5);
  await page.keyboard.press('v');
  const p = await mid(page, 'pixel', 4, 4);
  await page.mouse.move(...p);
  await page.mouse.down();
  await page.mouse.move(p[0] + 40, p[1], { steps: 3 });
  await page.mouse.move(...p, { steps: 3 });
  await page.mouse.up();
  await expect(pxDepths(page, 1)).toBeVisible();
});

test('pixel: the Move tool with nothing selected pans and leaves the document alone (AC-18)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await page.keyboard.press('v');
  const before = await pixelCanvas(page).boundingBox();
  const p = await mid(page, 'pixel', 8, 8);
  await dragFrom(page, p, [p[0] + 25, p[1] + 15]);
  const after = await pixelCanvas(page).boundingBox();
  expect(after?.x).toBeCloseTo((before?.x ?? 0) + 25, 0);
  await expect(pxDepths(page, 0)).toBeVisible();
});
