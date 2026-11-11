import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  menu,
  setColor,
  storedPref,
  waitForEditor,
} from './helpers';
import {
  SERIF,
  fontControl,
  imageReference,
  installLocalFonts,
  lfaLog,
  openFontList,
  openPixelText,
  pickFamily,
  pixelReference,
  sameInk,
  statusBar,
  type FakeFont,
} from './font-helpers';
import { H, W, at, chooseText, liveInput, setFontSize, textBox } from './text-helpers';

// docs/specs/system-fonts.md: the family is remembered per mode (AC-10); an
// unavailable family falls back to Liberation Sans with a mark and a status
// message (AC-11); an undecided permission is asked for from the first Text
// press (AC-22); faces are read once per session (AC-05); fonts cause no
// network request and leave no trace in files (AC-17).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const FALLBACK = 'Font "Pincel Test Serif" is not available — Liberation Sans is used instead';

/** The Image Text tool with a black foreground at size 32. */
async function imageText(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: W, height: H });
  await setColor(page, 'fg', '#000000');
  await chooseText(page);
  await setFontSize(page, 32);
}

/** Opens a live text box at (20, 24). */
async function imageOpen(page: Page): Promise<void> {
  const [x, y] = await at(page, [20.5, 24.5]);
  await page.mouse.click(x, y);
  await expect(liveInput(page)).toBeFocused();
}

/** Types "Hg" into the open box (or opens one at (20, 24)) and finishes it;
 *  returns the layer's pixels. */
async function imagePlace(page: Page, opened = false): Promise<number[]> {
  if (!opened) await imageOpen(page);
  await page.keyboard.type('Hg');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return (await copiedImage(page)).data;
}

const imageRef = (file: string) =>
  imageReference({
    width: W,
    height: H,
    file,
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: 0x000000ff,
  });

/** Opens a new document from File ▸ New… in the same page load. */
async function newFromMenu(
  page: Page,
  mode: 'pixel' | 'image',
  w: number,
  h: number,
): Promise<void> {
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: mode === 'pixel' ? /Pixel sprite/ : /^Image/ }).click();
  await dialog.getByLabel('Width').fill(String(w));
  await dialog.getByLabel('Height').fill(String(h));
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

/** Waits until the mode's family is stored. */
async function remembered(page: Page, mode: 'pixel' | 'image', family: string): Promise<void> {
  await expect
    .poll(
      async () =>
        ((await storedPref(page, `textFont.${mode}`)) as { family?: string } | undefined)?.family,
    )
    .toBe(family);
}

test('AC-10: each mode remembers its own family across a reload', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await remembered(page, 'pixel', 'Pincel Test Serif');

  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await createDocument(page, 'image', { width: 32, height: 32 });
  await chooseText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
});

test('AC-11: an uninstalled family falls back to Liberation Sans with a mark and a message', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await remembered(page, 'image', 'Pincel Test Serif');
  // The next session finds the family uninstalled.
  await installLocalFonts(page, { permission: 'granted', fonts: [] });
  await imageText(page);
  await expect(statusBar(page)).toContainText(FALLBACK);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await expect(fontControl(page).locator('[data-font-missing]')).toHaveText('(missing)');
  const placed = await imagePlace(page);
  expect(sameInk(placed, await imageRef('LiberationSans-Regular.ttf'))).toBe(true);
  // The choice is kept for when the family is back.
  expect(((await storedPref(page, 'textFont.image')) as { family: string }).family).toBe(
    'Pincel Test Serif',
  );
});

test('AC-11: with access denied the remembered system family falls back when the Pixel dialog opens', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await remembered(page, 'pixel', 'Pincel Test Serif');
  await installLocalFonts(page, { permission: 'denied', fonts: SERIF });
  await createDocument(page, 'pixel', { width: 48, height: 48 });
  await setColor(page, 'fg', '#000000');
  await expect(statusBar(page)).not.toContainText(FALLBACK);
  const dialog = await openPixelText(page, 2, 2, 48);
  await expect(statusBar(page)).toContainText(FALLBACK);
  await expect(fontControl(page).locator('[data-font-missing]')).toBeVisible();
  await dialog.getByLabel('Font size').fill('16');
  await dialog.getByLabel('Text content').fill('Hg');
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('text placed');
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  const placed = (await copiedImage(page)).data;
  const ref = await pixelReference({
    width: 48,
    height: 48,
    file: 'LiberationSans-Regular.ttf',
    text: 'Hg',
    x: 2,
    y: 2,
    size: 16,
    color: 0x000000ff,
  });
  expect(sameInk(placed, ref)).toBe(true);
});

test('AC-11: a face that cannot be parsed falls back and marks the family', async ({ page }) => {
  const broken: FakeFont = {
    family: 'Broken',
    style: 'Regular',
    postscriptName: 'Broken',
    file: 'PincelTestSerif-LICENSE.txt',
  };
  await installLocalFonts(page, { permission: 'granted', fonts: [...SERIF, broken] });
  await imageText(page);
  await pickFamily(page, 'Broken');
  await imageOpen(page);
  await expect(statusBar(page)).toContainText(
    'Font "Broken" is not available — Liberation Sans is used instead',
  );
  await expect(fontControl(page).locator('[data-font-missing]')).toBeVisible();
  const placed = await imagePlace(page, true);
  expect(sameInk(placed, await imageRef('LiberationSans-Regular.ttf'))).toBe(true);
});

test('AC-22: an undecided permission is asked for from the first Text press; approval uses the family', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await remembered(page, 'image', 'Pincel Test Serif');
  await installLocalFonts(page, { permission: 'prompt', answer: 'grant', fonts: SERIF });
  await imageText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await expect(fontControl(page).locator('[data-font-missing]')).toHaveCount(0);
  await expect(statusBar(page)).not.toContainText(FALLBACK);
  expect((await lfaLog(page)).queries).toBe(0);
  const placed = await imagePlace(page);
  const log = await lfaLog(page);
  expect(log.queries).toBe(1);
  expect(log.activations).toEqual([true]);
  expect(sameInk(placed, await imageRef('PincelTestSerif-Regular.ttf'))).toBe(true);
});

test('AC-22: a refused prompt from the first Text press falls back per AC-11', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await remembered(page, 'image', 'Pincel Test Serif');
  await installLocalFonts(page, { permission: 'prompt', answer: 'deny', fonts: SERIF });
  await imageText(page);
  await imageOpen(page);
  await expect(statusBar(page)).toContainText(FALLBACK);
  await expect(fontControl(page).locator('[data-font-missing]')).toBeVisible();
  expect((await lfaLog(page)).activations).toEqual([true]);
  const placed = await imagePlace(page, true);
  expect(sameInk(placed, await imageRef('LiberationSans-Regular.ttf'))).toBe(true);
});

test('AC-05: a face is read once per session, across commits and both modes', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await imagePlace(page);
  await imagePlace(page);
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  await imagePlace(page);
  expect((await lfaLog(page)).reads).toEqual({
    'PincelTestSerif-Regular': 1,
    'PincelTestSerif-Bold': 1,
  });
  // Pixel mode in the same page load reuses the read face.
  await newFromMenu(page, 'pixel', 32, 32);
  const dialog = await openPixelText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await dialog.getByLabel('Text content').fill('Hg');
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('text placed');
  expect((await lfaLog(page)).reads).toEqual({
    'PincelTestSerif-Regular': 1,
    'PincelTestSerif-Bold': 1,
  });
});

test('AC-17: fonts make no network request and leave no trace in an export', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await imageText(page);
  const origin = new URL(page.url()).origin;
  const requests: string[] = [];
  page.on('request', (r) => requests.push(r.url()));
  await openFontList(page);
  await pickFamily(page, 'Pincel Test Serif');
  await page.getByRole('checkbox', { name: 'Italic' }).check();
  await imagePlace(page);
  expect(
    requests.filter(
      (u) => !u.startsWith(origin) && !u.startsWith('data:') && !u.startsWith('blob:'),
    ),
  ).toEqual([]);
  expect(
    requests.filter(
      (u) => /\.(ttf|otf|ttc|woff2?)(\?|$)/.test(u) && !u.endsWith('/LiberationSans-Regular.ttf'),
    ),
  ).toEqual([]);

  const exportPng = async (): Promise<Buffer> => {
    const [d] = await Promise.all([
      page.waitForEvent('download'),
      menu(page, 'File', 'Export', 'PNG'),
    ]);
    return readFile(await d.path());
  };
  const withSerif = await exportPng();
  // The same pixels in a new image while Liberation Sans is selected.
  await pickFamily(page, 'Liberation Sans');
  await page.getByRole('checkbox', { name: 'Italic' }).uncheck();
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  await copiedImage(page);
  await newFromMenu(page, 'image', W, H);
  await chooseText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await menu(page, 'Edit', 'Paste');
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
  const withSans = await exportPng();
  expect(withSans.equals(withSerif)).toBe(true);
});
