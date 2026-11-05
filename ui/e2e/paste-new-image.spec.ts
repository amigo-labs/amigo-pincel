import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  gotoStart,
  menu,
  pixelCanvas,
  pixelOf,
  putClipboardImage,
  waitForEditor,
} from './helpers';

// Paste as New Image: Ctrl/⌘+V on the start screen, Edit ▸ Paste as New
// Image (Ctrl/⌘+Alt+V) in both editors (docs/specs/copy-cut-paste.md
// AC-22 – AC-26). Clipboard images are made test-side.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const imageSize = (page: Page, w: number, h: number) => page.getByText(`${w} × ${h} · sRGB`);
const tabs = (page: Page) => page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
const layerRows = (page: Page) =>
  page.getByRole('list', { name: 'Layer list' }).getByRole('button', { name: /^Drag to reorder / });

/** Copies the whole active layer (no selection) and waits for the write. */
async function copyLayer(page: Page): Promise<void> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  await copiedImage(page);
}

/** Makes the browser refuse every clipboard write from now on. */
async function refuseClipboardWrites(page: Page): Promise<void> {
  await page.evaluate(() => {
    navigator.clipboard.write = () => Promise.reject(new DOMException('denied', 'NotAllowedError'));
  });
}

/** Closes the only open tab (a clean one: no prompt). */
async function closeTab(page: Page): Promise<void> {
  await page
    .getByRole('tablist', { name: 'Open documents' })
    .getByRole('button', { name: /^Close / })
    .click();
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
}

test('Ctrl+V on the start screen opens the clipboard image as a new Image document (AC-22)', async ({
  page,
}) => {
  await gotoStart(page);
  await putClipboardImage(page, 40, 30, '#ff0000');
  await page.keyboard.press('ControlOrMeta+v');
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 40, 30)).toBeVisible();
  await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(1);
});

test('Ctrl+Alt+V on the start screen opens the clipboard image too (AC-22)', async ({ page }) => {
  await gotoStart(page);
  await putClipboardImage(page, 20, 20, '#0000ff');
  await page.keyboard.press('ControlOrMeta+Alt+v');
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 20, 20)).toBeVisible();
});

test('Ctrl+V on the start screen with no clipboard image opens the in-app copy (AC-22)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 20, height: 12 });
  await copyLayer(page);
  await clearClipboard(page);
  await closeTab(page);
  await page.keyboard.press('ControlOrMeta+v');
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 20, 12)).toBeVisible();
  await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
});

test('Ctrl+V on the start screen with text only does nothing', async ({ page }) => {
  await gotoStart(page);
  await page.evaluate(() => navigator.clipboard.writeText('hello'));
  await page.keyboard.press('ControlOrMeta+v');
  await page.waitForTimeout(300);
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
});

test('Ctrl+V in a start-screen text field creates no document (AC-26)', async ({ page }) => {
  await gotoStart(page);
  await putClipboardImage(page, 40, 30, '#ff0000');
  await page.getByRole('button', { name: /New image/i }).first().click();
  const width = page.getByLabel('Width').first();
  await width.focus();
  await page.keyboard.press('ControlOrMeta+v');
  await page.waitForTimeout(300);
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the Edit menu lists Paste as New Image below Paste (AC-24)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
    const edit = page.getByRole('menu', { name: 'Edit', exact: true });
    const items = await edit.getByRole('menuitem').allInnerTexts();
    const paste = items.findIndex((s) => s.startsWith('Paste') && !s.startsWith('Paste as'));
    expect(items[paste + 1]).toContain('Paste as New Image');
    await expect(edit.getByRole('menuitem', { name: /Paste as New Image/ })).toContainText('Ctrl+Alt+V');
  });

  test(`${mode}: Ctrl+Alt+V opens the clipboard image as a new tab of this mode (AC-23, document-tabs AC-02)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+Alt+v');
    if (mode === 'image') {
      await expect(imageSize(page, 24, 12)).toBeVisible();
      await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
    } else {
      await expect(page.getByTitle('Pasted image.aseprite', { exact: true })).toBeVisible();
      await expect(page.locator('footer')).toContainText('24×12');
    }
    await expect(page.getByRole('tab')).toHaveCount(2);
  });

  test(`${mode}: Paste as New Image with no image shows a notice and keeps the document (AC-25)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await clearClipboard(page);
    await menu(page, 'Edit', 'Paste as New Image');
    await expect(page.getByText('The clipboard holds no image')).toBeVisible();
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(1);
    await expect(tabs(page).first()).toHaveAttribute('aria-selected', 'true');
    if (mode === 'pixel') await expect(pixelCanvas(page)).toBeVisible();
    else await expect(imageSize(page, 16, 16)).toBeVisible();
  });
}

test('pixel: Paste as New Image with unsaved changes opens a tab without a prompt (AC-23, document-tabs AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.getByTitle('Pencil (B)').click();
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await putClipboardImage(page, 24, 12, '#00ff00');
  await menu(page, 'Edit', 'Paste as New Image');
  await expect(page.getByTitle('Pasted image.aseprite', { exact: true })).toBeVisible();
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
  await expect(page.getByRole('tab')).toHaveCount(2);
  await expect(page.getByRole('tab').first().getByText('unsaved changes')).toHaveCount(1);
});

test('pixel: Paste as New Image makes an RGBA sprite with one image layer and one frame holding the image (AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  // Opaque green left half, opaque blue right half.
  await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(6, 4);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.fillStyle = '#00ff00';
    ctx.fillRect(0, 0, 3, 4);
    ctx.fillStyle = '#0000ff';
    ctx.fillRect(3, 0, 3, 4);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
  });
  await menu(page, 'Edit', 'Paste as New Image');
  await expect(page.getByTitle('Pasted image.aseprite', { exact: true })).toBeVisible();
  await expect(page.locator('footer')).toContainText('6×4');
  await expect(layerRows(page)).toHaveCount(1);
  await expect(page.getByRole('region', { name: 'Timeline' }).getByText('frame 1/1')).toBeVisible();
  // The layer holds the image's RGBA colours, read back through Copy.
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  const img = await copiedImage(page);
  expect([img.width, img.height]).toEqual([6, 4]);
  expect(pixelOf(img, 0, 0)).toEqual([0, 255, 0, 255]);
  expect(pixelOf(img, 2, 3)).toEqual([0, 255, 0, 255]);
  expect(pixelOf(img, 3, 0)).toEqual([0, 0, 255, 255]);
  expect(pixelOf(img, 5, 3)).toEqual([0, 0, 255, 255]);
});

test('pixel: Paste as New Image opens the in-app copy when the system write failed (AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 20, height: 12 });
  // A stale image the system clipboard keeps when Pincel's write fails.
  await putClipboardImage(page, 3, 3, '#0000ff');
  await refuseClipboardWrites(page);
  await page.keyboard.press('ControlOrMeta+c');
  await menu(page, 'Edit', 'Paste as New Image');
  await expect(page.getByTitle('Pasted image.aseprite', { exact: true })).toBeVisible();
  await expect(page.locator('footer')).toContainText('20×12');
  await expect(tabs(page)).toHaveCount(2);
});

test('image: Paste as New Image opens the in-app copy when the system clipboard holds no image (AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 12 });
  await copyLayer(page);
  await clearClipboard(page);
  await menu(page, 'Edit', 'Paste as New Image');
  await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
  await expect(imageSize(page, 20, 12)).toBeVisible();
  await expect(tabs(page)).toHaveCount(2);
});
