import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  setColor,
  switchLanguage,
  type ClipImage,
  waitForEditor,
} from './helpers';

// The shared Blending tool option in Image mode
// (docs/specs/image-edit-extras-tool-options.md AC-08 – AC-10). Pixels are
// read from the clipboard after a Copy of the whole layer.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const SIZE = 32;
const blending = (page: Page) => page.getByRole('combobox', { name: 'Blending' });

/** Picks the tool `name` of the rail group behind `key`. */
async function useTool(page: Page, key: string, name: string): Promise<void> {
  const button = page.getByRole('button', { name: new RegExp(`(^|: )${name}$`) });
  await page.keyboard.press(key);
  // A variant of the group: pick it from the options bar's variant radios.
  if ((await button.count()) === 0) await page.getByRole('radio', { name, exact: true }).click();
  await expect(button).toHaveCount(1);
}

async function point(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

async function layerPixels(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

function pixel(img: ClipImage, x: number, y: number): number[] {
  const i = (y * img.width + x) * 4;
  return img.data.slice(i, i + 4);
}

/** An opaque blue document, then red as the foreground. */
async function blueDocument(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await setColor(page, 'fg', '#0000ff');
  await menu(page, 'Edit', 'Fill Selection');
  await setColor(page, 'fg', '#ff0000');
}

test('one Blending control, shared by the painting tools and all tabs, Normal at start (AC-08)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  for (const [key, name] of [
    ['b', 'Brush'],
    ['g', 'Fill'],
    ['n', 'Gradient'],
    ['l', 'Line / Curve'],
    ['u', 'Rectangle'],
    ['u', 'Ellipse'],
    ['t', 'Text'],
  ] as const) {
    await useTool(page, key, name);
    await expect(blending(page), name).toHaveValue('normal');
  }
  for (const [key, name] of [
    ['e', 'Eraser'],
    ['r', 'Recolor'],
    ['s', 'Smudge'],
    ['m', 'Ellipse Select'],
    ['w', 'Magic Wand'],
  ] as const) {
    await useTool(page, key, name);
    await expect(blending(page), name).toHaveCount(0);
  }
  // Hidden for the Gradient's Transparency channel.
  await useTool(page, 'n', 'Gradient');
  await page.getByRole('combobox', { name: 'Channel' }).selectOption('transparency');
  await expect(blending(page)).toHaveCount(0);
  await page.getByRole('combobox', { name: 'Channel' }).selectOption('color');

  // Overwrite in one tool is Overwrite in every other and in a new tab.
  await useTool(page, 'b', 'Brush');
  await blending(page).selectOption('overwrite');
  for (const [key, name] of [
    ['g', 'Fill'],
    ['t', 'Text'],
    ['u', 'Rectangle'],
  ] as const) {
    await useTool(page, key, name);
    await expect(blending(page), name).toHaveValue('overwrite');
  }
  await menu(page, 'File', 'New…');
  await page.getByRole('dialog', { name: 'New document' }).getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('tab')).toHaveCount(2);
  await waitForEditor(page, 'image');
  await useTool(page, 'b', 'Brush');
  await expect(blending(page)).toHaveValue('overwrite');

  // Normal again after the next app start; labelled in German too.
  await createDocument(page, 'image', { width: SIZE, height: SIZE });
  await useTool(page, 'b', 'Brush');
  await expect(blending(page)).toHaveValue('normal');
  await switchLanguage(page, 'Deutsch');
  const de = page.getByRole('combobox', { name: 'Mischmodus' });
  await expect(de.getByRole('option', { name: 'Überschreiben' })).toHaveCount(1);
});

test('Overwrite lowers the alpha the Brush and the Bucket leave, Normal does not (AC-09, AC-10)', async ({
  page,
}) => {
  await blueDocument(page);
  await useTool(page, 'b', 'Brush');
  await page.getByRole('slider', { name: /^Opacity/ }).fill('40');
  await page.mouse.click(...(await point(page, 8.5, 8.5)));
  // Normal composites over the opaque pixel: it stays opaque.
  expect(pixel(await layerPixels(page), 8, 8)[3]).toBe(255);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();

  await blending(page).selectOption('overwrite');
  await page.mouse.click(...(await point(page, 8.5, 8.5)));
  const [r, g, b, a] = pixel(await layerPixels(page), 8, 8);
  expect([r, g, b]).toEqual([255, 0, 0]);
  expect(Math.abs((a ?? 0) - 102)).toBeLessThanOrEqual(1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();

  await useTool(page, 'g', 'Fill');
  await page.getByRole('slider', { name: /^Opacity/ }).fill('40');
  await page.mouse.click(...(await point(page, 20.5, 20.5)));
  const filled = await layerPixels(page);
  for (const [x, y] of [
    [0, 0],
    [31, 31],
  ] as const) {
    const [fr, fg, fb, fa] = pixel(filled, x, y);
    expect([fr, fg, fb]).toEqual([255, 0, 0]);
    expect(Math.abs((fa ?? 0) - 102)).toBeLessThanOrEqual(1);
  }
});

test('a live gradient and a live shape follow Blending, also when it changes while live (AC-08, AC-10)', async ({
  page,
}) => {
  await blueDocument(page);
  await setColor(page, 'bg', '#ff0000');
  await useTool(page, 'n', 'Gradient');
  await page.getByRole('slider', { name: /^Opacity/ }).fill('40');
  await blending(page).selectOption('overwrite');
  await page.mouse.move(...(await point(page, 2, 16)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, 30, 16)), { steps: 5 });
  await page.mouse.up();
  await page.keyboard.press('Enter');
  const over = await layerPixels(page);
  expect(Math.abs((pixel(over, 16, 16)[3] ?? 0) - 102)).toBeLessThanOrEqual(1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();

  // Begun under Overwrite, the outline lowers the alpha.
  await useTool(page, 'u', 'Rectangle');
  const rect = async () => {
    await page.mouse.move(...(await point(page, 4, 4)));
    await page.mouse.down();
    await page.mouse.move(...(await point(page, 28, 28)), { steps: 5 });
    await page.mouse.up();
  };
  await rect();
  await page.keyboard.press('Enter');
  expect(pixel(await layerPixels(page), 4, 16)[3]).toBeLessThan(255);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();

  // Switched to Normal while the shape is live: the shape redraws.
  await rect();
  await blending(page).selectOption('normal');
  await imageCanvas(page).focus();
  await page.keyboard.press('Enter');
  expect(pixel(await layerPixels(page), 4, 16)[3]).toBe(255);
});
