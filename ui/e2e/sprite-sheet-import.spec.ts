import { crc32, deflateSync } from 'node:zlib';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, fakeFileHandles, fakeFs, fakeFsLog, menu, waitForEditor, byText } from './helpers';

// File ▸ Import Sprite Sheet… (docs/specs/sprite-sheet-import.md). Files
// come from the fake File System Access pickers (`fakeFileHandles`).

function pngChunk(type: string, data: Buffer): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
  return Buffer.concat([head, data, crc]);
}

/** An RGBA8 PNG of `w × h` whose pixel `(x, y)` is `pixel(x, y)`. */
function rgbaPng(w: number, h: number, pixel: (x: number, y: number) => number[]): Buffer {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const raw = Buffer.alloc(h * (1 + w * 4));
  let at = 0;
  for (let y = 0; y < h; y++) {
    raw[at++] = 0;
    for (let x = 0; x < w; x++) {
      raw.set(pixel(x, y), at);
      at += 4;
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', deflateSync(raw)),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

const WHITE = [255, 255, 255, 255];
const CLEAR = [0, 0, 0, 0];

/** Used cells of the hero sheet: 4 × 4 cells of 16 px, row 2 (0-based)
 *  empty, a gap inside row 3, trailing empty cells in every row. */
const HERO_CELLS = new Set(['0,0', '1,0', '2,0', '0,1', '0,3', '2,3']);

/** A white 64 × 64 sheet with a 6 × 6 red sprite at (4, 4) of each used cell. */
const HERO = rgbaPng(64, 64, (x, y) => {
  const used = HERO_CELLS.has(`${Math.floor(x / 16)},${Math.floor(y / 16)}`);
  const inside = x % 16 >= 4 && x % 16 < 10 && y % 16 >= 4 && y % 16 < 10;
  return used && inside ? [200, 30, 30, 255] : WHITE;
});

/** A fully transparent sheet: no grid is suggested. */
const BLANK = rgbaPng(40, 40, () => CLEAR);

/** A transparent sheet with one sprite: no suggestion, one used cell. */
const SINGLE = rgbaPng(40, 40, (x, y) =>
  x >= 2 && x < 10 && y >= 2 && y < 10 ? [0, 0, 200, 255] : CLEAR,
);

const FILES = {
  'hero.png': HERO,
  'blank.png': BLANK,
  'single.png': SINGLE,
  'junk.png': Buffer.from('not a png at all'),
  'wide.png': rgbaPng(16_385, 1, () => WHITE),
  'busy.png': rgbaPng(300, 300, (x, y) => [x % 256, y % 256, 0, 255]),
};

/** Opens the app with the fake pickers holding `FILES`, then a document of `mode`. */
async function start(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await fakeFileHandles(page, FILES);
  await createDocument(page, mode, { width: 8, height: 8 });
}

function dialogOf(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Import sprite sheet' });
}

/** File ▸ Import Sprite Sheet… with `name` picked; returns the dialog. */
async function openSheet(page: Page, name: string): Promise<Locator> {
  await fakeFs(page, { pick: name });
  await menu(page, 'File', 'Import Sprite Sheet…');
  const dialog = dialogOf(page);
  await expect(dialog).toBeVisible();
  return dialog;
}

async function gridValues(dialog: Locator): Promise<string[]> {
  const out: string[] = [];
  for (const label of [
    'Cell width',
    'Cell height',
    'Offset X',
    'Offset Y',
    'Spacing X',
    'Spacing Y',
  ]) {
    out.push(await dialog.getByLabel(label, { exact: true }).inputValue());
  }
  return out;
}

function overlay(dialog: Locator): Locator {
  return dialog.getByTestId('sheet-grid');
}

test('imports a sheet from Pixel mode as a new tagged document (AC-01, AC-03, AC-04, AC-06, AC-08, AC-15)', async ({
  page,
}) => {
  await start(page, 'pixel');
  const dialog = await openSheet(page, 'hero.png');
  // The suggestion (white background removed): 16 × 16, offset 0, spacing 0.
  await expect(dialog.getByRole('img', { name: 'Sheet preview' })).toBeVisible();
  expect(await gridValues(dialog)).toEqual(['16', '16', '0', '0', '0', '0']);
  await expect(dialog.getByText('No grid detected')).toBeHidden();
  await expect(overlay(dialog)).toHaveAttribute('data-grid', '16,16,0,0,0,0');
  await expect(overlay(dialog)).toHaveAttribute('data-cells', '16');
  await expect(dialog.getByText('7 frames, 3 tags')).toBeVisible();
  const before = await overlay(dialog).screenshot();

  // AC-03: the grid and the summary follow the fields.
  await dialog.getByLabel('Cell width', { exact: true }).fill('32');
  await expect(overlay(dialog)).toHaveAttribute('data-grid', '32,16,0,0,0,0');
  await expect(overlay(dialog)).toHaveAttribute('data-cells', '8');
  await expect(dialog.getByText('5 frames, 3 tags')).toBeVisible();
  expect((await overlay(dialog).screenshot()).equals(before)).toBe(false);
  await dialog.getByLabel('Rows as tags').uncheck();
  await expect(dialog.getByText('5 frames, 0 tags')).toBeVisible();
  await dialog.getByLabel('Rows as tags').check();
  await dialog.getByLabel('Cell width', { exact: true }).fill('16');
  await expect(dialog.getByText('7 frames, 3 tags')).toBeVisible();

  await dialog.getByLabel('Frame duration (ms)').fill('250');
  await dialog.getByRole('button', { name: 'Import' }).click();
  await expect(dialog).toBeHidden();
  await waitForEditor(page, 'pixel');

  // AC-04, AC-15: a new tab "hero.aseprite" next to the first, 16 × 16.
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(2);
  await expect(page.getByRole('tab', { name: /hero\.aseprite/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(page.locator('footer')).toContainText('16 × 16 px');
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await expect(timeline.getByRole('button', { name: /^Frame \d+$/ })).toHaveCount(7);
  for (let n = 1; n <= 7; n++) {
    await expect(timeline.getByLabel(`Frame ${n} duration (ms)`)).toHaveValue('250');
  }
  // AC-06: one forward tag per used row, named by its grid row.
  await expect(timeline.getByRole('button', { name: /^Tag / })).toHaveCount(3);
  await expect(timeline.getByRole('button', { name: 'Tag Row 1' })).toHaveAttribute(
    'title',
    'Row 1 · frames 1–3 · forward',
  );
  await expect(timeline.getByRole('button', { name: 'Tag Row 2' })).toHaveAttribute(
    'title',
    'Row 2 · frames 4–4 · forward',
  );
  await expect(timeline.getByRole('button', { name: 'Tag Row 4' })).toHaveAttribute(
    'title',
    'Row 4 · frames 5–7 · forward',
  );
  await expect(page.getByRole('region', { name: 'Layers' }).getByText('Layer 1')).toBeVisible();

  // AC-15: unsaved, so the first Save asks where to save, named after the sheet.
  await fakeFs(page, { savePick: null });
  await page.keyboard.press('Control+s');
  await expect
    .poll(async () => (await fakeFsLog(page)).pickers.filter((p) => p.picker === 'save'))
    .toEqual([expect.objectContaining({ suggestedName: 'hero.aseprite' })]);
});

test('imports from Image mode, keeps a name already open and drops the tags when asked (AC-01, AC-06, AC-15)', async ({
  page,
}) => {
  await start(page, 'image');
  let dialog = await openSheet(page, 'hero.png');
  await dialog.getByLabel('Rows as tags').uncheck();
  await dialog.getByRole('button', { name: 'Import' }).click();
  await waitForEditor(page, 'pixel');
  await expect(page.getByRole('tab', { name: /hero\.aseprite/ })).toHaveCount(1);
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await expect(timeline.getByRole('button', { name: /^Frame \d+$/ })).toHaveCount(7);
  await expect(timeline.getByRole('button', { name: /^Tag / })).toHaveCount(0);

  dialog = await openSheet(page, 'hero.png');
  await dialog.getByRole('button', { name: 'Import' }).click();
  await expect(page.getByRole('tab', { name: /hero\.aseprite/ })).toHaveCount(2);
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(3);
});

test('the background option starts from the top-left pixel and changes the cut (AC-07)', async ({
  page,
}) => {
  await start(page, 'pixel');
  let dialog = await openSheet(page, 'hero.png');
  const option = dialog.getByLabel('Make background transparent');
  await expect(option).toBeChecked();
  await expect(dialog.getByLabel('Background colour', { exact: true })).toHaveValue('#ffffff');
  // Off: every complete cell of the white sheet is used.
  await option.uncheck();
  await expect(dialog.getByText('16 frames, 4 tags')).toBeVisible();
  // On with another colour: no pixel matches, the same.
  await option.check();
  await expect(dialog.getByText('7 frames, 3 tags')).toBeVisible();
  await dialog.getByLabel('Background colour', { exact: true }).fill('#fffffe');
  await expect(dialog.getByText('16 frames, 4 tags')).toBeVisible();
  await dialog.getByLabel('Background colour', { exact: true }).fill('#ffffff');
  await expect(dialog.getByText('7 frames, 3 tags')).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  // A transparent top-left pixel: the option starts off.
  dialog = await openSheet(page, 'single.png');
  await expect(dialog.getByLabel('Make background transparent')).not.toBeChecked();
  await expect(dialog.getByLabel('Background colour', { exact: true })).toHaveValue('#00000000');
});

test('without a suggestion the dialog starts from the last confirmed import (AC-10, AC-11)', async ({
  page,
}) => {
  await start(page, 'pixel');
  let dialog = await openSheet(page, 'blank.png');
  await expect(dialog.getByText('No grid detected')).toBeVisible();
  expect(await gridValues(dialog)).toEqual(['32', '32', '0', '0', '0', '0']);
  await expect(dialog.getByLabel('Frame duration (ms)')).toHaveValue('100');
  await expect(dialog.getByLabel('Rows as tags')).toBeChecked();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  dialog = await openSheet(page, 'single.png');
  await expect(dialog.getByText('No grid detected')).toBeVisible();
  for (const [label, value] of <const>[
    ['Cell width', '10'],
    ['Cell height', '9'],
    ['Offset X', '0'],
    ['Offset Y', '0'],
    ['Spacing X', '2'],
    ['Spacing Y', '1'],
  ]) {
    await dialog.getByLabel(label, { exact: true }).fill(value);
  }
  await dialog.getByLabel('Frame duration (ms)').fill('70');
  await dialog.getByLabel('Rows as tags').uncheck();
  await expect(dialog.getByText('1 frame, 0 tags')).toBeVisible();
  // AC-11: Detect without a suggestion keeps the fields.
  await dialog.getByRole('button', { name: 'Detect' }).click();
  await expect(dialog.getByText('No grid detected')).toBeVisible();
  expect(await gridValues(dialog)).toEqual(['10', '9', '0', '0', '2', '1']);
  await dialog.getByRole('button', { name: 'Import' }).click();
  await waitForEditor(page, 'pixel');
  await expect(page.locator('footer')).toContainText('10 × 9 px');

  dialog = await openSheet(page, 'blank.png');
  expect(await gridValues(dialog)).toEqual(['10', '9', '0', '0', '2', '1']);
  await expect(dialog.getByLabel('Frame duration (ms)')).toHaveValue('70');
  await expect(dialog.getByLabel('Rows as tags')).not.toBeChecked();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  // A suggestion still wins over the remembered grid; Detect restores it.
  dialog = await openSheet(page, 'hero.png');
  expect(await gridValues(dialog)).toEqual(['16', '16', '0', '0', '0', '0']);
  await dialog.getByLabel('Cell width', { exact: true }).fill('5');
  await dialog.getByLabel('Offset Y', { exact: true }).fill('3');
  await dialog.getByRole('button', { name: 'Detect' }).click();
  expect(await gridValues(dialog)).toEqual(['16', '16', '0', '0', '0', '0']);
  await expect(dialog.getByText('No grid detected')).toBeHidden();
  // With the background kept, the white sheet has no suggestion.
  await dialog.getByLabel('Make background transparent').uncheck();
  await dialog.getByRole('button', { name: 'Detect' }).click();
  await expect(dialog.getByText('No grid detected')).toBeVisible();
  expect(await gridValues(dialog)).toEqual(['16', '16', '0', '0', '0', '0']);
});

test('Import is disabled with the reason for an unusable grid (AC-12)', async ({ page }) => {
  await start(page, 'pixel');
  const dialog = await openSheet(page, 'busy.png');
  const importButton = dialog.getByRole('button', { name: 'Import' });
  const field = (label: string) => dialog.getByLabel(label, { exact: true });
  const expectReason = async (text: string) => {
    await expect(dialog.getByRole('alert')).toHaveText(text);
    await expect(importButton).toBeDisabled();
  };
  await field('Cell width').fill('16');
  await field('Cell height').fill('16');
  await expect(importButton).toBeEnabled();
  await expect(dialog.getByRole('alert')).toBeHidden();

  await field('Cell width').fill('0');
  await expectReason('Cell width must be a whole number from 1 to 4096.');
  await field('Cell width').fill('1.5');
  await expectReason('Cell width must be a whole number from 1 to 4096.');
  await field('Cell width').fill('4097');
  await expectReason('Cell width must be a whole number from 1 to 4096.');
  await field('Cell width').fill('16');
  await field('Offset X').fill('300');
  await expectReason('Offset X must be a whole number from 0 to 299.');
  await field('Offset X').fill('0');
  await field('Spacing Y').fill('301');
  await expectReason('Spacing Y must be a whole number from 0 to 300.');
  await field('Spacing Y').fill('0');
  await dialog.getByLabel('Frame duration (ms)').fill('0');
  await expectReason('Frame duration must be a whole number from 1 to 65535.');
  await dialog.getByLabel('Frame duration (ms)').fill('100');
  await field('Cell height').fill('301');
  await expectReason('The grid has no complete cell inside the sheet.');
  await field('Cell height').fill('1');
  await field('Cell width').fill('1');
  await expectReason('The grid would create more than 65535 frames.');
  // A background equal to every pixel of a one-colour… here: the
  // transparent sheet has only empty cells.
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  const blank = await openSheet(page, 'blank.png');
  await expect(blank.getByRole('alert')).toHaveText('Every cell is empty.');
  await expect(blank.getByRole('button', { name: 'Import' })).toBeDisabled();
  await blank.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(1);
});

test('an unreadable or oversized file gives a notice and changes nothing (AC-13)', async ({
  page,
}) => {
  await start(page, 'pixel');
  await fakeFs(page, { pick: 'junk.png' });
  await menu(page, 'File', 'Import Sprite Sheet…');
  await expect(byText(page, 'junk.png cannot be read as a sprite sheet.')).toBeVisible();
  await expect(dialogOf(page)).toBeHidden();
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(1);

  await fakeFs(page, { pick: 'wide.png' });
  await menu(page, 'File', 'Import Sprite Sheet…');
  await expect(
    byText(page, 
      'wide.png cannot be read as a sprite sheet: it is wider or taller than 16384 px or has more than 67,108,864 pixels.',
    ),
  ).toBeVisible();
  await expect(dialogOf(page)).toBeHidden();
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(1);
});

test('cancelling the picker or the dialog creates nothing (AC-14)', async ({ page }) => {
  await start(page, 'image');
  await fakeFs(page, { pick: null });
  await menu(page, 'File', 'Import Sprite Sheet…');
  await expect.poll(async () => (await fakeFsLog(page)).pickers.length).toBe(1);
  await expect(dialogOf(page)).toBeHidden();
  const dialog = await openSheet(page, 'hero.png');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  const again = await openSheet(page, 'hero.png');
  await again.getByRole('button', { name: 'Cancel' }).click();
  await expect(again).toBeHidden();
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(1);
});

test('keys pressed while the dialog is open do not reach the editor behind it (AC-14)', async ({
  page,
}) => {
  await start(page, 'pixel');
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await expect(timeline.getByRole('button', { name: /^Frame \d+$/ })).toHaveCount(1);
  const dialog = await openSheet(page, 'hero.png');
  // Alt+N adds a frame in the editor; a click on the preview leaves no field focused.
  await dialog.getByRole('img', { name: 'Sheet preview' }).click();
  await page.keyboard.press('Alt+n');
  await dialog.getByLabel('Cell width', { exact: true }).focus();
  await page.keyboard.press('Alt+n');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
  await expect(timeline.getByRole('button', { name: /^Frame \d+$/ })).toHaveCount(1);
  await expect(page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab')).toHaveCount(1);
});
