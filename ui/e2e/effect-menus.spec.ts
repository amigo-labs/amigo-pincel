import { expect, test, type Page } from '@playwright/test';
import { EFFECT_SUBMENUS, EFFECTS, SUBMENU_LABELS, type EffectDef } from '../src/lib/effects/catalog';
import { en } from '../src/lib/i18n/en';
import { createDocument, gotoStart, waitForEditor } from './helpers';

// effect-catalog-v2 AC-01 – AC-06: the Effects menu follows paint.net's
// submenus, each mode shows only its entries, entries sort by their name in
// the UI language.

/** Opens a top-level menu of the menu bar. */
async function openTop(page: Page, top: string): Promise<void> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
}

/** The row labels of the open menu named `name`, in display order. */
async function entries(page: Page, name: string): Promise<string[]> {
  const menu = page.getByRole('menu', { name, exact: true });
  await expect(menu).toBeVisible();
  const rows = await menu.locator('[role="menuitem"] > span.flex-1').allTextContents();
  return rows.map((r) => r.trim());
}

/** Every submenu of the Effects menu with its entries. */
async function effectsTree(page: Page, top = 'Effects'): Promise<Record<string, string[]>> {
  await openTop(page, top);
  const subs = await entries(page, top);
  const tree: Record<string, string[]> = {};
  for (const sub of subs) {
    await page
      .getByRole('menu', { name: top, exact: true })
      .getByRole('menuitem', { name: sub, exact: true })
      .click();
    tree[sub] = await entries(page, sub);
  }
  await page.keyboard.press('Escape');
  return tree;
}

async function adjustments(page: Page, top = 'Adjustments'): Promise<string[]> {
  await openTop(page, top);
  const list = await entries(page, top);
  await page.keyboard.press('Escape');
  return list;
}

/** `list` is sorted by `locale`'s collation. */
function expectSorted(list: readonly string[], locale: string): void {
  expect([...list].sort(new Intl.Collator(locale).compare)).toEqual(list);
}

/** `actual` contains every name of `expected`. */
function expectContains(actual: readonly string[], expected: readonly string[]): void {
  for (const name of expected) expect(actual).toContain(name);
}

/** The English menu row of a catalog entry. */
function row(def: EffectDef): string {
  const name = (en as Record<string, string>)[`effect.${def.id}`] ?? def.label;
  return `${name}${def.params.length > 0 ? '…' : ''}`;
}

const SUBMENU_ORDER = EFFECT_SUBMENUS.map((g) => SUBMENU_LABELS[g]);

const ADJUSTMENTS_EN = [
  'Brightness / Contrast…',
  'Color Balance…',
  'Curves…',
  // Renamed from Grayscale by adjustments-paintnet.
  'Black and White…',
  'Hue / Saturation…',
  'Invert Colors',
  'Levels…',
  'Posterize…',
  'Threshold…',
];

test('image mode: paint.net submenus with the Image-mode entries, sorted', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  expect(SUBMENU_ORDER).toEqual([
    'Artistic',
    'Blurs',
    'Color',
    'Distort',
    'Noise',
    'Object',
    'Photo',
    'Render',
    'Stylize',
  ]);
  const tree = await effectsTree(page);
  // Submenus in paint.net's order, each with at least one entry; empty ones
  // are left out.
  const subs = Object.keys(tree);
  expect(subs).toEqual(SUBMENU_ORDER.filter((s) => subs.includes(s)));
  for (const list of Object.values(tree)) expect(list.length).toBeGreaterThan(0);
  expectContains(tree.Blurs ?? [], ['Gaussian Blur…', 'Motion Blur…', 'Radial Blur…', 'Square Blur…']);
  expectContains(tree.Noise ?? [], ['Add Noise…', 'Reduce Noise…']);
  expectContains(tree.Photo ?? [], ['Sharpen', 'Unsharp Mask…']);
  expectContains(tree.Stylize ?? [], ['Edge Detect…', 'Emboss…', 'Relief…']);
  for (const list of Object.values(tree)) expectSorted(list, 'en');
  const adjust = await adjustments(page);
  expectContains(adjust, ADJUSTMENTS_EN);
  expectSorted(adjust, 'en');
});

test('pixel mode: only per-pixel effects, no neighbourhood effect', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const tree = await effectsTree(page);
  // Exactly the catalog entries whose modes include Pixel mode.
  const pixel = EFFECTS.filter((e) => e.modes.includes('pixel'));
  const expected: Record<string, string[]> = {};
  for (const g of EFFECT_SUBMENUS) {
    const rows = pixel.filter((e) => e.group === g).map(row);
    if (rows.length > 0) expected[SUBMENU_LABELS[g]] = rows.sort(new Intl.Collator('en').compare);
  }
  expect(tree).toEqual(expected);
  expectContains(tree.Noise ?? [], ['Add Noise…']);
  const adjust = await adjustments(page);
  expect(adjust).toEqual(
    pixel
      .filter((e) => e.group === 'adjust')
      .map(row)
      .sort(new Intl.Collator('en').compare),
  );
  expectContains(adjust, ADJUSTMENTS_EN);
  const hidden = /Blur|Sharpen|Unsharp|Edge Detect|Emboss|Relief|Reduce Noise/;
  for (const name of [...Object.values(tree).flat(), ...adjust]) expect(name).not.toMatch(hidden);
});

test('switching the UI language re-sorts and renames the entries', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const before = await adjustments(page);
  expect(before.indexOf('Brightness / Contrast…')).toBeLessThan(before.indexOf('Color Balance…'));
  await openTop(page, 'View');
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();

  const adjust = await adjustments(page, 'Korrekturen');
  expectSorted(adjust, 'de');
  expect(adjust.indexOf('Farbbalance…')).toBeGreaterThanOrEqual(0);
  expect(adjust.indexOf('Farbbalance…')).toBeLessThan(adjust.indexOf('Helligkeit / Kontrast…'));
  const tree = await effectsTree(page, 'Effekte');
  // German submenu names (AC-06), still in paint.net's order.
  const subs = Object.keys(tree);
  const order = ['Künstlerisch', 'Weichzeichnen', 'Farbe', 'Verzerren', 'Rauschen', 'Objekt', 'Foto', 'Rendern', 'Stilisieren'];
  expect(subs).toEqual(order.filter((s) => subs.includes(s)));
  expectContains(subs, ['Weichzeichnen', 'Rauschen', 'Foto', 'Stilisieren']);
  expectContains(tree.Weichzeichnen ?? [], [
    'Bewegungsunschärfe…',
    'Gaußscher Weichzeichner…',
    'Quadratisches Weichzeichnen…',
    'Radialer Weichzeichner…',
  ]);
  for (const list of Object.values(tree)) expectSorted(list, 'de');
});

test('every effect entry is disabled until the document is loaded', async ({ page }) => {
  for (const mode of ['pixel', 'image'] as const) {
    // Hold the core's wasm: no document arrives until the request is
    // released.
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    await page.route('**/*.wasm', async (route) => {
      await held;
      await route.continue();
    });
    await gotoStart(page);
    const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
    await page.getByRole('button', { name }).click();
    await page.getByRole('form', { name }).getByRole('button', { name: 'Create' }).click();
    // Since document-tabs the editor stays hidden and inert behind the
    // loading screen until its document is open: no menu, so no effect
    // entry, can be reached before.
    const label = mode === 'pixel' ? 'Pixel' : 'Image';
    await expect(page.getByText(`Loading the ${label} editor…`)).toBeVisible();
    await expect(page.getByRole('menubar')).toHaveCount(0);
    release();
    await waitForEditor(page, mode);
    for (const top of ['Effects', 'Adjustments']) {
      await openTop(page, top);
      const rows = page.getByRole('menu', { name: top, exact: true }).getByRole('menuitem');
      await expect(rows.first()).toBeEnabled();
      await page.keyboard.press('Escape');
    }
    await page.unroute('**/*.wasm');
  }
});
