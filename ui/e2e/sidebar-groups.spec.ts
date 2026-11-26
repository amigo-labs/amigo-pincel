import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, storedPref, switchLanguage } from './helpers';
import { storeLayout } from './effect-panel-helpers';

// docs/specs/sidebar-tab-groups.md — the sidebar's fixed tab groups in the
// browser build (no Generate, no Assistant): structure, choosing a tab,
// collapsing, storing, keyboard, heights, the narrow sheet.

const DOCUMENT = 'Layers, History';
const ASSETS = 'Preview, Tilesets, Slices';

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

async function box(locator: Locator): Promise<Box> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const tablist = (page: Page, name: string) => sidebar(page).getByRole('tablist', { name, exact: true });
const tabOf = (page: Page, group: string, name: string) =>
  tablist(page, group).getByRole('tab', { name, exact: true });
const collapse = (page: Page, group: string) =>
  sidebar(page).getByRole('button', { name: group, exact: true });
/** A panel's own element; it exists only while the panel is on the page. */
const panel = (page: Page, name: string) => sidebar(page).locator(`section[aria-label="${name}"]`);
/** The collapse button in a single panel's own header. */
const header = (page: Page, name: string) => panel(page, name).locator('> header button[aria-expanded]');
const tabNames = (page: Page, group: string) =>
  tablist(page, group)
    .getByRole('tab')
    .evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim()));

async function expectActive(page: Page, group: string, name: string): Promise<void> {
  await expect(tabOf(page, group, name)).toHaveAttribute('aria-selected', 'true');
  await expect(tablist(page, group).getByRole('tab', { selected: true })).toHaveCount(1);
}

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 800 } });

  test('pixel: Colors, Palette, Layers | History, Preview | Tilesets | Slices, in their first-use state (AC-01, AC-03, AC-07)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // Top to bottom.
    const tops = [];
    for (const block of [panel(page, 'Colors'), panel(page, 'Palette'), tablist(page, DOCUMENT), tablist(page, ASSETS)]) {
      tops.push((await box(block)).y);
    }
    expect([...tops].sort((a, b) => a - b)).toEqual(tops);
    expect(await tabNames(page, DOCUMENT)).toEqual(['Layers', 'History']);
    expect(await tabNames(page, ASSETS)).toEqual(['Preview', 'Tilesets', 'Slices']);
    await expect(sidebar(page).getByRole('tablist')).toHaveCount(2);

    // Layers | History: expanded, Layers active; History is not on the page.
    await expectActive(page, DOCUMENT, 'Layers');
    await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'true');
    await expect(panel(page, 'Layers')).toBeVisible();
    await expect(panel(page, 'History')).toHaveCount(0);

    // Preview | Tilesets | Slices: collapsed, Preview marked, no panel on the page.
    await expectActive(page, ASSETS, 'Preview');
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'false');
    for (const name of ['Preview', 'Tilesets', 'Slices']) await expect(panel(page, name)).toHaveCount(0);

    // Colors and Palette: single panels with their own header, expanded.
    for (const name of ['Colors', 'Palette']) {
      await expect(header(page, name)).toHaveAttribute(
        'aria-expanded',
        'true',
      );
    }
    // No panel of a group keeps a header of its own.
    await expect(sidebar(page).getByRole('button', { name: /^(Layers|History|Preview|Tilesets|Slices)( \d+)?$/ })).toHaveCount(0);
  });

  test('image: Colors and Layers | History (AC-02, AC-03, AC-07)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await expect(sidebar(page).getByRole('tablist')).toHaveCount(1);
    expect(await tabNames(page, DOCUMENT)).toEqual(['Layers', 'History']);
    const colors = await box(panel(page, 'Colors'));
    const tabs = await box(tablist(page, DOCUMENT));
    expect(colors.y).toBeLessThan(tabs.y);
    await expectActive(page, DOCUMENT, 'Layers');
    await expect(panel(page, 'Layers')).toBeVisible();
    await expect(panel(page, 'History')).toHaveCount(0);
  });

  test('choosing a tab shows its panel, expands a collapsed group; the collapse control keeps the tab row (AC-04, AC-05, AC-06)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // Another tab of an expanded group replaces the panel shown.
    await tabOf(page, DOCUMENT, 'History').click();
    await expectActive(page, DOCUMENT, 'History');
    await expect(panel(page, 'History')).toBeVisible();
    await expect(panel(page, 'Layers')).toHaveCount(0);
    // The active tab again: nothing changes.
    await tabOf(page, DOCUMENT, 'History').click();
    await expectActive(page, DOCUMENT, 'History');
    await expect(panel(page, 'History')).toBeVisible();
    await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'true');

    // Collapse: the tab row stays, the active tab stays marked, no panel.
    await collapse(page, DOCUMENT).click();
    await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'false');
    await expect(tablist(page, DOCUMENT)).toBeVisible();
    await expectActive(page, DOCUMENT, 'History');
    await expect(panel(page, 'History')).toHaveCount(0);
    await expect(panel(page, 'Layers')).toHaveCount(0);
    // … and expand again.
    await collapse(page, DOCUMENT).click();
    await expect(panel(page, 'History')).toBeVisible();

    // A tab of a collapsed group expands it on that panel.
    await collapse(page, DOCUMENT).click();
    await tabOf(page, DOCUMENT, 'Layers').click();
    await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'true');
    await expect(panel(page, 'Layers')).toBeVisible();
    await tabOf(page, ASSETS, 'Tilesets').click();
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'true');
    await expectActive(page, ASSETS, 'Tilesets');
    await expect(panel(page, 'Tilesets')).toBeVisible();
    await expect(panel(page, 'Preview')).toHaveCount(0);
  });

  test('active tabs and collapse states are kept per mode across a reload, documents and the drawer (AC-08)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await tabOf(page, DOCUMENT, 'History').click();
    await tabOf(page, ASSETS, 'Slices').click();
    await header(page, 'Palette').click();
    await expect
      .poll(() => storedPref(page, 'layout:pixel'))
      .toMatchObject({
        collapsed: { palette: true },
        groups: { document: { active: 'history', collapsed: false }, assets: { active: 'slices', collapsed: false } },
      });

    const same = async () => {
      await expectActive(page, DOCUMENT, 'History');
      await expect(panel(page, 'History')).toBeVisible();
      await expectActive(page, ASSETS, 'Slices');
      await expect(panel(page, 'Slices')).toBeVisible();
      await expect(header(page, 'Palette')).toHaveAttribute(
        'aria-expanded',
        'false',
      );
    };
    // A reload (createDocument loads the page) and so another document.
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    await same();
    // The drawer of a medium window shows the same state.
    await page.setViewportSize({ width: 800, height: 900 });
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    await same();
    // Image mode keeps its own state.
    await page.setViewportSize({ width: 1280, height: 800 });
    await createDocument(page, 'image', { width: 16, height: 16 });
    await expectActive(page, DOCUMENT, 'Layers');
  });

  test('a layout stored before the groups keeps the single panels and starts the groups fresh (AC-09)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await storeLayout(page, 'pixel', {
      collapsed: { palette: true, layers: true, history: false, preview: false },
      sidebar: true,
    });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(header(page, 'Palette')).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'true');
    await expectActive(page, DOCUMENT, 'Layers');
    await expect(panel(page, 'Layers')).toBeVisible();
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'false');
    await expectActive(page, ASSETS, 'Preview');
  });

  test('a stored value that is no layout, or names a tab the group lacks, starts as on first use without an error (AC-10)', async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await storeLayout(page, 'pixel', 'broken');
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expectActive(page, DOCUMENT, 'Layers');
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'false');

    await storeLayout(page, 'pixel', {
      collapsed: {},
      sidebar: true,
      groups: { document: { active: 'assistant', collapsed: false }, assets: { active: 'gone', collapsed: false } },
    });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expectActive(page, DOCUMENT, 'Layers');
    await expectActive(page, ASSETS, 'Preview');
    await expect(panel(page, 'Preview')).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test('the tab row is a tab list: roles, the tab panel named by the active tab, arrow keys without wrapping, Home and End (AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(sidebar(page).getByRole('tabpanel', { name: 'Layers', exact: true })).toBeVisible();
    // A collapsed group has no tab panel.
    await expect(sidebar(page).getByRole('tabpanel', { name: 'Preview', exact: true })).toHaveCount(0);

    await tabOf(page, DOCUMENT, 'Layers').focus();
    await page.keyboard.press('ArrowLeft');
    await expectActive(page, DOCUMENT, 'Layers');
    await expect(tabOf(page, DOCUMENT, 'Layers')).toBeFocused();
    await page.keyboard.press('ArrowRight');
    await expectActive(page, DOCUMENT, 'History');
    await expect(tabOf(page, DOCUMENT, 'History')).toBeFocused();
    await expect(sidebar(page).getByRole('tabpanel', { name: 'History', exact: true })).toBeVisible();
    await expect(panel(page, 'History')).toBeVisible();
    // No wrapping at the end.
    await page.keyboard.press('ArrowRight');
    await expectActive(page, DOCUMENT, 'History');
    await expect(tabOf(page, DOCUMENT, 'History')).toBeFocused();

    // Home / End; an arrow key on a collapsed group expands it.
    await tabOf(page, ASSETS, 'Preview').focus();
    await page.keyboard.press('End');
    await expectActive(page, ASSETS, 'Slices');
    await expect(tabOf(page, ASSETS, 'Slices')).toBeFocused();
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'true');
    await expect(panel(page, 'Slices')).toBeVisible();
    await page.keyboard.press('Home');
    await expectActive(page, ASSETS, 'Preview');
    await page.keyboard.press('ArrowRight');
    await expectActive(page, ASSETS, 'Tilesets');
    // Only the active tab is a tab stop.
    await expect(tabOf(page, ASSETS, 'Tilesets')).toHaveAttribute('tabindex', '0');
    await expect(tabOf(page, ASSETS, 'Preview')).toHaveAttribute('tabindex', '-1');
    // Space chooses a tab.
    await tabOf(page, ASSETS, 'Tilesets').focus();
    await collapse(page, ASSETS).click();
    await tabOf(page, ASSETS, 'Tilesets').focus();
    await page.keyboard.press('Space');
    await expect(collapse(page, ASSETS)).toHaveAttribute('aria-expanded', 'true');
  });

  test('Layers | History takes the height the other blocks leave; Preview | Tilesets | Slices that of its content (AC-13)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const side = await box(sidebar(page));
    const assetsRow = await box(tablist(page, ASSETS));
    const layers = await box(panel(page, 'Layers'));
    // The collapsed assets group is one header row at the sidebar's foot,
    // the layers panel fills what is above it.
    expect(assetsRow.y + assetsRow.height).toBeGreaterThan(side.y + side.height - 40);
    expect(layers.y + layers.height).toBeGreaterThan(assetsRow.y - 12);
    expect(layers.height).toBeGreaterThan(150);

    // Collapsed, Layers | History takes only its header.
    await collapse(page, DOCUMENT).click();
    const row = await box(tablist(page, DOCUMENT));
    const below = await box(tablist(page, ASSETS));
    expect(below.y - row.y).toBeLessThan(40);

    // Expanded, the assets group is as high as its panel's content, not the rest.
    await tabOf(page, ASSETS, 'Slices').click();
    const slices = await box(panel(page, 'Slices'));
    expect(slices.height).toBeLessThan(side.height / 2);
    // The sidebar does not overflow sideways or need to scroll here.
    expect(await sidebar(page).evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  });

  for (const mode of ['pixel', 'image'] as const) {
    test(`${mode}: Enter on a tab chooses it and expands a collapsed group (AC-04, AC-05)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await tabOf(page, DOCUMENT, 'Layers').focus();
      await page.keyboard.press('ArrowRight');
      await expectActive(page, DOCUMENT, 'History');
      await page.keyboard.press('ArrowLeft');
      await expectActive(page, DOCUMENT, 'Layers');
      // Enter on the active tab of a collapsed group expands it.
      await collapse(page, DOCUMENT).click();
      await expect(panel(page, 'Layers')).toHaveCount(0);
      await tabOf(page, DOCUMENT, 'Layers').focus();
      await page.keyboard.press('Enter');
      await expect(collapse(page, DOCUMENT)).toHaveAttribute('aria-expanded', 'true');
      await expect(panel(page, 'Layers')).toBeVisible();
      // Enter on a focused tab that is not the active one chooses it.
      await tabOf(page, DOCUMENT, 'History').focus();
      await page.keyboard.press('Enter');
      await expectActive(page, DOCUMENT, 'History');
      await expect(panel(page, 'History')).toBeVisible();
    });
  }

  test('while an effect is open the tab rows and collapse controls take no input (AC-19)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    const inert = (selector: string) =>
      page.evaluate((s) => {
        const el = document.querySelector(`[data-editor]:not([inert]) aside ${s}`);
        return el !== null && el.closest('[inert]') !== null;
      }, selector);
    expect(await inert('[role="tablist"]')).toBe(true);
    expect(await inert('[role="tab"]')).toBe(true);
    expect(await inert('button[aria-expanded]')).toBe(true);
    expect(await inert('section[aria-label="Layers"]')).toBe(true);
  });
});

for (const language of ['English', 'Deutsch'] as const) {
  test.describe(`240 px sidebar, ${language}`, () => {
    test.use({ viewport: { width: 1280, height: 800 } });

    test('no sideways scroll; every tab can be brought into view and chosen (AC-14)', async ({ page }) => {
      await createDocument(page, 'pixel', { width: 16, height: 16 });
      await storeLayout(page, 'pixel', { collapsed: {}, sidebar: true, sidebarWidth: 240 });
      await createDocument(page, 'pixel', { width: 16, height: 16 });
      if (language === 'Deutsch') await switchLanguage(page, 'Deutsch');
      const side = page.locator('[data-editor]:not([inert]) aside');
      expect(Math.round((await box(side)).width)).toBe(240);
      const noSidewaysScroll = () =>
        side.evaluate((el) => {
          const scroller = el.firstElementChild as HTMLElement;
          return scroller.scrollWidth <= scroller.clientWidth && el.scrollWidth <= el.clientWidth;
        });
      expect(await noSidewaysScroll()).toBe(true);
      const tabs = side.getByRole('tab');
      await expect(tabs).toHaveCount(5);
      for (let i = 0; i < 5; i += 1) {
        const tab = tabs.nth(i);
        await tab.scrollIntoViewIfNeeded();
        await expect(tab).toBeInViewport({ ratio: 1 });
        const b = await box(tab);
        const s = await box(side);
        expect(b.x).toBeGreaterThanOrEqual(s.x);
        expect(b.x + b.width).toBeLessThanOrEqual(s.x + s.width + 0.5);
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
        expect(await noSidewaysScroll()).toBe(true);
      }
    });
  });
}

test.describe('short window', () => {
  test.use({ viewport: { width: 1280, height: 420 } });

  test('blocks that need more height than the window make the sidebar scroll vertically (AC-13)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await tabOf(page, ASSETS, 'Slices').click();
    await expect(panel(page, 'Slices')).toBeVisible();
    const scroller = sidebar(page).locator('> div').first();
    const sizes = await scroller.evaluate((el) => ({
      scroll: el.scrollHeight,
      client: el.clientHeight,
      overflow: getComputedStyle(el).overflowY,
      sideways: el.scrollWidth > el.clientWidth,
    }));
    expect(sizes.scroll).toBeGreaterThan(sizes.client);
    expect(sizes.overflow).toBe('auto');
    expect(sizes.sideways).toBe(false);
    // No block lies over another: top to bottom, each starts where the last ended.
    const blocks = [
      panel(page, 'Colors'),
      panel(page, 'Palette'),
      sidebar(page).locator('[data-panel-group="document"]'),
      sidebar(page).locator('[data-panel-group="assets"]'),
    ];
    let bottom = -Infinity;
    for (const block of blocks) {
      const b = await box(block);
      expect(b.y).toBeGreaterThanOrEqual(bottom - 0.5);
      bottom = b.y + b.height;
    }
    // The last block can be scrolled into view.
    await panel(page, 'Slices').scrollIntoViewIfNeeded();
    await expect(panel(page, 'Slices')).toBeInViewport();
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('the bottom sheet shows one tab per panel, as before (AC-15)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Panels', exact: true }).click();
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    const names = await sheet
      .getByRole('tab')
      .evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim()));
    expect([...names].sort()).toEqual(
      ['Colors', 'History', 'Layers', 'Palette', 'Preview', 'Slices', 'Tilesets', 'Timeline'].sort(),
    );
    await expect(sheet.getByRole('tablist')).toHaveCount(1);
    // Each tab shows its panel with its content.
    await sheet.getByRole('tab', { name: 'History' }).click();
    await expect(sheet.locator('section[aria-label="History"]')).toBeVisible();
    await sheet.getByRole('tab', { name: 'Layers' }).click();
    await expect(sheet.getByRole('button', { name: 'Add layer' })).toBeVisible();
  });
});
