import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  menuHint,
  pixelCanvas,
  pixelOf,
  setColor,
  switchLanguage,
  type ClipImage,
} from './helpers';

// Image mode: Edit ▸ Fill Selection and its keys
// (docs/specs/image-edit-extras.md). Pixels are read through Copy (the
// selection's box, or the whole layer without one), decoded test-side.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const SIZE = 32;
const PX = 16;
const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];
const CLEAR = [0, 0, 0, 0];

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const dirtyMarker = (page: Page) => page.getByTitle(/ — unsaved changes$/);

/** Screen point of the document position `(x, y)` (pixel corners at integers). */
async function point(page: Page, x: number, y: number, size = SIZE): Promise<[number, number]> {
  const canvas = size === SIZE ? imageCanvas(page) : pixelCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / size, box.y + (y * box.height) / size];
}

/** Rectangle Select of the document rect `(x, y, w, h)`. */
async function select(page: Page, x: number, y: number, w: number, h: number): Promise<void> {
  await page.keyboard.press('m');
  await page.mouse.move(...(await point(page, x, y)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, x + w, y + h)), { steps: 4 });
  await page.mouse.up();
}

/** A 32×32 Image document whose only layer is red. */
async function redImage(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#ff0000');
  await page.keyboard.press('g');
  await page.mouse.click(...(await point(page, 1.5, 1.5)));
  await expect(undoButton(page)).toBeEnabled();
}

/** What Copy puts on the clipboard: the selection's box, else the layer. */
async function copy(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  return copiedImage(page);
}

/** Every pixel of `img` equals `rgba`. */
function all(img: ClipImage, rgba: number[]): boolean {
  for (let y = 0; y < img.height; y += 1) {
    for (let x = 0; x < img.width; x += 1) {
      if (pixelOf(img, x, y).join() !== rgba.join()) return false;
    }
  }
  return true;
}

/** Menu item labels and separators (`''`) of the open Edit menu, in order. */
async function editEntries(page: Page, top: string): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  const edit = page.getByRole('menu', { name: top, exact: true });
  await expect(edit).toBeVisible();
  const entries = await edit
    .getByRole('menuitem')
    .or(edit.getByRole('separator'))
    .evaluateAll((els) =>
      els.map((el) =>
        el.getAttribute('role') === 'separator'
          ? ''
          : (el.querySelector('span.flex-1')?.textContent ?? '').trim(),
      ),
    );
  await page.keyboard.press('Escape');
  return entries;
}

test('Edit ▸ Fill Selection sits below Delete Selection with its key, in English and German (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  const entries = await editEntries(page, 'Edit');
  expect(entries[entries.indexOf('Delete Selection') + 1]).toBe('Fill Selection');
  expect(await menuHint(page, 'Edit', 'Fill Selection')).toBe('Backspace');
  expect(await menuHint(page, 'Edit', 'Delete Selection')).toBe('Del');
  // Enabled without a selection, while Delete Selection is not.
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
  const edit = page.getByRole('menu', { name: 'Edit', exact: true });
  await expect(edit.getByRole('menuitem', { name: 'Fill Selection', exact: true })).toBeEnabled();
  await expect(
    edit.getByRole('menuitem', { name: 'Delete Selection', exact: true }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');

  await switchLanguage(page, 'Deutsch');
  const de = await editEntries(page, 'Bearbeiten');
  expect(de[de.indexOf('Auswahl löschen') + 1]).toBe('Auswahl füllen');
  expect(await menuHint(page, 'Bearbeiten', 'Auswahl füllen')).toBe('Backspace');
});

test('Fill Selection paints the selected pixels exactly with the foreground color (AC-02)', async ({
  page,
}) => {
  await redImage(page);
  await select(page, 8, 4, 6, 3);
  await setColor(page, 'fg', '#00ff0080');
  await menu(page, 'Edit', 'Fill Selection');
  const box = await copy(page);
  expect([box.width, box.height]).toEqual([6, 3]);
  expect(all(box, [0, 255, 0, 128])).toBe(true);
  // Outside the selection nothing changed.
  await page.keyboard.press('ControlOrMeta+d');
  const layer = await copy(page);
  expect([layer.width, layer.height]).toEqual([SIZE, SIZE]);
  for (const [x, y] of [
    [7, 4],
    [14, 4],
    [8, 3],
    [8, 7],
    [0, 0],
    [31, 31],
  ] as const) {
    expect(pixelOf(layer, x, y), `(${x}, ${y})`).toEqual(RED);
  }
  expect(pixelOf(layer, 13, 6)).toEqual([0, 255, 0, 128]);
});

test('Backspace without a selection fills the whole layer (AC-04, AC-14)', async ({ page }) => {
  await redImage(page);
  await setColor(page, 'fg', '#0000ff');
  await imageCanvas(page).focus();
  await page.keyboard.press('Backspace');
  const layer = await copy(page);
  expect([layer.width, layer.height]).toEqual([SIZE, SIZE]);
  expect(all(layer, BLUE)).toBe(true);
});

test('Fill Selection is one undo step that keeps the selection; Redo repeats it (AC-05)', async ({
  page,
}) => {
  await redImage(page);
  await select(page, 8, 4, 6, 3);
  await setColor(page, 'fg', '#0000ff');
  await page.keyboard.press('Backspace');
  let box = await copy(page);
  expect([box.width, box.height]).toEqual([6, 3]);
  expect(all(box, BLUE)).toBe(true);

  await page.keyboard.press('ControlOrMeta+z');
  box = await copy(page);
  expect([box.width, box.height], 'the selection is still there').toEqual([6, 3]);
  expect(all(box, RED)).toBe(true);

  await page.keyboard.press('ControlOrMeta+Shift+z');
  box = await copy(page);
  expect([box.width, box.height]).toEqual([6, 3]);
  expect(all(box, BLUE)).toBe(true);

  // Undo twice: the fill, then the selection, then the bucket fill is next.
  await page.keyboard.press('ControlOrMeta+z');
  await page.keyboard.press('ControlOrMeta+z');
  const layer = await copy(page);
  expect([layer.width, layer.height]).toEqual([SIZE, SIZE]);
  expect(all(layer, RED)).toBe(true);
  await page.keyboard.press('ControlOrMeta+z');
  await expect(undoButton(page)).toBeDisabled();
});

test('a Fill Selection that changes nothing adds no undo step and no unsaved changes (AC-06)', async ({
  page,
}) => {
  // A new image is transparent: filling it with transparent black is a no-op.
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#00000000');
  await expect(dirtyMarker(page)).toHaveCount(0);
  await menu(page, 'Edit', 'Fill Selection');
  await expect(undoButton(page)).toBeDisabled();
  await expect(dirtyMarker(page)).toHaveCount(0);

  // The same fill twice: one Undo takes back the first. (Eight digits:
  // the picker keeps the alpha a six-digit hex leaves out.)
  await setColor(page, 'fg', '#ff0000ff');
  await page.keyboard.press('g');
  await page.mouse.click(...(await point(page, 1.5, 1.5)));
  await select(page, 2, 2, 4, 4);
  await setColor(page, 'fg', '#0000ffff');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('ControlOrMeta+z');
  const box = await copy(page);
  expect([box.width, box.height]).toEqual([4, 4]);
  expect(all(box, RED)).toBe(true);
});

test('Fill Selection on a locked layer or a group shows the locked notice and changes nothing (AC-08)', async ({
  page,
}) => {
  await redImage(page);
  await page.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
  await setColor(page, 'fg', '#0000ff');
  const notice = page.getByText('Layer “Layer 1” is locked', { exact: true });
  await expect(notice).toHaveCount(0);
  await menu(page, 'Edit', 'Fill Selection');
  await expect(notice).toBeVisible();
  expect(all(await copy(page), RED)).toBe(true);

  // A group has no pixels: Backspace is refused with the same notice.
  await page.getByRole('button', { name: 'Add group' }).click();
  const groupNotice = page.getByText('Layer “Group 1” is locked', { exact: true });
  await expect(groupNotice).toHaveCount(0);
  await page.keyboard.press('Backspace');
  await expect(groupNotice).toBeVisible();
});

/** The panel row of layer `name`. */
function layerRow(page: Page, name: string) {
  return page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });
}

/** Makes layer `name` active through its name button. */
async function activate(page: Page, name: string): Promise<void> {
  await layerRow(page, name)
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click();
}

/** The whole of layer `name`, read through Copy while it is active. */
async function layerPixels(page: Page, name: string): Promise<ClipImage> {
  await activate(page, name);
  return copy(page);
}

/** Hides the active layer `name` and sets its opacity to `percent`. */
async function hideAndFade(page: Page, name: string, percent: number): Promise<void> {
  await page.getByRole('button', { name: `Hide ${name}`, exact: true }).click();
  await page.getByLabel('Layer opacity').fill(String(percent));
}

test('Fill Selection fills only the active layer, also hidden and translucent (AC-02, AC-04, AC-09)', async ({
  page,
}) => {
  await redImage(page);
  await page.keyboard.press('Shift+N');
  const list = page.getByRole('list', { name: 'Layer list' });
  await expect(list.getByRole('button', { name: /^Drag to reorder / })).toHaveCount(2);
  const top = 'Layer 2';
  const bottom = 'Layer 1';

  // The upper layer active, hidden at 50 %: its selection is filled.
  await hideAndFade(page, top, 50);
  await select(page, 8, 4, 6, 3);
  await setColor(page, 'fg', '#0000ff');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('ControlOrMeta+d');
  let upper = await layerPixels(page, top);
  expect(pixelOf(upper, 8, 4)).toEqual(BLUE);
  expect(pixelOf(upper, 13, 6)).toEqual(BLUE);
  expect(pixelOf(upper, 7, 4)).toEqual(CLEAR);
  expect(all(await layerPixels(page, bottom), RED), 'lower layer unchanged').toBe(true);
  await expect(page.getByRole('button', { name: `Show ${top}`, exact: true })).toBeVisible();

  // The lower layer active, hidden at 40 %: the whole of it is filled.
  await activate(page, bottom);
  await hideAndFade(page, bottom, 40);
  await setColor(page, 'fg', '#00ff00');
  await menu(page, 'Edit', 'Fill Selection');
  expect(all(await layerPixels(page, bottom), [0, 255, 0, 255])).toBe(true);
  upper = await layerPixels(page, top);
  expect(pixelOf(upper, 8, 4), 'upper layer unchanged').toEqual(BLUE);
  expect(pixelOf(upper, 0, 0)).toEqual(CLEAR);
  // Both layers keep their visibility and opacity.
  await expect(page.getByRole('button', { name: `Show ${top}`, exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: `Show ${bottom}`, exact: true })).toBeVisible();
  await expect(page.getByLabel('Layer opacity')).toHaveValue('50');
  await activate(page, bottom);
  await expect(page.getByLabel('Layer opacity')).toHaveValue('40');
});

test('Delete runs Delete Selection; in Pixel mode Backspace does nothing (AC-14)', async ({
  page,
}) => {
  await redImage(page);
  await select(page, 8, 4, 6, 3);
  await page.keyboard.press('Delete');
  const box = await copy(page);
  expect(all(box, CLEAR)).toBe(true);

  await createDocument(page, 'pixel', { width: PX, height: PX });
  await setColor(page, 'fg', '#ff0000');
  await page.keyboard.press('b');
  await page.mouse.click(...(await point(page, 4.5, 4.5, PX)));
  await page.keyboard.press('m');
  await page.mouse.move(...(await point(page, 3.5, 3.5, PX)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, 5.5, 5.5, PX)), { steps: 5 });
  await page.mouse.up();
  // A blue foreground: a fill would show on the marquee's empty pixels.
  await setColor(page, 'fg', '#0000ff');
  await page.keyboard.press('Backspace');
  const marquee = await copy(page);
  expect([marquee.width, marquee.height]).toEqual([3, 3]);
  expect(pixelOf(marquee, 1, 1), 'Backspace leaves the marquee').toEqual(RED);
  expect(pixelOf(marquee, 0, 0), 'and fills nothing').toEqual(CLEAR);
  expect(pixelOf(marquee, 2, 2)).toEqual(CLEAR);
  await page.keyboard.press('Delete');
  expect(pixelOf(await copy(page), 1, 1), 'Delete clears it').toEqual(CLEAR);
});

test.describe('on macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36',
  });

  test('⌥Backspace fills; Backspace and Delete delete in both modes (AC-14)', async ({ page }) => {
    await redImage(page);
    expect(await menuHint(page, 'Edit', 'Fill Selection')).toBe('⌥Backspace');
    await select(page, 8, 4, 6, 3);
    await setColor(page, 'fg', '#0000ff');
    await page.keyboard.press('Alt+Backspace');
    expect(all(await copy(page), BLUE)).toBe(true);
    await page.keyboard.press('Backspace');
    expect(all(await copy(page), CLEAR)).toBe(true);
    await page.keyboard.press('ControlOrMeta+z');
    expect(all(await copy(page), BLUE)).toBe(true);
    await page.keyboard.press('Delete');
    expect(all(await copy(page), CLEAR)).toBe(true);

    await createDocument(page, 'pixel', { width: PX, height: PX });
    await setColor(page, 'fg', '#ff0000');
    await page.keyboard.press('b');
    await page.mouse.click(...(await point(page, 4.5, 4.5, PX)));
    await page.keyboard.press('m');
    await page.mouse.move(...(await point(page, 3.5, 3.5, PX)));
    await page.mouse.down();
    await page.mouse.move(...(await point(page, 5.5, 5.5, PX)), { steps: 5 });
    await page.mouse.up();
    await page.keyboard.press('Backspace');
    expect(pixelOf(await copy(page), 1, 1), 'Backspace clears the marquee').toEqual(CLEAR);
    await page.keyboard.press('ControlOrMeta+z');
    expect(pixelOf(await copy(page), 1, 1)).toEqual(RED);
    await page.keyboard.press('Delete');
    expect(pixelOf(await copy(page), 1, 1), 'Delete clears it').toEqual(CLEAR);
  });
});
