import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';

// Docked effect panel (effect-panel spec): an effect's controls live in the
// right sidebar — drawer on tablets, an extra bottom-sheet tab on phones —
// instead of a floating box over the canvas.

type Mode = 'pixel' | 'image';

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

/** The effect each mode opens in these tests, with its menu path. */
const EFFECT = {
  pixel: { path: ['Adjustments', 'Levels…'], title: 'Levels', first: 'Channel' },
  image: { path: ['Effects', 'Blurs', 'Gaussian Blur…'], title: 'Gaussian Blur', first: 'Radius' },
} as const;

function effectPanel(page: Page, title: string): Locator {
  return page.getByRole('dialog', { name: title });
}

async function openEffect(page: Page, mode: Mode): Promise<Locator> {
  const [top, ...rest] = EFFECT[mode].path;
  await menu(page, top, ...rest);
  const panel = effectPanel(page, EFFECT[mode].title);
  await expect(panel).toBeVisible();
  return panel;
}

/** Runs a menu path from the phone ☰ sheet: top-level menus are headings
 *  there, submenus expand inline. */
async function phoneMenu(page: Page, _top: string, ...path: string[]): Promise<void> {
  // "Menu" / German "Menü".
  await page.getByRole('button', { name: /^Men[uü]$/ }).click();
  const sheet = page.getByRole('menu', { name: /^Men[uü]$/ });
  for (const label of path) {
    await sheet
      .getByRole('menuitem', { name: label, exact: true })
      .or(sheet.getByRole('menuitemcheckbox', { name: label, exact: true }))
      .click();
  }
}

/** The stored `layout:<mode>` pref, read straight from IndexedDB. */
function storedLayout(page: Page, mode: Mode): Promise<unknown> {
  return page.evaluate(
    (key) =>
      new Promise<unknown>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const get = open.result.transaction('prefs').objectStore('prefs').get(key);
          get.onsuccess = () => resolve((get.result as { value?: unknown } | undefined)?.value);
          get.onerror = () => resolve(null);
        };
        open.onerror = () => resolve(null);
      }),
    `layout:${mode}`,
  );
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the effect panel is the topmost sidebar panel, clear of the canvas (AC-01)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const panel = await openEffect(page, mode);
    // Docked in the sidebar, with the usual controls plus a close button.
    await expect(sidebar(page).getByRole('dialog', { name: EFFECT[mode].title })).toBeVisible();
    await expect(panel.getByRole('heading', { name: EFFECT[mode].title })).toBeVisible();
    await expect(panel.getByLabel(EFFECT[mode].first, { exact: true }).first()).toBeVisible();
    await expect(panel.getByRole('checkbox', { name: 'Preview' })).toBeChecked();
    for (const name of ['Reset', 'Cancel', 'OK', 'Close']) {
      await expect(panel.getByRole('button', { name, exact: true })).toBeVisible();
    }
    // Topmost: above every other sidebar panel.
    const panelBox = await panel.boundingBox();
    const regions = sidebar(page).getByRole('region');
    expect(await regions.count()).toBeGreaterThan(0);
    for (const region of await regions.all()) {
      const box = await region.boundingBox();
      if (box && panelBox) expect(box.y).toBeGreaterThan(panelBox.y);
    }
    // Nothing of it over the canvas area.
    const area = await page.getByRole('main').boundingBox();
    if (!area || !panelBox) throw new Error('layout not visible');
    expect(panelBox.x).toBeGreaterThanOrEqual(area.x + area.width);
  });

  test(`${mode}: OK applies and Cancel, × and Escape leave the document alone (AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const colors = sidebar(page).getByRole('region', { name: 'Colors' });
    await expect(colors).toBeVisible();

    for (const close of ['Cancel', 'Close', 'Escape'] as const) {
      const panel = await openEffect(page, mode);
      if (close === 'Escape') await page.keyboard.press('Escape');
      else await panel.getByRole('button', { name: close, exact: true }).click();
      await expect(panel).toHaveCount(0);
      await expect(undoButton(page)).toBeDisabled();
      await expect(colors).toBeVisible();
    }

    let panel = await openEffect(page, mode);
    await panel.getByRole('button', { name: 'OK', exact: true }).click();
    await expect(panel).toHaveCount(0);
    await expect(undoButton(page)).toBeEnabled();
    await expect(colors).toBeVisible();

    // Enter outside a multi-line field is OK.
    await undoButton(page).click();
    await expect(undoButton(page)).toBeDisabled();
    panel = await openEffect(page, mode);
    await page.keyboard.press('Enter');
    await expect(panel).toHaveCount(0);
    await expect(undoButton(page)).toBeEnabled();
  });

  test(`${mode}: choosing an effect while one is open restarts at the defaults (AC-05)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    let panel = await openEffect(page, mode);
    const field =
      mode === 'image'
        ? panel.getByLabel('Radius value', { exact: true })
        : panel.getByLabel('Input black value', { exact: true });
    const defaultValue = mode === 'image' ? '2' : '0';
    await field.fill('9');
    await expect(field).toHaveValue('9');

    // The same effect again: back at its defaults, focus on the first control.
    panel = await openEffect(page, mode);
    await expect(panel).toHaveCount(1);
    await expect(field).toHaveValue(defaultValue);
    const first =
      mode === 'image'
        ? panel.getByRole('slider', { name: 'Radius' })
        : panel.getByRole('combobox', { name: 'Channel' });
    await expect(first).toBeFocused();

    // Another effect replaces it; the first one left no trace.
    await menu(page, 'Adjustments', 'Invert Colors');
    const invert = effectPanel(page, 'Invert Colors');
    await expect(invert).toBeVisible();
    await expect(effectPanel(page, EFFECT[mode].title)).toHaveCount(0);
    await invert.getByRole('button', { name: 'Cancel' }).click();
    await expect(undoButton(page)).toBeDisabled();
  });

  test(`${mode}: focus goes to the first control, or OK, and back to the canvas area (AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const panel = await openEffect(page, mode);
    const first =
      mode === 'image'
        ? panel.getByRole('slider', { name: 'Radius' })
        : panel.getByRole('combobox', { name: 'Channel' });
    await expect(first).toBeFocused();
    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByRole('main')).toBeFocused();

    // No parameters: OK has focus, so Enter applies at once.
    await menu(page, 'Adjustments', 'Invert Colors');
    const invert = effectPanel(page, 'Invert Colors');
    await expect(invert.getByText('This effect has no parameters.')).toBeVisible();
    await expect(invert.getByRole('button', { name: 'OK' })).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(invert).toHaveCount(0);
    await expect(undoButton(page)).toBeEnabled();
    await expect(page.getByRole('main')).toBeFocused();
  });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: keys on the menu or a panel button do what that control does (AC-04, AC-05)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const invert = effectPanel(page, 'Invert Colors');
    const levels = effectPanel(page, 'Levels');
    const menuItem = (top: string, name: string) =>
      page
        .getByRole('menu', { name: top, exact: true })
        .getByRole('menuitem', { name, exact: true });
    const openMenu = (top: string) =>
      page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();

    // Enter on another effect's menu entry restarts with that effect; the
    // open one is cancelled, not applied (AC-05).
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(invert).toBeVisible();
    await openMenu('Adjustments');
    await menuItem('Adjustments', 'Levels…').focus();
    await page.keyboard.press('Enter');
    await expect(levels).toBeVisible();
    await expect(invert).toHaveCount(0);
    await expect(undoButton(page)).toBeDisabled();

    // Escape with a menu open closes the menu only.
    await openMenu('Adjustments');
    await expect(page.getByRole('menu', { name: 'Adjustments', exact: true })).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('menu', { name: 'Adjustments', exact: true })).toHaveCount(0);
    await expect(levels).toBeVisible();

    // Enter on a focused Cancel cancels (AC-04).
    await levels.getByRole('button', { name: 'Cancel' }).focus();
    await page.keyboard.press('Enter');
    await expect(levels).toHaveCount(0);
    await expect(undoButton(page)).toBeDisabled();
  });
}

test('a hidden sidebar shows for the effect only; the stored visibility stays (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const toggle = page.getByRole('button', { name: 'Toggle panels' });
  await toggle.click();
  await expect(sidebar(page)).toHaveCount(0);
  await expect
    .poll(async () => (await storedLayout(page, 'image')) as object)
    .toMatchObject({
      sidebar: false,
    });

  for (const end of ['OK', 'Cancel'] as const) {
    const panel = await openEffect(page, 'image');
    await expect(sidebar(page).getByRole('dialog', { name: 'Gaussian Blur' })).toBeVisible();
    await panel.getByRole('button', { name: end, exact: true }).click();
    await expect(sidebar(page)).toHaveCount(0);
  }
  expect(await storedLayout(page, 'image')).toMatchObject({ sidebar: false });

  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.waitForTimeout(300);
  await expect(sidebar(page)).toHaveCount(0);
});

test.describe('tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } });

  test('the drawer opens with the effect panel on top and closes again (AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await expect(sidebar(page)).toHaveCount(0);
    let panel = await openEffect(page, 'image');
    await expect(sidebar(page).getByRole('dialog', { name: 'Gaussian Blur' })).toBeVisible();
    const panelBox = await panel.boundingBox();
    const layersBox = await sidebar(page).getByRole('region', { name: 'Layers' }).boundingBox();
    if (!panelBox || !layersBox) throw new Error('drawer not visible');
    expect(layersBox.y).toBeGreaterThan(panelBox.y);
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(sidebar(page)).toHaveCount(0);

    // An open drawer stays open after the effect.
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    await expect(sidebar(page)).toBeVisible();
    panel = await openEffect(page, 'image');
    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(panel).toHaveCount(0);
    await expect(sidebar(page).getByRole('region', { name: 'Layers' })).toBeVisible();
  });
});

test.describe('phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('the bottom sheet gets a selected tab for the effect (AC-13)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await expect(sheet).toHaveCount(0);

    await phoneMenu(page, 'Adjustments', 'Invert Colors');
    await expect(sheet.getByRole('tab', { name: 'Invert Colors' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    const panel = sheet.getByRole('dialog', { name: 'Invert Colors' });
    await expect(panel).toBeVisible();
    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(sheet).toHaveCount(0);

    // Opened on the Layers tab: the sheet returns to it.
    await page.getByRole('button', { name: 'Panels' }).click();
    await sheet.getByRole('tab', { name: 'Layers' }).click();
    await phoneMenu(page, 'Adjustments', 'Invert Colors');
    await expect(sheet.getByRole('tab', { name: 'Invert Colors' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await sheet
      .getByRole('dialog', { name: 'Invert Colors' })
      .getByRole('button', { name: 'OK' })
      .click();
    await expect(sheet.getByRole('tab', { name: 'Invert Colors' })).toHaveCount(0);
    await expect(sheet.getByRole('tab', { name: 'Layers' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });
});

test('resizing across layouts keeps the effect and its values (AC-14)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('button', { name: 'Toggle panels' }).click();
  await expect(sidebar(page)).toHaveCount(0);
  const panel = await openEffect(page, 'image');
  await panel.getByLabel('Radius value', { exact: true }).fill('7');

  // Tablet: the drawer carries the panel.
  await page.setViewportSize({ width: 820, height: 1000 });
  await expect(sidebar(page).getByRole('dialog', { name: 'Gaussian Blur' })).toBeVisible();
  await expect(panel.getByLabel('Radius value', { exact: true })).toHaveValue('7');

  // Phone: an extra sheet tab carries it.
  await page.setViewportSize({ width: 390, height: 844 });
  const sheet = page.getByRole('dialog', { name: 'Panels' });
  await expect(sheet.getByRole('tab', { name: 'Gaussian Blur' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(panel.getByLabel('Radius value', { exact: true })).toHaveValue('7');

  // Ending it leaves each layout as it was before: sheet closed, drawer
  // closed, sidebar hidden.
  await panel.getByRole('button', { name: 'Cancel' }).click();
  await expect(sheet).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
  await page.setViewportSize({ width: 820, height: 1000 });
  await expect(sidebar(page)).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 720 });
  await expect(sidebar(page)).toHaveCount(0);
});

test.describe('German UI', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('close button, splitter and sheet tab are German (AC-15)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await phoneMenu(page, 'View', 'Language', 'Deutsch');
    await phoneMenu(page, 'Korrekturen', 'Farben umkehren');
    const sheet = page.getByRole('dialog', { name: 'Bedienfelder' });
    await expect(sheet.getByRole('tab', { name: 'Farben umkehren' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    const panel = sheet.getByRole('dialog', { name: 'Farben umkehren' });
    await expect(panel.getByRole('button', { name: 'Schließen', exact: true })).toBeVisible();
    await panel.getByRole('button', { name: 'Schließen', exact: true }).click();
    await expect(sheet).toHaveCount(0);

    await page.setViewportSize({ width: 1280, height: 720 });
    await expect(page.getByRole('separator', { name: 'Breite der Seitenleiste' })).toBeVisible();
  });
});
