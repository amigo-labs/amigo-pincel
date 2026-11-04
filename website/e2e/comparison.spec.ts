import { expect, test, type Locator } from '@playwright/test';

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
    ['Open source', '✅ MIT/Apache', '✅ EULA-restricted', '✅ Apache 2.0'],
    ['Active development', '✅', '✅', '🟡 modernization in progress'],
  ];
  await expect(rows).toHaveCount(expected.length);
  const actual = [];
  for (let i = 0; i < expected.length; i++) actual.push(await cellTexts(rows.nth(i)));
  expect(actual).toEqual(expected);
  await expect(page.locator('.footnote')).toContainText('Aseprite has 14 years of features');
});
