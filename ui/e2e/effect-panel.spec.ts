import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';
import {
  boxOf,
  brightness,
  dragBy,
  effectPanel,
  inside,
  liesInside,
  phoneMenu,
  storedLayout,
  titleBar,
  widthOf,
  type Mode,
} from './effect-panel-helpers';

// The effect panel (effect-panel spec, revision 2026-11-05): a small dialog
// over the canvas area that the title bar drags, docked to the bottom edge
// on phones; the sidebar, drawer and bottom sheet keep what they showed.

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const canvasArea = (page: Page) => page.getByRole('main');
const toolRail = (page: Page) => page.getByRole('toolbar', { name: 'Tools' });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

/** The effect each mode opens in these tests, with its menu path. */
const EFFECT = {
  pixel: { path: ['Adjustments', 'Levels…'], title: 'Levels', first: 'Channel' },
  image: { path: ['Effects', 'Blurs', 'Gaussian Blur…'], title: 'Gaussian Blur', first: 'Radius' },
} as const;

async function openEffect(page: Page, mode: Mode): Promise<Locator> {
  const [top, ...rest] = EFFECT[mode].path;
  await menu(page, top, ...rest);
  const panel = effectPanel(page, EFFECT[mode].title);
  await expect(panel).toBeVisible();
  return panel;
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the effect panel is a small dialog over the canvas area (AC-01)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const panel = await openEffect(page, mode);
    await expect(panel.getByRole('heading', { name: EFFECT[mode].title })).toBeVisible();
    await expect(panel.getByLabel(EFFECT[mode].first, { exact: true }).first()).toBeVisible();
    await expect(panel.getByRole('checkbox', { name: 'Preview' })).toBeChecked();
    for (const name of ['Reset', 'Cancel', 'OK']) {
      await expect(panel.getByRole('button', { name, exact: true })).toBeVisible();
    }
    // The close button sits in the title bar.
    await expect(titleBar(panel).getByRole('button', { name: 'Close', exact: true })).toBeVisible();

    // 280–360 px wide, wholly over the canvas area.
    const box = await boxOf(panel);
    expect(box.width).toBeGreaterThanOrEqual(280);
    expect(box.width).toBeLessThanOrEqual(360);
    expect(inside(box, await boxOf(canvasArea(page)))).toBe(true);

    // The sidebar keeps its panels and holds no effect.
    await expect(sidebar(page).getByRole('region', { name: 'Colors' })).toBeVisible();
    await expect(sidebar(page).getByRole('region', { name: 'Layers' })).toBeVisible();
    await expect(sidebar(page).getByRole('dialog')).toHaveCount(0);
  });

  test(`${mode}: OK applies and Cancel, × and Escape leave the document alone (AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const colors = sidebar(page).getByRole('region', { name: 'Colors' });
    await expect(colors).toBeVisible();
    const lit = await brightness(page, toolRail(page));
    // Dimmed while the panel is open, lit again once it is gone.
    const dimmed = () => expect.poll(() => brightness(page, toolRail(page))).toBeLessThan(lit * 0.8);
    const undimmed = () =>
      expect.poll(() => brightness(page, toolRail(page))).toBeGreaterThan(lit * 0.95);

    for (const close of ['Cancel', 'Close', 'Escape'] as const) {
      const panel = await openEffect(page, mode);
      await dimmed();
      if (close === 'Escape') await page.keyboard.press('Escape');
      else await panel.getByRole('button', { name: close, exact: true }).click();
      await expect(panel).toHaveCount(0);
      await expect(undoButton(page)).toBeDisabled();
      await expect(colors).toBeVisible();
      await undimmed();
    }

    let panel = await openEffect(page, mode);
    await dimmed();
    await panel.getByRole('button', { name: 'OK', exact: true }).click();
    await expect(panel).toHaveCount(0);
    await expect(undoButton(page)).toBeEnabled();
    await expect(colors).toBeVisible();
    await undimmed();

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


test('the sidebar keeps its visibility, stored too, while an effect is open (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('button', { name: 'Toggle panels' }).click();
  await expect(sidebar(page)).toHaveCount(0);
  await expect
    .poll(async () => (await storedLayout(page, 'image')) as object)
    .toMatchObject({ sidebar: false });

  for (const end of ['OK', 'Cancel'] as const) {
    const panel = await openEffect(page, 'image');
    await expect(sidebar(page)).toHaveCount(0);
    await panel.getByRole('button', { name: end, exact: true }).click();
    await expect(panel).toHaveCount(0);
    await expect(sidebar(page)).toHaveCount(0);
  }
  expect(await storedLayout(page, 'image')).toMatchObject({ sidebar: false });
});

test.describe('tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } });

  test('the panel floats over the canvas area and the drawer stays as it was (AC-12, AC-02)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await expect(sidebar(page)).toHaveCount(0);
    let panel = await openEffect(page, 'image');
    const box = await boxOf(panel);
    expect(box.width).toBeGreaterThanOrEqual(280);
    expect(box.width).toBeLessThanOrEqual(360);
    expect(inside(box, await boxOf(canvasArea(page)))).toBe(true);
    // A closed drawer stays closed.
    await expect(sidebar(page)).toHaveCount(0);
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(panel).toHaveCount(0);
    await expect(sidebar(page)).toHaveCount(0);

    // An open drawer stays open, with its panels, during and after.
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    await expect(sidebar(page)).toBeVisible();
    panel = await openEffect(page, 'image');
    await expect(sidebar(page).getByRole('region', { name: 'Layers' })).toBeVisible();
    await expect(sidebar(page).getByRole('dialog')).toHaveCount(0);
    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(panel).toHaveCount(0);
    await expect(sidebar(page).getByRole('region', { name: 'Layers' })).toBeVisible();
  });
});

test.describe('phone', () => {
  test.use({ viewport: { width: 390, height: 600 } });

  test('the panel is docked to the bottom across the window, over the sheet (AC-13, AC-02)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await page.getByRole('button', { name: 'Panels' }).click();
    await sheet.getByRole('tab', { name: 'Layers' }).click();
    const tabs = await sheet.getByRole('tab').count();
    const sheetBox = await boxOf(sheet);

    await phoneMenu(page, 'Levels…');
    const panel = effectPanel(page, 'Levels');
    await expect(panel).toBeVisible();
    // The sheet keeps its tabs and its selection, under the panel.
    await expect(sheet.getByRole('tab', { name: 'Layers' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(sheet.getByRole('tab')).toHaveCount(tabs);
    const box = await boxOf(panel);
    expect(Math.round(box.x)).toBe(0);
    expect(Math.round(box.width)).toBe(390);
    expect(Math.round(box.y + box.height)).toBe(600);
    const over = await panel.evaluate(
      (el, [x, y]) => el.contains(document.elementFromPoint(x, y)),
      [sheetBox.x + sheetBox.width / 2, sheetBox.y + sheetBox.height - 10] as const,
    );
    expect(over).toBe(true);

    // At most half the window: the controls scroll, OK stays in view.
    expect(box.height).toBeLessThanOrEqual(301);
    const scrolls = await panel.evaluate((el) =>
      Array.from(el.querySelectorAll<HTMLElement>('*')).some(
        (n) => n.scrollHeight > n.clientHeight + 1 && getComputedStyle(n).overflowY === 'auto',
      ),
    );
    expect(scrolls).toBe(true);
    await expect(panel.getByRole('button', { name: 'OK' })).toBeInViewport();

    // It cannot be dragged.
    await dragBy(page, titleBar(panel), 0, -200);
    expect(await boxOf(panel)).toEqual(box);

    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect(panel).toHaveCount(0);
    await expect(sheet.getByRole('tab', { name: 'Layers' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });

  test('a closed bottom sheet stays closed (AC-02)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await phoneMenu(page, 'Invert Colors');
    const panel = effectPanel(page, 'Invert Colors');
    await expect(panel).toBeVisible();
    await expect(sheet).toHaveCount(0);
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(panel).toHaveCount(0);
    await expect(sheet).toHaveCount(0);
  });
});

test('resizing across 640 px keeps the effect, its values and the panel inside (AC-14)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const panel = await openEffect(page, 'image');
  await panel.getByLabel('Radius value', { exact: true }).fill('7');
  // To the bottom-left corner of the canvas area.
  await dragBy(page, titleBar(panel), -2000, 2000);
  const area = await boxOf(canvasArea(page));
  const box = await boxOf(panel);
  expect(Math.round(box.x)).toBe(Math.round(area.x));
  expect(Math.round(box.y + box.height)).toBe(Math.round(area.y + area.height));

  // Tablet with a smaller canvas area: the panel moves back inside it.
  await page.setViewportSize({ width: 700, height: 500 });
  await expect.poll(() => liesInside(panel, canvasArea(page))).toBe(true);

  // Phone: docked to the bottom across the window, values kept. The frame
  // re-mounts the panel in the other layout, so its box is read only
  // through polls that outlast the swap (`widthOf`, `liesInside`).
  await page.setViewportSize({ width: 390, height: 844 });
  await expect
    .poll(async () => {
      const width = await widthOf(panel);
      return width === null ? null : Math.round(width);
    })
    .toBe(390);
  await expect(panel.getByLabel('Radius value', { exact: true })).toHaveValue('7');

  // Back to a wide window: floating again, inside the canvas area.
  await page.setViewportSize({ width: 1280, height: 720 });
  await expect.poll(() => widthOf(panel)).toBeLessThanOrEqual(360);
  await expect.poll(() => liesInside(panel, canvasArea(page))).toBe(true);
  await expect(panel.getByLabel('Radius value', { exact: true })).toHaveValue('7');

  await panel.getByRole('button', { name: 'Cancel' }).click();
  await expect(panel).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
});

test('close button, title bar and splitter are German (AC-15)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await menu(page, 'Korrekturen', 'Farben umkehren');
  const panel = effectPanel(page, 'Farben umkehren');
  await expect(panel).toBeVisible();
  await expect(titleBar(panel, 'Dialog verschieben')).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Schließen', exact: true })).toBeVisible();
  await panel.getByRole('button', { name: 'Schließen', exact: true }).click();
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole('separator', { name: 'Breite der Seitenleiste' })).toBeVisible();
});
