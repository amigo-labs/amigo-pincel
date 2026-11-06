import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, storedPref, waitForEditor } from './helpers';
import { installTauriMock } from './tauri-mock';

// image-view-aids: View ▸ Rulers, Pixel Grid and Units in Image mode. The
// unit arithmetic and the ruler steps are pinned without a page in
// view-units.spec.ts.

type Box = { x: number; y: number; width: number; height: number };

async function box(l: Locator): Promise<Box> {
  const b = await l.boundingBox();
  if (!b) throw new Error('not visible');
  return b;
}

function viewItem(page: Page, name: string): Locator {
  return page.getByRole('menuitemcheckbox', { name, exact: true });
}

/** Opens View (and a submenu) and reads whether `name` is checked. */
async function checked(page: Page, name: string, submenu?: string): Promise<boolean> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  if (submenu) await page.getByRole('menuitem', { name: submenu }).click();
  const state = await viewItem(page, name).getAttribute('aria-checked');
  await page.keyboard.press('Escape');
  if (submenu) await page.keyboard.press('Escape');
  return state === 'true';
}

const area = (page: Page): Locator => page.getByTestId('canvas-area');
const pointerStatus = (page: Page): Locator => page.getByTestId('status-pointer');

/** Image coordinate → screen: the surface covers the image exactly. */
async function mapping(page: Page, width: number): Promise<{ x: number; y: number; z: number }> {
  const b = await box(imageCanvas(page));
  return { x: b.x, y: b.y, z: b.width / width };
}

/** The zoom readout in percent. */
async function zoomPercent(page: Page): Promise<number> {
  const text = (await page.getByRole('button', { name: 'Fit to window' }).textContent()) ?? '';
  return Number(text.replace('%', ''));
}

/** Zooms in with `+` (×1.25) until the zoom is at least `percent`. */
async function zoomTo(page: Page, percent: number): Promise<void> {
  while ((await zoomPercent(page)) < percent) {
    const before = await zoomPercent(page);
    await page.keyboard.press('+');
    await expect.poll(() => zoomPercent(page)).not.toBe(before);
  }
}

/** Chooses a checkbox entry of a top menu, through its submenus. */
async function check(page: Page, top: string, ...path: string[]): Promise<void> {
  const last = path.pop() ?? '';
  await menu(page, top, ...path);
  await page
    .getByRole('menu', { name: path.at(-1) ?? top, exact: true })
    .getByRole('menuitemcheckbox', { name: last, exact: true })
    .click();
}

test('rulers: toggle, labels at their coordinates, markers and Fit (AC-01 – AC-05, AC-10, AC-13, AC-24)', async ({
  page,
}) => {
  // Measures every tick and label at three zoom levels.
  test.setTimeout(120_000);
  await createDocument(page, 'image', { width: 200, height: 100 });
  // A fresh profile: no rulers, no pixel grid, Pixels.
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  expect(await checked(page, 'Rulers')).toBe(false);
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  expect(await checked(page, 'Pixels', 'Units')).toBe(true);

  await check(page, 'View', 'Rulers');
  expect(await checked(page, 'Rulers')).toBe(true);
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  await expect(page.getByTestId('ruler-v')).toBeVisible();
  await menu(page, 'View', 'Fit to Window');

  // Fit shows the whole image beside the rulers.
  const h = await box(page.getByTestId('ruler-h'));
  const v = await box(page.getByTestId('ruler-v'));
  const img = await box(imageCanvas(page));
  const a = await box(area(page));
  expect(img.y).toBeGreaterThanOrEqual(h.y + h.height);
  expect(img.x).toBeGreaterThanOrEqual(v.x + v.width);
  expect(img.x + img.width).toBeLessThanOrEqual(a.x + a.width + 0.5);
  expect(img.y + img.height).toBeLessThanOrEqual(a.y + a.height + 0.5);

  async function expectTicksAligned(negatives = false): Promise<void> {
    const m = await mapping(page, 200);
    for (const axis of ['h', 'v'] as const) {
      const ticks = page.getByTestId(`ruler-${axis}-tick`);
      const labels = page.getByTestId(`ruler-${axis}-label`);
      const n = await ticks.count();
      expect(n).toBeGreaterThan(1);
      const values: number[] = [];
      for (let i = 0; i < n; i++) {
        const value = Number(await ticks.nth(i).getAttribute('data-value'));
        values.push(value);
        const t = await box(ticks.nth(i));
        const l = await box(labels.nth(i));
        const at = axis === 'h' ? m.x + value * m.z : m.y + value * m.z;
        expect(Math.abs((axis === 'h' ? t.x : t.y) - at)).toBeLessThanOrEqual(1);
        const gap = axis === 'h' ? l.x - t.x : l.y - t.y;
        expect(gap).toBeGreaterThanOrEqual(0);
        expect(gap).toBeLessThanOrEqual(4);
        // Labels never overlap and lie at least 40 px apart.
        if (i > 0) {
          const prev = await box(labels.nth(i - 1));
          const prevEnd = axis === 'h' ? prev.x + prev.width : prev.y + prev.height;
          expect(axis === 'h' ? l.x : l.y).toBeGreaterThan(prevEnd);
          const pt = await box(ticks.nth(i - 1));
          expect((axis === 'h' ? t.x - pt.x : t.y - pt.y) >= 40 - 1).toBe(true);
        }
      }
      if (negatives) {
        expect(values).toContain(0);
        expect(values.some((x) => x < 0)).toBe(true);
      }
    }
  }
  await expectTicksAligned();
  // After a zoom the labels move to the new positions.
  await page.keyboard.press('+');
  await page.keyboard.press('+');
  await expectTicksAligned();
  // Zoomed out, the rulers reach left of and above the image: negative values.
  for (let i = 0; i < 5; i++) await page.keyboard.press('-');
  await expectTicksAligned(true);

  // Markers and the status position follow a hovering mouse.
  await expect(page.getByTestId('ruler-h-marker')).toHaveCount(0);
  await expect(pointerStatus(page)).toHaveCount(0);
  const m = await mapping(page, 200);
  const sx = m.x + 37.5 * m.z;
  const sy = m.y + 12.5 * m.z;
  await page.mouse.move(sx, sy);
  await expect(pointerStatus(page)).toHaveText('37, 12 px');
  const mh = await box(page.getByTestId('ruler-h-marker'));
  const mv = await box(page.getByTestId('ruler-v-marker'));
  expect(Math.abs(mh.x - sx)).toBeLessThanOrEqual(1);
  expect(Math.abs(mv.y - sy)).toBeLessThanOrEqual(1);
  // Beside the image: negative values.
  await page.mouse.move(m.x - 3.5 * m.z, sy);
  await expect(pointerStatus(page)).toHaveText('-4, 12 px');
  // Over a ruler: outside the canvas area.
  await page.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
  await expect(pointerStatus(page)).toHaveCount(0);
  await expect(page.getByTestId('ruler-h-marker')).toHaveCount(0);

  // Turning the rulers off in fit mode re-fits to the larger area.
  await menu(page, 'View', 'Fit to Window');
  const fitted = await box(imageCanvas(page));
  await check(page, 'View', 'Rulers');
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  await expect.poll(async () => (await box(imageCanvas(page))).width).toBeGreaterThan(fitted.width);
});

test('pixel grid: from 400 % inside the image only (AC-06 – AC-08, AC-16)', async ({ page }) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await page.mouse.move(0, 0);
  const canvasArea = area(page);
  await zoomTo(page, 400);
  const off = await canvasArea.screenshot();
  const corner = { x: 0, y: 0, width: 12, height: 12 };
  const cornerOff = await page.screenshot({ clip: { ...corner, ...(await cornerAt(canvasArea)) } });
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await page.mouse.move(0, 0);
  const on = await canvasArea.screenshot();
  expect(on.equals(off)).toBe(false);
  const cornerOn = await page.screenshot({ clip: { ...corner, ...(await cornerAt(canvasArea)) } });
  expect(cornerOn.equals(cornerOff)).toBe(true);

  // Ctrl+' (Pixel mode's custom grid) does nothing in Image mode.
  await page.keyboard.press('ControlOrMeta+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(true);

  // Below 400 % the grid is not drawn.
  while ((await zoomPercent(page)) >= 400) await page.keyboard.press('-');
  await page.mouse.move(0, 0);
  const lowOn = await canvasArea.screenshot();
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  await page.mouse.move(0, 0);
  const lowOff = await canvasArea.screenshot();
  expect(lowOn.equals(lowOff)).toBe(true);
});

async function cornerAt(l: Locator): Promise<{ x: number; y: number }> {
  const b = await box(l);
  return { x: b.x + 2, y: b.y + 2 };
}

test('units switch the rulers and the status bar, in either language (AC-11, AC-12, AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await check(page, 'View', 'Units', 'Inches');
  expect(await checked(page, 'Inches', 'Units')).toBe(true);
  expect(await checked(page, 'Pixels', 'Units')).toBe(false);
  expect(await checked(page, 'Centimeters', 'Units')).toBe(false);
  // 96 DPI, the stored resolution of every new document.
  await expect(page.getByText('0.21 × 0.10 in · sRGB', { exact: true })).toBeVisible();
  await check(page, 'View', 'Rulers');
  const label = page.getByTestId('ruler-h-label').first();
  await expect(label).toHaveText(/^-?\d+\.\d\d$/);
  const m = await mapping(page, 20);
  await page.mouse.move(m.x + 1.5 * m.z, m.y + 1.5 * m.z);
  await expect(pointerStatus(page)).toHaveText('0.01, 0.01 in');
  await check(page, 'View', 'Units', 'Centimeters');
  await expect(page.getByText('0.53 × 0.26 cm · sRGB', { exact: true })).toBeVisible();

  await check(page, 'View', 'Language', 'Deutsch');
  await check(page, 'Ansicht', 'Einheiten', 'Zoll');
  await expect(page.getByText('0,21 × 0,10 Zoll · sRGB', { exact: true })).toBeVisible();
  await expect(page.getByTestId('ruler-h-label').first()).toHaveText(/^-?\d+,\d\d$/);
  await page.mouse.move(m.x + 1.5 * m.z, m.y + 1.5 * m.z);
  await expect(pointerStatus(page)).toHaveText('0,01, 0,01 Zoll');
});

test('view state persists per user and is no edit (AC-14, AC-15, AC-19)', async ({ page }) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await check(page, 'View', 'Rulers');
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await check(page, 'View', 'Units', 'Centimeters');
  await expect
    .poll(() => storedPref(page, 'imageView'))
    .toEqual({ showRulers: true, showPixelGrid: true, units: 'cm' });
  // No undo step, no unsaved changes, Pixel mode's grid untouched.
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);
  expect(await storedPref(page, 'pixelGrid')).toBeUndefined();

  // A second Image tab of another size: the states stay, the status follows.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  for (const [label, v] of [
    ['Width', '96'],
    ['Height', '48'],
  ] as const) {
    await dialog.getByLabel(label).fill(v);
  }
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await expect(page.getByText('2.54 × 1.27 cm · sRGB', { exact: true })).toBeVisible();
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab').first().click();
  await expect(page.getByText('0.53 × 0.26 cm · sRGB', { exact: true })).toBeVisible();

  // After a reload every Image tab starts from the stored state.
  await createDocument(page, 'image', { width: 20, height: 10 });
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  expect(await checked(page, 'Centimeters', 'Units')).toBe(true);
});

test('a malformed pref or a failing store keeps the editor working (AC-20, AC-25)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({ key: 'imageView', value: { units: 'furlong' } });
          tx.oncomplete = () => resolve();
        };
      }),
  );
  await createDocument(page, 'image', { width: 20, height: 10 });
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  expect(await checked(page, 'Pixels', 'Units')).toBe(true);

  // Every write fails from now on: the change still applies, silently.
  await page.evaluate(() => {
    IDBObjectStore.prototype.put = () => {
      throw new DOMException('blocked', 'InvalidStateError');
    };
  });
  await check(page, 'View', 'Rulers');
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  await expect(page.getByText(/blocked|failed/i)).toHaveCount(0);
});

test('web: Ctrl+Alt+R has no default and the shortcut list is clean (AC-17, AC-18)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await page.keyboard.press('Control+Alt+KeyR');
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  await expect(page.getByText('20 × 10 px · sRGB', { exact: true })).toBeVisible();
  const { rows } = await overview(page);
  expect(
    rows.some((r) => r.label === 'Rulers' && r.chords.length === 0),
    JSON.stringify(rows),
  ).toBe(true);
  expectCleanChords(rows, ['Show Pixel Grid']);
});

test.describe('desktop', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
  });
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  test('Ctrl+Alt+R toggles the rulers and rotates nothing (AC-17, AC-18)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Got it' }).click();
    await page.getByRole('button', { name: 'New image' }).click();
    const card = page.getByRole('form', { name: 'New image' });
    await card.getByLabel('Width').fill('20');
    await card.getByLabel('Height').fill('10');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
    await page.keyboard.press('Control+Alt+KeyR');
    await expect(page.getByTestId('ruler-h')).toBeVisible();
    await expect(page.getByText('20 × 10 px · sRGB', { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    await page.keyboard.press('Control+Alt+KeyR');
    await expect(page.getByTestId('ruler-h')).toHaveCount(0);
    const { rows } = await overview(page);
    expect(rows.some((r) => r.label === 'Rulers' && r.chords.includes('Ctrl+Alt+R'))).toBe(true);
    expectCleanChords(rows, ['Rulers', 'Show Pixel Grid'], false);
  });
});

type Row = { label: string; chords: string[] };

/** Help ▸ Keyboard Shortcuts: each row's label and chords. */
async function overview(page: Page): Promise<{ rows: Row[] }> {
  await page.keyboard.press('Control+/');
  const dialog = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  await expect(dialog).toBeVisible();
  const rows: Row[] = [];
  for (const row of await dialog.getByRole('row').all()) {
    const chords = (await row.locator('kbd').allTextContents()).map((c) => c.trim());
    const head = row.locator('th');
    if ((await head.count()) === 0) continue;
    rows.push({ label: ((await head.first().textContent()) ?? '').trim(), chords });
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  return { rows };
}

/** `labels` are listed with a chord, no chord is listed twice, none is one
 *  a browser tab cannot capture. */
function expectCleanChords(rows: readonly Row[], labels: readonly string[], web = true): void {
  for (const l of labels) {
    expect(
      rows.some((r) => r.label === l && r.chords.length > 0),
      l,
    ).toBe(true);
  }
  const all = rows.flatMap((r) => r.chords);
  expect(new Set(all).size).toBe(all.length);
  if (!web) return;
  for (const banned of [
    'Ctrl+N',
    'Ctrl+T',
    'Ctrl+W',
    'Ctrl+Tab',
    'Ctrl+Shift+Tab',
    'Ctrl+R',
    'Ctrl+Shift+R',
    'F5',
  ]) {
    expect(all).not.toContain(banned);
  }
}
