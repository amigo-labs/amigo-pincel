import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, setColor } from './helpers';

// Image mode's Colors panel (color-picker AC-25 – AC-29): below the
// swatches, the recent colors and the 48-color default palette, always
// visible; a left click sets the foreground, a right click or a long
// press the background.

const colorsPanel = (page: Page) => page.getByRole('region', { name: 'Colors' });
const recentRow = (page: Page) =>
  colorsPanel(page).getByRole('group', { name: 'Recent colors' }).last(); // after the picker's own row
const palette = (page: Page) => colorsPanel(page).getByRole('group', { name: 'Color palette' });

/** R / G / B / A of a swatch, read from its picker. */
async function rgbaOf(page: Page, which: 'fg' | 'bg'): Promise<number[]> {
  const name = which === 'fg' ? 'Foreground color' : 'Background color';
  await page.getByRole('button', { name, exact: true }).click();
  const picker = page.getByRole('dialog', { name: `${name} picker` });
  const out: number[] = [];
  for (const label of ['Red', 'Green', 'Blue', 'Alpha value']) {
    out.push(
      Number(await picker.getByRole('spinbutton', { name: label, exact: true }).inputValue()),
    );
  }
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);
  return out;
}

/** True when `target` is visible and `cover` does not overlap it. The
 *  picker's backdrop is transparent, so only the popover can hide it. */
async function clearOf(target: Locator, cover: Locator): Promise<boolean> {
  const a = await target.boundingBox();
  const b = await cover.boundingBox();
  if (!a || !b) return false;
  return (
    a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y
  );
}

test('the panel shows the recent colors and the default palette, also with the picker open (AC-25, AC-27, AC-29)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const swatches = palette(page).getByRole('button');
  await expect(swatches).toHaveCount(48);
  await expect(swatches.nth(0)).toHaveAccessibleName('#000000');
  await expect(swatches.nth(11)).toHaveAccessibleName('#ffffff');
  await expect(swatches.nth(13)).toHaveAccessibleName('#ff8000');
  await expect(swatches.nth(25)).toHaveAccessibleName('#b35900');
  await expect(swatches.nth(47)).toHaveAccessibleName('#660033');
  // Every swatch at least 16 px.
  for (const box of await swatches.evaluateAll((els) =>
    els.map((el) => el.getBoundingClientRect()),
  )) {
    expect(box.width).toBeGreaterThanOrEqual(16);
    expect(box.height).toBeGreaterThanOrEqual(16);
  }

  // Empty recent row: its label and a row-high empty area, so the palette
  // stays where it is when the first color arrives.
  await expect(colorsPanel(page).getByText('Recent colors', { exact: true })).toBeVisible();
  await expect(recentRow(page).getByRole('button')).toHaveCount(0);
  const rowBox = await recentRow(page).boundingBox();
  expect(rowBox?.height ?? 0).toBeGreaterThanOrEqual(16);
  const paletteTop = (await palette(page).boundingBox())?.y ?? 0;

  await setColor(page, 'fg', '#123456ff');
  await page.keyboard.press('b');
  const canvas = imageCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(recentRow(page).getByRole('button')).toHaveCount(1);
  await expect(recentRow(page).getByRole('button', { name: '#123456' })).toBeVisible();
  expect((await palette(page).boundingBox())?.y ?? 0).toBe(paletteTop);

  // With the picker open both stay visible.
  await page.getByRole('button', { name: 'Foreground color', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
  await expect(picker).toBeVisible();
  await expect(palette(page)).toBeVisible();
  await expect(recentRow(page)).toBeVisible();
  expect(await clearOf(palette(page), picker)).toBe(true);
  expect(await clearOf(recentRow(page), picker)).toBe(true);
});

test('a left click sets the foreground, a right click the background, without a context menu (AC-26)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.evaluate(() => {
    const w = window as unknown as { ctxLog: boolean[] };
    w.ctxLog = [];
    window.addEventListener('contextmenu', (e) => w.ctxLog.push(e.defaultPrevented));
  });
  await palette(page).getByRole('button', { name: '#ff0000' }).click();
  expect(await rgbaOf(page, 'fg')).toEqual([255, 0, 0, 255]);
  await palette(page).getByRole('button', { name: '#0000ff' }).click({ button: 'right' });
  expect(await rgbaOf(page, 'bg')).toEqual([0, 0, 255, 255]);
  expect(await rgbaOf(page, 'fg')).toEqual([255, 0, 0, 255]);
  const log = await page.evaluate(() => (window as unknown as { ctxLog: boolean[] }).ctxLog);
  expect(log.length).toBeGreaterThan(0);
  expect(log.every(Boolean)).toBe(true);

  // The recent row works the same way, alpha included.
  await setColor(page, 'fg', '#11223380');
  await page.keyboard.press('b');
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await recentRow(page).getByRole('button', { name: '#11223380' }).click({ button: 'right' });
  expect(await rgbaOf(page, 'bg')).toEqual([17, 34, 51, 128]);
  await setColor(page, 'fg', '#ffffffff');
  await recentRow(page).getByRole('button', { name: '#11223380' }).click();
  expect(await rgbaOf(page, 'fg')).toEqual([17, 34, 51, 128]);
});

/** Records whether each `contextmenu` event was cancelled (no menu opens). */
async function logContextMenus(page: Page): Promise<() => Promise<boolean[]>> {
  await page.evaluate(() => {
    const w = window as unknown as { ctxLog: boolean[] };
    w.ctxLog = [];
    window.addEventListener('contextmenu', (e) => w.ctxLog.push(e.defaultPrevented));
  });
  return () => page.evaluate(() => (window as unknown as { ctxLog: boolean[] }).ctxLog);
}

test('a pen long press sets the background color (AC-26)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const log = await logContextMenus(page);
  const b = await palette(page).getByRole('button', { name: '#00ff00' }).boundingBox();
  if (!b) throw new Error('not visible');
  const pen = {
    x: b.x + b.width / 2,
    y: b.y + b.height / 2,
    button: 'left',
    clickCount: 1,
    pointerType: 'pen',
    force: 0.5,
  } as const;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.dispatchMouseEvent', { ...pen, type: 'mousePressed', buttons: 1 });
  await page.waitForTimeout(700);
  await cdp.send('Input.dispatchMouseEvent', { ...pen, type: 'mouseReleased', buttons: 0 });
  await cdp.detach();
  expect(await rgbaOf(page, 'bg')).toEqual([0, 255, 0, 255]);
  // No click on the release: the foreground stayed.
  expect(await rgbaOf(page, 'fg')).toEqual([0, 0, 0, 255]);
  expect((await log()).every(Boolean)).toBe(true);
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  test('a long press sets the background color (AC-26)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    const log = await logContextMenus(page);
    const target = palette(page).getByRole('button', { name: '#00ff00' });
    const b = await target.boundingBox();
    if (!b) throw new Error('not visible');
    const at = { x: b.x + b.width / 2, y: b.y + b.height / 2 };
    const cdp = await page.context().newCDPSession(page);
    const touch = (type: 'touchStart' | 'touchEnd', points: { x: number; y: number }[]) =>
      cdp.send('Input.dispatchTouchEvent', {
        type,
        touchPoints: points.map((p, id) => ({ ...p, id, radiusX: 2, radiusY: 2, force: 1 })),
      });
    await touch('touchStart', [at]);
    await page.waitForTimeout(700);
    await touch('touchEnd', []);
    await cdp.detach();
    expect(await rgbaOf(page, 'bg')).toEqual([0, 255, 0, 255]);
    // The foreground stayed, and no context menu opened.
    expect(await rgbaOf(page, 'fg')).toEqual([0, 0, 0, 255]);
    expect((await log()).every(Boolean)).toBe(true);
  });
});

test('the panel rows are German in the German UI (AC-28)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const panel = page.getByRole('region', { name: 'Farben' });
  await expect(panel.getByRole('group', { name: 'Zuletzt verwendete Farben' })).toBeVisible();
  const pal = panel.getByRole('group', { name: 'Farbpalette' });
  await expect(pal.getByRole('button')).toHaveCount(48);
  await expect(pal.getByRole('button').nth(12)).toHaveAccessibleName('#ff0000');
});
