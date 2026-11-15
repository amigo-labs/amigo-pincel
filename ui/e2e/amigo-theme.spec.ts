import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  storedPref,
  switchLanguage,
} from './helpers';

// The shared amigo app theme (docs/specs/amigo-theme.md): colours, radii,
// focus ring and font come from the generated theme files with Pincel's
// accent; Settings ▸ Theme chooses Dark (default), Light or System.

type Rgb = [number, number, number];

const DARK_BG = '#15161a';
const LIGHT_BG = '#f4f5f7';

/** The theme token `--am-<name>` of the page as `#rrggbb` (the build may shorten it to `#rgb`). */
async function token(page: Page, name: string): Promise<string> {
  const raw = await page.evaluate(
    (n) => getComputedStyle(document.documentElement).getPropertyValue(`--am-${n}`).trim(),
    name,
  );
  return /^#[0-9a-f]{3}$/i.test(raw) ? '#' + [...raw.slice(1)].map((c) => c + c).join('') : raw;
}

/**
 * What `el` paints: its text colour and its own fill, each composited over
 * everything behind it (ancestors' fills from the page ground up), as sRGB.
 * Colours go through a canvas, so `color-mix()` and `oklab()` values resolve.
 */
async function painted(el: Locator): Promise<{ text: Rgb; fill: Rgb; behind: Rgb }> {
  return el.evaluate((node) => {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
    const px = (): [number, number, number] => {
      const d = ctx.getImageData(0, 0, 1, 1).data;
      return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0];
    };
    const chain: Element[] = [];
    for (let e: Element | null = node; e; e = e.parentElement) chain.unshift(e);
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0, 0, 1, 1);
    let behind = px();
    for (const e of chain) {
      if (e === node) behind = px();
      ctx.fillStyle = getComputedStyle(e).backgroundColor;
      ctx.fillRect(0, 0, 1, 1);
    }
    const fill = px();
    ctx.fillStyle = getComputedStyle(node).color;
    ctx.fillRect(0, 0, 1, 1);
    return { text: px(), fill, behind };
  });
}

function rgbOf(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [n >> 16, (n >> 8) & 255, n & 255];
}

function luminance([r, g, b]: Rgb): number {
  const lin = (v: number): number => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** `el`'s fill is the token `name` (one step of 8-bit rounding allowed). */
async function expectFill(page: Page, el: Locator, name: string): Promise<void> {
  const want = rgbOf(await token(page, name));
  const { fill } = await painted(el);
  for (let i = 0; i < 3; i++)
    expect(Math.abs(fill[i]! - want[i]!), `${name} fill`).toBeLessThanOrEqual(1);
}

async function expectText(page: Page, el: Locator, name: string): Promise<void> {
  const want = rgbOf(await token(page, name));
  const { text } = await painted(el);
  for (let i = 0; i < 3; i++)
    expect(Math.abs(text[i]! - want[i]!), `${name} text`).toBeLessThanOrEqual(1);
}

function settings(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Settings', exact: true });
}

function themeChoice(page: Page, name: string): Locator {
  return settings(page)
    .getByRole('radiogroup', { name: 'Theme' })
    .getByRole('radio', { name, exact: true });
}

/** Picks a theme in Settings and closes the dialog; waits for the pref. */
async function pickTheme(page: Page, name: 'Dark' | 'Light' | 'System'): Promise<void> {
  await page.keyboard.press('Control+,');
  await themeChoice(page, name).check();
  await expect.poll(() => storedPref(page, 'theme')).toBe(name.toLowerCase());
  await settings(page).getByRole('button', { name: 'Close' }).click();
  await expect(settings(page)).toBeHidden();
}

const activeTool = (page: Page): Locator =>
  page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]').first();

test('the start screen is dark by default, whatever the system prefers (AC-04)', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await gotoStart(page);
  expect(await token(page, 'bg')).toBe(DARK_BG);
  await expectFill(page, page.locator('body'), 'bg');
  await expectText(page, page.locator('body'), 'text');
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the editor's surfaces, text and accent are the theme's tokens (AC-02, AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await expectFill(page, page.locator('body'), 'bg');
    await expectFill(page, page.getByRole('menubar'), 'surface');
    await expectText(
      page,
      page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }),
      'text',
    );
    // The active tool is filled with the accent and carries on-accent.
    await expectFill(page, activeTool(page), 'accent-solid');
    await expectText(page, activeTool(page), 'on-accent');

    await page.keyboard.press('Control+,');
    const panel = settings(page).locator(':scope > div');
    await expectFill(page, panel, 'surface');
    const border = await panel.evaluate((el) => getComputedStyle(el).borderTopColor);
    const want = rgbOf(await token(page, 'border'));
    expect(border).toBe(`rgb(${want[0]}, ${want[1]}, ${want[2]})`);
    await expectFill(
      page,
      settings(page).getByRole('combobox', { name: 'Language' }),
      'surface-raised',
    );
    await expectFill(page, themeChoice(page, 'Dark'), 'accent-solid');
  });

  test(`${mode}: nothing keeps the former accent blue (AC-03)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Control+,');
    const stale = await page.evaluate(() => {
      const old = ['59, 130, 246', '37, 99, 235', '126, 170, 255', '59 130 246'];
      const hits: string[] = [];
      for (const el of document.querySelectorAll('*')) {
        const cs = getComputedStyle(el);
        const all = [
          cs.color,
          cs.backgroundColor,
          cs.borderTopColor,
          cs.outlineColor,
          cs.boxShadow,
          cs.fill,
          cs.stroke,
        ].join(' ');
        if (old.some((o) => all.includes(o))) hits.push(`${el.tagName}.${el.className}`);
      }
      return hits;
    });
    expect(stale).toEqual([]);
  });
}

test('Settings shows Theme with Dark selected, and a pick applies at once and is kept (AC-05, AC-06, AC-13)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('Control+,');
  await expect(themeChoice(page, 'Dark')).toBeChecked();
  await expect(themeChoice(page, 'Light')).not.toBeChecked();
  await expect(themeChoice(page, 'System')).not.toBeChecked();
  const themeColor = page.locator('meta[name="theme-color"]');
  await expect(themeColor).toHaveAttribute('content', DARK_BG);

  await themeChoice(page, 'Light').check();
  // The editor and the open dialog switch without a reload.
  expect(await token(page, 'bg')).toBe(LIGHT_BG);
  await expectFill(page, page.locator('body'), 'bg');
  await expectFill(page, settings(page).locator(':scope > div'), 'surface');
  await expect(themeChoice(page, 'Light')).toBeFocused();
  await expect(themeColor).toHaveAttribute('content', LIGHT_BG);

  await expect.poll(() => storedPref(page, 'theme')).toBe('light');
  await page.reload();
  await gotoStart(page);
  expect(await token(page, 'bg')).toBe(LIGHT_BG);
  await page.keyboard.press('Control+,');
  await expect(themeChoice(page, 'Light')).toBeChecked();
});

test('System follows the system preference, also while the app is open (AC-07, AC-13)', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await gotoStart(page);
  await pickTheme(page, 'System');
  expect(await token(page, 'bg')).toBe(DARK_BG);
  await page.emulateMedia({ colorScheme: 'light' });
  await expect.poll(() => token(page, 'bg')).toBe(LIGHT_BG);
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute('content', LIGHT_BG);
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect.poll(() => token(page, 'bg')).toBe(DARK_BG);
  // Dark and Light ignore the system.
  await pickTheme(page, 'Light');
  await page.emulateMedia({ colorScheme: 'dark' });
  expect(await token(page, 'bg')).toBe(LIGHT_BG);
});

test('a stored choice is in place when the UI first appears (AC-08)', async ({ page }) => {
  await gotoStart(page);
  await pickTheme(page, 'Light');
  // Record the palette at the moment the app first puts anything into #app.
  await page.addInitScript(() => {
    const seen = { first: null as string | null };
    Object.assign(window, { __firstPaint: seen });
    new MutationObserver(() => {
      const app = document.getElementById('app');
      if (seen.first === null && app && app.childElementCount > 0) {
        seen.first = getComputedStyle(document.documentElement).getPropertyValue('--am-bg').trim();
      }
    }).observe(document, { childList: true, subtree: true });
  });
  await page.reload();
  await gotoStart(page);
  const first = await page.evaluate(
    () => (window as unknown as { __firstPaint: { first: string | null } }).__firstPaint.first,
  );
  expect(first).toBe(LIGHT_BG);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: light: the workspace is light, a transparent document looks as in dark (AC-09)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    // A patch inside the document: what shows through a transparent document
    // is part of how it looks, and must not change with the palette.
    const size = await canvas.boundingBox();
    const clip = { x: size!.x + 2, y: size!.y + 2, width: 12, height: 12 };
    const darkShot = await page.screenshot({ clip });
    await pickTheme(page, 'Light');
    // The stage around the document is the light ground.
    const stage = canvas.locator('xpath=ancestor::*[contains(@class,"select-none")][1]');
    const { fill } = await painted(stage);
    const want = rgbOf(LIGHT_BG);
    for (let i = 0; i < 3; i++) expect(Math.abs(fill[i]! - want[i]!)).toBeLessThanOrEqual(1);
    expect(Buffer.compare(await page.screenshot({ clip }), darkShot)).toBe(0);
    if (mode === 'pixel') {
      // The document's edge is a line that differs from the workspace.
      const edge = await canvas
        .locator('xpath=..')
        .evaluate((el) => getComputedStyle(el).borderTopColor);
      const m = /rgb\((\d+), (\d+), (\d+)\)/.exec(edge);
      expect(m).not.toBeNull();
      expect(contrast([Number(m![1]), Number(m![2]), Number(m![3])], want)).toBeGreaterThan(1.15);
    }
  });
}

for (const palette of ['Dark', 'Light'] as const) {
  test(`${palette}: text holds 4.5:1 on what is behind it (AC-10)`, async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    if (palette === 'Light') await pickTheme(page, 'Light');
    const check = async (el: Locator, label: string, floor = 4.5): Promise<void> => {
      const { text, fill } = await painted(el);
      expect(contrast(text, fill), `${palette}: ${label}`).toBeGreaterThanOrEqual(floor);
    };
    const bar = page.getByRole('menubar');
    await check(bar.getByRole('menuitem', { name: 'File', exact: true }), 'menu bar entry');
    await check(activeTool(page), 'active tool icon', 3);
    await check(
      page.getByRole('region', { name: 'Layers' }).locator('header button').first(),
      'panel title',
    );
    await check(page.locator('.text-pc-muted:visible').first(), 'secondary text');
    await check(page.locator('.text-pc-faint:visible').first(), 'faint text');

    await bar.getByRole('menuitem', { name: 'Edit', exact: true }).click();
    const items = page.getByRole('menu', { name: 'Edit', exact: true }).getByRole('menuitem');
    await check(items.first(), 'menu item');
    await check(items.locator('.text-pc-faint').first(), 'menu shortcut');
    await page.keyboard.press('Escape');

    await page.keyboard.press('Control+,');
    await check(settings(page).getByRole('combobox', { name: 'Language' }), 'field text');
    await check(settings(page).getByRole('button', { name: 'Close' }), 'default button');
    await settings(page).getByRole('button', { name: 'Close' }).click();

    await menu(page, 'File', 'New…');
    const dialog = page.getByRole('dialog').last();
    await check(dialog.locator('.panel-btn-primary').first(), 'primary button');
    await check(
      dialog.locator('.panel-btn:not(.panel-btn-primary)').first(),
      'default button in a dialog',
    );
  });
}

test('UI text is Inter from the app itself (AC-11)', async ({ page }) => {
  const foreign: string[] = [];
  const origin = new URL(test.info().project.use.baseURL!).origin;
  page.on('request', (r) => {
    const url = r.url();
    if (/^https?:/.test(url) && new URL(url).origin !== origin) foreign.push(url);
  });
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const family = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
  expect(family).toMatch(/^"?Inter Variable"?/);
  await page.evaluate(() => document.fonts.ready);
  expect(await page.evaluate(() => document.fonts.check('14px "Inter Variable"'))).toBe(true);
  const loaded = await page.evaluate(() =>
    [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family.replace(/"/g, '')),
  );
  expect(loaded).toContain('Inter Variable');
  expect(foreign).toEqual([]);
});

test('radii and the focus ring come from the tokens (AC-12)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const radius = (el: Locator): Promise<string> =>
    el.evaluate((e) => getComputedStyle(e).borderTopLeftRadius);
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
  expect(await radius(page.getByRole('menu', { name: 'Edit', exact: true }))).toBe('8px');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+,');
  expect(await radius(settings(page).locator(':scope > div'))).toBe('12px');
  const close = settings(page).getByRole('button', { name: 'Close' });
  expect(await radius(close)).toBe('6px');
  expect(await radius(settings(page).getByRole('combobox', { name: 'Language' }))).toBe('6px');
  for (const name of ['Dark', 'Light'] as const) {
    await themeChoice(page, name).check();
    await close.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expect(close).toBeFocused();
    const ring = await close.evaluate((e) => {
      const cs = getComputedStyle(e);
      return { color: cs.outlineColor, style: cs.outlineStyle, width: cs.outlineWidth };
    });
    const want = rgbOf(await token(page, 'focus'));
    expect(ring).toEqual({
      color: `rgb(${want[0]}, ${want[1]}, ${want[2]})`,
      style: 'solid',
      width: '2px',
    });
  }
});

test('every language names the theme setting in its own words (AC-14)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const words: [string, RegExp, string, string[]][] = [
    ['Deutsch', /^Einstellungen$/, 'Erscheinungsbild', ['Dunkel', 'Hell', 'System']],
    ['Español', /^Ajustes|Configuración|Preferencias$/, 'Tema', ['Oscuro', 'Claro', 'Sistema']],
    ['Français', /.+/, 'Thème', ['Sombre', 'Clair', 'Système']],
    ['日本語', /.+/, 'テーマ', ['ダーク', 'ライト', 'システム']],
    ['Português (Brasil)', /.+/, 'Tema', ['Escuro', 'Claro', 'Sistema']],
  ];
  await page.keyboard.press('Control+,');
  const dialog = page.getByRole('dialog');
  for (const [language, , group, choices] of words) {
    await dialog.getByRole('combobox').first().selectOption({ label: language });
    const radios = dialog.getByRole('radiogroup', { name: group, exact: true }).getByRole('radio');
    await expect(radios).toHaveCount(3);
    for (const c of choices)
      await expect(dialog.getByRole('radio', { name: c, exact: true })).toBeVisible();
  }
  await dialog.getByRole('combobox').first().selectOption({ label: 'English' });
});

test('a choice that cannot be stored applies until the app is closed (AC-15)', async ({ page }) => {
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (value: unknown, key?: IDBValidKey) {
      if ((value as { key?: unknown } | null)?.key === 'theme')
        throw new DOMException('blocked', 'QuotaExceededError');
      return put.call(this, value, key);
    };
  });
  await gotoStart(page);
  await page.keyboard.press('Control+,');
  await themeChoice(page, 'Light').check();
  expect(await token(page, 'bg')).toBe(LIGHT_BG);
  await settings(page).getByRole('button', { name: 'Close' }).click();
  expect(await token(page, 'bg')).toBe(LIGHT_BG);
  await page.reload();
  await gotoStart(page);
  expect(await token(page, 'bg')).toBe(DARK_BG);
});

test('a stored choice that cannot be read starts dark and usable (AC-15)', async ({ page }) => {
  await gotoStart(page);
  await pickTheme(page, 'Light');
  await page.addInitScript(() => {
    const get = IDBObjectStore.prototype.get;
    IDBObjectStore.prototype.get = function (query: IDBValidKey | IDBKeyRange) {
      if (query === 'theme') throw new DOMException('blocked', 'UnknownError');
      return get.call(this, query);
    };
  });
  await page.reload();
  await gotoStart(page);
  expect(await token(page, 'bg')).toBe(DARK_BG);
  await createDocument(page, 'pixel', { width: 16, height: 16 });
});

test('an unknown stored value counts as no choice (AC-04)', async ({ page }) => {
  await gotoStart(page);
  await pickTheme(page, 'Light');
  await page.evaluate(
    () =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({ key: 'theme', value: 'sepia' });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        };
      }),
  );
  await page.reload();
  await gotoStart(page);
  expect(await token(page, 'bg')).toBe(DARK_BG);
  await switchLanguage(page, 'English');
});
