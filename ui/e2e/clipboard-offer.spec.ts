import { expect, test, type Locator, type Page } from '@playwright/test';
import { gotoStart, putClipboardImage, waitForEditor } from './helpers';
import {
  clipboardReads,
  focusWindow,
  heldClipboardReads,
  holdClipboard,
  installTauriMock,
  putFile,
  releaseClipboardRead,
  setClipboard,
  setMock,
  solidImage,
  type ClipContent,
} from './tauri-mock';

// The desktop app's clipboard offer on the start screen
// (docs/specs/copy-cut-paste.md AC-27 – AC-32), against the mocked Tauri
// bridge of `tauri-mock.ts`: its clipboard-manager read stands in for the
// operating system's clipboard, `tauri://focus` for the window gaining
// focus. The real clipboards stay a manual check (AC-33).

/** A valid 1×1 PNG. */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

const imageSize = (page: Page, w: number, h: number) => page.getByText(`${w} × ${h} · sRGB`);

function card(page: Page): Locator {
  return page.getByRole('region', { name: 'Image on the clipboard' });
}

function preview(page: Page): Locator {
  return card(page).getByRole('img', { name: 'Preview of the clipboard image' }).locator('canvas');
}

function tabs(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
}

/** Launches the app with `clip` on the clipboard; `note` keeps the
 *  first-launch file-association note open. */
async function launch(page: Page, clip: ClipContent, opts: { note?: boolean } = {}) {
  await page.addInitScript((c) => {
    (window as unknown as { __mock: { clipboard: ClipContent } }).__mock.clipboard = c;
  }, clip);
  await page.goto('./');
  if (!opts.note) await page.getByRole('button', { name: 'Got it' }).click();
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
}

/** The card shows nothing, also after the reads so far have answered. */
async function expectNoCard(page: Page): Promise<void> {
  await page.waitForTimeout(300);
  await expect(card(page)).toHaveCount(0);
}

async function createPixel(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'New pixel sprite' }).click();
  const form = page.getByRole('form', { name: 'New pixel sprite' });
  for (const label of ['Width', 'Height']) await form.getByLabel(label).fill('16');
  await form.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'pixel');
}

/** Closes the only open tab (a clean one: no prompt). */
async function closeTab(page: Page): Promise<void> {
  await page
    .getByRole('tablist', { name: 'Open documents' })
    .getByRole('button', { name: /^Close / })
    .click();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

test.describe('desktop app', () => {
  test.beforeEach(async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
  });

  test('at launch the card offers the clipboard image with a preview, its size, Open and Ignore (AC-27)', async ({
    page,
  }) => {
    await launch(page, solidImage(400, 300));
    await expect(card(page)).toBeVisible();
    await expect(card(page)).toContainText('400 × 300');
    await expect(preview(page)).toHaveAttribute('width', '160');
    await expect(preview(page)).toHaveAttribute('height', '120');
    await expect(card(page).getByRole('button', { name: 'Open', exact: true })).toBeVisible();
    await expect(card(page).getByRole('button', { name: 'Ignore', exact: true })).toBeVisible();
  });

  test('the preview keeps the aspect ratio and never scales up (AC-27)', async ({ page }) => {
    await launch(page, solidImage(100, 400));
    await expect(preview(page)).toHaveAttribute('width', '30');
    await expect(preview(page)).toHaveAttribute('height', '120');
    await setClipboard(page, solidImage(1, 1));
    await focusWindow(page);
    await expect(card(page)).toContainText('1 × 1');
    await expect(preview(page)).toHaveAttribute('width', '1');
    await expect(preview(page)).toHaveAttribute('height', '1');
  });

  test('a card waits behind a modal dialog and appears when it closes (AC-32)', async ({ page }) => {
    await launch(page, solidImage(40, 30), { note: true });
    await expect(page.getByRole('dialog')).toBeVisible();
    await expectNoCard(page);
    await page.getByRole('button', { name: 'Got it' }).click();
    await expect(card(page)).toBeVisible();
  });

  test('New and Ctrl+V still work while the card shows (AC-27)', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await launch(page, solidImage(40, 30));
    await expect(card(page)).toBeVisible();
    await createPixel(page);
    await expect(card(page)).toHaveCount(0);
    await closeTab(page);
    // The same image was offered: no card when the start screen returns.
    await expectNoCard(page);
    await setClipboard(page, solidImage(20, 10));
    await focusWindow(page);
    await expect(card(page)).toContainText('20 × 10');
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+v');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 24, 12)).toBeVisible();
    await expect(tabs(page)).toHaveCount(1);
  });

  test('Open… and a recent file still work while the card shows (AC-27)', async ({ page }) => {
    await launch(page, null);
    await putFile(page, '/pics/dot.png', PNG_1X1);
    await setMock(page, { openPick: '/pics/dot.png' });
    await setClipboard(page, solidImage(40, 30));
    await focusWindow(page);
    await expect(card(page)).toBeVisible();
    await page.getByRole('button', { name: 'Open…' }).click();
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 1, 1)).toBeVisible();
    await closeTab(page);
    // A new image after the tab closed: the card appears with it.
    await setClipboard(page, solidImage(30, 20));
    await focusWindow(page);
    await expect(card(page)).toContainText('30 × 20');
    await page.getByRole('list', { name: 'Recent files' }).getByRole('button', { name: /dot\.png/ }).click();
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 1, 1)).toBeVisible();
    await expect(tabs(page)).toHaveCount(1);
  });

  test('the card appears when the last tab closes and the clipboard holds a new image (AC-27)', async ({
    page,
  }) => {
    await launch(page, null);
    await expectNoCard(page);
    await createPixel(page);
    await setClipboard(page, solidImage(50, 40));
    await closeTab(page);
    await expect(card(page)).toContainText('50 × 40');
  });

  test('on window focus the card follows the clipboard (AC-28)', async ({ page }) => {
    await launch(page, null);
    await expectNoCard(page);
    const a = solidImage(40, 30, [255, 0, 0, 255]);
    const b = solidImage(50, 50, [0, 0, 255, 255]);
    await setClipboard(page, a);
    await focusWindow(page);
    await expect(card(page)).toContainText('40 × 30');
    // The same image again: the card stays as it is.
    const reads = await clipboardReads(page);
    await focusWindow(page);
    await expect.poll(() => clipboardReads(page)).toBe(reads + 1);
    await expect(card(page)).toContainText('40 × 30');
    // A new image replaces the shown one.
    await setClipboard(page, b);
    await focusWindow(page);
    await expect(card(page)).toContainText('50 × 50');
    // An image offered earlier, other than the shown one: the card goes.
    await setClipboard(page, a);
    await focusWindow(page);
    await expect(card(page)).toHaveCount(0);
    // A new image, then no image at all: the card goes again.
    await setClipboard(page, solidImage(8, 8));
    await focusWindow(page);
    await expect(card(page)).toContainText('8 × 8');
    await setClipboard(page, null);
    await focusWindow(page);
    await expect(card(page)).toHaveCount(0);
  });

  test('the same pixels at the same size count as the same image (AC-28)', async ({ page }) => {
    await launch(page, solidImage(4, 4, [10, 20, 30, 255]));
    await expect(card(page)).toContainText('4 × 4');
    await card(page).getByRole('button', { name: 'Ignore', exact: true }).click();
    await expect(card(page)).toHaveCount(0);
    // Copied again elsewhere: a fresh copy of the same pixels.
    await setClipboard(page, solidImage(4, 4, [10, 20, 30, 255]));
    await focusWindow(page);
    await expectNoCard(page);
    // One pixel differs: a new image.
    const other = solidImage(4, 4, [10, 20, 30, 255]);
    other.rgba[0] = 11;
    await setClipboard(page, other);
    await focusWindow(page);
    await expect(card(page)).toContainText('4 × 4');
  });

  test('Open opens the image the card shows, once; Ignore hides the card (AC-29)', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await launch(page, solidImage(40, 30));
    await expect(card(page)).toContainText('40 × 30');
    // The clipboard changes while the window keeps focus.
    await setClipboard(page, solidImage(50, 50));
    await putClipboardImage(page, 24, 12, '#00ff00');
    await card(page).getByRole('button', { name: 'Open', exact: true }).click();
    await page.keyboard.press('ControlOrMeta+v');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 40, 30)).toBeVisible();
    await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(1);
    await expect(card(page)).toHaveCount(0);
  });

  test('a double click on Open opens one document (AC-29)', async ({ page }) => {
    await launch(page, solidImage(40, 30));
    await card(page).getByRole('button', { name: 'Open', exact: true }).dblclick();
    await waitForEditor(page, 'image');
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(1);
  });

  test('a paste right after Open, while the image opens, opens nothing more (AC-29)', async ({
    page,
  }) => {
    await launch(page, solidImage(40, 30));
    await expect(card(page)).toBeVisible();
    // Open and a Ctrl+V paste in the same task: the paste comes while the
    // card's image is still being opened.
    await page.evaluate(async () => {
      const canvas = new OffscreenCanvas(24, 12);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillRect(0, 0, 24, 12);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      const data = new DataTransfer();
      data.items.add(new File([blob], 'image.png', { type: 'image/png' }));
      const open = Array.from(document.querySelectorAll('button')).find(
        (b) => b.textContent?.trim() === 'Open',
      );
      if (!open) throw new Error('no Open button');
      open.click();
      window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data }));
    });
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 40, 30)).toBeVisible();
    await page.waitForTimeout(500);
    await expect(tabs(page)).toHaveCount(1);
  });

  test('Ignore hides the card (AC-29)', async ({ page }) => {
    await launch(page, solidImage(40, 30));
    await card(page).getByRole('button', { name: 'Ignore', exact: true }).click();
    await expect(card(page)).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
  });

  test('a failed, refused or broken read shows no card and no notice and removes a shown one (AC-31)', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await launch(page, 'error');
    await expectNoCard(page);
    await expect(page.getByRole('alert')).toHaveCount(0);
    for (const [i, broken] of [
      'error' as const,
      { width: 0, height: 0, rgba: [] },
      { width: 0, height: 5, rgba: [] },
      // RGBA of the wrong length cannot be decoded.
      { width: 4, height: 4, rgba: [1, 2, 3] },
    ].entries()) {
      await setClipboard(page, solidImage(40, 30, [i, 0, 0, 255]));
      await focusWindow(page);
      await expect(card(page)).toBeVisible();
      await setClipboard(page, broken);
      await focusWindow(page);
      await expect(card(page)).toHaveCount(0);
      await expect(page.getByRole('alert')).toHaveCount(0);
    }
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+v');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 24, 12)).toBeVisible();
  });

  test('a read started before a tab opened shows no card when it answers; the last read counts (AC-32)', async ({
    page,
  }) => {
    await launch(page, null);
    await expectNoCard(page);
    await holdClipboard(page, true);
    await setClipboard(page, solidImage(40, 30));
    await focusWindow(page);
    await heldClipboardReads(page, 1);
    await createPixel(page);
    await setClipboard(page, solidImage(50, 40));
    await closeTab(page);
    await heldClipboardReads(page, 2);
    // The older read answers last-but-one: it shows nothing.
    await releaseClipboardRead(page, 0);
    await expectNoCard(page);
    await releaseClipboardRead(page, 1);
    await expect(card(page)).toContainText('50 × 40');
    // The 40 × 30 image never showed, so it was not offered.
    await holdClipboard(page, false);
    await setClipboard(page, solidImage(40, 30));
    await focusWindow(page);
    await expect(card(page)).toContainText('40 × 30');
  });

  test('a read that answers after a newer one is dropped (AC-32)', async ({ page }) => {
    await launch(page, null);
    await holdClipboard(page, true);
    await setClipboard(page, solidImage(40, 30));
    await focusWindow(page);
    await setClipboard(page, solidImage(50, 40));
    await focusWindow(page);
    await heldClipboardReads(page, 2);
    await releaseClipboardRead(page, 1);
    await expect(card(page)).toContainText('50 × 40');
    await releaseClipboardRead(page, 0);
    await page.waitForTimeout(300);
    await expect(card(page)).toContainText('50 × 40');
  });

  test('a read started before Ignore shows no card when it answers (AC-32)', async ({ page }) => {
    await launch(page, solidImage(40, 30));
    await expect(card(page)).toBeVisible();
    await holdClipboard(page, true);
    await setClipboard(page, solidImage(50, 40));
    await focusWindow(page);
    await heldClipboardReads(page, 1);
    await card(page).getByRole('button', { name: 'Ignore', exact: true }).click();
    await releaseClipboardRead(page, 0);
    await expectNoCard(page);
  });

  test('no card while a tab is open, also on window focus (AC-32)', async ({ page }) => {
    await launch(page, null);
    await createPixel(page);
    await setClipboard(page, solidImage(40, 30));
    await focusWindow(page);
    await expectNoCard(page);
  });
});

test.describe('browser app', () => {
  test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

  test('the PWA never shows the card; Ctrl+V still opens the clipboard image (AC-30)', async ({
    page,
  }) => {
    await gotoStart(page);
    await putClipboardImage(page, 40, 30, '#ff0000');
    await page.evaluate(() => window.dispatchEvent(new FocusEvent('focus')));
    await expectNoCard(page);
    await page.keyboard.press('ControlOrMeta+v');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 40, 30)).toBeVisible();
  });
});
