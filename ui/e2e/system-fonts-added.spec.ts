import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, createDocument, menu, setColor } from './helpers';
import {
  SERIF,
  fixtureBytes,
  fontControl,
  fontEntries,
  fontList,
  imageReference,
  installLocalFonts,
  openFontList,
  openPixelText,
  pickFamily,
  sameInk,
  statusBar,
  withoutLocalFonts,
  type FakeFont,
} from './font-helpers';
import { H, W, at, chooseText, liveInput, setFontSize, textBox } from './text-helpers';

// docs/specs/system-fonts.md: "Add font file…" — added faces join their
// families in both modes and survive a reload (AC-20), refused files are
// named with their reasons (AC-21), a newer face replaces an older one with
// the same family and style (AC-24), and an added font gone from IndexedDB
// falls back (AC-11).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** A file for the picker. */
interface Pick {
  name: string;
  mimeType: string;
  buffer: Buffer;
}

async function fixturePick(name: string): Promise<Pick> {
  return { name, mimeType: 'font/ttf', buffer: Buffer.from(await fixtureBytes(name)) };
}

/** Adds `files` through the open-able font list. */
async function addFiles(page: Page, files: Pick[]): Promise<void> {
  if (!(await fontList(page).isVisible())) await openFontList(page);
  const chooser = page.waitForEvent('filechooser');
  await fontList(page).getByRole('button', { name: 'Add font file…' }).click();
  await (await chooser).setFiles(files);
}

async function entryNames(page: Page): Promise<string[]> {
  return (await fontEntries(page).allTextContents()).map((s) => s.trim());
}

/** The Image Text tool, black, size 32. */
async function imageText(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: W, height: H });
  await setColor(page, 'fg', '#000000');
  await chooseText(page);
  await setFontSize(page, 32);
}

/** Types "Hg" at (20, 24), finishes it and returns the layer's pixels. */
async function imagePlace(page: Page): Promise<number[]> {
  const [x, y] = await at(page, [20.5, 24.5]);
  await page.mouse.click(x, y);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('Hg');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return (await copiedImage(page)).data;
}

const imageRef = (file: string, index = 0) =>
  imageReference({
    width: W,
    height: H,
    file,
    index,
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: 0x000000ff,
  });

/** The stored face rows, as `family/style → file id`. */
async function storedFaceRows(
  page: Page,
): Promise<{ faces: Record<string, string>; files: number }> {
  return page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const tx = open.result.transaction(['font_faces', 'font_files']);
          const faces = tx.objectStore('font_faces').getAll();
          const files = tx.objectStore('font_files').count();
          tx.oncomplete = () => {
            open.result.close();
            const out: Record<string, string> = {};
            for (const f of faces.result as { family: string; style: string; fileId: string }[]) {
              out[`${f.family}/${f.style}`] = f.fileId;
            }
            resolve({ faces: out, files: files.result });
          };
        };
      }),
  );
}

test('AC-20: added faces join both modes, become current, and survive a reload', async ({
  page,
}) => {
  await withoutLocalFonts(page);
  await imageText(page);
  await addFiles(page, [
    await fixturePick('PincelTestSerif-Regular.ttf'),
    await fixturePick('PincelTestSerif-Bold.ttf'),
  ]);
  await expect(statusBar(page)).toContainText('Added font: Pincel Test Serif');
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  expect(sameInk(await imagePlace(page), await imageRef('PincelTestSerif-Regular.ttf'))).toBe(true);

  await page.reload();
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  // Shared with Pixel mode, which keeps its own current family (AC-10).
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Liberation Sans', 'Pincel Test Serif']);

  // Bold uses the added Bold face, after a reload.
  await imageText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  expect(sameInk(await imagePlace(page), await imageRef('PincelTestSerif-Bold.ttf'))).toBe(true);
});

test('AC-20: every face of a collection is added; added families stay once access is granted', async ({
  page,
}) => {
  const alpha: FakeFont = {
    family: 'Alpha',
    style: 'Regular',
    postscriptName: 'Alpha',
    file: 'PincelTestSerif-Italic.ttf',
  };
  await installLocalFonts(page, { permission: 'prompt', answer: 'deny', fonts: [alpha] });
  await imageText(page);
  await addFiles(page, [await fixturePick('PincelTestSerif.ttc')]);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  expect(sameInk(await imagePlace(page), await imageRef('PincelTestSerif.ttc', 1))).toBe(true);

  await installLocalFonts(page, { permission: 'granted', fonts: [alpha, ...SERIF.slice(0, 1)] });
  await imageText(page);
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Alpha', 'Liberation Sans', 'Pincel Test Serif']);
});

test('AC-21: refused files are named with their reasons; the others are added', async ({
  page,
}) => {
  await withoutLocalFonts(page);
  await imageText(page);
  const woff: Pick = {
    name: 'web.woff2',
    mimeType: 'font/woff2',
    buffer: Buffer.concat([Buffer.from('wOF2'), Buffer.alloc(64)]),
  };
  const png: Pick = {
    name: 'photo.png',
    mimeType: 'image/png',
    buffer: Buffer.concat([
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
      Buffer.alloc(64),
    ]),
  };
  // Only refused files: the family stays.
  await addFiles(page, [woff]);
  await expect(statusBar(page)).toContainText(
    'Not added: web.woff2 (web fonts (WOFF) are not supported)',
  );
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await addFiles(page, [woff, png, await fixturePick('PincelTestSerif-Italic.ttf')]);
  const status = statusBar(page);
  await expect(status).toContainText('Added font: Pincel Test Serif');
  await expect(status).toContainText(
    'Not added: web.woff2 (web fonts (WOFF) are not supported); photo.png (not a TrueType or OpenType font)',
  );
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
});

test('AC-21: a file that cannot be stored is refused with that reason', async ({ page }) => {
  await withoutLocalFonts(page);
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (
      this: IDBObjectStore,
      ...args: Parameters<IDBObjectStore['put']>
    ) {
      if (this.name === 'font_files')
        throw new DOMException('Quota exceeded', 'QuotaExceededError');
      return put.apply(this, args);
    };
  });
  await imageText(page);
  await addFiles(page, [await fixturePick('PincelTestSerif-Regular.ttf')]);
  await expect(statusBar(page)).toContainText(
    'Not added: PincelTestSerif-Regular.ttf (could not be stored)',
  );
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await expect(fontEntries(page)).toHaveCount(1);
});

test('AC-24: a newer face with the same family and style replaces the older one', async ({
  page,
}) => {
  await withoutLocalFonts(page);
  await imageText(page);
  await addFiles(page, [await fixturePick('PincelTestSerif-Regular.ttf')]);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  const first = await storedFaceRows(page);
  expect(Object.keys(first.faces)).toEqual(['Pincel Test Serif/Regular']);
  await addFiles(page, [await fixturePick('PincelTestSerif.ttc')]);
  await expect(statusBar(page)).toContainText('Added font: Pincel Test Serif');
  const second = await storedFaceRows(page);
  expect(Object.keys(second.faces).sort()).toEqual([
    'Pincel Test Serif/Bold',
    'Pincel Test Serif/Regular',
  ]);
  expect(second.faces['Pincel Test Serif/Regular']).not.toBe(
    first.faces['Pincel Test Serif/Regular'],
  );
  expect(second.faces['Pincel Test Serif/Regular']).toBe(second.faces['Pincel Test Serif/Bold']);
  expect(second.files).toBe(1);
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Liberation Sans', 'Pincel Test Serif']);
});

test('AC-11: an added family gone from IndexedDB falls back to Liberation Sans', async ({
  page,
}) => {
  await withoutLocalFonts(page);
  await imageText(page);
  await addFiles(page, [await fixturePick('PincelTestSerif-Regular.ttf')]);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await pickFamily(page, 'Pincel Test Serif');
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction(['font_faces', 'font_files'], 'readwrite');
          tx.objectStore('font_faces').clear();
          tx.objectStore('font_files').clear();
          tx.oncomplete = () => {
            open.result.close();
            resolve();
          };
        };
      }),
  );
  await imageText(page);
  await expect(statusBar(page)).toContainText(
    'Font "Pincel Test Serif" is not available — Liberation Sans is used instead',
  );
  await expect(fontControl(page).locator('[data-font-missing]')).toBeVisible();
  expect(sameInk(await imagePlace(page), await imageRef('LiberationSans-Regular.ttf'))).toBe(true);
});
