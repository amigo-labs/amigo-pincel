import { expect, test, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, waitForEditor } from './helpers';
import { installTauriMock } from './tauri-mock';
import {
  area,
  box,
  check,
  checked,
  decimalUnit,
  expectApart,
  expectTicksAligned,
  mapping,
  PIXELS,
  pointerStatus,
  shownTicks,
  zoomPercent,
  zoomTo,
} from './view-aids-helpers';

// image-view-aids: View ▸ Rulers, Pixel Grid and Units in Image mode
// (docs/specs/image-view-aids.md). The pointer position and the markers are
// in image-view-aids-pointer.spec.ts, persistence, tabs and the resolution
// in image-view-aids-state.spec.ts, the unit arithmetic without a page in
// view-units.spec.ts.

/** The whole image lies in the canvas area, below and right of the rulers,
 *  fitted with Fit's 16 px padding on its tighter side (AC-05). */
async function fitsBesideRulers(page: Page, docW: number, docH: number): Promise<boolean> {
  const h = await box(page.getByTestId('ruler-h'));
  const v = await box(page.getByTestId('ruler-v'));
  const img = await box(imageCanvas(page));
  const a = await box(area(page));
  const fitZoom = Math.min((a.width - 32) / docW, (a.height - 32) / docH);
  return (
    a.y >= h.y + h.height - 0.5 &&
    a.x >= v.x + v.width - 0.5 &&
    img.y >= a.y &&
    img.x >= a.x &&
    img.x + img.width <= a.x + a.width + 0.5 &&
    img.y + img.height <= a.y + a.height + 0.5 &&
    Math.abs(img.width - fitZoom * docW) <= 1
  );
}

test('rulers: toggle, labels at their coordinates and Fit (AC-01 – AC-03, AC-05, AC-13)', async ({
  page,
}) => {
  // Measures every tick and label at several views.
  test.setTimeout(120_000);
  await createDocument(page, 'image', { width: 200, height: 100 });
  // A fresh profile: no rulers, no pixel grid, Pixels.
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  await expect(page.getByTestId('ruler-v')).toHaveCount(0);
  expect(await checked(page, 'Rulers')).toBe(false);
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  expect(await checked(page, 'Pixels', 'Units')).toBe(true);
  await expect(page.getByText('200 × 100 px · sRGB', { exact: true })).toBeVisible();

  // Turning the rulers on in fit mode re-fits beside them, without Fit.
  await check(page, 'View', 'Rulers');
  await expect(page.getByTestId('ruler-h')).toBeVisible();
  await expect(page.getByTestId('ruler-v')).toBeVisible();
  await expect.poll(() => fitsBesideRulers(page, 200, 100)).toBe(true);
  expect(await checked(page, 'Rulers')).toBe(true);

  const fitValues = await expectTicksAligned(page, 200);
  expect(fitValues).toContain(0);
  // After a zoom the labels move to the new positions.
  await page.keyboard.press('+');
  await page.keyboard.press('+');
  await expectTicksAligned(page, 200);
  // Zoomed out, the rulers reach left of and above the image: negative values.
  for (let i = 0; i < 5; i++) await page.keyboard.press('-');
  const values = await expectTicksAligned(page, 200);
  expect(values).toContain(0);
  expect(values.some((x) => x < 0)).toBe(true);
  const vValues = (await shownTicks(page, 'v')).map((t) => t.value);
  expect(vValues).toContain(0);
  expect(vValues.some((y) => y < 0)).toBe(true);

  // After a pan (middle-button drag) the labels follow the image.
  const before = await box(imageCanvas(page));
  const a = await box(area(page));
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(a.x + a.width / 2 + 73, a.y + a.height / 2 + 41, { steps: 4 });
  await page.mouse.up({ button: 'middle' });
  await expect.poll(async () => (await box(imageCanvas(page))).x).toBeCloseTo(before.x + 73, 0);
  await expectTicksAligned(page, 200);

  // Fit shows the whole image beside the rulers.
  await menu(page, 'View', 'Fit to Window');
  await expect.poll(() => fitsBesideRulers(page, 200, 100)).toBe(true);

  // Turning the rulers off in fit mode re-fits to the larger area.
  const fitted = await box(imageCanvas(page));
  await check(page, 'View', 'Rulers');
  await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  await expect(page.getByTestId('ruler-v')).toHaveCount(0);
  expect(await checked(page, 'Rulers')).toBe(false);
  await expect.poll(async () => (await box(imageCanvas(page))).width).toBeGreaterThan(fitted.width);
  const img = await box(imageCanvas(page));
  const whole = await box(area(page));
  expect(Math.abs(img.width - Math.min(whole.width - 32, (whole.height - 32) * 2))).toBeLessThan(1);
});

/** The zoom read from the image's on-screen height (a box's width
 *  saturates at 2²⁰ px, which a 20000 px wide image passes). */
async function zoomOf(page: Page, docH: number): Promise<number> {
  return (await box(imageCanvas(page))).height / docH;
}

test('ruler labels never overlap from the minimum to the maximum zoom, in German too (AC-03)', async ({
  page,
}) => {
  // Three sweeps of ~37 zoom steps each.
  test.setTimeout(180_000);
  const H = 100;
  await createDocument(page, 'image', { width: 20000, height: H });
  await check(page, 'View', 'Language', 'Deutsch');
  await check(page, 'Ansicht', 'Lineale');
  for (const unit of ['Zentimeter', 'Zoll', 'Pixel']) {
    await check(page, 'Ansicht', 'Einheiten', unit);
    // Down to the minimum zoom (2 %).
    while ((await zoomOf(page, H)) > 0.02 + 1e-6) {
      const z = await zoomOf(page, H);
      await page.keyboard.press('-');
      await expect.poll(() => zoomOf(page, H)).toBeLessThan(z);
    }
    let longest = 0;
    for (;;) {
      for (const axis of ['h', 'v'] as const) {
        const ticks = await shownTicks(page, axis);
        expect(ticks.length).toBeGreaterThan(1);
        for (let i = 1; i < ticks.length; i++) {
          const a = ticks[i - 1];
          const b = ticks[i];
          if (!a || !b) continue;
          expectApart(axis, a.label, b.label);
          expect(b.tick - a.tick).toBeGreaterThanOrEqual(40 - 1);
        }
        longest = Math.max(longest, ...ticks.map((t) => t.text.length));
      }
      const z = await zoomOf(page, H);
      if (z >= 64 - 1e-6) break;
      await page.keyboard.press('+');
      await expect.poll(() => zoomOf(page, H)).toBeGreaterThan(z);
    }
    // The sweep met the widest labels: `-600,00`-style decimals, five-digit
    // negative pixels.
    expect(longest, unit).toBeGreaterThanOrEqual(unit === 'Pixel' ? 6 : 7);
  }
});

/**
 * Resizes the window so Fit shows a `docH`-high document `docH × zoom +
 * delta` CSS px high (Fit: 16 px padding; the document is narrow, so its
 * height decides).
 */
async function fitHeightTo(page: Page, docH: number, zoom: number, delta = 0): Promise<void> {
  const size = page.viewportSize();
  if (!size) throw new Error('no viewport');
  const areaH = await area(page).evaluate((el) => el.clientHeight);
  const want = docH * zoom + 32 + delta;
  await page.setViewportSize({ width: size.width, height: size.height + want - areaH });
  await expect.poll(() => area(page).evaluate((el) => el.clientHeight)).toBe(want);
  await expect
    .poll(async () => Math.abs((await box(imageCanvas(page))).height - (docH * zoom + delta)))
    .toBeLessThan(1e-6);
}

/** Screenshots of the canvas area beside the image: three grid cells right
 *  of it along its rows and three below it along its columns. */
async function besideImage(page: Page, cell: number): Promise<Buffer[]> {
  const img = await box(imageCanvas(page));
  const a = await box(area(page));
  const right = Math.ceil(img.x + img.width) + 1;
  const bottom = Math.ceil(img.y + img.height) + 1;
  const top = Math.ceil(Math.max(img.y, a.y));
  const rowsEnd = Math.floor(Math.min(img.y + img.height, a.y + a.height));
  const shots = [
    await page.screenshot({ clip: { x: right, y: top, width: 3 * cell, height: rowsEnd - top } }),
  ];
  // Below the image, when it ends inside the canvas area.
  if (bottom + 3 * cell <= a.y + a.height) {
    shots.push(
      await page.screenshot({
        clip: { x: Math.ceil(img.x), y: bottom, width: Math.floor(img.width), height: 3 * cell },
      }),
    );
  }
  return shots;
}

function expectSameShots(a: readonly Buffer[], b: readonly Buffer[]): void {
  expect(a.length).toBe(b.length);
  a.forEach((shot, i) => expect(shot.equals(b[i] ?? Buffer.alloc(0)), `region ${i}`).toBe(true));
}

test('pixel grid: View ▸ Pixel Grid, drawn from exactly 400 % inside the image only (AC-06 – AC-08, AC-16)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 100 });
  await page.mouse.move(0, 0);
  const canvasArea = area(page);
  // Exactly 400 %.
  await fitHeightTo(page, 100, 4);
  expect(await zoomPercent(page)).toBe(400);
  const off = await canvasArea.screenshot();
  const besideOff = await besideImage(page, 4);

  // The menu entry turns it on: lines inside the image, nothing beside it.
  await check(page, 'View', 'Pixel Grid');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await page.mouse.move(0, 0);
  const on = await canvasArea.screenshot();
  expect(on.equals(off)).toBe(false);
  expectSameShots(await besideImage(page, 4), besideOff);
  // … and off again.
  await check(page, 'View', 'Pixel Grid');
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  await page.mouse.move(0, 0);
  expect((await canvasArea.screenshot()).equals(off)).toBe(true);

  // Ctrl+Shift+' toggles it too; Ctrl+' (Pixel mode's custom grid) does nothing.
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(true);
  await page.mouse.move(0, 0);
  expect((await canvasArea.screenshot()).equals(on)).toBe(true);
  await page.keyboard.press('ControlOrMeta+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(true);

  // Just below 400 % (399 %) the grid is not drawn; the entry stays checked.
  await fitHeightTo(page, 100, 4, -1);
  expect(await zoomPercent(page)).toBe(399);
  await page.mouse.move(0, 0);
  const lowOn = await canvasArea.screenshot();
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  expect(await checked(page, 'Pixel Grid')).toBe(false);
  await page.mouse.move(0, 0);
  expect((await canvasArea.screenshot()).equals(lowOn)).toBe(true);

  // Zoomed in further, lines again inside the image only.
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await zoomTo(page, 600);
  await page.mouse.move(0, 0);
  const cell = Math.ceil((await box(imageCanvas(page))).height / 100);
  const highOn = await canvasArea.screenshot();
  const besideHighOn = await besideImage(page, cell);
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await page.mouse.move(0, 0);
  expect((await canvasArea.screenshot()).equals(highOn)).toBe(false);
  expectSameShots(await besideImage(page, cell), besideHighOn);
});

test('pixel grid: its backing store follows a device-pixel-ratio change without a resize (AC-09)', async ({
  page,
}) => {
  // CDP's ratio emulation fires no media-query `change` event, which a real
  // move between monitors does: keep every query list the page creates and
  // fire `change` on those that stopped matching, as the browser would.
  // Only resolution queries: other lists see no change.
  await page.addInitScript(() => {
    const lists: MediaQueryList[] = [];
    const original = window.matchMedia.bind(window);
    window.matchMedia = (query: string): MediaQueryList => {
      const list = original(query);
      if (query.includes('resolution')) lists.push(list);
      return list;
    };
    Object.assign(window, {
      ratioChanged: (): void => {
        for (const l of lists.splice(0)) {
          if (!l.matches) l.dispatchEvent(new Event('change'));
        }
      },
    });
  });
  await createDocument(page, 'image', { width: 40, height: 30 });
  await page.keyboard.press('ControlOrMeta+Shift+Quote');
  await zoomTo(page, 400);
  const grid = page.getByTestId('pixel-grid');
  /** Device pixels per CSS pixel the grid canvas is backed by. */
  const backing = (): Promise<number> =>
    grid.evaluate((c: HTMLCanvasElement) => c.width / c.clientWidth);
  await expect.poll(backing).toBeCloseTo(1, 2);
  // A move to a monitor with another ratio: same CSS size, new ratio.
  const vp = page.viewportSize();
  if (!vp) throw new Error('no viewport');
  const cdp = await page.context().newCDPSession(page);
  for (const ratio of [2, 1.5, 1]) {
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: vp.width,
      height: vp.height,
      deviceScaleFactor: ratio,
      mobile: false,
    });
    await expect.poll(() => page.evaluate(() => window.devicePixelRatio)).toBe(ratio);
    await page.evaluate(() => (window as unknown as { ratioChanged: () => void }).ratioChanged());
    await expect.poll(backing).toBeCloseTo(ratio, 2);
  }
  await cdp.detach();
});

test('units switch the rulers and the status bar, in either language (AC-11, AC-12)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await createDocument(page, 'image', { width: 200, height: 100 });
  await check(page, 'View', 'Rulers');
  await expect.poll(() => fitsBesideRulers(page, 200, 100)).toBe(true);

  /** Hovers image pixel (37, 20). */
  async function hover(): Promise<void> {
    const m = await mapping(page, 200);
    await page.mouse.move(m.x + 37.5 * m.z, m.y + 20.5 * m.z);
  }
  /** Exactly `name` is checked among `units`. */
  async function only(
    name: string,
    units: readonly string[],
    submenu = 'Units',
    top = 'View',
  ): Promise<void> {
    for (const u of units) expect(await checked(page, u, submenu, top), u).toBe(u === name);
  }
  const EN = ['Pixels', 'Inches', 'Centimeters'];

  await check(page, 'View', 'Units', 'Inches');
  await only('Inches', EN);
  // 96 DPI: 200 px = 2.08 in.
  await expect(page.getByText('2.08 × 1.04 in · sRGB', { exact: true })).toBeVisible();
  await expectTicksAligned(page, 200, decimalUnit('in'));
  await hover();
  await expect(pointerStatus(page)).toHaveText('0.39, 0.21 in');

  await check(page, 'View', 'Units', 'Centimeters');
  await only('Centimeters', EN);
  await expect(page.getByText('5.29 × 2.65 cm · sRGB', { exact: true })).toBeVisible();
  await expectTicksAligned(page, 200, decimalUnit('cm'));
  await hover();
  await expect(pointerStatus(page)).toHaveText('0.98, 0.53 cm');

  // Back to Pixels from another unit.
  await check(page, 'View', 'Units', 'Pixels');
  await only('Pixels', EN);
  await expect(page.getByText('200 × 100 px · sRGB', { exact: true })).toBeVisible();
  await expectTicksAligned(page, 200, PIXELS);
  await hover();
  await expect(pointerStatus(page)).toHaveText('37, 20 px');

  // German: `Zoll` and a decimal comma; ruler labels stay bare numbers.
  await check(page, 'View', 'Language', 'Deutsch');
  await check(page, 'Ansicht', 'Einheiten', 'Zoll');
  await only('Zoll', ['Pixel', 'Zoll', 'Zentimeter'], 'Einheiten', 'Ansicht');
  await expect(page.getByText('2,08 × 1,04 Zoll · sRGB', { exact: true })).toBeVisible();
  await expectTicksAligned(page, 200, decimalUnit('in', 96, true));
  await hover();
  await expect(pointerStatus(page)).toHaveText('0,39, 0,21 Zoll');
  await check(page, 'Ansicht', 'Einheiten', 'Zentimeter');
  await expect(page.getByText('5,29 × 2,65 cm · sRGB', { exact: true })).toBeVisible();
  await expectTicksAligned(page, 200, decimalUnit('cm', 96, true));
});

/** A keydown as a keyboard layout sends it: German Windows turns Ctrl+Alt
 *  into AltGr, and AltGr+R types `®` (see shortcuts-coverage.spec.ts). */
async function layoutKey(
  page: Page,
  init: { key: string; code: string; ctrlKey?: boolean; altKey?: boolean },
): Promise<void> {
  await page.evaluate((i) => {
    const target = document.activeElement ?? document.body;
    target.dispatchEvent(new KeyboardEvent('keydown', { ...i, bubbles: true, cancelable: true }));
  }, init);
}

const AltGrR = { key: '®', code: 'KeyR', ctrlKey: true, altKey: true };

test('web: Ctrl+Alt+R has no default and the shortcut list is clean (AC-17, AC-18)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 20, height: 10 });
  await page.keyboard.press('Control+Alt+KeyR');
  await layoutKey(page, AltGrR);
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

  test('Ctrl+Alt+R toggles the rulers by the physical R key and rotates nothing (AC-17, AC-18)', async ({
    page,
  }) => {
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
    // German Windows: Ctrl+Alt is AltGr, and AltGr+R types `®`.
    await layoutKey(page, AltGrR);
    await expect(page.getByTestId('ruler-h')).toBeVisible();
    await layoutKey(page, AltGrR);
    await expect(page.getByTestId('ruler-h')).toHaveCount(0);
    await expect(page.getByText('20 × 10 px · sRGB', { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
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
