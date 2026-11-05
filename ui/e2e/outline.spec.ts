import { expect, test, type Locator, type Page } from '@playwright/test';
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

// Effects ▸ Outline… in both modes (docs/specs/outline-effect.md): the
// dialog and its defaults, the live preview, cancel, one undo step and the
// colour field. Document state is read through Edit ▸ Copy (the
// clipboard); what the canvas shows is compared through screenshots.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const MODES = ['pixel', 'image'] as const;
type Mode = (typeof MODES)[number];

const SIZE = 5;
const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];
const CLEAR = [0, 0, 0, 0];

const canvasOf = (page: Page, mode: Mode): Locator =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
const dialogOf = (page: Page) => page.getByRole('dialog');
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });

/** Screen point of the document position `(x, y)` (pixel corners at integers). */
async function point(page: Page, mode: Mode, x: number, y: number): Promise<[number, number]> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

/** A `SIZE` × `SIZE` document whose only opaque pixel is red at the centre. */
async function centreDot(page: Page, mode: Mode): Promise<void> {
  await createDocument(page, mode, { width: SIZE, height: SIZE });
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  const c = Math.floor(SIZE / 2);
  if (mode === 'pixel') {
    await page.getByTitle('Pencil (B)').click();
    await page.mouse.click(...(await point(page, mode, c + 0.5, c + 0.5)));
  } else {
    // Rectangle Select of the one pixel, Paint Bucket inside it, Deselect.
    await page.keyboard.press('m');
    await page.mouse.move(...(await point(page, mode, c, c)));
    await page.mouse.down();
    await page.mouse.move(...(await point(page, mode, c + 1, c + 1)), { steps: 4 });
    await page.mouse.up();
    await page.keyboard.press('g');
    await page.mouse.click(...(await point(page, mode, c + 0.5, c + 0.5)));
    await page.keyboard.press('ControlOrMeta+d');
  }
  expect(pixelOf(await activeLayer(page), c, c)).toEqual(RED);
}

/** The active layer, read through Edit ▸ Copy (call without a selection). */
async function activeLayer(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

/** A screenshot of a small patch at the centre of document pixel `(x, y)`. */
async function patch(page: Page, mode: Mode, x: number, y: number): Promise<() => Promise<Buffer>> {
  const [cx, cy] = await point(page, mode, x + 0.5, y + 0.5);
  const clip = { x: cx - 2, y: cy - 2, width: 4, height: 4 };
  return async () => {
    await page.mouse.move(1, 1);
    return page.screenshot({ clip });
  };
}

async function openOutline(page: Page): Promise<Locator> {
  await menu(page, 'Effects', 'Outline…');
  const dialog = dialogOf(page);
  await expect(dialog.getByRole('heading', { name: 'Outline', exact: true })).toBeVisible();
  return dialog;
}

for (const mode of MODES) {
  test(`${mode}: Effects ▸ Outline… opens with Place, Shape, Thickness and Color at their defaults (AC-01)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: SIZE, height: SIZE });
    await page.getByLabel('Foreground color', { exact: true }).fill('#336699');
    const dialog = await openOutline(page);

    const place = dialog.getByRole('combobox', { name: 'Place', exact: true });
    await expect(place.locator('option')).toHaveText(['Outside', 'Inside', 'Both']);
    await expect(place.locator('option:checked')).toHaveText('Outside');
    const shape = dialog.getByRole('combobox', { name: 'Shape', exact: true });
    await expect(shape.locator('option')).toHaveText(['Circle', 'Square']);
    await expect(shape.locator('option:checked')).toHaveText('Square');
    const thickness = dialog.getByRole('slider', { name: 'Thickness', exact: true });
    await expect(thickness).toHaveValue('1');
    await expect(thickness).toHaveAttribute('min', '1');
    await expect(thickness).toHaveAttribute('max', '8');
    await expect(dialog.getByLabel('Color', { exact: true })).toHaveValue('#336699');
    await expect(dialog.getByRole('textbox', { name: 'Color hex', exact: true })).toHaveValue(
      '#336699',
    );
  });

  test(`${mode}: the outline applies as one undo step and redo brings it back (AC-09)`, async ({
    page,
  }) => {
    await centreDot(page, mode);
    const dialog = await openOutline(page);
    await dialog.getByRole('textbox', { name: 'Color hex', exact: true }).fill('#0000ff');
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);

    let layer = await activeLayer(page);
    expect(pixelOf(layer, 1, 1), 'a neighbour').toEqual(BLUE);
    expect(pixelOf(layer, 3, 2), 'a neighbour').toEqual(BLUE);
    expect(pixelOf(layer, 2, 2), 'the sprite').toEqual(RED);
    expect(pixelOf(layer, 0, 0), 'not a neighbour').toEqual(CLEAR);
    expect(pixelOf(layer, 4, 2), 'not a neighbour').toEqual(CLEAR);

    await undoButton(page).click();
    layer = await activeLayer(page);
    expect(pixelOf(layer, 1, 1)).toEqual(CLEAR);
    expect(pixelOf(layer, 2, 2)).toEqual(RED);

    await redoButton(page).click();
    layer = await activeLayer(page);
    expect(pixelOf(layer, 1, 1)).toEqual(BLUE);
    expect(pixelOf(layer, 0, 0)).toEqual(CLEAR);
  });

  test(`${mode}: the canvas previews the outline live; Cancel leaves the layer unchanged (AC-08)`, async ({
    page,
  }) => {
    await centreDot(page, mode);
    const near = await patch(page, mode, 1, 1);
    const far = await patch(page, mode, 0, 0);
    const nearBefore = await near();
    const farBefore = await far();

    const dialog = await openOutline(page);
    await expect.poll(async () => (await near()).equals(nearBefore)).toBe(false);
    expect((await far()).equals(farBefore)).toBe(true);
    // Thickness 2 reaches the corner: the preview follows the option.
    await dialog.getByRole('spinbutton', { name: 'Thickness value', exact: true }).fill('2');
    await expect.poll(async () => (await far()).equals(farBefore)).toBe(false);

    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => (await near()).equals(nearBefore)).toBe(true);
    const layer = await activeLayer(page);
    expect(pixelOf(layer, 1, 1)).toEqual(CLEAR);
    expect(pixelOf(layer, 0, 0)).toEqual(CLEAR);
    await expect(redoButton(page)).toBeDisabled();
  });

  test(`${mode}: a hex that is not #RRGGBB or #RRGGBBAA is marked invalid and not used (AC-11)`, async ({
    page,
  }) => {
    await centreDot(page, mode);
    const dialog = await openOutline(page);
    const hex = dialog.getByRole('textbox', { name: 'Color hex', exact: true });
    await hex.fill('#112233cc');
    await expect(hex).toHaveAttribute('aria-invalid', 'false');
    for (const bad of ['zzz', '#12345', '#1122334', 'red', '#11223g44', '112233']) {
      await hex.fill(bad);
      await expect(hex, bad).toHaveAttribute('aria-invalid', 'true');
    }
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toHaveCount(0);
    // The last valid colour, alpha included (AC-06).
    expect(pixelOf(await activeLayer(page), 1, 1)).toEqual([0x11, 0x22, 0x33, 0xcc]);
  });
}
