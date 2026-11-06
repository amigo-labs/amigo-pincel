import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  pixelCanvas,
  menu,
  setColor,
  storedPref,
  waitForEditor,
} from './helpers';
import { effectPanel } from './effect-panel-helpers';
import { liveInput, textBox, undoButton } from './text-helpers';
import { dropFiles, tabNames } from './tab-helpers';
import { closeWindow, destroyed, installTauriMock, launchDesktop } from './tauri-mock';

// Recent colors (color-picker AC-12 – AC-15, AC-24, AC-30): a color joins
// its mode's recent row when a tool paints with it, most recent first,
// without duplicates; the row and the last colors survive a reload.

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** The recent row's colors as their accessible names (hex), in order. */
async function recent(page: Page): Promise<string[]> {
  await page.getByRole('button', { name: 'Foreground color', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
  const names = await picker
    .getByRole('group', { name: 'Recent colors' })
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);
  return names;
}

/** Image mode's Colors panel recent row, as accessible names (hex). */
function panelRecent(page: Page): Promise<string[]> {
  return page
    .getByRole('region', { name: 'Colors' })
    .getByRole('group', { name: 'Recent colors' })
    .last()
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
}

/** R / G / B / A of a swatch, read from its picker. */
async function rgbaOf(page: Page, which: 'fg' | 'bg'): Promise<number[]> {
  const name = which === 'fg' ? 'Foreground color' : 'Background color';
  await page.getByRole('button', { name, exact: true }).click();
  const picker = page.getByRole('dialog', { name: `${name} picker` });
  const out: number[] = [];
  for (const label of ['Red', 'Green', 'Blue', 'Alpha value']) {
    out.push(
      Number(await picker.getByRole('spinbutton', { name: label, exact: true }).inputValue()),
    );
  }
  await page.keyboard.press('Escape');
  return out;
}

async function stroke(page: Page, canvas: Locator, y: number, button: 'left' | 'right' = 'left') {
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * y);
  await page.mouse.down({ button });
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * y, { steps: 4 });
  await page.mouse.up({ button });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: painting adds its color, most recent first, once (AC-12, AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const canvas = canvasOf(page, mode);
    expect(await recent(page)).toEqual([]);

    await setColor(page, 'fg', '#11223380');
    await page.keyboard.press('b');
    await stroke(page, canvas, 0.2);
    expect(await recent(page)).toEqual(['#11223380']);
    // The same color again: no change.
    await stroke(page, canvas, 0.3);
    expect(await recent(page)).toEqual(['#11223380']);

    // The right button paints the background color.
    await setColor(page, 'bg', '#00ff00ff');
    await stroke(page, canvas, 0.4, 'right');
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // The Eraser and the Eyedropper add nothing.
    await setColor(page, 'fg', '#0000ffff');
    await page.keyboard.press('e');
    await stroke(page, canvas, 0.2);
    await page.keyboard.press('i');
    await stroke(page, canvas, 0.4);
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // A stroke beside the canvas changes no pixel and adds nothing.
    await setColor(page, 'fg', '#ff00ffff');
    await page.keyboard.press('b');
    await drag(page, canvas, [1.05, 0.5], [1.2, 0.5]);
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // Paint Bucket with the foreground moves an old color to the front.
    await setColor(page, 'fg', '#11223380');
    await page.keyboard.press('g');
    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.9);
    expect(await recent(page)).toEqual(['#11223380', '#00ff00']);
  });

  test(`${mode}: a filled and outlined shape adds the foreground, then the background (AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await setColor(page, 'fg', '#ff0000ff');
    await setColor(page, 'bg', '#0000ffff');
    await page.keyboard.press('u');
    await page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name: 'Rectangle', exact: true })
      .click();
    if (mode === 'pixel') await page.getByLabel('Shape mode').selectOption('fill_outline');
    else {
      await page
        .getByRole('combobox', { name: 'Mode', exact: true })
        .selectOption('fill_and_outline');
    }
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.8]);
    expect(await recent(page)).toEqual(['#ff0000', '#0000ff']);
  });
}

test('pixel: a right-button filled and outlined shape adds the foreground first too (AC-24)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await setColor(page, 'fg', '#ff0000ff');
  await setColor(page, 'bg', '#0000ffff');
  await page.keyboard.press('u');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name: 'Rectangle', exact: true })
    .click();
  await page.getByLabel('Shape mode').selectOption('fill_outline');
  // The right button outlines with the background and fills with the
  // foreground; the foreground still comes first.
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.2);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.8, { steps: 4 });
  await page.mouse.up({ button: 'right' });
  expect(await recent(page)).toEqual(['#ff0000', '#0000ff']);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: placed text adds the foreground (AC-24)`, async ({ page }) => {
    await createDocument(page, mode, { width: 48, height: 48 });
    await setColor(page, 'fg', '#ff0000ff');
    await setColor(page, 'bg', '#0000ffff');
    await page.keyboard.press('t');
    const box = await canvasOf(page, mode).boundingBox();
    if (!box) throw new Error('canvas is not visible');
    await page.mouse.click(box.x + box.width * 0.1, box.y + box.height * 0.2);
    if (mode === 'pixel') {
      const dialog = page.getByRole('dialog', { name: 'Text' });
      await dialog.getByLabel('Text content').fill('Hi');
      await dialog.getByRole('button', { name: 'Place' }).click();
      await expect(dialog).toBeHidden();
    } else {
      // The live text (image-live-text): type into it, Ctrl+Enter places it.
      await expect(liveInput(page)).toBeFocused();
      await page.keyboard.type('Hi');
      await page.keyboard.press('Control+Enter');
      await expect(textBox(page)).toHaveCount(0);
      await expect(undoButton(page)).toBeEnabled();
    }
    await expect.poll(() => recent(page)).toEqual(['#ff0000']);
  });
}

test('image: a live line adds the foreground, a live gradient both its colors (AC-24)', async ({
  page,
}) => {
  // Line and Gradient drags create live objects (image-live-objects-ui); the
  // drag that creates one counts as painting (color-picker Decisions 2026-11-06).
  await createDocument(page, 'image', { width: 32, height: 32 });
  await setColor(page, 'fg', '#ff0000ff');
  await setColor(page, 'bg', '#0000ffff');
  await page.keyboard.press('l');
  await drag(page, imageCanvas(page), [0.2, 0.3], [0.8, 0.3]);
  expect(await recent(page)).toEqual(['#ff0000']);

  // Commit first, or the new foreground would edit the live line.
  await page.keyboard.press('Enter');
  await setColor(page, 'fg', '#00ff00ff');
  await page.keyboard.press('n');
  await drag(page, imageCanvas(page), [0.2, 0.6], [0.8, 0.6]);
  expect(await recent(page)).toEqual(['#00ff00', '#0000ff', '#ff0000']);
});

test('pixel: a line adds the color it draws with, a gradient both its colors (AC-24)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const canvas = pixelCanvas(page);
  await setColor(page, 'fg', '#ff0000ff');
  await setColor(page, 'bg', '#ffff00ff');
  await page.keyboard.press('l');
  await stroke(page, canvas, 0.2);
  expect(await recent(page)).toEqual(['#ff0000']);
  // The right button draws the line in the background color.
  await stroke(page, canvas, 0.35, 'right');
  expect(await recent(page)).toEqual(['#ffff00', '#ff0000']);

  // Gradient (G twice): foreground at the press, background at the release.
  await setColor(page, 'fg', '#00ff00ff');
  await setColor(page, 'bg', '#0000ffff');
  await page.keyboard.press('g');
  await page.keyboard.press('g');
  await expect(page.getByLabel('Gradient kind')).toBeVisible();
  await stroke(page, canvas, 0.6);
  expect(await recent(page)).toEqual(['#00ff00', '#0000ff', '#ffff00', '#ff0000']);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a locked layer, the marquee and adjustments add nothing, nor does an Image stroke outside the selection (AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const canvas = canvasOf(page, mode);
    const layers = page.getByRole('list', { name: 'Layer list' });
    await setColor(page, 'fg', '#ff0000ff');
    await page.keyboard.press('b');

    // A locked layer changes no pixel. (A hidden one does: both modes
    // paint on it, see the spec's Decisions.)
    await layers.getByRole('button', { name: /^Lock / }).first().click();
    await stroke(page, canvas, 0.5);
    await layers.getByRole('button', { name: /^Unlock / }).first().click();
    expect(await recent(page)).toEqual([]);

    // The marquee adds nothing.
    await page.keyboard.press('m');
    await drag(page, canvas, [0.05, 0.05], [0.3, 0.3]);
    expect(await recent(page)).toEqual([]);
    if (mode === 'image') {
      // Image mode's Brush keeps to the selection: a stroke outside it
      // changes no pixel and adds nothing. (Pixel mode's Pencil paints
      // regardless of the selection.)
      await page.keyboard.press('b');
      await stroke(page, canvas, 0.8);
      expect(await recent(page)).toEqual([]);
    }

    // An adjustment changes pixels but paints with no chosen color.
    await setColor(page, 'fg', '#00ff00ff');
    await page.keyboard.press('Control+a');
    await page.keyboard.press('b');
    await stroke(page, canvas, 0.8);
    expect(await recent(page)).toEqual(['#00ff00']);
    // Neither the foreground nor the background joins the row.
    await setColor(page, 'fg', '#0000ffff');
    await setColor(page, 'bg', '#ffff00ff');
    await menu(page, 'Adjustments', 'Invert Colors');
    const panel = effectPanel(page, 'Invert Colors');
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(panel).toHaveCount(0);
    expect(await recent(page)).toEqual(['#00ff00']);
  });
}

test('pixel: a fill that changes no pixel adds nothing (AC-24)', async ({ page }) => {
  // The fill reaches the core, which adds no undo step for it: the
  // recent row records changes that altered pixels only.
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const canvas = pixelCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  const bucketAt = (y: number) => page.mouse.click(box.x + box.width * 0.5, box.y + box.height * y);
  await setColor(page, 'fg', '#ff0000ff');
  await page.keyboard.press('g');
  await bucketAt(0.9);
  await setColor(page, 'fg', '#0000ffff');
  await page.keyboard.press('b');
  await stroke(page, canvas, 0.2);
  expect(await recent(page)).toEqual(['#0000ff', '#ff0000']);

  // Red into the area that is red already.
  await setColor(page, 'fg', '#ff0000ff');
  await page.keyboard.press('g');
  await bucketAt(0.9);
  expect(await recent(page)).toEqual(['#0000ff', '#ff0000']);
});

test('image: the Eraser to the background adds nothing (AC-24)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const canvas = imageCanvas(page);
  await setColor(page, 'fg', '#ff0000ff');
  await page.keyboard.press('b');
  await stroke(page, canvas, 0.5);
  expect(await recent(page)).toEqual(['#ff0000']);

  // A background no tool painted with yet, erased onto the red stroke.
  await setColor(page, 'bg', '#00ff00ff');
  await page.keyboard.press('e');
  await page.getByRole('combobox', { name: 'Mode', exact: true }).selectOption('to_background');
  await stroke(page, canvas, 0.5);
  expect(await recent(page)).toEqual(['#ff0000']);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: an effect whose colors start at the foreground and background adds nothing (AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    // Clouds paints with Color 1 = foreground and Color 2 = background.
    await setColor(page, 'fg', '#ff0000ff');
    await setColor(page, 'bg', '#0000ffff');
    await menu(page, 'Effects', 'Render', 'Clouds…');
    const panel = effectPanel(page, 'Clouds');
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(panel).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
    expect(await recent(page)).toEqual([]);
  });
}

test('pixel: an indexed document is refused, so nothing is painted in one (AC-24)', async ({
  page,
}) => {
  // Indexed painting and reading are Phase 2 (pincel.md §15, 2026-10-29):
  // Pixel mode opens no indexed document, and the recent row stays.
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#ff0000ff');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  await dropFiles(page, [{ name: 'indexed.aseprite', bytes: indexedAseprite() }]);
  await expect(page.getByRole('contentinfo')).toContainText(
    'Open failed: failed to open Aseprite: unsupported aseprite color mode',
  );
  expect(await tabNames(page)).toEqual(['pincel.aseprite']);
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.7);
  expect(await recent(page)).toEqual(['#ff0000']);
});

/** A valid 2 × 2 indexed (8 bpp) `.aseprite`: one layer, a two-color
 *  palette and one raw cel (ase-file-specs.md). */
function indexedAseprite(): Buffer {
  const chunk = (type: number, body: Buffer): Buffer => {
    const head = Buffer.alloc(6);
    head.writeUInt32LE(6 + body.length, 0);
    head.writeUInt16LE(type, 4);
    return Buffer.concat([head, body]);
  };
  const name = Buffer.from('Layer 1');
  const layer = Buffer.alloc(18 + name.length);
  layer.writeUInt16LE(3, 0); // visible, editable
  layer.writeUInt8(255, 12); // opacity
  layer.writeUInt16LE(name.length, 16);
  name.copy(layer, 18);
  const palette = Buffer.alloc(20 + 2 * 6);
  palette.writeUInt32LE(2, 0);
  palette.writeUInt32LE(1, 8); // last index
  palette.set([0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 255], 20);
  const cel = Buffer.alloc(16 + 4 + 4);
  cel.writeUInt8(255, 6); // opacity
  cel.writeUInt16LE(2, 16);
  cel.writeUInt16LE(2, 18);
  cel.set([1, 0, 0, 1], 20);
  const chunks = [chunk(0x2004, layer), chunk(0x2019, palette), chunk(0x2005, cel)];
  const body = Buffer.concat(chunks);
  const frame = Buffer.alloc(16);
  frame.writeUInt32LE(16 + body.length, 0);
  frame.writeUInt16LE(0xf1fa, 4);
  frame.writeUInt16LE(chunks.length, 6);
  frame.writeUInt16LE(100, 8);
  frame.writeUInt32LE(chunks.length, 12);
  const header = Buffer.alloc(128);
  header.writeUInt32LE(128 + 16 + body.length, 0);
  header.writeUInt16LE(0xa5e0, 4);
  header.writeUInt16LE(1, 6); // frames
  header.writeUInt16LE(2, 8);
  header.writeUInt16LE(2, 10);
  header.writeUInt16LE(8, 12); // color depth: indexed
  header.writeUInt32LE(1, 14);
  header.writeUInt16LE(100, 18);
  header.writeUInt16LE(2, 32); // colors
  header.writeUInt8(1, 34);
  header.writeUInt8(1, 35);
  header.writeUInt16LE(16, 40);
  header.writeUInt16LE(16, 42);
  return Buffer.concat([header, frame, body]);
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the 13th color drops the oldest; the same RGB with another alpha is another color (AC-12)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    // Image mode shows the row in its Colors panel, Pixel mode in the picker.
    const names = () => (mode === 'image' ? panelRecent(page) : recent(page));
    const box = await canvasOf(page, mode).boundingBox();
    if (!box) throw new Error('canvas is not visible');
    /** A click on its own pixel, so every click alters one. */
    const paintAt = (i: number, row: number) =>
      page.mouse.click(box.x + (box.width * (i + 1.5)) / 16, box.y + (box.height * row) / 16);
    await page.keyboard.press('b');
    const colors = Array.from(
      { length: 13 },
      (_, i) => `#0000${(i * 16).toString(16).padStart(2, '0')}`,
    );
    for (const [i, c] of colors.entries()) {
      await setColor(page, 'fg', `${c}ff`);
      await paintAt(i, 8.5);
    }
    await expect.poll(names).toEqual(colors.slice(1).reverse());
    // Same RGB, alpha 128: a new entry, the opaque one stays.
    await setColor(page, 'fg', `${colors[12]}80`);
    await paintAt(0, 3.5);
    await expect.poll(names).toEqual([`${colors[12]}80`, ...colors.slice(2).reverse()]);
  });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a right-click Paint Bucket adds the background color (AC-24)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await setColor(page, 'fg', '#ff0000ff');
    await setColor(page, 'bg', '#00ff00ff');
    await page.keyboard.press('g');
    const box = await canvasOf(page, mode).boundingBox();
    if (!box) throw new Error('canvas is not visible');
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: 'right' });
    await expect.poll(() => recent(page)).toEqual(['#00ff00']);
  });
}

test('a recent color sets the edited color, alpha included (AC-13)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#11223380');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  await setColor(page, 'bg', '#ffffffff');
  await page.getByRole('button', { name: 'Background color', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Background color picker' });
  await picker
    .getByRole('group', { name: 'Recent colors' })
    .getByRole('button', { name: '#11223380' })
    .click();
  await page.keyboard.press('Escape');
  expect(await rgbaOf(page, 'bg')).toEqual([17, 34, 51, 128]);

  // The foreground picker sets the foreground, not the background.
  await setColor(page, 'fg', '#000000ff');
  await setColor(page, 'bg', '#ffffffff');
  await page.getByRole('button', { name: 'Foreground color', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Foreground color picker' })
    .getByRole('group', { name: 'Recent colors' })
    .getByRole('button', { name: '#11223380' })
    .click();
  await page.keyboard.press('Escape');
  expect(await rgbaOf(page, 'fg')).toEqual([17, 34, 51, 128]);
  expect(await rgbaOf(page, 'bg')).toEqual([255, 255, 255, 255]);
});

test('each mode keeps its own recent colors (AC-14)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#123456ff');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  expect(await recent(page)).toEqual(['#123456']);
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual([]);
});

test('each mode keeps its own recent colors within one page load (AC-14)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#123456ff');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  expect(await recent(page)).toEqual(['#123456']);
  // File ▸ New… opens an Image tab beside the Pixel one, no reload.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByLabel('Width').fill('16');
  await dialog.getByLabel('Height').fill('16');
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  expect(await recent(page)).toEqual([]);
  expect(await panelRecent(page)).toEqual([]);
  await setColor(page, 'fg', '#abcdefff');
  await page.keyboard.press('b');
  await stroke(page, imageCanvas(page), 0.5);
  expect(await recent(page)).toEqual(['#abcdef']);
  await expect.poll(() => panelRecent(page)).toEqual(['#abcdef']);
  // Back in the Pixel tab: its row lacks the Image color.
  const tabs = page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
  await tabs.first().click();
  await expect(pixelCanvas(page)).toBeVisible();
  expect(await recent(page)).toEqual(['#123456']);
});

test('recent colors and the last colors survive a reload, per mode (AC-15)', async ({ page }) => {
  for (const [mode, colors] of [
    ['pixel', ['#11223380', '#445566ff']],
    ['image', ['#aabbcc40', '#010203ff']],
  ] as const) {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('b');
    for (const [i, c] of colors.entries()) {
      await setColor(page, 'fg', c);
      await stroke(page, canvasOf(page, mode), 0.3 + i * 0.3);
    }
    await setColor(page, 'bg', mode === 'pixel' ? '#99887766' : '#55443322');
    await expect
      .poll(async () => ((await storedPref(page, `recentColors:${mode}`)) as unknown[])?.length)
      .toBe(2);
  }
  await page.waitForTimeout(300);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#445566', '#11223380']);
  expect(await rgbaOf(page, 'fg')).toEqual([0x44, 0x55, 0x66, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([0x99, 0x88, 0x77, 0x66]);
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#010203', '#aabbcc40']);
  // Image mode's Colors panel shows the same row, without the picker.
  await expect.poll(() => panelRecent(page)).toEqual(['#010203', '#aabbcc40']);
  expect(await rgbaOf(page, 'fg')).toEqual([1, 2, 3, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([0x55, 0x44, 0x33, 0x22]);
});

test.describe('desktop', () => {
  test.beforeEach(async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
  });

  test('recent colors and the last colors survive quitting the app (AC-15)', async ({ page }) => {
    await launchDesktop(page);
    await page.getByRole('button', { name: 'New image' }).click();
    const card = page.getByRole('form', { name: 'New image' });
    for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
    await setColor(page, 'fg', '#aabbcc40');
    await page.keyboard.press('b');
    await stroke(page, imageCanvas(page), 0.5);
    await setColor(page, 'bg', '#55443322');
    await expect.poll(() => storedPref(page, 'lastBgColor:image')).toBe('#55443322');
    await expect.poll(() => storedPref(page, 'recentColors:image')).toEqual(['#aabbcc40']);

    // Quit (the window's close button), then start the app again.
    await closeWindow(page);
    await page
      .getByRole('alertdialog', { name: 'Unsaved changes' })
      .getByRole('button', { name: "Don't Save" })
      .click();
    await expect.poll(() => destroyed(page)).toBe(1);
    await launchDesktop(page);
    await page.getByRole('button', { name: 'New image' }).click();
    await page
      .getByRole('form', { name: 'New image' })
      .getByRole('button', { name: 'Create' })
      .click();
    await waitForEditor(page, 'image');
    await expect.poll(() => panelRecent(page)).toEqual(['#aabbcc40']);
    expect(await rgbaOf(page, 'fg')).toEqual([0xaa, 0xbb, 0xcc, 0x40]);
    expect(await rgbaOf(page, 'bg')).toEqual([0x55, 0x44, 0x33, 0x22]);
  });
});

test('unreadable stored colors fall back silently (AC-30)', async ({ page }) => {
  await gotoStart(page);
  await page.evaluate(
    (entries) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          for (const [key, value] of entries) tx.objectStore('prefs').put({ key, value });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(new Error('pref write failed'));
        };
        open.onerror = () => reject(new Error('IndexedDB unavailable'));
      }),
    [
      ['recentColors:image', ['#zzz', 7, '#11223380']],
      ['recentColors:pixel', 'not a list'],
      ['lastColor:image', 42],
      ['lastBgColor:image', '#12'],
      // Pixel mode: a readable foreground stays, the background falls back.
      ['lastColor', '#11223380'],
      ['lastBgColor', ['#ff0000']],
    ] as const,
  );
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#11223380']);
  expect(await rgbaOf(page, 'fg')).toEqual([0, 0, 0, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([255, 255, 255, 255]);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await recent(page)).toEqual([]);
  expect(await rgbaOf(page, 'fg')).toEqual([17, 34, 51, 128]);
  expect(await rgbaOf(page, 'bg')).toEqual([255, 255, 255, 255]);
  await expect(page.getByRole('alert')).toHaveCount(0);
});
