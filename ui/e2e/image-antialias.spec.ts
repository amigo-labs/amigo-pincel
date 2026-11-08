import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  setColor,
  waitForEditor,
  type ClipImage,
} from './helpers';

// The shared Antialiasing tool option in Image mode
// (docs/specs/image-edit-extras-tool-options.md, Antialiasing block). Pixels
// are read from the clipboard after a Copy of the whole layer.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const SIZE = 32;
const antiAlias = (page: Page) => page.getByRole('checkbox', { name: 'Antialiasing' });

/** Picks the tool `name` of the rail group behind `key`. */
async function useTool(page: Page, key: string, name: string): Promise<void> {
  const button = page.getByRole('button', { name: new RegExp(`(^|: )${name}$`) });
  await page.keyboard.press(key);
  // A variant of the group: pick it from the options bar's variant radios.
  if ((await button.count()) === 0) await page.getByRole('radio', { name, exact: true }).click();
  await expect(button).toHaveCount(1);
}

/** Screen point of the document position `(x, y)` (pixel corners at integers). */
async function point(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

async function dragDoc(page: Page, a: [number, number], b: [number, number]): Promise<void> {
  await page.mouse.move(...(await point(page, ...a)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, ...b)), { steps: 6 });
  await page.mouse.up();
}

/** The whole active layer, copied with nothing selected. */
async function layerPixels(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

/** The alpha values of the layer that are neither 0 nor 255. */
function partialAlphas(img: ClipImage): number[] {
  const out: number[] = [];
  for (let i = 3; i < img.data.length; i += 4) {
    const a = img.data[i] ?? 0;
    if (a > 0 && a < 255) out.push(a);
  }
  return out;
}

function alphaAt(img: ClipImage, x: number, y: number): number {
  return img.data[(y * img.width + x) * 4 + 3] ?? -1;
}

async function brushDab(page: Page, size: number): Promise<void> {
  await useTool(page, 'b', 'Brush');
  await page.getByRole('combobox', { name: 'Shape' }).selectOption('hard_round');
  await page.getByRole('slider', { name: 'Size' }).fill(String(size));
  await page.mouse.click(...(await point(page, 16, 16)));
}

test('one Antialiasing checkbox, shared by the edge tools and all tabs, on at start (AC-01, AC-07)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  const shown: Array<[string, string]> = [
    ['b', 'Brush'],
    ['e', 'Eraser'],
    ['r', 'Recolor'],
    ['s', 'Smudge'],
    ['l', 'Line'],
    ['u', 'Rectangle'],
    ['u', 'Ellipse'],
    ['t', 'Text'],
    ['m', 'Ellipse Select'],
    ['q', 'Lasso'],
    ['q', 'Polygonal Lasso'],
  ];
  for (const [key, name] of shown) {
    await useTool(page, key, name);
    await expect(antiAlias(page), name).toHaveCount(1);
    await expect(antiAlias(page), name).toBeChecked();
  }
  for (const [key, name] of [
    ['m', 'Rectangle Select'],
    ['w', 'Magic Wand'],
    ['g', 'Fill'],
    ['n', 'Gradient'],
  ] as const) {
    await useTool(page, key, name);
    await expect(antiAlias(page), name).toHaveCount(0);
  }

  // Off in one tool is off in every other.
  await useTool(page, 'b', 'Brush');
  await antiAlias(page).uncheck();
  for (const [key, name] of [
    ['t', 'Text'],
    ['m', 'Ellipse Select'],
    ['u', 'Rectangle'],
  ] as const) {
    await useTool(page, key, name);
    await expect(antiAlias(page), name).not.toBeChecked();
  }

  // A second Image tab shares it.
  await menu(page, 'File', 'New…');
  await page.getByRole('dialog', { name: 'New document' }).getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('tab')).toHaveCount(2);
  await waitForEditor(page, 'image');
  await useTool(page, 'b', 'Brush');
  await expect(antiAlias(page)).not.toBeChecked();

  // On again after the next app start.
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await useTool(page, 'b', 'Brush');
  await expect(antiAlias(page)).toBeChecked();
});

test('a hard round Brush dab has a smooth edge with Antialiasing and a hard one without (AC-02, AC-03)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#ff0000');
  await brushDab(page, 11);
  const smooth = await layerPixels(page);
  expect(alphaAt(smooth, 16, 16)).toBe(255);
  expect(alphaAt(smooth, 16, 3)).toBe(0);
  expect(partialAlphas(smooth).length).toBeGreaterThan(0);

  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await antiAlias(page).uncheck();
  await brushDab(page, 11);
  const hard = await layerPixels(page);
  expect(alphaAt(hard, 16, 16)).toBe(255);
  expect(partialAlphas(hard)).toEqual([]);
});

test('an Ellipse Select and a Lasso edge are anti-aliased only with Antialiasing (AC-06)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#0000ff');

  const fillShape = async (select: () => Promise<void>): Promise<ClipImage> => {
    await select();
    await menu(page, 'Edit', 'Fill Selection');
    await page.keyboard.press('Control+d');
    const img = await layerPixels(page);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    return img;
  };
  const ellipse = async () => {
    await useTool(page, 'm', 'Ellipse Select');
    await dragDoc(page, [4, 4], [28, 26]);
  };
  const lasso = async () => {
    await useTool(page, 'q', 'Lasso');
    await page.mouse.move(...(await point(page, 4, 4)));
    await page.mouse.down();
    await page.mouse.move(...(await point(page, 28, 10)), { steps: 6 });
    await page.mouse.move(...(await point(page, 12, 28)), { steps: 6 });
    await page.mouse.up();
  };

  for (const select of [ellipse, lasso]) {
    const smooth = await fillShape(select);
    expect(alphaAt(smooth, 0, 31)).toBe(0);
    expect(partialAlphas(smooth).length).toBeGreaterThan(0);
  }
  await antiAlias(page).uncheck();
  for (const select of [ellipse, lasso]) {
    const hard = await fillShape(select);
    expect(hard.data.some((v, i) => i % 4 === 3 && v === 255)).toBe(true);
    expect(partialAlphas(hard)).toEqual([]);
  }
});

test('Line and shapes follow the shared Antialiasing setting (AC-05)', async ({ page }) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#00ff00');
  await useTool(page, 'l', 'Line');
  await dragDoc(page, [3, 5], [29, 21]);
  await page.keyboard.press('Enter');
  expect(partialAlphas(await layerPixels(page)).length).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();

  await antiAlias(page).uncheck();
  await dragDoc(page, [3, 5], [29, 21]);
  await page.keyboard.press('Enter');
  const hard = await layerPixels(page);
  expect(hard.data.some((v, i) => i % 4 === 3 && v === 255)).toBe(true);
  expect(partialAlphas(hard)).toEqual([]);
});
