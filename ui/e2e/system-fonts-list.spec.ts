import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';
import {
  SERIF,
  fontControl,
  fontEntries,
  fontList,
  installLocalFonts,
  lfaLog,
  openFontList,
  openPixelText,
  pickFamily,
  withoutLocalFonts,
  type FakeFont,
} from './font-helpers';
import { chooseText } from './text-helpers';

// docs/specs/system-fonts.md: the font family control and the font list in
// both Text tools — always Liberation Sans (AC-01), every family once and
// sorted (AC-02), search (AC-03), entries in their own font (AC-04), no face
// read for listing (AC-05), the permission flow of the PWA (AC-12 – AC-14).

const MORE: FakeFont[] = [
  {
    family: 'beta Sans',
    style: 'Regular',
    postscriptName: 'BetaSans',
    file: 'PincelTestSerif-Regular.ttf',
  },
  {
    family: 'Alpha',
    style: 'Regular',
    postscriptName: 'Alpha-Regular',
    file: 'PincelTestSerif-Regular.ttf',
  },
  {
    family: 'Alpha',
    style: 'Bold',
    postscriptName: 'Alpha-Bold',
    file: 'PincelTestSerif-Bold.ttf',
  },
  {
    family: 'Alpha',
    style: 'Black Italic',
    postscriptName: 'Alpha-BlackItalic',
    file: 'PincelTestSerif-BoldItalic.ttf',
  },
  {
    family: 'alpha',
    style: 'Regular',
    postscriptName: 'alpha-lower',
    file: 'PincelTestSerif-Regular.ttf',
  },
  {
    family: 'Liberation Sans',
    style: 'Bold',
    postscriptName: 'LiberationSans-Bold',
    file: 'PincelTestSerif-Bold.ttf',
  },
];

/** The entries' names, in list order. */
async function entryNames(page: Page): Promise<string[]> {
  return (await fontEntries(page).allTextContents()).map((s) => s.trim());
}

test('AC-01: both Text tools show the family; Liberation Sans is always listed', async ({
  page,
}) => {
  await withoutLocalFonts(page);
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Liberation Sans']);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toBeVisible();

  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Liberation Sans');
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Liberation Sans']);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toBeVisible();
});

test('AC-01: with system fonts granted the list offers no "Add font file…"', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  await expect(fontEntries(page)).toHaveCount(2);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toHaveCount(0);
});

test('AC-02 / AC-03: each family once, sorted, and searchable', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: [...SERIF, ...MORE] });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  expect(await entryNames(page)).toEqual([
    'Alpha',
    'alpha',
    'beta Sans',
    'Liberation Sans',
    'Pincel Test Serif',
  ]);

  const search = fontList(page).getByRole('searchbox', { name: 'Search fonts' });
  await expect(search).toBeFocused();
  await search.fill('SERIF');
  expect(await entryNames(page)).toEqual(['Pincel Test Serif']);
  await search.fill('sans');
  expect(await entryNames(page)).toEqual(['beta Sans', 'Liberation Sans']);
  await search.fill('zzz');
  await expect(fontEntries(page)).toHaveCount(0);
  await expect(fontList(page)).toContainText('No font matches');
  await search.fill('');
  await expect(fontEntries(page)).toHaveCount(5);
});

test('AC-04: each entry renders its name in its own font', async ({ page }) => {
  await installLocalFonts(page, { permission: 'granted', fonts: [...SERIF, ...MORE] });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  await expect
    .poll(() =>
      fontEntries(page).evaluateAll((items) =>
        items.map((el) => {
          const name = (el.textContent ?? '').trim();
          const first =
            getComputedStyle(el).fontFamily.split(',')[0]?.trim().replace(/^"|"$/g, '') ?? '';
          // A family rendered from bytes needs its FontFace loaded.
          const loaded =
            first === name ||
            [...document.fonts].some(
              (f) => f.family.replace(/^"|"$/g, '') === first && f.status === 'loaded',
            );
          return `${name}:${loaded}`;
        }),
      ),
    )
    .toEqual([
      'Alpha:true',
      'alpha:true',
      'beta Sans:true',
      'Liberation Sans:true',
      'Pincel Test Serif:true',
    ]);
  // The bundled entry is drawn with the bundled face, not a fallback.
  const bundled = fontEntries(page).filter({ hasText: /^Liberation Sans$/ });
  expect(await bundled.evaluate((el) => getComputedStyle(el).fontFamily)).toMatch(
    /^"?pincel-font-\d+"?, "Liberation Sans"/,
  );
});

test('AC-05: opening, searching and scrolling the list reads no system face', async ({ page }) => {
  const many: FakeFont[] = Array.from({ length: 300 }, (_, i) => ({
    family: `Family ${String(i).padStart(3, '0')}`,
    style: 'Regular',
    postscriptName: `Family${i}`,
    file: 'PincelTestSerif-Regular.ttf',
  }));
  await installLocalFonts(page, { permission: 'granted', fonts: [...SERIF, ...many] });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  await expect(fontEntries(page)).toHaveCount(302);
  const list = fontList(page).getByRole('listbox');
  await list.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await list.evaluate((el) => (el.scrollTop = 0));
  await fontList(page).getByRole('searchbox').fill('family 2');
  await expect(fontEntries(page)).toHaveCount(100);
  await page.keyboard.press('Escape');
  await expect(fontList(page)).toHaveCount(0);
  expect((await lfaLog(page)).reads).toEqual({});
});

test('AC-12: opening the list asks for the permission from that click and fills in on approval', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'prompt', answer: 'grant', fonts: SERIF });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  expect((await lfaLog(page)).queries).toBe(0);
  await openFontList(page);
  await expect(fontEntries(page)).toHaveCount(2);
  expect(await entryNames(page)).toEqual(['Liberation Sans', 'Pincel Test Serif']);
  const log = await lfaLog(page);
  expect(log.queries).toBe(1);
  expect(log.activations).toEqual([true]);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toHaveCount(0);
});

test('AC-13: denied access lists the bundled family with a note, no error dialog', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'denied', fonts: SERIF });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  expect(await entryNames(page)).toEqual(['Liberation Sans']);
  await expect(fontList(page).locator('[data-font-note]')).toContainText('site settings');
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toBeVisible();
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

test('AC-13: a dismissed prompt says that opening the list asks again, and it does', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'prompt', answer: 'dismiss', fonts: SERIF });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await openFontList(page);
  await expect(fontList(page).locator('[data-font-note]')).toContainText(
    'Opening this list again asks again',
  );
  expect(await entryNames(page)).toEqual(['Liberation Sans']);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toBeVisible();
  await page.keyboard.press('Escape');
  await openFontList(page);
  await expect.poll(async () => (await lfaLog(page)).queries).toBe(2);
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

test('AC-13: a browser without Local Font Access says so in the list', async ({ page }) => {
  await withoutLocalFonts(page);
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await openPixelText(page);
  await openFontList(page);
  await expect(fontList(page).locator('[data-font-note]')).toContainText(
    'cannot list system fonts',
  );
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toBeVisible();
});

test('AC-14: with access granted the remembered family is there at start, without a prompt', async ({
  page,
}) => {
  await installLocalFonts(page, { permission: 'granted', fonts: SERIF });
  await createDocument(page, 'image', { width: 64, height: 48 });
  await chooseText(page);
  await pickFamily(page, 'Pincel Test Serif');
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          new Promise<unknown>((resolve) => {
            const open = indexedDB.open('pincel');
            open.onsuccess = () => {
              const get = open.result
                .transaction('prefs')
                .objectStore('prefs')
                .get('textFont.image');
              get.onsuccess = () => {
                open.result.close();
                resolve((get.result as { value?: { family?: string } } | undefined)?.value?.family);
              };
            };
          }),
      ),
    )
    .toBe('Pincel Test Serif');

  await createDocument(page, 'image', { width: 64, height: 48 });
  // Listed at start, before the Text tool or the list: no prompt shows
  // while the permission is granted.
  await expect.poll(async () => (await lfaLog(page)).queries).toBe(1);
  expect((await lfaLog(page)).permission).toBe('granted');
  await chooseText(page);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
  await expect(fontControl(page).locator('[data-font-missing]')).toHaveCount(0);
  await expect(fontList(page)).toHaveCount(0);
});
