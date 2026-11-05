import { expect, test, type Locator, type Page } from '@playwright/test';

// The Home comparison table (docs/specs/website.md AC-09).

/** Each cell's text with its whitespace collapsed (`✅ native`). */
async function cellTexts(row: Locator): Promise<string[]> {
  return row
    .locator('th, td')
    .evaluateAll((cells) => cells.map((c) => (c.textContent ?? '').replace(/\s+/g, ' ').trim()));
}

test('AC-09: the home page compares Pincel, Aseprite and Piskel in 14 rows', async ({ page }) => {
  await page.goto('/');
  const table = page.getByRole('region', { name: 'Feature comparison table' }).getByRole('table');
  expect(await cellTexts(table.locator('thead tr'))).toEqual([
    'Feature',
    'Pincel',
    'Aseprite',
    'Piskel',
  ]);
  const rows = table.locator('tbody tr');
  const expected = [
    ['Aseprite file format (read+write)', '✅', '✅ native', '❌'],
    ['Tilemaps', '✅', '✅', '❌'],
    ['Slices (9-patch, pivot)', '✅', '✅', '❌'],
    ['Animation timeline + tags', '✅', '✅', '✅ basic'],
    ['Tablet / pen support', '✅', '✅', '❌'],
    ['Runs in browser', '✅', '❌', '✅'],
    ['Layered image editing (photos, screenshots)', '✅ Image mode', '❌ pixel art only', '❌'],
    ['Native desktop app', '✅ Tauri', '✅', '✅ via NW.js, ageing'],
    ['Embeddable as a library', '✅', '❌', '❌'],
    ['Lua scripting / extensions', '❌ Phase 2', '✅', '❌'],
    ['Custom brushes', '❌ Phase 2', '✅', '✅'],
    ['Price', 'Free', 'Paid', 'Free'],
    [
      'Open source',
      '✅ MIT/Apache',
      '🟡 source-available, EULA forbids redistribution',
      '✅ Apache 2.0',
    ],
    [
      'Active development',
      '✅',
      '✅',
      '🟡 no tagged release since 0.15.0 (2018); master still gets fixes',
    ],
  ];
  await expect(rows).toHaveCount(expected.length);
  const actual = [];
  for (let i = 0; i < expected.length; i++) actual.push(await cellTexts(rows.nth(i)));
  expect(actual).toEqual(expected);
  await expect(page.locator('.footnote')).toContainText('Aseprite has 14 years of features');
});

/** The text of the cell in row `feature` under column header `column`, on the English home page. */
async function cellText(page: Page, feature: string, column: string): Promise<string> {
  const table = page.getByRole('region', { name: 'Feature comparison table' }).getByRole('table');
  const headers = await cellTexts(table.locator('thead tr'));
  const index = headers.indexOf(column);
  expect(index, column).toBeGreaterThan(0);
  const row = table.locator('tbody tr').filter({
    has: page.getByRole('rowheader', { name: feature, exact: true }),
  });
  await expect(row).toHaveCount(1);
  return (await cellTexts(row))[index] ?? '';
}

// docs/specs/website-comparison.md
test('website-comparison AC-06: Aseprite is source-available, not open source', async ({
  page,
}) => {
  await page.goto('/');
  const cell = await cellText(page, 'Open source', 'Aseprite');
  expect(cell).toContain('source-available');
  expect(cell).not.toContain('✅');
  expect(cell).not.toMatch(/open source/i);
});

test('website-comparison AC-07: Piskel has had no tagged release since 0.15.0', async ({
  page,
}) => {
  await page.goto('/');
  const cell = await cellText(page, 'Active development', 'Piskel');
  expect(cell).toContain('no tagged release since 0.15.0 (2018)');
  expect(cell).toContain('master still gets fixes');
  expect(cell).not.toContain('modernization in progress');
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
      expect(wraps.length).toBeGreaterThan(0);
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
