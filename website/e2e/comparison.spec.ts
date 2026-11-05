import { expect, test, type Locator, type Page } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';

// The two home-page comparison tables (docs/specs/website-comparison.md;
// docs/specs/website.md AC-09 defers to them). Every competitor cell follows
// pincel.md §18.

/** Each cell's text with its whitespace collapsed (`✅ native`). */
async function cellTexts(row: Locator): Promise<string[]> {
  return row
    .locator('th, td')
    .evaluateAll((cells) => cells.map((c) => (c.textContent ?? '').replace(/\s+/g, ' ').trim()));
}

const pixelTable = (page: Page): Locator =>
  page.getByRole('region', { name: en.comparison.pixel.label }).getByRole('table');
const imageTable = (page: Page): Locator =>
  page.getByRole('region', { name: en.comparison.image.label }).getByRole('table');

/** Every body row of `table` as cell texts. */
async function bodyRows(table: Locator): Promise<string[][]> {
  const rows = table.locator('tbody tr');
  const out: string[][] = [];
  for (let i = 0; i < (await rows.count()); i++) out.push(await cellTexts(rows.nth(i)));
  return out;
}

const PIXEL_ROWS = [
  ['Aseprite file format (read+write)', '🟡 RGBA only', '✅ native', '✅ import / export', '❌'],
  ['Tilemaps', '✅', '✅', '✅', '❌'],
  ['Slices (9-patch, pivot)', '✅', '✅', '❌', '❌'],
  ['Animation timeline + tags', '✅', '✅', '✅', '✅ basic'],
  ['Tablet / pen support', '✅', '✅', '✅', '❌'],
  ['Runs in browser', '✅', '❌', '✅', '✅'],
  [
    'Layered image editing (photos, screenshots)',
    '✅ Image mode',
    '❌ pixel art only',
    '❌ pixel art only',
    '❌',
  ],
  ['Native desktop app', '✅ Tauri', '✅', '✅', '✅ via NW.js, ageing'],
  ['Embeddable as a library', '✅', '❌', '❌', '❌'],
  ['Lua scripting / extensions', '❌ Phase 2', '✅', '✅ extensions', '❌'],
  ['Custom brushes', '❌ Phase 2', '✅', '✅', '❌'],
  ['Pixel-perfect pencil', '✅', '✅', '✅', '❌'],
  ['Animated export (GIF / APNG)', '✅', '🟡 GIF only', '✅', '🟡 GIF only'],
  ['Pen pressure', '✅', '✅', '✅', '❌'],
  ['Indexed-color painting', '❌', '✅', '✅', '❌'],
  ['PSD import', '🟡 read, 8-bit RGB (Image mode)', '❌', '✅ import', '❌'],
  ['Price', 'Free', 'Paid', 'Free', 'Free'],
  [
    'Open source',
    '✅ MIT/Apache',
    '🟡 source-available, EULA forbids redistribution',
    '✅ MIT',
    '✅ Apache 2.0',
  ],
  [
    'Active development',
    '✅',
    '✅',
    '✅',
    '🟡 no tagged release since 0.15.0 (2018); master still gets fixes',
  ],
];

const IMAGE_ROWS = [
  ['Windows', '✅', '✅', '✅', '🟡 in the browser'],
  ['Mac', '✅', '🟡 Wine, beta', '✅', '🟡 in the browser'],
  ['Linux', '✅', '🟡 Wine, beta', '✅', '🟡 in the browser'],
  ['Web', '✅', '❌', '❌', '✅'],
  ['Layers + blend modes', '✅', '✅', '✅', '✅'],
  ['Adjustments and effects', '✅', '✅', '✅', '✅'],
  ['Selections + magic wand', '✅', '✅', '✅', '✅'],
  ['Text', '✅', '🟡 simple', '✅', '✅'],
  ['Pen pressure', '✅', '✅', '❌', '✅'],
  ['PSD import', '🟡 read, 8-bit RGB (Image mode)', '🟡 third-party plugin', '❌', '✅'],
  [
    'Price / license',
    'Free MIT/Apache',
    'Free proprietary',
    'Free MIT',
    'Free with ads; Premium subscription',
  ],
  ['Offline use', '✅', '✅', '✅', '✅ once loaded'],
];

test('website-comparison AC-01: two tables, Pixel art and Image editing, with their columns', async ({
  page,
}) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Pixel art', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Image editing', exact: true })).toBeVisible();
  expect(await cellTexts(pixelTable(page).locator('thead tr'))).toEqual([
    'Feature',
    'Pincel',
    'Aseprite',
    'Pixelorama',
    'Piskel',
  ]);
  expect(await cellTexts(imageTable(page).locator('thead tr'))).toEqual([
    'Feature',
    'Pincel',
    'Paint.NET',
    'Pinta',
    'Photopea',
  ]);
});

test('website-comparison AC-02: the Pixel art table keeps the old rows and adds the gap rows', async ({
  page,
}) => {
  await page.goto('/');
  expect(await bodyRows(pixelTable(page))).toEqual(PIXEL_ROWS);
});

test('website-comparison AC-03: the Image editing table has its twelve rows', async ({ page }) => {
  await page.goto('/');
  expect(await bodyRows(imageTable(page))).toEqual(IMAGE_ROWS);
});

/** The text of the cell in row `feature` under column header `column`. */
async function cellText(table: Locator, feature: string, column: string): Promise<string> {
  const headers = await cellTexts(table.locator('thead tr'));
  const index = headers.indexOf(column);
  expect(index, column).toBeGreaterThan(0);
  const row = table.locator('tbody tr').filter({
    has: table.page().getByRole('rowheader', { name: feature, exact: true }),
  });
  await expect(row).toHaveCount(1);
  return (await cellTexts(row))[index] ?? '';
}

test('website-comparison AC-05: Pincel cells show what ships, with notes for the limits', async ({
  page,
}) => {
  await page.goto('/');
  const pixel = pixelTable(page);
  expect(await cellText(pixel, 'Pixel-perfect pencil', 'Pincel')).toBe('✅');
  expect(await cellText(pixel, 'Animated export (GIF / APNG)', 'Pincel')).toBe('✅');
  expect(await cellText(pixel, 'Pen pressure', 'Pincel')).toBe('✅');
  expect(await cellText(pixel, 'Indexed-color painting', 'Pincel')).toBe('❌');
  expect(await cellText(pixel, 'Aseprite file format (read+write)', 'Pincel')).toBe('🟡 RGBA only');
  for (const table of [pixel, imageTable(page)]) {
    expect(await cellText(table, 'PSD import', 'Pincel')).toBe('🟡 read, 8-bit RGB (Image mode)');
  }
  expect(await cellText(imageTable(page), 'Pen pressure', 'Pincel')).toBe('✅');
});

test('website-comparison AC-06: Aseprite is source-available, not open source', async ({
  page,
}) => {
  await page.goto('/');
  const cell = await cellText(pixelTable(page), 'Open source', 'Aseprite');
  expect(cell).toContain('source-available');
  expect(cell).not.toContain('✅');
  expect(cell).not.toMatch(/open source/i);
});

test('website-comparison AC-07: Piskel has had no tagged release since 0.15.0', async ({
  page,
}) => {
  await page.goto('/');
  const cell = await cellText(pixelTable(page), 'Active development', 'Piskel');
  expect(cell).toContain('no tagged release since 0.15.0 (2018)');
  expect(cell).toContain('master still gets fixes');
  expect(cell).not.toContain('modernization in progress');
});

test('website-comparison AC-08: each table names its snapshot and its sources', async ({
  page,
}) => {
  await page.goto('/');
  const pixel = page.locator('[data-footnotes="pixel"]');
  const image = page.locator('[data-footnotes="image"]');
  for (const notes of [pixel, image]) {
    await expect(notes).toContainText('as of 2026-11');
    await notes.getByText('Sources', { exact: true }).click();
  }
  // The honesty footnote stays under the Pixel art table.
  await expect(pixel).toContainText('Aseprite has 14 years of features');
  await expect(image).not.toContainText('Aseprite has 14 years of features');
  for (const source of [
    'aseprite.org',
    'github.com/Orama-Interactive/Pixelorama',
    'github.com/piskelapp/piskel',
  ]) {
    await expect(pixel.getByRole('listitem').filter({ hasText: source })).toHaveCount(1);
  }
  await expect(pixel).toContainText('LibreSprite and Resprite');
  for (const source of ['paint.net', 'github.com/PintaProject/Pinta', 'photopea.com/learn']) {
    await expect(image.getByRole('listitem').filter({ hasText: source })).toHaveCount(1);
  }
  await expect(image).not.toContainText('LibreSprite');
});

test('website-comparison AC-10: the German site shows every heading, label, note and footnote in German', async ({
  page,
}) => {
  const copyOf = async (
    url: string,
  ): Promise<{ headings: string[]; rows: string[][]; notes: string }> => {
    await page.goto(url);
    const regions = page.locator('.table-wrap');
    const rows: string[][] = [];
    for (let i = 0; i < (await regions.count()); i++)
      rows.push(...(await bodyRows(regions.nth(i).getByRole('table'))));
    return {
      headings: (await page.locator('.table-title').allTextContents()).map((s) => s.trim()),
      rows,
      notes: (await page.locator('.footnotes').allTextContents()).join(' ').replace(/\s+/g, ' '),
    };
  };
  const english = await copyOf('/');
  const german = await copyOf('/de');
  expect(german.headings).toEqual([de.comparison.pixel.title, de.comparison.image.title]);
  expect(english.headings).toEqual([en.comparison.pixel.title, en.comparison.image.title]);
  expect(german.rows.length).toBe(english.rows.length);

  const words = (copy: typeof en.comparison): Set<string> =>
    new Set([...Object.values(copy.rows), ...Object.values(copy.notes), copy.free, copy.paid]);
  const enOnly = [...words(en.comparison)].filter((w) => !words(de.comparison).has(w));
  const deOnly = [...words(de.comparison)].filter((w) => !words(en.comparison).has(w));
  // A row's label, and each cell's word or note after its mark.
  const pieces = (rows: string[][]): Set<string> =>
    new Set(
      rows.flatMap((r) => [r[0] ?? '', ...r.slice(1).map((c) => c.replace(/^(✅|❌|🟡)\s*/u, ''))]),
    );
  const germanPieces = pieces(german.rows);
  const englishPieces = pieces(english.rows);
  // No English-only label or note on the German site, and the reverse.
  for (const w of enOnly) expect(germanPieces.has(w), w).toBe(false);
  for (const w of deOnly) expect(englishPieces.has(w), w).toBe(false);
  // Every label and note the German site shows is German copy.
  for (const p of germanPieces) {
    if (p !== '')
      expect(words(de.comparison).has(p) || p.startsWith(de.comparison.free), p).toBe(true);
  }
  // Row labels are exactly the language's own.
  expect(german.rows.map((r) => r[0])).toEqual(
    english.rows.map((r) => {
      const key = Object.entries(en.comparison.rows).find(([, v]) => v === r[0])?.[0];
      return de.comparison.rows[key as keyof typeof de.comparison.rows];
    }),
  );
  expect(german.notes).toContain(de.comparison.snapshot);
  expect(german.notes).toContain(de.comparison.footnote);
  expect(english.notes).toContain(en.comparison.snapshot);
});

test.describe('on a phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  for (const url of ['/', '/de']) {
    test(`website-comparison AC-09: ${url} tables scroll inside their wrappers with Pincel in view`, async ({
      page,
    }) => {
      await page.goto(url);
      const width = await page.evaluate(() => ({
        scroll: document.scrollingElement?.scrollWidth ?? 0,
        view: window.innerWidth,
      }));
      expect(width.scroll).toBe(width.view);

      const wraps = await page.locator('.table-wrap').all();
      expect(wraps.length).toBe(2);
      for (const wrap of wraps) {
        await wrap.scrollIntoViewIfNeeded();
        expect(await wrap.evaluate((el) => el.scrollWidth - el.clientWidth)).toBeGreaterThan(0);
        await wrap.evaluate((el) => {
          el.scrollLeft = el.scrollWidth;
        });
        expect(await wrap.evaluate((el) => el.scrollLeft)).toBeGreaterThan(0);
        const box = await wrap.boundingBox();
        // The Pincel header and every Pincel cell stay inside the wrapper.
        const cells = await wrap.locator('.pincel').all();
        expect(cells.length).toBeGreaterThan(1);
        for (const cell of cells) {
          const c = await cell.boundingBox();
          if (!box || !c) throw new Error('table not laid out');
          expect(c.x).toBeGreaterThanOrEqual(box.x - 0.5);
          expect(c.x + c.width).toBeLessThanOrEqual(box.x + box.width + 0.5);
        }
        // Scrolled to the end, the last column is not hidden under Pincel.
        const pincel = await wrap.locator('thead .pincel').boundingBox();
        const last = await wrap.locator('thead th').last().boundingBox();
        if (!pincel || !last) throw new Error('table not laid out');
        expect(last.x).toBeGreaterThanOrEqual(pincel.x + pincel.width - 0.5);
      }
      // Scrolling a table never scrolls the page sideways.
      expect(await page.evaluate(() => window.scrollX)).toBe(0);
    });
  }
});
