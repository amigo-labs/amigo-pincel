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
