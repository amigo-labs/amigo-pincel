import { readFile } from 'node:fs/promises';
import { expect, test, type Download, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, storedPref } from './helpers';

// Scaled PNG frame and sprite-sheet export (docs/specs/scaled-export.md).
// Force the download fallback: the File System Access picker would open a
// native dialog the test cannot drive.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

const SCALES = ['×1', '×2', '×3', '×4', '×5', '×6', '×8', '×10'];

const pngDialog = (page: Page) => page.getByRole('dialog', { name: 'Export PNG' });
const sheetDialog = (page: Page) => page.getByRole('dialog', { name: 'Export sprite sheet' });

/** Collects every download the page starts, in order. */
function collectDownloads(page: Page): Download[] {
  const downloads: Download[] = [];
  page.on('download', (d) => downloads.push(d));
  return downloads;
}

/** Width and height from a PNG's IHDR (bytes 16–23). */
function pngSize(png: Buffer): [number, number] {
  return [png.readUInt32BE(16), png.readUInt32BE(20)];
}

/** Runs `action` and returns the bytes of the download it triggers. */
async function downloaded(page: Page, action: () => Promise<void>): Promise<Buffer> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return readFile(await d.path());
}

/** The selected option's label of a dialog's Scale choice. */
async function selectedScale(dialog: Locator): Promise<string> {
  return dialog
    .getByLabel('Scale')
    .evaluate((el) => (el as HTMLSelectElement).selectedOptions[0]?.textContent?.trim() ?? '');
}

test('PNG (current frame) opens a Scale dialog and exports only on confirm (AC-01, AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 4, height: 4 });
  const downloads = collectDownloads(page);
  for (const open of [
    () => menu(page, 'File', 'Export', 'PNG (current frame)'),
    () => page.keyboard.press('ControlOrMeta+e'),
  ]) {
    await open();
    const dialog = pngDialog(page);
    await expect(dialog).toBeVisible();
    await expect(dialog.getByLabel('Scale').locator('option')).toHaveText(SCALES);
    expect(await selectedScale(dialog)).toBe('×1');
    await page.waitForTimeout(300);
    expect(downloads).toHaveLength(0);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
  }
  expect(downloads).toHaveLength(0);
});

test('the sprite sheet dialog offers the same Scale choice, ×1 on a fresh profile (AC-02, AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 4, height: 4 });
  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  const dialog = sheetDialog(page);
  await expect(dialog.getByLabel('Scale').locator('option')).toHaveText(SCALES);
  expect(await selectedScale(dialog)).toBe('×1');
});

test('a 4×4 frame exported at ×4 is a 16×16 PNG (AC-03)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 4, height: 4 });
  await menu(page, 'File', 'Export', 'PNG (current frame)');
  await pngDialog(page).getByLabel('Scale').selectOption({ label: '×4' });
  const png = await downloaded(page, () =>
    pngDialog(page).getByRole('button', { name: 'Export' }).click(),
  );
  expect(pngSize(png)).toEqual([16, 16]);
});

test('a ×2 sheet is twice the ×1 sheet and its JSON describes the scaled sheet (AC-04, AC-06)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 8, height: 4 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByRole('button', { name: 'Frame 2', exact: true })).toBeVisible();

  const sheet = async (scale: string) => {
    const downloads = collectDownloads(page);
    await menu(page, 'File', 'Export', 'Sprite Sheet…');
    const dialog = sheetDialog(page);
    await dialog.getByLabel('Columns').fill('2');
    await dialog.getByLabel('Scale').selectOption({ label: scale });
    await dialog.getByRole('button', { name: 'Export' }).click();
    await expect.poll(() => downloads.length).toBe(2);
    const [png, json] = await Promise.all(downloads.map(async (d) => readFile(await d.path())));
    return {
      png: png!,
      data: JSON.parse(json!.toString('utf8')) as {
        frames: { frame: object; spriteSourceSize: object; sourceSize: object }[];
        meta: { size: object; scale: string };
      },
    };
  };
  const one = await sheet('×1');
  const two = await sheet('×2');
  expect(pngSize(one.png)).toEqual([16, 4]);
  expect(pngSize(two.png)).toEqual([32, 8]);
  expect(two.data.frames[1]).toMatchObject({
    frame: { x: 16, y: 0, w: 16, h: 8 },
    spriteSourceSize: { x: 0, y: 0, w: 16, h: 8 },
    sourceSize: { w: 16, h: 8 },
  });
  expect(two.data.meta.size).toEqual({ w: 32, h: 8 });
  expect(two.data.meta.scale).toBe('1');
});

test('an output over 16384 px fails in the status bar without a file (AC-09)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 1639, height: 1 });
  const downloads = collectDownloads(page);
  await menu(page, 'File', 'Export', 'PNG (current frame)');
  await pngDialog(page).getByLabel('Scale').selectOption({ label: '×10' });
  await pngDialog(page).getByRole('button', { name: 'Export' }).click();
  await expect(page.getByText(/^Export failed: .*16384/)).toBeVisible();

  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  await sheetDialog(page).getByLabel('Scale').selectOption({ label: '×10' });
  await sheetDialog(page).getByRole('button', { name: 'Export' }).click();
  await expect(page.getByText(/^Sprite-sheet export failed: .*16384/)).toBeVisible();
  expect(downloads).toHaveLength(0);
});

test('the last used scale is preselected in both dialogs, also after a reload (AC-10)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 4, height: 4 });
  await menu(page, 'File', 'Export', 'PNG (current frame)');
  await pngDialog(page).getByLabel('Scale').selectOption({ label: '×3' });
  const png = await downloaded(page, () =>
    pngDialog(page).getByRole('button', { name: 'Export' }).click(),
  );
  expect(pngSize(png)).toEqual([12, 12]);

  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  expect(await selectedScale(sheetDialog(page))).toBe('×3');
  await sheetDialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect.poll(() => storedPref(page, 'exportScale')).toBe(3);

  await page.reload();
  await createDocument(page, 'pixel', { width: 4, height: 4 });
  await page.keyboard.press('ControlOrMeta+e');
  expect(await selectedScale(pngDialog(page))).toBe('×3');
  await pngDialog(page).getByRole('button', { name: 'Cancel' }).click();
  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  expect(await selectedScale(sheetDialog(page))).toBe('×3');
});
