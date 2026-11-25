import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, newDocument, switchLanguage } from './helpers';
import { tab, tabBar } from './tab-helpers';
import { installTauriMock, launchDesktop } from './tauri-mock';

// docs/specs/frame-layout.md — the order and content of the editor frame's
// bars: app bar, tab bar, options bar, status bar.

const MAC_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

const WIDE = { width: 1280, height: 800 };
const MEDIUM = { width: 800, height: 900 };
const NARROW = { width: 390, height: 844 };

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

function region(page: Page, name: 'appbar' | 'tabbar' | 'options' | 'status'): Locator {
  return page.locator(`[data-frame="${name}"]`);
}

const appBar = (page: Page) => region(page, 'appbar');
const undo = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redo = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });
const search = (page: Page) => page.getByRole('button', { name: 'Search commands' });
const newButton = (page: Page) => page.getByRole('button', { name: 'New document', exact: true });
const panelsToggle = (page: Page) =>
  page.getByRole('button', { name: /^(Toggle panels|Panels)$/ });
const palette = (page: Page) => page.getByRole('dialog', { name: 'Command palette' });

/** Each locator's left edge lies right of the one before it. */
async function expectLeftToRight(...items: Locator[]): Promise<void> {
  let last = -1;
  for (const item of items) {
    const b = await box(item);
    expect(b.x, `x of item ${items.indexOf(item)}`).toBeGreaterThan(last);
    last = b.x;
  }
}

/** The locator lies wholly inside the window. */
async function expectInWindow(page: Page, item: Locator): Promise<void> {
  const b = await box(item);
  const width = page.viewportSize()?.width ?? 0;
  expect(b.x).toBeGreaterThanOrEqual(0);
  expect(b.x + b.width).toBeLessThanOrEqual(width + 0.5);
}

/** Opens `count` further documents through the New button. */
async function addTabs(page: Page, count: number): Promise<void> {
  for (let i = 0; i < count; i += 1) {
    await newButton(page).click();
    const dialog = page.getByRole('dialog', { name: 'New document' });
    await dialog.getByRole('button', { name: 'Create' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(tabBar(page).getByRole('tab')).toHaveCount(i + 2);
  }
}

for (const mode of ['pixel', 'image'] as const) {
  test.describe(`${mode}, wide`, () => {
    test.use({ viewport: WIDE });

    test('app bar, tab bar, middle row and status bar stack in that order; the bars span the window (AC-01, AC-16)', async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 32, height: 32 });
      const bar = await box(appBar(page));
      const tabsBox = await box(region(page, 'tabbar'));
      const options = await box(region(page, 'options'));
      const status = await box(region(page, 'status'));
      const rail = await box(page.getByRole('toolbar', { name: 'Tools' }));
      const stage = await box(page.locator('[data-stage]'));

      expect(bar.y).toBe(0);
      expect(tabsBox.y).toBeCloseTo(bar.y + bar.height, 0);
      expect(options.y).toBeCloseTo(tabsBox.y + tabsBox.height, 0);
      expect(stage.y).toBeCloseTo(options.y + options.height, 0);
      expect(status.y + status.height).toBeCloseTo(WIDE.height, 0);
      expect(stage.y + stage.height).toBeLessThanOrEqual(status.y + 0.5);

      for (const full of [bar, tabsBox, status]) {
        expect(full.x).toBe(0);
        expect(full.width).toBeCloseTo(WIDE.width, 0);
      }
      // Middle row: rail, centre column, sidebar.
      const sidebar = await box(page.getByRole('complementary', { name: 'Panels' }));
      expect(rail.x).toBe(0);
      expect(rail.y).toBeCloseTo(tabsBox.y + tabsBox.height, 0);
      expect(options.x).toBeCloseTo(rail.x + rail.width, 0);
      expect(sidebar.x).toBeGreaterThan(options.x + options.width - 1);
      expect(sidebar.y).toBeCloseTo(tabsBox.y + tabsBox.height, 0);
      expect(sidebar.y + sidebar.height).toBeCloseTo(status.y, 0);

      expect(Math.abs(bar.height - 40)).toBeLessThanOrEqual(1);
      expect(Math.abs(tabsBox.height - 36)).toBeLessThanOrEqual(1);
      expect(Math.abs(options.height - 40)).toBeLessThanOrEqual(1);
      expect(Math.abs(status.height - 28)).toBeLessThanOrEqual(1);
    });

    test('the app bar holds wordmark, menus, Undo, Redo, search and panels toggle; no name, marker or mode badge (AC-02, AC-03)', async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 32, height: 32 });
      const name = mode === 'pixel' ? 'pincel.aseprite' : 'image.ora';
      const bar = appBar(page);
      await expectLeftToRight(
        bar.getByText('Pincel', { exact: true }),
        bar.getByRole('menubar'),
        undo(page),
        redo(page),
        search(page),
        panelsToggle(page),
      );
      await expect(search(page)).toContainText('Search commands');
      await expect(search(page).locator('kbd')).toHaveText(/K/);

      await expect(bar.getByText(name)).toHaveCount(0);
      // No mode badge: "Image" is only the Image menu's title.
      await expect(bar.getByText('Pixel', { exact: true })).toHaveCount(0);
      await expect(bar.getByText('Image', { exact: true })).toHaveCount(1);
      await expect(bar.getByRole('menuitem', { name: 'Image', exact: true })).toHaveCount(1);
      await expect(bar.getByTitle(/unsaved changes/)).toHaveCount(0);

      // The tab carries the name, the mode and the tooltip.
      await expect(tab(page, name)).toHaveAttribute('title', name);
      await expect(tab(page, name)).toHaveAccessibleName(new RegExp(mode === 'pixel' ? 'Pixel' : 'Image'));
    });
  });
}

test.describe('wide', () => {
  test.use({ viewport: WIDE });

  test('an unsaved document shows in its tab tooltip (AC-03)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const name = 'pincel.aseprite';
    await page.getByTitle('Pencil (B)').click();
    const stage = await box(page.locator('[data-stage] canvas').first());
    await page.mouse.click(stage.x + stage.width / 2, stage.y + stage.height / 2);
    await expect(tab(page, name)).toHaveAttribute('title', `${name} — unsaved changes`);
    await expect(appBar(page).getByTitle(/unsaved changes/)).toHaveCount(0);
  });

  test('the search control opens the command palette (AC-04, AC-19)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await search(page).click();
    await expect(palette(page)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(palette(page)).toHaveCount(0);
    // Keyboard: focus and press.
    await search(page).focus();
    await page.keyboard.press('Space');
    await expect(palette(page)).toBeVisible();
    await page.keyboard.press('Escape');
    await search(page).focus();
    await page.keyboard.press('Enter');
    await expect(palette(page)).toBeVisible();
  });

  test('the New button opens the New-document dialog and names its shortcut (AC-05, AC-19)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const button = newButton(page);
    await expect(button).toHaveAttribute('title', /^New document/);
    // It follows the last tab while the tabs fit.
    const last = await box(tabBar(page).getByRole('tab').last());
    const b = await box(button);
    expect(b.x).toBeGreaterThan(last.x + last.width - 1);
    expect(b.x).toBeLessThan(last.x + last.width + 80);
    await button.click();
    await expect(page.getByRole('dialog', { name: 'New document' })).toBeVisible();
    await page.keyboard.press('Escape');
    await button.focus();
    await page.keyboard.press('Space');
    await expect(page.getByRole('dialog', { name: 'New document' })).toBeVisible();
  });

  test('the New button stays visible when the tabs overflow (AC-05)', async ({ page }) => {
    test.setTimeout(120_000);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await addTabs(page, 14);
    const bar = tabBar(page);
    expect(await bar.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
    await expectInWindow(page, newButton(page));
    await expect(newButton(page)).toBeInViewport({ ratio: 1 });
    const tabsBox = await box(bar);
    const b = await box(newButton(page));
    expect(b.x).toBeGreaterThanOrEqual(tabsBox.x + tabsBox.width - 1);
  });

  test('Toggle UI keeps app bar, tab bar and status bar (AC-13)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.locator('[data-stage]').click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('Tab');
    await expect(page.getByRole('toolbar', { name: 'Tools' })).toHaveCount(0);
    await expect(region(page, 'options')).toHaveCount(0);
    await expect(appBar(page)).toBeVisible();
    await expect(region(page, 'tabbar')).toBeVisible();
    await expect(region(page, 'status')).toBeVisible();
  });

  test('German names the new controls (AC-20)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    await expect(page.getByRole('button', { name: 'Befehle suchen' })).toContainText('Befehle suchen');
    await expect(page.getByRole('button', { name: 'Neues Dokument', exact: true })).toBeVisible();
  });
});

test.describe('medium', () => {
  test.use({ viewport: MEDIUM });

  test('no wordmark; the search control is an icon button with its shortcut in the tooltip (AC-02, AC-04)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    const bar = appBar(page);
    await expect(bar.getByText('Pincel', { exact: true })).toHaveCount(0);
    await expectLeftToRight(bar.getByRole('menubar'), undo(page), redo(page), search(page), panelsToggle(page));
    await expect(search(page)).toHaveText('');
    await expect(search(page)).toHaveAttribute('title', /^Search commands \(.*K\)$/);
    await search(page).click();
    await expect(palette(page)).toBeVisible();
  });
});

const LANGUAGES = ['English', 'Deutsch', 'Español', 'Français', '日本語', 'Português (Brasil)'];

test.describe('640 px', () => {
  test.use({ viewport: { width: 640, height: 800 } });

  for (const language of LANGUAGES) {
    test(`${language}: Undo, Redo, search and panels toggle stay visible; the last menu opens (AC-18)`, async ({
      page,
    }) => {
      await createDocument(page, 'pixel', { width: 16, height: 16 });
      if (language !== 'English') await switchLanguage(page, language);
      const bar = appBar(page);
      const buttons = bar.locator('[data-tour="quick"] button');
      await expect(buttons).toHaveCount(2);
      for (const item of [buttons.nth(0), buttons.nth(1), bar.locator('[data-search]'), bar.locator('[data-panels-toggle]')]) {
        await expectInWindow(page, item);
        await expect(item).toBeInViewport({ ratio: 1 });
      }
      // Every menu can be opened by keyboard: open the first, step left
      // onto the last, which scrolls into view.
      const menus = bar.getByRole('menubar').getByRole('menuitem');
      const last = menus.last();
      await menus.first().focus();
      await page.keyboard.press('Space');
      await expect(menus.first()).toHaveAttribute('aria-expanded', 'true');
      await page.keyboard.press('ArrowLeft');
      await expect(last).toHaveAttribute('aria-expanded', 'true');
      await expect(last).toBeInViewport({ ratio: 0.95 });
      await expect(page.getByRole('menu').first()).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('menu')).toHaveCount(0);
      // … and with the mouse, after scrolling the bar back.
      await menus.first().scrollIntoViewIfNeeded();
      await last.scrollIntoViewIfNeeded();
      await last.click();
      await expect(last).toHaveAttribute('aria-expanded', 'true');
      await expect(page.getByRole('menu').first()).toBeVisible();
    });
  }
});

test.describe('narrow', () => {
  test.use({ viewport: NARROW });

  test('app bar, tab bar, options bar, canvas, tool strip; no status bar (AC-15, AC-16)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const bar = await box(appBar(page));
    const tabsBox = await box(region(page, 'tabbar'));
    const options = await box(region(page, 'options'));
    const stage = await box(page.locator('[data-stage]'));
    const strip = await box(page.getByRole('toolbar', { name: 'Tools' }));
    expect(bar.y).toBe(0);
    expect(tabsBox.y).toBeCloseTo(bar.y + bar.height, 0);
    expect(options.y).toBeCloseTo(tabsBox.y + tabsBox.height, 0);
    expect(stage.y).toBeCloseTo(options.y + options.height, 0);
    expect(strip.y).toBeCloseTo(stage.y + stage.height, 0);
    await expect(region(page, 'status')).toHaveCount(0);
    expect(Math.abs(bar.height - 44)).toBeLessThanOrEqual(1);
    expect(Math.abs(options.height - 44)).toBeLessThanOrEqual(1);
    expect(Math.abs(tabsBox.height - 36)).toBeLessThanOrEqual(1);

    await expectLeftToRight(
      page.getByRole('button', { name: 'Menu', exact: true }),
      undo(page),
      redo(page),
      search(page),
      panelsToggle(page),
    );
    await expect(search(page)).toHaveText('');
  });

  test('the New button stays visible when the tabs overflow (AC-05)', async ({ page }) => {
    test.setTimeout(90_000);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await addTabs(page, 4);
    expect(await tabBar(page).evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
    await expect(newButton(page)).toBeInViewport({ ratio: 1 });
    await expectInWindow(page, newButton(page));
  });
});

test.describe('macOS desktop app', () => {
  test.use({ userAgent: MAC_UA, viewport: { width: 1024, height: 800 } });

  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await launchDesktop(page);
  });

  test('one top row holds the tabs and the app bar controls (AC-14, AC-16)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await expect(page.getByRole('menubar')).toHaveCount(0);
    await expect(region(page, 'tabbar')).toHaveCount(0);
    const bar = appBar(page);
    const barBox = await box(bar);
    expect(Math.abs(barBox.height - 40)).toBeLessThanOrEqual(1);
    await expect(bar.getByRole('tablist', { name: 'Open documents' })).toBeVisible();
    await expectLeftToRight(
      bar.getByText('Pincel', { exact: true }),
      tab(page, 'pincel.aseprite'),
      newButton(page),
      undo(page),
      redo(page),
      search(page),
      panelsToggle(page),
    );
    // The regions below are those of the other platforms.
    const options = await box(region(page, 'options'));
    expect(options.y).toBeCloseTo(barBox.height, 0);
    await expect(region(page, 'status')).toBeVisible();
  });

  test('many tabs scroll inside the top row; the controls stay visible (AC-14)', async ({ page }) => {
    test.setTimeout(120_000);
    await newDocument(page, 'image', { width: 16, height: 16 });
    await addTabs(page, 11);
    expect(await tabBar(page).evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
    for (const item of [newButton(page), undo(page), redo(page), search(page), panelsToggle(page)]) {
      await expect(item).toBeInViewport({ ratio: 1 });
      await expectInWindow(page, item);
    }
  });

  test('narrow: app bar without ☰, then the tab bar (AC-14)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await page.setViewportSize({ width: 520, height: 800 });
    await expect(page.getByRole('button', { name: 'Menu', exact: true })).toHaveCount(0);
    const bar = await box(appBar(page));
    const tabsBox = await box(region(page, 'tabbar'));
    expect(tabsBox.y).toBeCloseTo(bar.y + bar.height, 0);
    await expectLeftToRight(undo(page), redo(page), search(page), panelsToggle(page));
  });
});
