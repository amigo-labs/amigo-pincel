import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  menu,
  pixelCanvas,
  setColor,
} from './helpers';
import {
  SERIF,
  fontControl,
  holdReads,
  imageReference,
  installLocalFonts,
  lfaLog,
  openPixelText,
  pickFamily,
  pixelReference,
  releaseReads,
  sameInk,
  statusBar,
  type FakeFont,
  type RefText,
} from './font-helpers';
import {
  W,
  H,
  canvasShot,
  chooseText,
  liveInput,
  setFontSize,
  textBox,
  undoAll,
} from './text-helpers';
import { at } from './text-helpers';

// docs/specs/system-fonts.md: text placed with a chosen family equals the
// cores' output for the face the spec's rules pick (AC-06 – AC-08, AC-23),
// the previews show that face (AC-09) and a commit waits for a face still
// being read (AC-18). Reference pixels come from the wasm packages in Node.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const SPRITE = 48;
const BLACK = 0x000000ff;

/** The active layer, read through Copy. */
async function layerPixels(page: Page): Promise<number[]> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return (await copiedImage(page)).data;
}

/** A Pixel sprite with a black foreground. */
async function pixelDoc(page: Page): Promise<void> {
  await createDocument(page, 'pixel', { width: SPRITE, height: SPRITE });
  await setColor(page, 'fg', '#000000');
}

/** Types `text` at size 16 into the open Pixel Text dialog. */
async function fillDialog(
  dialog: Locator,
  text: string,
  opts: { bold?: boolean; italic?: boolean } = {},
) {
  await dialog.getByLabel('Font size').fill('16');
  if (opts.bold) await dialog.getByRole('checkbox', { name: 'Bold' }).check();
  if (opts.italic) await dialog.getByRole('checkbox', { name: 'Italic' }).check();
  await dialog.getByLabel('Text content').fill(text);
}

/** Places "Hg" at (2, 2) in Pixel mode with `family`; returns the layer. */
async function pixelPlace(
  page: Page,
  family: string,
  opts: { bold?: boolean; italic?: boolean } = {},
): Promise<number[]> {
  const dialog = await openPixelText(page, 2, 2, SPRITE);
  if (family !== 'Liberation Sans') await pickFamily(page, family);
  await fillDialog(dialog, 'Hg', opts);
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('text placed');
  return layerPixels(page);
}

const pixelRef = (file: string, extra: Partial<RefText> = {}) =>
  pixelReference({
    width: SPRITE,
    height: SPRITE,
    file,
    text: 'Hg',
    x: 2,
    y: 2,
    size: 16,
    color: BLACK,
    ...extra,
  });

const imageRef = (file: string, extra: Partial<RefText> = {}) =>
  imageReference({
    width: W,
    height: H,
    file,
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: BLACK,
    ...extra,
  });

/** An Image document with a black foreground and the Text tool at size 32. */
async function imageDoc(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: W, height: H });
  await setColor(page, 'fg', '#000000');
  await chooseText(page);
  await setFontSize(page, 32);
}

/** Opens a live text at (20, 24) and types `text`. */
async function imageType(page: Page, text: string): Promise<void> {
  const [x, y] = await at(page, [20.5, 24.5]);
  await page.mouse.click(x, y);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type(text);
}

/** Finishes the live text. */
async function finish(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
}

test('AC-06 (pixel): placed text equals the core output for the family Regular face', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await pixelDoc(page);
  const placed = await pixelPlace(page, 'Pincel Test Serif');
  expect(sameInk(placed, await pixelRef('PincelTestSerif-Regular.ttf'))).toBe(true);
  expect(sameInk(placed, await pixelRef('LiberationSans-Regular.ttf'))).toBe(false);
  // One undo step, as before.
  expect(await undoAll(page)).toBe(1);
});

test('AC-06 (image): committed text equals the core output for the family Regular face', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageDoc(page);
  await pickFamily(page, 'Pincel Test Serif');
  await imageType(page, 'Hg');
  await finish(page);
  const placed = await layerPixels(page);
  expect(sameInk(placed, await imageRef('PincelTestSerif-Regular.ttf'))).toBe(true);
  expect(sameInk(placed, await imageRef('LiberationSans-Regular.ttf'))).toBe(false);
  expect(await undoAll(page)).toBe(1);
});

test('AC-07 (pixel): Bold and Italic use the real faces, not synthesis', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await pixelDoc(page);
  const bold = await pixelPlace(page, 'Pincel Test Serif', { bold: true });
  expect(sameInk(bold, await pixelRef('PincelTestSerif-Bold.ttf'))).toBe(true);
  expect(await undoAll(page)).toBe(1);
  const both = await pixelPlace(page, 'Pincel Test Serif', { bold: true, italic: true });
  expect(sameInk(both, await pixelRef('PincelTestSerif-BoldItalic.ttf'))).toBe(true);
});

test('AC-07 (pixel): a missing Bold Italic falls back to Bold plus synthesized italic', async ({
  page,
}) => {
  const noBoldItalic = SERIF.filter((f) => f.style !== 'Bold Italic');
  await installLocalFonts(page, { permission: 'granted', fonts: noBoldItalic });
  await pixelDoc(page);
  const both = await pixelPlace(page, 'Pincel Test Serif', { bold: true, italic: true });
  expect(sameInk(both, await pixelRef('PincelTestSerif-Bold.ttf', { italic: true }))).toBe(true);
});

test('AC-07 (image): Bold Italic uses the Bold Italic face; Bold alone without a Bold face is synthesized', async ({
  page,
}) => {
  const solo: FakeFont = {
    family: 'Solo',
    style: 'Regular',
    postscriptName: 'Solo',
    file: 'PincelTestSerif-Italic.ttf',
  };
  await installLocalFonts(page, { permission: 'granted', fonts: [...SERIF, solo] });
  await imageDoc(page);
  await pickFamily(page, 'Pincel Test Serif');
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  await page.getByRole('checkbox', { name: 'Italic' }).check();
  await imageType(page, 'Hg');
  await finish(page);
  expect(sameInk(await layerPixels(page), await imageRef('PincelTestSerif-BoldItalic.ttf'))).toBe(
    true,
  );
  await undoAll(page);

  await page.getByRole('checkbox', { name: 'Italic' }).uncheck();
  await pickFamily(page, 'Solo');
  await imageType(page, 'Hg');
  await finish(page);
  expect(
    sameInk(await layerPixels(page), await imageRef('PincelTestSerif-Italic.ttf', { bold: true })),
  ).toBe(true);
});

test('AC-08: a face of a collection is used, not the collection first face', async ({ page }) => {
  const ttc: FakeFont[] = [
    {
      family: 'Pincel Test Serif',
      style: 'Regular',
      postscriptName: 'PincelTestSerif-Regular',
      file: 'PincelTestSerif.ttc',
    },
    {
      family: 'Pincel Test Serif',
      style: 'Bold',
      postscriptName: 'PincelTestSerif-Bold',
      file: 'PincelTestSerif.ttc',
    },
  ];
  await installLocalFonts(page, { permission: 'granted', fonts: ttc });
  await pixelDoc(page);
  const bold = await pixelPlace(page, 'Pincel Test Serif', { bold: true });
  expect(sameInk(bold, await pixelRef('PincelTestSerif.ttc', { index: 1 }))).toBe(true);
  expect(sameInk(bold, await pixelRef('PincelTestSerif.ttc', { index: 0, bold: true }))).toBe(
    false,
  );
});

test('AC-23: the Liberation Sans entry keeps the bundled face upright and takes system Bold', async ({
  page,
}) => {
  // Faked "Liberation Sans" system faces, recognizable by their serif glyphs.
  const sys: FakeFont[] = [
    {
      family: 'Liberation Sans',
      style: 'Regular',
      postscriptName: 'LiberationSans',
      file: 'PincelTestSerif-Regular.ttf',
    },
    {
      family: 'Liberation Sans',
      style: 'Bold',
      postscriptName: 'LiberationSans-Bold',
      file: 'PincelTestSerif-Bold.ttf',
    },
  ];
  await installLocalFonts(page, { permission: 'granted', fonts: sys });
  await pixelDoc(page);
  const regular = await pixelPlace(page, 'Liberation Sans');
  expect(sameInk(regular, await pixelRef('LiberationSans-Regular.ttf'))).toBe(true);
  expect(await undoAll(page)).toBe(1);
  const bold = await pixelPlace(page, 'Liberation Sans', { bold: true });
  expect(sameInk(bold, await pixelRef('PincelTestSerif-Bold.ttf'))).toBe(true);
  expect((await lfaLog(page)).reads).toEqual({ 'LiberationSans-Bold': 1 });
});

test('AC-09 (pixel): the preview follows the family and keeps the previous face while one is read', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await pixelDoc(page);
  const canvas = pixelCanvas(page);
  const blank = await canvas.screenshot();
  const dialog = await openPixelText(page, 2, 2, SPRITE);
  await fillDialog(dialog, 'Hg');
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  const sans = await canvas.screenshot();
  await holdReads(page);
  await pickFamily(page, 'Pincel Test Serif');
  await expect.poll(async () => (await lfaLog(page)).reads['PincelTestSerif-Regular']).toBe(1);
  // Still the previous face while the new one is read.
  expect((await canvas.screenshot()).equals(sans)).toBe(true);
  await releaseReads(page);
  await expect.poll(async () => (await canvas.screenshot()).equals(sans)).toBe(false);
  const serif = await canvas.screenshot();
  // The preview shows what Place commits.
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('text placed');
  expect((await canvas.screenshot()).equals(serif)).toBe(true);
  // Bold switches the preview to the Bold face.
  const dialog2 = await openPixelText(page, 2, 2, SPRITE);
  await fillDialog(dialog2, 'Hg', { bold: true });
  await expect.poll(async () => (await lfaLog(page)).reads['PincelTestSerif-Bold']).toBe(1);
  await expect.poll(async () => (await canvas.screenshot()).equals(serif)).toBe(false);
});

test('AC-09 (image): the live text shows the chosen face and follows a family change', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageDoc(page);
  await imageType(page, 'Hg');
  const sans = await canvasShot(page);
  await pickFamily(page, 'Pincel Test Serif');
  await expect.poll(async () => (await canvasShot(page)).equals(sans)).toBe(false);
  await expect(textBox(page)).toBeVisible();
  const live = await canvasShot(page);
  await finish(page);
  expect((await canvasShot(page)).equals(live)).toBe(true);
  expect(sameInk(await layerPixels(page), await imageRef('PincelTestSerif-Regular.ttf'))).toBe(
    true,
  );
});

test('AC-18 (pixel): Place waits for a face still being read, with a loading notice', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await pixelDoc(page);
  const dialog = await openPixelText(page, 2, 2, SPRITE);
  await holdReads(page);
  await pickFamily(page, 'Pincel Test Serif');
  await fillDialog(dialog, 'Hg');
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('Loading font "Pincel Test Serif"');
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  await releaseReads(page);
  await expect(statusBar(page)).toContainText('text placed');
  expect(sameInk(await layerPixels(page), await pixelRef('PincelTestSerif-Regular.ttf'))).toBe(
    true,
  );
});

test('AC-18 (image): Finish waits for a family change still being read and uses that family', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageDoc(page);
  await imageType(page, 'Hg');
  await holdReads(page);
  await pickFamily(page, 'Pincel Test Serif');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(statusBar(page)).toContainText('Loading font "Pincel Test Serif"');
  await expect(textBox(page)).toBeVisible();
  await releaseReads(page);
  await expect(textBox(page)).toHaveCount(0);
  expect(sameInk(await layerPixels(page), await imageRef('PincelTestSerif-Regular.ttf'))).toBe(
    true,
  );
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
});
