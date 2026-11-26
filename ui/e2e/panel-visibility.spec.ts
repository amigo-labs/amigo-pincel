import { expect, test, type Locator, type Page } from '@playwright/test';
import { colorOf, createDocument, menu, setColor, storedPref, switchLanguage } from './helpers';
import { storeLayout } from './effect-panel-helpers';

// docs/specs/panel-visibility.md — View ▸ Panels in the browser build (no
// Generate, no Assistant): the entries and their check marks, removing and
// enabling a panel, what a group with one or no panel looks like, storing,
// Reset Panels, the Timeline, the narrow sheet.

const PIXEL = ['Colors', 'Palette', 'Layers', 'History', 'Preview', 'Tilesets', 'Slices', 'Timeline'];
const IMAGE = ['Colors', 'Layers', 'History'];

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
/** A panel's own element; it exists only while the panel is on the page. */
const panel = (page: Page, name: string) => sidebar(page).locator(`section[aria-label="${name}"]`);
/** The collapse button in a panel's own header. */
const header = (page: Page, name: string) => panel(page, name).locator('> header button[aria-expanded]');
const toggle = (page: Page) => page.locator('[data-panels-toggle] button');
const timeline = (page: Page) => page.getByRole('region', { name: 'Timeline' });
const stage = (page: Page) => page.locator('[data-editor]:not([inert]) [data-stage]');
const frameItem = (page: Page) => page.locator('[data-frame="status"] [data-status-frame]');

/** Opens View ▸ Panels and returns the submenu. */
async function openPanels(page: Page, view = 'View', panels = 'Panels'): Promise<Locator> {
  await page.getByRole('menubar').getByRole('menuitem', { name: view, exact: true }).click();
  await page.getByRole('menu', { name: view, exact: true }).getByRole('menuitem', { name: panels, exact: true }).click();
  const sub = page.getByRole('menu', { name: panels, exact: true });
  await expect(sub).toBeVisible();
  return sub;
}

async function closeMenus(page: Page): Promise<void> {
  for (let i = 0; i < 3 && (await page.getByRole('menu').count()) > 0; i += 1) {
    await page.keyboard.press('Escape');
  }
  await expect(page.getByRole('menu')).toHaveCount(0);
}

/** Chooses the entry `name` of View ▸ Panels. */
async function choose(page: Page, name: string, view = 'View', panels = 'Panels'): Promise<void> {
  const sub = await openPanels(page, view, panels);
  await sub.getByRole('menuitemcheckbox', { name, exact: true }).click();
  await expect(page.getByRole('menu')).toHaveCount(0);
}

async function reset(page: Page): Promise<void> {
  const sub = await openPanels(page);
  await sub.getByRole('menuitem', { name: 'Reset Panels', exact: true }).click();
  await expect(page.getByRole('menu')).toHaveCount(0);
}

/** The rows of a menu top to bottom: labels, `—` for a separator. */
const rowsOf = (menuEl: Locator) =>
  menuEl.evaluate((el) =>
    [...el.children].map((child) =>
      child.tagName === 'HR' ? '—' : (child.querySelector('.am-menu-label')?.textContent ?? '').trim(),
    ),
  );

/** The entries of View ▸ Panels with their check state; closes the menu. */
async function entries(page: Page, view = 'View', panels = 'Panels'): Promise<[string, boolean][]> {
  const sub = await openPanels(page, view, panels);
  const out = await sub
    .getByRole('menuitemcheckbox')
    .evaluateAll((els) =>
      els.map(
        (el) =>
          [
            (el.querySelector('.am-menu-label')?.textContent ?? '').trim(),
            el.getAttribute('aria-checked') === 'true',
          ] as [string, boolean],
      ),
    );
  await closeMenus(page);
  return out;
}

const removedOf = async (page: Page, mode: 'pixel' | 'image') =>
  ((await storedPref(page, `layout:${mode}`)) as { removed?: string[] } | undefined)?.removed;

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 800 } });

  test('pixel: View ▸ Panels sits before Toggle UI and lists every panel, checked, then Reset Panels (AC-01, AC-02, AC-10)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
    const view = await rowsOf(page.getByRole('menu', { name: 'View', exact: true }));
    // Directly before Toggle UI, in the same separator group.
    expect(view.slice(-3)).toEqual(['—', 'Panels', 'Toggle UI']);
    await page.getByRole('menu', { name: 'View', exact: true }).getByRole('menuitem', { name: 'Panels' }).click();
    expect(await rowsOf(page.getByRole('menu', { name: 'Panels', exact: true }))).toEqual([
      ...PIXEL,
      '—',
      'Reset Panels',
    ]);
    await closeMenus(page);
    expect(await entries(page)).toEqual(PIXEL.map((name) => [name, true]));
  });

  test('image: the entries are Colors, Layers, History; a check mark does not depend on the panel being in view (AC-01, AC-02)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
    expect((await rowsOf(page.getByRole('menu', { name: 'View', exact: true }))).slice(-3)).toEqual([
      '—',
      'Panels',
      'Toggle UI',
    ]);
    await closeMenus(page);
    expect(await entries(page)).toEqual(IMAGE.map((name) => [name, true]));
    // History is not the active tab, Colors is collapsed, then the sidebar is hidden.
    await header(page, 'Colors').click();
    await expect(panel(page, 'History')).toHaveCount(0);
    expect(await entries(page)).toEqual(IMAGE.map((name) => [name, true]));
    await toggle(page).click();
    await expect(sidebar(page)).toHaveCount(0);
    expect(await entries(page)).toEqual(IMAGE.map((name) => [name, true]));
  });

  test('choosing an enabled panel removes it, also while it is out of view; its entry loses the check mark (AC-02, AC-03)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await choose(page, 'Palette');
    await expect(panel(page, 'Palette')).toHaveCount(0);
    await expect(sidebar(page).getByText('Palette', { exact: true })).toHaveCount(0);
    // Tilesets is a tab that is not active, in a collapsed group.
    await choose(page, 'Tilesets');
    await expect(sidebar(page).getByRole('tab', { name: 'Tilesets' })).toHaveCount(0);
    expect(await entries(page)).toEqual(
      PIXEL.map((name) => [name, name !== 'Palette' && name !== 'Tilesets']),
    );
    await expect(tablist(page, 'Preview, Slices')).toBeVisible();
    await expect(panel(page, 'Colors')).toBeVisible();
  });

  test('a removed active tab hands over to the right, else to the left; the group keeps its collapse state (AC-04)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // Layers is active: History, to its right, takes over.
    await choose(page, 'Layers');
    await expect(panel(page, 'History')).toBeVisible();
    await choose(page, 'Layers');
    // Slices active in the collapsed group: nothing to the right, so Tilesets.
    await tabOf(page, 'Preview, Tilesets, Slices', 'Slices').click();
    await sidebar(page).getByRole('button', { name: 'Preview, Tilesets, Slices', exact: true }).click();
    await expect(panel(page, 'Slices')).toHaveCount(0);
    await choose(page, 'Slices');
    const group = 'Preview, Tilesets';
    await expect(tabOf(page, group, 'Tilesets')).toHaveAttribute('aria-selected', 'true');
    await expect(sidebar(page).getByRole('button', { name: group, exact: true })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    await expect(panel(page, 'Tilesets')).toHaveCount(0);
  });

  test('a stored layout that names a removed panel as active shows its neighbour (AC-04, AC-09)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await storeLayout(page, 'pixel', {
      collapsed: {},
      sidebar: true,
      groups: { assets: { active: 'preview', collapsed: false } },
      removed: ['preview'],
    });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(tabOf(page, 'Tilesets, Slices', 'Tilesets')).toHaveAttribute('aria-selected', 'true');
    await expect(panel(page, 'Tilesets')).toBeVisible();
    await expect(panel(page, 'Preview')).toHaveCount(0);
  });

  test('a group with one enabled panel is drawn like a single panel, with none it is not drawn (AC-05)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await choose(page, 'History');
    // No tab row: Layers has its own header with the collapse control.
    await expect(sidebar(page).getByRole('tablist')).toHaveCount(0);
    await expect(sidebar(page).getByRole('tabpanel')).toHaveCount(0);
    await expect(header(page, 'Layers')).toHaveAttribute('aria-expanded', 'true');
    await expect(panel(page, 'Layers').getByRole('button', { name: 'Add layer' })).toBeVisible();
    // The header collapses the group, and the state is the group's.
    await header(page, 'Layers').click();
    await expect(header(page, 'Layers')).toHaveAttribute('aria-expanded', 'false');
    await expect(panel(page, 'Layers').getByRole('button', { name: 'Add layer' })).toHaveCount(0);
    // A second panel again: the tab row returns, collapsed as the group was.
    await choose(page, 'History');
    await expect(tablist(page, 'Layers, History')).toBeVisible();
    await expect(tabOf(page, 'Layers, History', 'History')).toHaveAttribute('aria-selected', 'true');
    // Enabling brought it into view, so the group is expanded now.
    await expect(panel(page, 'History')).toBeVisible();
    await expect(header(page, 'Layers')).toHaveCount(0);

    // None enabled: no trace of the group.
    await choose(page, 'Layers');
    await choose(page, 'History');
    await expect(sidebar(page).locator('[data-panel-group="document"]')).toBeHidden();
    await expect(panel(page, 'Colors')).toBeVisible();
  });

  test('a lone panel takes the collapse state of its group, and the tab row takes the lone panel’s (AC-05)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // A collapsed two-tab group loses a tab: the panel left is collapsed.
    const collapse = sidebar(page).getByRole('button', { name: 'Layers, History', exact: true });
    await collapse.click();
    await expect(collapse).toHaveAttribute('aria-expanded', 'false');
    await choose(page, 'History');
    await expect(header(page, 'Layers')).toHaveAttribute('aria-expanded', 'false');
    await expect(panel(page, 'Layers').getByRole('button', { name: 'Add layer' })).toHaveCount(0);
    // Expanded through the lone header, the state is stored as the group's …
    await header(page, 'Layers').click();
    await expect(panel(page, 'Layers').getByRole('button', { name: 'Add layer' })).toBeVisible();
    await expect
      .poll(async () => ((await storedPref(page, 'layout:pixel')) as { groups?: Record<string, unknown> }).groups)
      .toEqual({ document: { active: 'layers', collapsed: false } });
    // … and collapsed again, it is what the tab row shows after a reload.
    await header(page, 'Layers').click();
    await expect
      .poll(async () => ((await storedPref(page, 'layout:pixel')) as { groups?: Record<string, unknown> }).groups)
      .toEqual({ document: { active: 'layers', collapsed: true } });
    await storeLayout(page, 'pixel', {
      collapsed: {},
      sidebar: true,
      groups: { document: { active: 'layers', collapsed: true } },
    });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(sidebar(page).getByRole('button', { name: 'Layers, History', exact: true })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    await expect(panel(page, 'Layers')).toHaveCount(0);
  });

  test('choosing a removed panel enables it and brings it into view: active tab, expanded, sidebar shown (AC-06)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await choose(page, 'Slices');
    // Colors is collapsed when it is removed.
    await header(page, 'Colors').click();
    await expect(header(page, 'Colors')).toHaveAttribute('aria-expanded', 'false');
    await choose(page, 'Colors');
    await header(page, 'Palette').click();
    await toggle(page).click();
    await expect(sidebar(page)).toHaveCount(0);

    // A tab of a collapsed group, with the sidebar hidden.
    await choose(page, 'Slices');
    await expect(sidebar(page)).toBeVisible();
    await expect(tabOf(page, 'Preview, Tilesets, Slices', 'Slices')).toHaveAttribute('aria-selected', 'true');
    await expect(panel(page, 'Slices')).toBeVisible();
    // A single panel comes back expanded.
    await choose(page, 'Colors');
    await expect(header(page, 'Colors')).toHaveAttribute('aria-expanded', 'true');
    expect(await entries(page)).toEqual(PIXEL.map((name) => [name, true]));
  });

  test('the Timeline: removed, nothing is docked and the canvas area takes its height; frames still step and play (AC-07, AC-08)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await menu(page, 'Frame', 'New Frame');
    await expect(frameItem(page)).toHaveText('Frame 2/2');
    const before = await box(stage(page));
    const dock = await box(timeline(page));
    await choose(page, 'Timeline');
    await expect(timeline(page)).toHaveCount(0);
    const after = await box(stage(page));
    expect(after.height).toBeGreaterThanOrEqual(before.height + dock.height - 1);
    expect(after.y).toBe(before.y);

    // The Frame menu and the keys still work.
    await menu(page, 'Frame', 'Previous Frame');
    await expect(frameItem(page)).toHaveText('Frame 1/2');
    await page.keyboard.press('.');
    await expect(frameItem(page)).toHaveText('Frame 2/2');
    await menu(page, 'Frame', 'Play');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Frame', exact: true }).click();
    await expect(page.getByRole('menu', { name: 'Frame' }).getByRole('menuitem', { name: 'Pause' })).toBeVisible();
    await page.getByRole('menu', { name: 'Frame' }).getByRole('menuitem', { name: 'Pause' }).click();

    // Enabled again: docked under the canvas.
    await choose(page, 'Timeline');
    await expect(timeline(page)).toBeVisible();
    const again = await box(stage(page));
    expect(Math.abs(again.height - before.height)).toBeLessThanOrEqual(1);
    expect((await box(timeline(page))).y).toBeGreaterThanOrEqual(again.y + again.height - 1);
  });

  test('which panels are enabled is stored per mode and survives a reload and a new document (AC-09)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await choose(page, 'Palette');
    await choose(page, 'Timeline');
    await expect.poll(() => removedOf(page, 'pixel')).toEqual(['palette', 'timeline']);
    expect(await removedOf(page, 'image')).toBeUndefined();

    await createDocument(page, 'pixel', { width: 8, height: 8 });
    await expect(panel(page, 'Colors')).toBeVisible();
    await expect(panel(page, 'Palette')).toHaveCount(0);
    await expect(timeline(page)).toHaveCount(0);
    expect(await entries(page)).toEqual(
      PIXEL.map((name) => [name, name !== 'Palette' && name !== 'Timeline']),
    );
    // Image mode has its own list.
    await createDocument(page, 'image', { width: 16, height: 16 });
    expect(await entries(page)).toEqual(IMAGE.map((name) => [name, true]));
  });

  test('a stored layout without the list, or one that removes a panel the platform lacks, enables every panel (AC-10)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await storeLayout(page, 'pixel', { collapsed: { colors: true }, sidebar: true });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    expect(await entries(page)).toEqual(PIXEL.map((name) => [name, true]));
    await storeLayout(page, 'pixel', { collapsed: {}, sidebar: true, removed: ['assistant', 'nonsense'] });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    expect(await entries(page)).toEqual(PIXEL.map((name) => [name, true]));
    await expect(tablist(page, 'Layers, History')).toBeVisible();
  });

  test('Reset Panels enables everything in its first-use state and keeps the width and the hidden sidebar (AC-11)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await storeLayout(page, 'pixel', {
      collapsed: { colors: true },
      sidebar: false,
      sidebarWidth: 340,
      effectPos: { x: 40, y: 60 },
      groups: { document: { active: 'history', collapsed: true }, assets: { active: 'slices', collapsed: false } },
      removed: ['palette', 'layers', 'timeline'],
    });
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(sidebar(page)).toHaveCount(0);
    await reset(page);
    // Reset does not open the sidebar.
    await expect(sidebar(page)).toHaveCount(0);
    await expect(timeline(page)).toBeVisible();
    await expect
      .poll(() => storedPref(page, 'layout:pixel'))
      .toEqual({ collapsed: {}, sidebar: false, sidebarWidth: 340, effectPos: { x: 40, y: 60 } });
    await toggle(page).click();
    expect(Math.round((await box(sidebar(page))).width)).toBe(340);
    expect(await entries(page)).toEqual(PIXEL.map((name) => [name, true]));
    await expect(header(page, 'Colors')).toHaveAttribute('aria-expanded', 'true');
    await expect(tabOf(page, 'Layers, History', 'Layers')).toHaveAttribute('aria-selected', 'true');
    await expect(panel(page, 'Layers')).toBeVisible();
    await expect(tabOf(page, 'Preview, Tilesets, Slices', 'Preview')).toHaveAttribute('aria-selected', 'true');
    await expect(panel(page, 'Preview')).toHaveCount(0);
  });

  test('with every sidebar panel removed there is no sidebar and the panels toggle is disabled (AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    const before = await box(stage(page));
    const side = await box(sidebar(page));
    for (const name of IMAGE) await choose(page, name);
    await expect(sidebar(page)).toHaveCount(0);
    await expect(page.getByRole('separator', { name: 'Sidebar width' })).toHaveCount(0);
    await expect(toggle(page)).toBeDisabled();
    const after = await box(stage(page));
    expect(after.width).toBeGreaterThanOrEqual(before.width + side.width);
    // Toggle UI twice: the bars return, the sidebar stays absent.
    await menu(page, 'View', 'Toggle UI');
    await menu(page, 'View', 'Toggle UI');
    await expect(page.locator('[data-frame="options"]')).toBeVisible();
    await expect(sidebar(page)).toHaveCount(0);
    // One panel back: sidebar and toggle return.
    await choose(page, 'Layers');
    await expect(panel(page, 'Layers')).toBeVisible();
    await expect(toggle(page)).toBeEnabled();
  });

  test('commands outside a removed panel keep working: undo, Layer menu, color swap (AC-13)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await setColor(page, 'fg', '#ff0000');
    const fg = await colorOf(page, 'fg');
    const bg = await colorOf(page, 'bg');
    for (const name of IMAGE) await choose(page, name);
    await menu(page, 'Layer', 'New Layer');
    await page.keyboard.press('x');
    await page.keyboard.press('Control+z');
    // The panels stay removed.
    await expect(sidebar(page)).toHaveCount(0);
    await choose(page, 'Layers');
    const rows = panel(page, 'Layers').getByRole('list', { name: 'Layer list' }).getByRole('listitem');
    await expect(rows).toHaveCount(1);
    await page.keyboard.press('Control+y');
    await expect(rows).toHaveCount(2);
    await choose(page, 'Colors');
    expect(await colorOf(page, 'fg')).toBe(bg);
    expect(await colorOf(page, 'bg')).toBe(fg);
    // Reset Colors (D) with Colors removed.
    await choose(page, 'Colors');
    await page.locator('[data-editor]:not([inert]) [data-stage]').click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('d');
    await choose(page, 'Colors');
    expect(await colorOf(page, 'fg')).toBe('#000000');
    expect(await colorOf(page, 'bg')).toBe('#ffffff');
    await choose(page, 'History');
    await expect(panel(page, 'History').getByRole('option')).toHaveCount(2);
  });

  test('a panel removed and enabled again shows what it showed before (AC-14)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await menu(page, 'Layer', 'New Layer');
    await menu(page, 'Layer', 'New Layer');
    await page.keyboard.press('Control+z');
    const layers = panel(page, 'Layers').getByRole('list', { name: 'Layer list' }).getByRole('listitem');
    await expect(layers).toHaveCount(2);
    await tabOf(page, 'Layers, History', 'History').click();
    const steps = panel(page, 'History').getByRole('option');
    const count = await steps.count();
    expect(count).toBeGreaterThanOrEqual(3);
    const selected = panel(page, 'History').getByRole('option', { selected: true });
    const current = await selected.textContent();

    await choose(page, 'History');
    await choose(page, 'Layers');
    await expect(sidebar(page).locator('[data-panel-group="document"]')).toBeHidden();
    await choose(page, 'Layers');
    await expect(layers).toHaveCount(2);
    await choose(page, 'History');
    await expect(steps).toHaveCount(count);
    await expect(selected).toHaveCount(1);
    expect(await selected.textContent()).toBe(current);
  });

  test('without Layers | History the remaining blocks keep the height of their content (AC-16)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const colors = await box(panel(page, 'Colors'));
    const palette = await box(panel(page, 'Palette'));
    await choose(page, 'Layers');
    await choose(page, 'History');
    expect((await box(panel(page, 'Colors'))).height).toBe(colors.height);
    expect((await box(panel(page, 'Palette'))).height).toBe(palette.height);
    const assets = await box(sidebar(page).locator('[data-panel-group="assets"]'));
    expect(assets.height).toBeLessThan(40);
    expect(Math.round(assets.y)).toBe(Math.round(palette.y + palette.height));
  });

  test('the command palette lists every panel entry and Reset Panels, and runs them (AC-17)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.keyboard.press('Control+k');
    const palette = page.getByRole('dialog', { name: 'Command Palette' });
    await palette.getByRole('combobox').fill('View Panels');
    const labels = await palette
      .getByRole('listbox')
      .getByRole('option')
      .evaluateAll((els) => els.map((el) => el.querySelector('span')?.textContent?.trim() ?? ''));
    for (const name of [...PIXEL, 'Reset Panels']) expect(labels).toContain(`View ▸ Panels ▸ ${name}`);
    await palette.getByRole('combobox').fill('View Panels Palette');
    await page.keyboard.press('Enter');
    await expect(palette).toHaveCount(0);
    await expect(panel(page, 'Palette')).toHaveCount(0);
  });

  test('German: Bedienfelder, Bedienfelder zurücksetzen and the panels’ German titles (AC-19)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Ansicht', exact: true }).click();
    await page.getByRole('menu', { name: 'Ansicht', exact: true }).getByRole('menuitem', { name: 'Bedienfelder', exact: true }).click();
    expect(await rowsOf(page.getByRole('menu', { name: 'Bedienfelder', exact: true }))).toEqual([
      'Farben',
      'Palette',
      'Ebenen',
      'Verlauf',
      'Vorschau',
      'Tilesets',
      'Slices',
      'Zeitleiste',
      '—',
      'Bedienfelder zurücksetzen',
    ]);
  });
});

test.describe('medium', () => {
  test.use({ viewport: { width: 800, height: 700 } });

  test('enabling a removed panel opens the drawer; with no sidebar panel the toggle is disabled (AC-06, AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await choose(page, 'History');
    await expect(sidebar(page)).toHaveCount(0);
    await choose(page, 'History');
    await expect(sidebar(page)).toBeVisible();
    await expect(panel(page, 'History')).toBeVisible();
    for (const name of IMAGE) await choose(page, name);
    await expect(sidebar(page)).toHaveCount(0);
    await expect(toggle(page)).toBeDisabled();
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  const sheet = (page: Page) => page.getByRole('dialog', { name: 'Panels' });
  const sheetTabs = (page: Page) =>
    sheet(page)
      .getByRole('tab')
      .evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim()));

  /** The ☰ sheet with View ▸ Panels expanded (the sheet remembers that). */
  async function phonePanels(page: Page): Promise<Locator> {
    await page.getByRole('button', { name: 'Menu', exact: true }).click();
    const list = page.getByRole('menu', { name: 'Menu', exact: true });
    const panels = list.getByRole('menuitem', { name: 'Panels', exact: true });
    if ((await panels.getAttribute('aria-expanded')) !== 'true') await panels.click();
    await expect(panels).toHaveAttribute('aria-expanded', 'true');
    return list;
  }

  /** Chooses the entry `name` of View ▸ Panels in the ☰ sheet. */
  async function phoneMenu(page: Page, _panels: 'Panels', name: string): Promise<void> {
    const list = await phonePanels(page);
    await list.getByRole('menuitemcheckbox', { name, exact: true }).click();
    await expect(list).toHaveCount(0);
  }

  test('the ☰ sheet shows View ▸ Panels with the entries and check marks (AC-18)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await phoneMenu(page, 'Panels', 'Tilesets');
    const list = await phonePanels(page);
    const marks = await list
      .getByRole('menuitemcheckbox')
      .evaluateAll((els) =>
        els.map((el) => [
          (el.querySelector('.am-menu-label')?.textContent ?? '').trim(),
          el.getAttribute('aria-checked') === 'true',
        ]),
      );
    for (const name of PIXEL) expect(marks).toContainEqual([name, name !== 'Tilesets']);
    await expect(list.getByRole('menuitem', { name: 'Reset Panels', exact: true })).toBeVisible();
  });

  test('a removed panel has no tab in the bottom sheet; removing the open tab moves on, the last one closes it (AC-03, AC-04)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await toggle(page).click();
    expect(await sheetTabs(page)).toEqual(IMAGE);
    await sheet(page).getByRole('tab', { name: 'Layers' }).click();
    await phoneMenu(page, 'Panels', 'Layers');
    expect(await sheetTabs(page)).toEqual(['Colors', 'History']);
    await expect(sheet(page).getByRole('tab', { name: 'History' })).toHaveAttribute('aria-selected', 'true');
    // Nothing to the right of History: Colors, to the left.
    await phoneMenu(page, 'Panels', 'History');
    await expect(sheet(page).getByRole('tab', { name: 'Colors' })).toHaveAttribute('aria-selected', 'true');
    await phoneMenu(page, 'Panels', 'Colors');
    await expect(sheet(page)).toHaveCount(0);
    // Image mode has no Timeline: nothing left to show.
    await expect(toggle(page)).toBeDisabled();
  });

  test('enabling a panel opens the bottom sheet on its tab; the Timeline too (AC-06, AC-07, AC-12)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    for (const name of PIXEL) await phoneMenu(page, 'Panels', name);
    await expect(toggle(page)).toBeDisabled();
    await phoneMenu(page, 'Panels', 'Timeline');
    // Only the Timeline is left: the toggle works again.
    await expect(toggle(page)).toBeEnabled();
    expect(await sheetTabs(page)).toEqual(['Timeline']);
    await expect(sheet(page).getByRole('tab', { name: 'Timeline' })).toHaveAttribute('aria-selected', 'true');
    await phoneMenu(page, 'Panels', 'Slices');
    await expect(sheet(page).getByRole('tab', { name: 'Slices' })).toHaveAttribute('aria-selected', 'true');
    await expect(sheet(page).locator('section[aria-label="Slices"]')).toBeVisible();
  });
});
