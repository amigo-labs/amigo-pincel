import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, setColor, storedPref, waitForEditor } from './helpers';
import {
  colorsPanel,
  loadPalette,
  noFilePickers,
  openPaletteMenu,
  red,
  redsGpl,
  savePalette,
  statusBar,
  swatchNames,
  swatches,
} from './palette-helpers';
import { expectDirty, tab } from './tab-helpers';
import {
  closeWindow,
  destroyed,
  installTauriMock,
  launchDesktop,
  putFile,
  setMock,
} from './tauri-mock';

// Image mode's palette as app state (docs/specs/image-palette-gradient.md):
// Add color, Reset, persistence in IndexedDB, one palette for every Image
// tab and none shared with Pixel mode, swatch behavior, the narrow-sidebar
// layout and the German UI (AC-09 – AC-20, AC-22, AC-25).

const PREF = 'palette:image';
const NAMED_GPL = 'GIMP Palette\nName: Test\n0 0 0 Black\n255 255 255\n10 20 30 Deep sea\n';

test.beforeEach(async ({ page }) => {
  await noFilePickers(page);
});

const addButton = (page: Page) =>
  colorsPanel(page).getByRole('button', { name: 'Add color', exact: true });

async function resetPalette(page: Page): Promise<void> {
  await openPaletteMenu(page);
  await page.getByRole('menuitem', { name: 'Reset to default palette' }).click();
}

/** R / G / B / A of the foreground or background, read from its picker. */
async function rgbaOf(page: Page, which: 'fg' | 'bg'): Promise<number[]> {
  const name = which === 'fg' ? 'Foreground color' : 'Background color';
  await colorsPanel(page).getByRole('button', { name, exact: true }).click();
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

/** Writes the IndexedDB pref `key` (the start screen must be shown). */
async function storePref(page: Page, key: string, value: unknown): Promise<void> {
  await page.evaluate(
    ([k, v]) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({ key: k, value: v });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(new Error('pref write failed'));
        };
        open.onerror = () => reject(new Error('IndexedDB unavailable'));
      }),
    [key, value] as const,
  );
}

/** Opens a new Image document as another tab through File ▸ New…. */
async function newImageTab(page: Page): Promise<void> {
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(dialog).toHaveCount(0);
}

test('Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  // Never customized: no stored palette, the built-in default shows.
  await expect(swatches(page)).toHaveCount(48);
  expect(await storedPref(page, PREF)).toBeUndefined();

  await setColor(page, 'fg', '#11223380');
  await addButton(page).click();
  await expect(swatches(page)).toHaveCount(49);
  await expect(swatches(page).last()).toHaveAccessibleName('#11223380');
  await expect.poll(() => storedPref(page, PREF)).toHaveLength(49);
  expect(((await storedPref(page, PREF)) as unknown[])[48]).toEqual({ rgba: 0x11223380 });

  await addButton(page).click();
  await expect(statusBar(page)).toContainText('The color is already in the palette');
  await setColor(page, 'fg', '#ff0000ff');
  await addButton(page).click();
  await expect(statusBar(page)).toContainText('The color is already in the palette');
  await expect(swatches(page)).toHaveCount(49);

  // Load, save, add and reset are no document change (AC-15).
  await loadPalette(page, 'named.gpl', Buffer.from(NAMED_GPL));
  await expect(swatches(page)).toHaveCount(3);
  await savePalette(page, 'JASC .pal');
  await expect(statusBar(page)).toContainText('saved 3 palette colours');

  await resetPalette(page);
  await expect(swatches(page)).toHaveCount(48);
  await expect(swatches(page).first()).toHaveAccessibleName('#000000');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
  await expect.poll(() => storedPref(page, PREF)).toBeUndefined();

  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  await expectDirty(page, 'image.ora', false);
});

test('Add color on a full palette says it is full, unless the color is in it (AC-10, AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await loadPalette(page, 'reds.gpl', redsGpl(256));
  await expect(swatches(page)).toHaveCount(256);
  await setColor(page, 'fg', '#ffffff');
  await addButton(page).click();
  await expect(statusBar(page)).toContainText('The palette is full (256 colours)');
  await expect(swatches(page)).toHaveCount(256);
  await setColor(page, 'fg', red(5));
  await addButton(page).click();
  await expect(statusBar(page)).toContainText('The color is already in the palette');
  await expect(swatches(page)).toHaveCount(256);
  await expect(swatches(page).last()).toHaveAccessibleName(red(255));
});

test('the palette survives a reload with colors, alpha and names (AC-13)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await loadPalette(page, 'named.gpl', Buffer.from(NAMED_GPL));
  await setColor(page, 'fg', '#11223380');
  await addButton(page).click();
  const expected = ['Black #000000', '#ffffff', 'Deep sea #0a141e', '#11223380'];
  await expect.poll(() => swatchNames(page)).toEqual(expected);
  await expect.poll(() => storedPref(page, PREF)).toHaveLength(4);
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect.poll(() => swatchNames(page)).toEqual(expected);
});

test.describe('desktop', () => {
  test.beforeEach(async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
  });

  /** Creates a 16 × 16 Image document from the desktop start screen. */
  async function newImage(page: Page): Promise<void> {
    await page.getByRole('button', { name: 'New image' }).click();
    const card = page.getByRole('form', { name: 'New image' });
    for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
  }

  test('the palette survives quitting the app (AC-13)', async ({ page }) => {
    await launchDesktop(page);
    await newImage(page);
    // Load palette… through the native open dialog.
    await putFile(page, '/home/u/named.gpl', Buffer.from(NAMED_GPL));
    await setMock(page, { openPick: '/home/u/named.gpl' });
    await openPaletteMenu(page);
    await page.getByRole('menuitem', { name: 'Load palette…' }).click();
    await expect(statusBar(page)).toContainText('loaded 3 palette colours');
    await setColor(page, 'fg', '#11223380');
    await addButton(page).click();
    const expected = ['Black #000000', '#ffffff', 'Deep sea #0a141e', '#11223380'];
    await expect.poll(() => swatchNames(page)).toEqual(expected);
    await expect.poll(() => storedPref(page, PREF)).toHaveLength(4);

    // Quit (the window's close button), then start the app again.
    await closeWindow(page);
    await expect.poll(() => destroyed(page)).toBe(1);
    await launchDesktop(page);
    await newImage(page);
    await expect.poll(() => swatchNames(page)).toEqual(expected);
  });
});

test('an unreadable stored palette falls back silently (AC-14)', async ({ page }) => {
  const many = Array.from({ length: 300 }, (_, i) => ({
    rgba: (((i & 0xff) << 24) | ((i >> 8) << 16) | 0xff) >>> 0,
  }));
  const cases: [unknown, string[] | number][] = [
    ['not a list', 48],
    [[], 48],
    [[7, null, { rgba: 'x' }, { rgba: -1 }], 48],
    [
      [
        7,
        { rgba: 'x' },
        { rgba: 0x11223380, name: 'Ink' },
        { rgba: 0xff0000ff, name: 3 },
        null,
        { rgba: 2 ** 32 },
        { rgba: 0x00ff00ff },
      ],
      ['Ink #11223380', '#00ff00'],
    ],
    [many, 256],
  ];
  for (const [value, expected] of cases) {
    await gotoStart(page);
    await storePref(page, PREF, value);
    await createDocument(page, 'image', { width: 16, height: 16 });
    if (typeof expected === 'number') {
      await expect(swatches(page)).toHaveCount(expected);
    } else {
      await expect.poll(() => swatchNames(page)).toEqual(expected);
    }
    if (expected === 48) await expect(swatches(page).nth(12)).toHaveAccessibleName('#ff0000');
    if (expected === 256) await expect(swatches(page).last()).toHaveAccessibleName(red(255));
    await expect(statusBar(page)).not.toContainText(/palette/i);
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
});

test('every Image tab shows the same palette (AC-16)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await newImageTab(page);
  await setColor(page, 'fg', '#11223380');
  await addButton(page).click();
  await expect(swatches(page)).toHaveCount(49);
  await page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab').first().click();
  await expect(swatches(page)).toHaveCount(49);
  await expect(swatches(page).last()).toHaveAccessibleName('#11223380');
  await resetPalette(page);
  await page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab').last().click();
  await expect(swatches(page)).toHaveCount(48);
});

test('Image and Pixel palettes do not touch each other (AC-17)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const pixelPanel = page.getByRole('region', { name: 'Palette' });
  const pixelSwatches = pixelPanel
    .getByRole('group', { name: 'Palette colors' })
    .getByRole('button');
  await expect(pixelSwatches).toHaveCount(32);
  await newImageTab(page);
  await loadPalette(page, 'reds.gpl', redsGpl(3));
  await expect(swatches(page)).toHaveCount(3);
  await setColor(page, 'fg', '#11223380');
  await addButton(page).click();
  await expect(swatches(page)).toHaveCount(4);

  await tab(page, 'pincel.aseprite').click();
  await expect(pixelSwatches).toHaveCount(32);
  await expect(pixelSwatches.first()).toHaveAttribute('title', /#000000/);
  await expect(pixelSwatches.nth(1)).toHaveAttribute('title', /#222034/);
  await pixelPanel.getByRole('button', { name: 'Palette options' }).click();
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('menuitem', { name: 'Import palette…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'two.hex',
    mimeType: 'text/plain',
    buffer: Buffer.from('ffffff\n00ff00\n'),
  });
  await expect(pixelSwatches).toHaveCount(2);
  await pixelPanel.getByRole('button', { name: 'Palette options' }).click();
  await page.getByRole('menuitem', { name: 'PICO-8' }).click();
  await expect(pixelSwatches).toHaveCount(16);

  await tab(page, 'image.ora').click();
  expect(await swatchNames(page)).toEqual([red(0), red(1), red(2), '#11223380']);
  await resetPalette(page);
  await expect(swatches(page)).toHaveCount(48);
  await tab(page, 'pincel.aseprite').click();
  await expect(pixelSwatches).toHaveCount(16);
});

test('loaded swatches pick foreground and background with alpha, without a context menu (AC-18)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.evaluate(() => {
    const w = window as unknown as { ctxLog: boolean[] };
    w.ctxLog = [];
    window.addEventListener('contextmenu', (e) => w.ctxLog.push(e.defaultPrevented));
  });
  await loadPalette(
    page,
    'clear.pal',
    Buffer.from('JASC-PAL\n0100\n2\n10 20 30 0\n200 100 50 128\n'),
  );
  expect(await swatchNames(page)).toEqual(['#0a141e00', '#c8643280']);
  await swatches(page).first().click();
  expect(await rgbaOf(page, 'fg')).toEqual([10, 20, 30, 0]);
  await swatches(page).last().click({ button: 'right' });
  expect(await rgbaOf(page, 'bg')).toEqual([200, 100, 50, 128]);
  expect(await rgbaOf(page, 'fg')).toEqual([10, 20, 30, 0]);
  const log = await page.evaluate(() => (window as unknown as { ctxLog: boolean[] }).ctxLog);
  expect(log.length).toBeGreaterThan(0);
  expect(log.every(Boolean)).toBe(true);
});

test('a 256-color palette wraps, stops at 8 rows and scrolls on its own in a 240 px sidebar (AC-19)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const sep = page.getByRole('separator', { name: 'Sidebar width' });
  await sep.focus();
  for (let i = 0; i < 12; i += 1) await page.keyboard.press('ArrowRight');
  await expect(sep).toHaveAttribute('aria-valuenow', '240');
  await loadPalette(page, 'reds.gpl', redsGpl(256));
  await expect(swatches(page)).toHaveCount(256);

  const grid = colorsPanel(page).getByRole('group', { name: 'Color palette' });
  const m = await grid.evaluate((el) => {
    const kids = [...el.children] as HTMLElement[];
    const tops = new Set(kids.map((k) => k.offsetTop));
    const first = kids[0]?.getBoundingClientRect();
    return {
      minSide: Math.min(...kids.map((k) => Math.min(k.offsetWidth, k.offsetHeight))),
      rows: tops.size,
      gap: Number.parseFloat(getComputedStyle(el).rowGap),
      swatch: first?.height ?? 0,
      clientHeight: el.clientHeight,
      scrollHeight: el.scrollHeight,
      clientWidth: el.clientWidth,
      scrollWidth: el.scrollWidth,
      overflowY: getComputedStyle(el).overflowY,
    };
  });
  expect(m.minSide).toBeGreaterThanOrEqual(16);
  expect(m.rows).toBeGreaterThan(8);
  expect(m.scrollWidth).toBeLessThanOrEqual(m.clientWidth);
  expect(m.overflowY).toBe('auto');
  expect(m.scrollHeight).toBeGreaterThan(m.clientHeight);
  expect(Math.abs(m.clientHeight - (8 * m.swatch + 7 * m.gap))).toBeLessThanOrEqual(2);

  // The palette scrolls by itself; the color pair and recent row stay in view.
  await swatches(page).last().scrollIntoViewIfNeeded();
  await expect(swatches(page).last()).toBeInViewport();
  expect(await grid.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  for (const name of ['Foreground color', 'Background color']) {
    await expect(colorsPanel(page).getByRole('button', { name, exact: true })).toBeInViewport();
  }
  await expect(
    colorsPanel(page).getByRole('group', { name: 'Recent colors' }).last(),
  ).toBeInViewport();
  await expect(colorsPanel(page).getByText('Recent colors', { exact: true })).toBeInViewport();
});

test('the palette menu, Add color and the palette messages are German in the German UI (AC-20)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  // Set while the swatches still have their English names.
  await setColor(page, 'fg', '#ffffffff');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const panel = page.getByRole('region', { name: 'Farben' });
  const status = page.getByRole('contentinfo');
  const add = panel.getByRole('button', { name: 'Farbe hinzufügen', exact: true });
  await expect(add).toBeVisible();
  const openMenu = async () => {
    await panel.getByRole('button', { name: 'Palettenoptionen' }).click();
    return page.getByRole('menu', { name: 'Palettenoptionen' });
  };
  const m = await openMenu();
  for (const [i, name] of [
    'Palette laden…',
    'Palette speichern',
    'Auf Standardpalette zurücksetzen',
  ].entries()) {
    await expect(m.getByRole('menuitem').nth(i)).toHaveAccessibleName(name);
  }
  await m.getByRole('menuitem', { name: 'Palette speichern' }).click();
  await expect(
    page.getByRole('menu', { name: 'Palette speichern' }).getByRole('menuitem'),
  ).toHaveText([
    'GIMP .gpl',
    'Lospec .hex',
    'JASC .pal',
    'PNG-Streifen (1 px)',
    'PNG-Streifen (8 px)',
  ]);
  await page.keyboard.press('Escape');

  const load = async (name: string, buffer: Buffer) => {
    await openMenu();
    const [chooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.getByRole('menuitem', { name: 'Palette laden…' }).click(),
    ]);
    await chooser.setFiles({ name, mimeType: 'text/plain', buffer });
  };
  await load('bad.hex', Buffer.from('ff0000\nnope\n'));
  await expect(status).toContainText('Palette laden fehlgeschlagen: line 2: not a colour');
  await load('reds.gpl', redsGpl(256));
  await expect(status).toContainText('256 Palettenfarben geladen');
  await add.click();
  await expect(status).toContainText('Die Palette ist voll (256 Farben)');
  await load('white.hex', Buffer.from('ffffff\n000000\n'));
  await expect(status).toContainText('2 Palettenfarben geladen');
  await add.click();
  await expect(status).toContainText('Die Farbe ist bereits in der Palette');
  await openMenu();
  await page.getByRole('menuitem', { name: 'Palette speichern' }).click();
  await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('menuitem', { name: 'Lospec .hex' }).click(),
  ]);
  await expect(status).toContainText('2 Palettenfarben gespeichert');
});

test('a palette that cannot be stored stays until reload and says so (AC-25)', async ({ page }) => {
  await page.addInitScript((key) => {
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (value: unknown, k?: IDBValidKey) {
      if ((value as { key?: unknown } | null)?.key === key) {
        throw new DOMException('The quota has been exceeded.', 'QuotaExceededError');
      }
      return put.call(this, value, k);
    };
  }, PREF);
  await createDocument(page, 'image', { width: 16, height: 16 });
  await setColor(page, 'fg', '#11223380');
  await addButton(page).click();
  await expect(swatches(page)).toHaveCount(49);
  await expect(statusBar(page)).toContainText(
    'The palette could not be stored; it is kept until the app reloads',
  );
  await expect(swatches(page).last()).toHaveAccessibleName('#11223380');
  expect(await storedPref(page, PREF)).toBeUndefined();
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect(swatches(page)).toHaveCount(48);
});
