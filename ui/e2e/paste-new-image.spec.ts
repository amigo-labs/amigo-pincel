import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  createDocument,
  gotoStart,
  menu,
  pixelCanvas,
  putClipboardImage,
  waitForEditor,
} from './helpers';

// Paste as New Image: Ctrl/⌘+V on the start screen, Edit ▸ Paste as New
// Image (Ctrl/⌘+Alt+V) in both editors (docs/specs/copy-cut-paste.md
// AC-22 – AC-26). Clipboard images are made test-side.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const imageSize = (page: Page, w: number, h: number) => page.getByText(`${w} × ${h} · sRGB`);

test('Ctrl+V on the start screen opens the clipboard image as a new Image document (AC-22)', async ({
  page,
}) => {
  await gotoStart(page);
  await putClipboardImage(page, 40, 30, '#ff0000');
  await page.keyboard.press('ControlOrMeta+v');
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 40, 30)).toBeVisible();
  await expect(page.getByText('Pasted image.png')).toBeVisible();
  await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(1);
});

test('Ctrl+Alt+V on the start screen opens the clipboard image too (AC-22)', async ({ page }) => {
  await gotoStart(page);
  await putClipboardImage(page, 20, 20, '#0000ff');
  await page.keyboard.press('ControlOrMeta+Alt+v');
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 20, 20)).toBeVisible();
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

  test(`${mode}: Ctrl+Alt+V opens the clipboard image as a new Image document (AC-23)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+Alt+v');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 24, 12)).toBeVisible();
    await expect(page.getByText('Pasted image.png')).toBeVisible();
  });

  test(`${mode}: Paste as New Image with no image shows a notice and keeps the document (AC-25)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await clearClipboard(page);
    await menu(page, 'Edit', 'Paste as New Image');
    await expect(page.getByText('The clipboard holds no image')).toBeVisible();
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    if (mode === 'pixel') await expect(pixelCanvas(page)).toBeVisible();
    else await expect(imageSize(page, 16, 16)).toBeVisible();
  });
}

test('pixel: Paste as New Image asks before discarding unsaved changes (AC-23)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.getByTitle('Pencil (B)').click();
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await putClipboardImage(page, 24, 12, '#00ff00');
  await menu(page, 'Edit', 'Paste as New Image');
  const confirm = page.getByRole('alertdialog');
  await expect(confirm).toBeVisible();
  await confirm.getByRole('button', { name: 'Cancel' }).click();
  await expect(pixelCanvas(page)).toBeVisible();
  await menu(page, 'Edit', 'Paste as New Image');
  await confirm.getByRole('button', { name: /Discard/ }).click();
  await waitForEditor(page, 'image');
  await expect(imageSize(page, 24, 12)).toBeVisible();
});
