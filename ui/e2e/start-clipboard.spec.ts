import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  gotoStart,
  menu,
  pixelOf,
  putClipboardImage,
  waitForEditor,
} from './helpers';
import { installTauriMock, solidImage, type ClipContent } from './tauri-mock';

// The start screen's "Open from clipboard" row with "As pixel sprite" and
// "As image" (docs/specs/copy-cut-paste.md AC-34 – AC-38). Clipboard images
// are made test-side; a refused, unsupported, broken or held read is a
// stand-in for `navigator.clipboard.read`.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const imageSize = (page: Page, w: number, h: number) => page.getByText(`${w} × ${h} px · sRGB`);
const tabs = (page: Page) => page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
const layerRows = (page: Page) =>
  page.getByRole('list', { name: 'Layer list' }).getByRole('button', { name: /^Drag to reorder / });

function row(page: Page): Locator {
  return page.getByRole('group', { name: 'Open from clipboard' });
}

function asPixel(page: Page): Locator {
  return row(page).getByRole('button', { name: 'As pixel sprite', exact: true });
}

function asImage(page: Page): Locator {
  return row(page).getByRole('button', { name: 'As image', exact: true });
}

/** Closes the only open tab (a clean one: no prompt). */
async function closeTab(page: Page): Promise<void> {
  await page
    .getByRole('tablist', { name: 'Open documents' })
    .getByRole('button', { name: /^Close / })
    .click();
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
}

/** The start screen as it was: no tab, no editor, the New cards enabled. */
async function expectStartUnchanged(page: Page): Promise<void> {
  await page.waitForTimeout(300);
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
  await expect(tabs(page)).toHaveCount(0);
  await expect(page.locator('canvas[data-ready]')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'New image' })).toBeEnabled();
}

/** Every following `navigator.clipboard.read` waits until `releaseRead`. */
async function holdRead(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __release: (() => void)[]; __reads: number };
    const real = navigator.clipboard.read.bind(navigator.clipboard);
    w.__release = [];
    w.__reads = 0;
    navigator.clipboard.read = () => {
      w.__reads += 1;
      return new Promise((resolve, reject) => {
        w.__release.push(() => void real().then(resolve, reject));
      });
    };
  });
}

/** Lets every held read answer with the real clipboard. */
async function releaseRead(page: Page): Promise<void> {
  await page.evaluate(() => {
    for (const go of (window as unknown as { __release: (() => void)[] }).__release) go();
  });
}

async function heldReads(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __reads: number }).__reads);
}

/** How a read finds no image (AC-37). */
type NoImage = 'empty' | 'text' | 'refused' | 'unsupported' | 'broken';

async function makeNoImage(page: Page, how: NoImage): Promise<void> {
  if (how === 'empty' || how === 'text') {
    await page.evaluate((h) => navigator.clipboard.writeText(h === 'text' ? 'hello' : ''), how);
    return;
  }
  await page.evaluate((h) => {
    const clip = navigator.clipboard as { read?: unknown };
    if (h === 'unsupported') clip.read = undefined;
    else if (h === 'refused') {
      clip.read = () => Promise.reject(new DOMException('denied', 'NotAllowedError'));
    } else {
      // Bytes that claim to be a PNG and decode to nothing.
      const blob = new Blob([new Uint8Array([137, 80, 78, 71, 1, 2, 3, 4])], { type: 'image/png' });
      clip.read = () => Promise.resolve([new ClipboardItem({ 'image/png': blob })]);
    }
  }, how);
}

test.describe('browser app', () => {
  test('the row sits directly below the New cards with both buttons (AC-34)', async ({ page }) => {
    await gotoStart(page);
    await expect(row(page)).toBeVisible();
    await expect(asPixel(page)).toBeEnabled();
    await expect(asImage(page)).toBeEnabled();
    const pixelCard = await page.getByRole('button', { name: 'New pixel sprite' }).boundingBox();
    const imageCard = await page.getByRole('button', { name: 'New image' }).boundingBox();
    const box = await row(page).boundingBox();
    if (!pixelCard || !imageCard || !box) throw new Error('start screen not laid out');
    const cardsBottom = Math.max(pixelCard.y + pixelCard.height, imageCard.y + imageCard.height);
    expect(box.y).toBeGreaterThanOrEqual(cardsBottom);
    expect(box.y - cardsBottom).toBeLessThanOrEqual(24);
    expect(Math.abs(box.x - Math.min(pixelCard.x, imageCard.x))).toBeLessThanOrEqual(1);
    // Nothing else sits between the cards and the row.
    const open = await page.getByRole('button', { name: 'Open…' }).boundingBox();
    if (open) expect(open.y).toBeGreaterThan(box.y);
  });

  test('the buttons work by keyboard (AC-34, AC-35)', async ({ page }) => {
    await gotoStart(page);
    await putClipboardImage(page, 18, 14, '#ff0000');
    await asImage(page).focus();
    await page.keyboard.press('Enter');
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 18, 14)).toBeVisible();
  });

  test('the buttons are disabled while the start screen opens a file (AC-34)', async ({ page }) => {
    await gotoStart(page);
    await page.evaluate(() => {
      Object.assign(window, { showOpenFilePicker: () => new Promise(() => {}) });
    });
    await page.getByRole('button', { name: 'Open…' }).click();
    await expect(page.getByRole('button', { name: 'New image' })).toBeDisabled();
    await expect(asPixel(page)).toBeDisabled();
    await expect(asImage(page)).toBeDisabled();
  });

  test('As image opens the clipboard image as a new Image document (AC-35)', async ({ page }) => {
    await gotoStart(page);
    await putClipboardImage(page, 40, 30, '#ff0000');
    await asImage(page).click();
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 40, 30)).toBeVisible();
    await expect(page.getByTitle('Pasted image.png', { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(1);
    await expect(tabs(page)).toHaveCount(1);
  });

  test('As image opens the in-app copy when the clipboard holds no image (AC-35)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 20, height: 12 });
    await clearClipboard(page);
    await menu(page, 'Edit', 'Copy');
    await copiedImage(page);
    await clearClipboard(page);
    await closeTab(page);
    await asImage(page).click();
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 20, 12)).toBeVisible();
  });

  test('As pixel sprite opens an RGBA sprite with one layer and one frame holding the image (AC-36)', async ({
    page,
  }) => {
    await page.addInitScript(() => {
      delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
    });
    await gotoStart(page);
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
    await asPixel(page).click();
    await waitForEditor(page, 'pixel');
    await expect(page.getByTitle('Pasted image.aseprite', { exact: true })).toBeVisible();
    await expect(page.locator('footer')).toContainText('6×4');
    await expect(layerRows(page)).toHaveCount(1);
    await expect(page.getByRole('region', { name: 'Timeline' }).getByText('frame 1/1')).toBeVisible();
    await clearClipboard(page);
    await menu(page, 'Edit', 'Copy');
    const img = await copiedImage(page);
    expect([img.width, img.height]).toEqual([6, 4]);
    expect(pixelOf(img, 0, 0)).toEqual([0, 255, 0, 255]);
    expect(pixelOf(img, 5, 3)).toEqual([0, 0, 255, 255]);
    // No file behind it: Save hands over a download, as for a new sprite.
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.keyboard.press('ControlOrMeta+s'),
    ]);
    expect(download.suggestedFilename()).toBe('Pasted image.aseprite');
  });

  test.describe('in German', () => {
    test.use({ locale: 'de-DE' });

    test('As pixel sprite titles the sprite "Eingefügtes Bild.aseprite" (AC-34, AC-36)', async ({
      page,
    }) => {
      await page.goto('./');
      await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
      const group = page.getByRole('group', { name: 'Aus der Zwischenablage öffnen' });
      await expect(group.getByRole('button', { name: 'Als Bild', exact: true })).toBeVisible();
      await putClipboardImage(page, 8, 8, '#ff00ff');
      await group.getByRole('button', { name: 'Als Pixel-Sprite', exact: true }).click();
      await waitForEditor(page, 'pixel');
      await expect(page.getByTitle('Eingefügtes Bild.aseprite', { exact: true })).toBeVisible();
    });
  });

  for (const how of ['empty', 'text', 'refused', 'unsupported', 'broken'] as const) {
    test(`with no image (${how}) both buttons show the no-image notice and open nothing (AC-37)`, async ({
      page,
    }) => {
      await gotoStart(page);
      await makeNoImage(page, how);
      for (const button of [asPixel(page), asImage(page)]) {
        await button.click();
        const notice = page.getByRole('alert').filter({ hasText: 'The clipboard holds no image' });
        await expect(notice).toBeVisible();
        await expectStartUnchanged(page);
        await notice.getByRole('button').click();
        await expect(notice).toHaveCount(0);
      }
    });
  }

  test('while a read waits, the buttons, Ctrl+V and Ctrl+Alt+V open nothing more (AC-38)', async ({
    page,
  }) => {
    await gotoStart(page);
    await putClipboardImage(page, 30, 20, '#ff0000');
    await holdRead(page);
    await asImage(page).click();
    await expect.poll(() => heldReads(page)).toBe(1);
    await asPixel(page).click();
    await asImage(page).click();
    await page.keyboard.press('ControlOrMeta+v');
    await page.keyboard.press('ControlOrMeta+Alt+v');
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(0);
    await releaseRead(page);
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 30, 20)).toBeVisible();
    await page.waitForTimeout(500);
    await expect(tabs(page)).toHaveCount(1);
  });

  test('a double click on As pixel sprite opens one document (AC-38)', async ({ page }) => {
    await gotoStart(page);
    await putClipboardImage(page, 12, 10, '#00ff00');
    await asPixel(page).dblclick();
    await waitForEditor(page, 'pixel');
    await page.waitForTimeout(500);
    await expect(tabs(page)).toHaveCount(1);
  });
});

test.describe('desktop app', () => {
  test.beforeEach(async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
  });

  async function launch(page: Page, clip: ClipContent): Promise<void> {
    await page.addInitScript((c) => {
      (window as unknown as { __mock: { clipboard: ClipContent } }).__mock.clipboard = c;
    }, clip);
    await page.goto('./');
    await page.getByRole('button', { name: 'Got it' }).click();
    await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
  }

  test('the desktop start screen shows the row too (AC-34)', async ({ page }) => {
    await launch(page, null);
    await expect(asPixel(page)).toBeEnabled();
    await expect(asImage(page)).toBeEnabled();
  });

  test('while a read waits, Open on the clipboard card opens nothing more (AC-38)', async ({
    page,
  }) => {
    await launch(page, solidImage(40, 30));
    const offer = page.getByRole('region', { name: 'Image on the clipboard' });
    await expect(offer).toBeVisible();
    await putClipboardImage(page, 24, 12, '#0000ff');
    await holdRead(page);
    await asImage(page).click();
    await expect.poll(() => heldReads(page)).toBe(1);
    await offer.getByRole('button', { name: 'Open', exact: true }).click();
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(0);
    await releaseRead(page);
    await waitForEditor(page, 'image');
    await expect(imageSize(page, 24, 12)).toBeVisible();
    await page.waitForTimeout(500);
    await expect(tabs(page)).toHaveCount(1);
  });
});
