import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  dragFiles,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  setColor,
  storedPref,
  swatch,
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
  await // The radio itself is hidden inside its segment: the press lands on its label.
  await themeChoice(page, name).check({ force: true });
  await expect.poll(() => storedPref(page, 'theme')).toBe(name.toLowerCase());
  await settings(page).getByRole('button', { name: 'Close' }).click();
  await expect(settings(page)).toBeHidden();
}

const activeTool = (page: Page): Locator =>
  page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]').first();

const MODES = ['pixel', 'image'] as const;
const PALETTES = ['Dark', 'Light'] as const;
type Mode = (typeof MODES)[number];

const cssRgb = ([r, g, b]: Rgb): string => `rgb(${r}, ${g}, ${b})`;

/** Opens every collapsed panel of the sidebar (and Pixel mode's timeline). */
async function openAllPanels(page: Page): Promise<void> {
  const closed = page.locator('section[aria-label] > header button[aria-expanded="false"]');
  while ((await closed.count()) > 0) await closed.first().click();
}

/** The panels of the sidebar, by their accessible names. */
async function panelNames(page: Page): Promise<string[]> {
  const names = await page
    .getByRole('complementary', { name: 'Panels' })
    .locator('section[aria-label]')
    .evaluateAll((els) => els.map((e) => e.getAttribute('aria-label') ?? ''));
  expect(names.length).toBeGreaterThan(1);
  return names;
}

/** Every drawn border side of `el` is the hairline token `border`; returns how many sides are drawn. */
async function expectHairlines(page: Page, el: Locator, label: string): Promise<number> {
  const want = cssRgb(rgbOf(await token(page, 'border')));
  const sides = await el.evaluate((e) => {
    const cs = getComputedStyle(e);
    return (['Top', 'Right', 'Bottom', 'Left'] as const)
      .filter((s) => parseFloat(cs[`border${s}Width`]) > 0 && cs[`border${s}Style`] !== 'none')
      .map((s) => cs[`border${s}Color`]);
  });
  for (const c of sides) expect(c, `${label} hairline`).toBe(want);
  return sides.length;
}

/** The surface of a dialog: the element itself, or its first child where the dialog element is the backdrop. */
function dialogSurface(dialog: Locator): Locator {
  return dialog.locator(
    'xpath=self::*[not(contains(@class,"inset-0"))] | self::*[contains(@class,"inset-0")]/*[1]',
  );
}

const radiusOf = (el: Locator): Promise<string> =>
  el.evaluate((e) => getComputedStyle(e).borderTopLeftRadius);

/** The dialogs AC-02 names besides Settings: how to open each, and a field of it where it has one. */
interface DialogCase {
  name: string | RegExp;
  open: (page: Page) => Promise<void>;
  field?: (dialog: Locator) => Locator;
}
function dialogCases(mode: Mode): DialogCase[] {
  return [
    {
      name: 'New document',
      open: (page) => menu(page, 'File', 'New…'),
      field: (d) => d.getByRole('spinbutton', { name: 'Width' }),
    },
    { name: 'About Pincel', open: (page) => menu(page, 'Help', 'About Pincel') },
    {
      name: 'Keyboard Shortcuts',
      open: (page) => menu(page, 'Help', 'Keyboard Shortcuts'),
      field: (d) => d.getByRole('searchbox', { name: 'Filter shortcuts' }),
    },
    // Pixel mode's PNG export asks for the scale; Image mode exports a PNG
    // without a dialog, so its export dialog is the JPEG one.
    mode === 'pixel'
      ? {
          name: 'Export PNG',
          open: (page) => menu(page, 'File', 'Export', 'PNG (current frame)'),
          field: (d) => d.getByRole('combobox', { name: 'Scale' }),
        }
      : { name: /JPEG/, open: (page) => menu(page, 'File', 'Export', 'JPEG…') },
    {
      name: 'Rename layer',
      open: (page) => menu(page, 'Layer', 'Rename…'),
      field: (d) => d.getByRole('textbox', { name: 'Name' }),
    },
  ];
}

/** Runs `check` with each dialog of `dialogCases` open, closing it afterwards. */
async function withEachDialog(
  page: Page,
  mode: Mode,
  check: (dialog: Locator, c: DialogCase) => Promise<void>,
): Promise<void> {
  for (const c of dialogCases(mode)) {
    await c.open(page);
    const dialog = page.getByRole('dialog', { name: c.name });
    await expect(dialog).toBeVisible();
    await check(dialog, c);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  }
}

/** Elements that still paint one of the former accent blues (AC-03). */
async function formerBlue(page: Page): Promise<string[]> {
  return page.evaluate(() => {
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
}

interface Clip {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The pixels of a page region as the screenshot shows them (one CSS px is one device px here). */
async function pixelsOf(
  page: Page,
  clip: Clip,
): Promise<{ width: number; height: number; at: (x: number, y: number) => Rgb }> {
  const png = await page.screenshot({ clip });
  const img = await page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    return { width: bitmap.width, height: bitmap.height, data: Array.from(data) };
  }, png.toString('base64'));
  return {
    width: img.width,
    height: img.height,
    at: (x, y) => {
      const i = (y * img.width + x) * 4;
      return [img.data[i] ?? 0, img.data[i + 1] ?? 0, img.data[i + 2] ?? 0];
    },
  };
}

const distance = (a: Rgb, b: Rgb): number =>
  Math.max(Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1]), Math.abs(a[2] - b[2]));

async function boxOf(el: Locator): Promise<Clip> {
  await el.scrollIntoViewIfNeeded();
  const box = await el.boundingBox();
  if (!box) throw new Error('element has no box');
  return box;
}

/** Moves keyboard focus to `el` the way a key does, so `:focus-visible` applies. */
async function keyFocus(page: Page, el: Locator): Promise<void> {
  await el.focus();
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Tab');
  await expect(el).toBeFocused();
}

/** `el` shows the focus ring: a solid outline in the `focus` token. A soft
 *  check, so one run names every control that misses the ring. */
async function expectRing(page: Page, el: Locator, label: string, width = '2px'): Promise<void> {
  const ring = await el.evaluate((e) => {
    const cs = getComputedStyle(e);
    return { color: cs.outlineColor, style: cs.outlineStyle, width: cs.outlineWidth };
  });
  expect.soft(ring, label).toEqual({
    color: cssRgb(rgbOf(await token(page, 'focus'))),
    style: 'solid',
    width,
  });
}

const layerRow = (page: Page, name: string): Locator =>
  page
    .getByRole('list', { name: 'Layer list' })
    .getByRole('listitem')
    .filter({
      has: page.getByRole('button', { name, exact: true }),
    });

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
    // The theme choice is a segmented control: the selected segment is raised on `surface`.
    await expectFill(
      page,
      settings(page).getByRole('radiogroup', { name: 'Theme' }).locator('label').filter({ hasText: 'Dark' }),
      'surface',
    );
  });

  test(`${mode}: nothing keeps the former accent blue (AC-03)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Control+,');
    expect(await formerBlue(page)).toEqual([]);
  });
}

test('the start screen keeps no former accent blue (AC-03)', async ({ page }) => {
  await gotoStart(page);
  expect(await formerBlue(page)).toEqual([]);
  await pickTheme(page, 'Light');
  expect(await formerBlue(page)).toEqual([]);
});

for (const mode of MODES) {
  for (const palette of PALETTES) {
    test(`${mode}, ${palette}: panels, bars, an open menu and the dialogs are drawn in the tokens (AC-02)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light') await pickTheme(page, 'Light');
      await openAllPanels(page);
      await expectFill(page, page.locator('body'), 'bg');

      // Bars and the sidebar: ground `surface`, hairlines `border`.
      const sidebar = page.getByRole('complementary', { name: 'Panels' });
      const bars: [string, Locator][] = [
        ['top bar', page.getByRole('banner')],
        ['tool rail', page.getByRole('toolbar', { name: 'Tools' })],
        ['tool options', page.getByRole('toolbar', { name: 'Tool options' })],
        ['status bar', page.getByRole('contentinfo')],
        ['sidebar', sidebar],
      ];
      if (mode === 'pixel') bars.push(['timeline', page.getByRole('region', { name: 'Timeline' })]);
      let hairlines = 0;
      for (const [label, bar] of bars) {
        await expectFill(page, bar, 'surface');
        hairlines += await expectHairlines(page, bar, label);
      }
      expect(hairlines).toBeGreaterThan(0);
      await expectText(page, page.getByRole('banner'), 'text');

      // Every panel, open: ground `surface`, body text `text`.
      for (const name of await panelNames(page)) {
        const panel = sidebar.locator(`section[aria-label="${name}"]`);
        await expect(panel.locator('header button[aria-expanded]')).toHaveAttribute(
          'aria-expanded',
          'true',
        );
        await expectFill(page, panel, 'surface');
        await expectText(page, panel, 'text');
      }
      await expectFill(
        page,
        sidebar.getByRole('textbox', { name: 'Foreground color hex value' }),
        'surface-raised',
      );

      // An open menu.
      await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
      const open = page.getByRole('menu', { name: 'Edit', exact: true });
      await expectFill(page, open, 'surface');
      expect(await expectHairlines(page, open, 'menu')).toBe(4);
      await expectText(page, open.getByRole('menuitem', { name: 'Copy', exact: true }), 'text');
      await page.keyboard.press('Escape');

      // The New, About, Keyboard Shortcuts, export and Rename dialogs.
      await withEachDialog(page, mode, async (dialog, c) => {
        const surface = dialogSurface(dialog);
        await expectFill(page, surface, 'surface');
        await expectText(page, surface, 'text');
        expect(await expectHairlines(page, surface, String(c.name))).toBe(4);
        if (c.field) await expectFill(page, c.field(dialog), 'surface-raised');
      });
    });

    test(`${mode}, ${palette}: the accent sits in its roles, and no dialog keeps the former blue (AC-03)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light') await pickTheme(page, 'Light');
      await openAllPanels(page);
      const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
      // One stroke, so the History panel has a current row.
      await canvas.click();
      const sidebar = page.getByRole('complementary', { name: 'Panels' });

      // Accent-coloured text: the mode badge of the top bar.
      const badge = page
        .getByRole('banner')
        .locator('span', { hasText: mode === 'pixel' ? /^Pixel$/ : /^Image$/ });
      await expectText(page, badge, 'accent-ink');

      // Selected rows: the layer, the history step, the frame.
      await layerRow(page, 'Layer 1').getByRole('button', { name: 'Layer 1', exact: true }).click();
      await page.mouse.move(0, 0);
      await expectFill(page, layerRow(page, 'Layer 1'), 'accent-subtle');
      await expectFill(
        page,
        page.getByRole('region', { name: 'History' }).getByRole('option', { selected: true }),
        'accent-subtle',
      );
      if (mode === 'pixel')
        await expectFill(
          page,
          page.getByRole('button', { name: 'Frame 1', exact: true }),
          'accent-subtle',
        );

      // A slider's thumb (at its maximum, so the thumb sits at the right end).
      const accent = rgbOf(await token(page, 'accent'));
      const slider = sidebar.getByRole('slider', { name: 'Foreground alpha' });
      await expect(slider).toHaveValue('255');
      const rail = await pixelsOf(page, await boxOf(slider));
      expect(
        distance(rail.at(rail.width - 7, Math.floor(rail.height / 2)), accent),
        'slider thumb',
      ).toBeLessThanOrEqual(2);

      // A progress fill. The only progress bar of the app sits in the desktop
      // app's ComfyUI install wizard and is the shared one (amigo-theme-controls
      // AC-08); a full bar of that kind put into the page shows the fill.
      await page.evaluate(() => {
        const bar = document.createElement('div');
        bar.id = 'theme-probe-progress';
        bar.className = 'am-progress';
        bar.append(document.createElement('i'));
        (bar.firstChild as HTMLElement).style.width = '100%';
        bar.style.cssText = 'position:fixed;left:8px;top:8px;width:120px;z-index:1000';
        document.body.append(bar);
      });
      const fill = await pixelsOf(page, await boxOf(page.locator('#theme-probe-progress')));
      expect(
        distance(fill.at(Math.floor(fill.width / 2), Math.floor(fill.height / 2)), accent),
        'progress fill',
      ).toBeLessThanOrEqual(2);
      await page.evaluate(() => document.getElementById('theme-probe-progress')?.remove());

      expect(await formerBlue(page)).toEqual([]);
      await withEachDialog(page, mode, async (dialog, c) => {
        expect(await formerBlue(page), String(c.name)).toEqual([]);
        if (c.name !== 'New document') return;
        // The primary button, and the chosen document type as a selected row.
        const create = dialog.getByRole('button', { name: 'Create' });
        await expectFill(page, create, 'accent-solid');
        await expectText(page, create, 'on-accent');
        await expectFill(page, dialog.getByRole('radio', { checked: true }), 'accent-subtle');
      });
    });
  }
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

  await themeChoice(page, 'Light').check({ force: true });
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

/** The four 1-px strips along the document's edge, corners left out. `inset` moves them into the document. */
function edgeStrips(box: Clip, inset: number, width: number): Clip[] {
  const x0 = Math.round(box.x);
  const y0 = Math.round(box.y);
  const w = Math.round(box.width);
  const h = Math.round(box.height);
  const m = 12;
  return [
    { x: x0 + inset, y: y0 + m, width, height: h - 2 * m },
    { x: x0 + w - inset - width, y: y0 + m, width, height: h - 2 * m },
    { x: x0 + m, y: y0 + inset, width: w - 2 * m, height: width },
    { x: x0 + m, y: y0 + h - inset - width, width: w - 2 * m, height: width },
  ];
}

for (const mode of MODES) {
  for (const palette of PALETTES) {
    test(`${mode}, ${palette}: the document's edge and a selection outline on it differ from the workspace (AC-09)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light') await pickTheme(page, 'Light');
      const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
      const box = await boxOf(canvas);
      const ground = rgbOf(await token(page, 'bg'));
      // The workspace next to the document is the palette's ground.
      const beside = await pixelsOf(page, {
        x: Math.round(box.x) - 24,
        y: Math.round(box.y + box.height / 2),
        width: 4,
        height: 4,
      });
      expect(distance(beside.at(1, 1), ground)).toBeLessThanOrEqual(1);

      // The document's edge: its outermost line of pixels. Pixel mode draws
      // a line around the document (one px outside the canvas), Image mode
      // the document's own first pixels.
      const edgeDiffers = async (label: string): Promise<void> => {
        for (const strip of edgeStrips(box, mode === 'pixel' ? -1 : 0, 1)) {
          const px = await pixelsOf(page, strip);
          for (let y = 0; y < px.height; y++)
            for (let x = 0; x < px.width; x++)
              expect(
                distance(px.at(x, y), ground),
                `${label}: edge pixel ${strip.x + x},${strip.y + y}`,
              ).toBeGreaterThanOrEqual(3);
        }
      };
      await edgeDiffers('empty document');

      // A selection outline along the document's edge, where it meets the
      // workspace. The marching ants move, so the outline is what changed
      // against the shot without a selection, not a whole-canvas comparison.
      const strips = edgeStrips(box, mode === 'pixel' ? 0 : -1, 3);
      const before = [];
      for (const strip of strips) before.push(await pixelsOf(page, strip));
      await menu(page, 'Select', 'Select All');
      for (const [i, strip] of strips.entries()) {
        const was = before[i]!;
        const now = await pixelsOf(page, strip);
        let outline = 0;
        let strong = 0;
        for (let y = 0; y < now.height; y++)
          for (let x = 0; x < now.width; x++) {
            if (distance(now.at(x, y), was.at(x, y)) < 3) continue;
            outline += 1;
            expect(
              distance(now.at(x, y), ground),
              `outline pixel ${strip.x + x},${strip.y + y}`,
            ).toBeGreaterThanOrEqual(3);
            if (contrast(now.at(x, y), ground) >= 3) strong += 1;
          }
        // The outline runs along the whole strip, and enough of it holds 3:1
        // against the workspace to read as a line (its other dashes are the
        // opposite colour).
        expect(outline, `outline along strip ${i}`).toBeGreaterThanOrEqual(
          Math.max(now.width, now.height),
        );
        expect(strong / outline, `outline contrast along strip ${i}`).toBeGreaterThanOrEqual(1 / 3);
      }
      await menu(page, 'Select', 'Deselect');

      // A white document (the hardest case on the light workspace).
      await setColor(page, 'fg', '#ffffff');
      await page
        .getByRole('toolbar', { name: 'Tools' })
        .getByRole('button', { name: /^Fill/ })
        .click();
      await canvas.click();
      const centre = await pixelsOf(page, {
        x: Math.round(box.x + box.width / 2) + 3,
        y: Math.round(box.y + box.height / 2) + 3,
        width: 2,
        height: 2,
      });
      expect(distance(centre.at(0, 0), [255, 255, 255])).toBeLessThanOrEqual(12);
      await edgeDiffers('white document');
    });
  }
}

/** The colour of a 1-px control outline (its top border) of `el`. */
async function outlineOf(el: Locator): Promise<Rgb> {
  const css = await el.evaluate((e) => getComputedStyle(e).borderTopColor);
  const m = /^rgb\((\d+), (\d+), (\d+)\)$/.exec(css);
  if (!m) throw new Error(`unexpected border colour ${css}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

for (const mode of MODES) {
  for (const palette of PALETTES) {
    test(`${mode}, ${palette}: text holds 4.5:1, icons and control outlines 3:1 on what is behind them (AC-10)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light') await pickTheme(page, 'Light');
      const check = async (el: Locator, label: string, floor = 4.5): Promise<void> => {
        const { text, fill } = await painted(el);
        expect(contrast(text, fill), `${palette}: ${label}`).toBeGreaterThanOrEqual(floor);
      };
      /** A control's outline against what is behind the control. */
      const checkOutline = async (el: Locator, label: string): Promise<void> => {
        // At rest: the shared field shows the focus ring instead of its outline.
        await el.blur();
        const { behind, fill } = await painted(el);
        // A checked toggle of the shared theme has no outline: its fill is the edge.
        const checked = await el.evaluate((e) => e instanceof HTMLInputElement && e.checked);
        expect(
          contrast(checked ? fill : await outlineOf(el), behind),
          `${palette}: ${label}`,
        ).toBeGreaterThanOrEqual(3);
      };
      const bar = page.getByRole('menubar');
      const sidebar = page.getByRole('complementary', { name: 'Panels' });
      const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
      await check(bar.getByRole('menuitem', { name: 'File', exact: true }), 'menu bar entry');
      await check(activeTool(page), 'active tool icon', 3);
      await check(sidebar.getByRole('button', { name: 'Add layer' }), 'icon button', 3);
      await check(
        page.getByRole('region', { name: 'Layers' }).locator('header button').first(),
        'panel title',
      );
      await check(
        page.getByRole('region', { name: 'Colors' }).getByText('Hex', { exact: true }),
        'panel label',
      );
      await check(page.locator('.text-pc-muted:visible').first(), 'secondary text');
      await check(page.locator('.text-pc-faint:visible').first(), 'faint text');
      await check(page.getByRole('contentinfo'), 'status bar');
      await check(
        sidebar.getByRole('textbox', { name: 'Foreground color hex value' }),
        'field text in a panel',
      );

      // A slider's rail is a control outline: 3:1 against its panel.
      const slider = sidebar.getByRole('slider', { name: 'Foreground alpha' });
      await expect(slider).toHaveValue('255');
      const rail = await pixelsOf(page, await boxOf(slider));
      expect(
        contrast(
          rail.at(Math.floor(rail.width / 2), Math.floor(rail.height / 2)),
          (await painted(slider)).fill,
        ),
        `${palette}: slider rail`,
      ).toBeGreaterThanOrEqual(3);
      await checkOutline(
        page.getByRole('toolbar', { name: 'Tool options' }).getByRole('checkbox').first(),
        'checkbox outline',
      );

      // A selected layer row, and a hovered one.
      await check(
        layerRow(page, 'Layer 1').getByRole('button', { name: 'Layer 1', exact: true }),
        'selected layer row',
      );
      await sidebar.getByRole('button', { name: 'Add layer' }).click();
      const other = layerRow(page, 'Layer 1');
      await other.hover();
      await check(other.getByRole('button', { name: 'Layer 1', exact: true }), 'hovered layer row');

      // A tooltip.
      await page.getByRole('button', { name: 'Zoom in' }).hover();
      const tip = page.locator('[data-tooltip]');
      await expect(tip).toBeVisible();
      await check(tip, 'tooltip');
      await check(tip.locator('.am-kbd'), 'tooltip keycap');
      await page.mouse.move(0, 0);

      // An open menu: an item, its shortcut, and the item under the pointer.
      await bar.getByRole('menuitem', { name: 'Edit', exact: true }).click();
      const copy = page
        .getByRole('menu', { name: 'Edit', exact: true })
        .getByRole('menuitem', { name: 'Copy', exact: true });
      await check(copy, 'menu item');
      await check(copy.locator('kbd'), 'menu shortcut');
      await copy.hover();
      await check(copy, 'hovered menu item');
      await check(copy.locator('kbd'), 'hovered menu shortcut');
      await page.keyboard.press('Escape');

      await page.keyboard.press('Control+,');
      await check(settings(page).getByRole('combobox', { name: 'Language' }), 'field text');
      await checkOutline(settings(page).getByRole('combobox', { name: 'Language' }), 'field outline');
      await check(settings(page).getByRole('button', { name: 'Close' }), 'default button');
      await settings(page).getByRole('button', { name: 'Close' }).click();

      await menu(page, 'File', 'New…');
      const dialog = page.getByRole('dialog', { name: 'New document' });
      await check(dialog.locator('.am-btn--primary').first(), 'primary button');
      await check(
        dialog.locator('.am-btn:not(.am-btn--primary)').first(),
        'default button in a dialog',
      );
      await checkOutline(dialog.getByRole('checkbox'), 'checkbox outline in a dialog');
      await page.keyboard.press('Escape');
      await expect(dialog).toBeHidden();

      // Ghost buttons and a placeholder: the Keyboard Shortcuts dialog.
      await menu(page, 'Help', 'Keyboard Shortcuts');
      const keys = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
      await check(keys.getByRole('button', { name: 'Change' }).first(), 'ghost button');
      const filter = keys.getByRole('searchbox', { name: 'Filter shortcuts' });
      await expect(filter).toHaveValue('');
      const placeholder = await filter.evaluate((e) => {
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 1;
        const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
        const field = getComputedStyle(e).backgroundColor;
        const cs = getComputedStyle(e, '::placeholder');
        // The placeholder's own opacity thins its colour over the field.
        ctx.fillStyle = field;
        ctx.fillRect(0, 0, 1, 1);
        ctx.globalAlpha = Number(cs.opacity);
        ctx.fillStyle = cs.color;
        ctx.fillRect(0, 0, 1, 1);
        const d = ctx.getImageData(0, 0, 1, 1).data;
        return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0] as [number, number, number];
      });
      expect(
        contrast(placeholder, (await painted(filter)).fill),
        `${palette}: placeholder`,
      ).toBeGreaterThanOrEqual(4.5);
      await page.keyboard.press('Escape');
      await expect(keys).toBeHidden();

      // Error text: a file that cannot be opened. Pixel mode reports it in
      // the shell's alert, Image mode in the notice of its status bar.
      await dragFiles(page.getByRole('main'), 'drop', [{ name: 'broken.png', bytes: [1, 2, 3] }]);
      await page
        .getByRole('dialog', { name: 'Dropped images' })
        .getByRole('button', { name: 'Open as document' })
        .click();
      if (mode === 'pixel') {
        const error = page.getByRole('alert').filter({ hasText: 'broken.png' });
        await expect(error).toBeVisible();
        await check(error.getByText(/broken\.png/).first(), 'error text');
        await error.getByRole('button').click();
      } else {
        const notice = page.getByRole('contentinfo').getByText(/^Could not open image/);
        await expect(notice).toBeVisible();
        await check(notice, 'error text');
      }

      // A danger button: closing a document with unsaved changes.
      await canvas.click();
      await page.getByRole('button', { name: /^Close (pincel\.aseprite|image\.ora)$/ }).click();
      const prompt = page.getByRole('alertdialog');
      const dontSave = prompt.locator('.am-btn--danger');
      await check(dontSave, 'danger button');
      await dontSave.hover();
      await check(dontSave, 'hovered danger button');
    });
  }
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

/** The radius token `--am-radius-<name>` as the page resolves it (`6px`). */
const radiusToken = (page: Page, name: 'sm' | 'md' | 'lg'): Promise<string> =>
  page.evaluate(
    (n) => getComputedStyle(document.documentElement).getPropertyValue(`--am-radius-${n}`).trim(),
    name,
  );

for (const mode of MODES) {
  test(`${mode}: radii come from the tokens (AC-12)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const sm = await radiusToken(page, 'sm');
    const md = await radiusToken(page, 'md');
    const lg = await radiusToken(page, 'lg');
    expect([sm, md, lg].every((r) => /^\d+px$/.test(r))).toBe(true);

    // A menu and a menu item, in the menu and in the menu bar.
    const edit = page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true });
    expect(await radiusOf(edit)).toBe(sm);
    await edit.click();
    const open = page.getByRole('menu', { name: 'Edit', exact: true });
    expect(await radiusOf(open)).toBe(md);
    expect(await radiusOf(open.getByRole('menuitem', { name: 'Copy', exact: true }))).toBe(sm);
    await page.keyboard.press('Escape');

    // A floating panel: the colour picker.
    await swatch(page, 'fg').click();
    const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
    expect(await radiusOf(picker)).toBe(md);
    expect(await radiusOf(picker.getByRole('textbox', { name: 'Hex color' }))).toBe(sm);
    await page.keyboard.press('Escape');
    await expect(picker).toHaveCount(0);

    // A small icon button outside the dialogs.
    expect(
      await radiusOf(page.getByRole('button', { name: 'Swap foreground and background colors' })),
    ).toBe(sm);

    // Dialogs, their buttons and fields.
    await page.keyboard.press('Control+,');
    expect(await radiusOf(settings(page).locator(':scope > div'))).toBe(lg);
    const close = settings(page).getByRole('button', { name: 'Close' });
    expect(await radiusOf(close)).toBe(sm);
    expect(await radiusOf(settings(page).getByRole('combobox', { name: 'Language' }))).toBe(sm);
    await close.click();
    await withEachDialog(page, mode, async (dialog, c) => {
      expect(await radiusOf(dialogSurface(dialog)), String(c.name)).toBe(lg);
      if (c.field) expect(await radiusOf(c.field(dialog)), `${String(c.name)} field`).toBe(sm);
      for (const button of await dialog.getByRole('button', { name: /^(Cancel|Close)$/ }).all())
        expect(await radiusOf(button), `${String(c.name)} button`).toBe(sm);
    });
  });

  test(`${mode}: keyboard focus shows the focus ring on buttons, fields, menu items, tabs and rows in both palettes (AC-12)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const sidebar = page.getByRole('complementary', { name: 'Panels' });
    const tools = page.getByRole('toolbar', { name: 'Tools' });
    for (const palette of PALETTES) {
      // Dark is the palette the app starts in.
      if (palette === 'Light') await pickTheme(page, 'Light');
      const at = (what: string): string => `${palette}: ${what}`;

      // A button and a field of a dialog.
      await page.keyboard.press('Control+,');
      const close = settings(page).getByRole('button', { name: 'Close' });
      await keyFocus(page, close);
      await expectRing(page, close, at('button'));
      const language = settings(page).getByRole('combobox', { name: 'Language' });
      await keyFocus(page, language);
      await expectRing(page, language, at('select'));
      await close.click();

      // A text field, the colour buttons and a slider of a panel.
      const hex = sidebar.getByRole('textbox', { name: 'Foreground color hex value' });
      await keyFocus(page, hex);
      await expectRing(page, hex, at('field'));
      for (const which of ['fg', 'bg'] as const) {
        await keyFocus(page, swatch(page, which));
        await expectRing(page, swatch(page, which), at(`${which} colour button`));
      }
      const alpha = sidebar.getByRole('slider', { name: 'Foreground alpha' });
      await keyFocus(page, alpha);
      await expectRing(page, alpha, at('slider'));

      // A row: the layer's row button.
      const row = layerRow(page, 'Layer 1').getByRole('button', { name: 'Layer 1', exact: true });
      await keyFocus(page, row);
      await expectRing(page, row, at('layer row'));

      // A row of a list: the History panel's current step.
      await openAllPanels(page);
      const history = page.getByRole('region', { name: 'History' }).getByRole('listbox');
      await keyFocus(page, history);
      await expectRing(page, history, at('history list'));
      await expectRing(page, history.getByRole('option').first(), at('history row'));

      // A tab.
      const tab = page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
      await keyFocus(page, tab);
      await expectRing(page, tab, at('tab'));

      // The sidebar's splitter.
      const splitter = page.getByRole('separator', { name: 'Sidebar width' });
      await keyFocus(page, splitter);
      await expectRing(page, splitter, at('splitter'));

      // A menu item, reached with the arrow keys.
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      await page.keyboard.press('ArrowDown');
      const item = page
        .getByRole('menu', { name: 'File', exact: true })
        .getByRole('menuitem', { name: 'New…', exact: true });
      await expect(item).toBeFocused();
      await expectRing(page, item, at('menu item'));
      // The focused item keeps its fill.
      await expectFill(page, item, 'hover');
      await page.keyboard.press('Escape');

      // A row of the palette's options menu.
      const options = sidebar.getByRole('button', { name: 'Palette options' });
      await options.click();
      // A key press, so the focus that follows counts as keyboard focus.
      await page.keyboard.press('Shift');
      const paletteRow = page
        .getByRole('menu', { name: 'Palette options' })
        .getByRole('menuitem')
        .first();
      await paletteRow.focus();
      await expect(paletteRow).toBeFocused();
      await expectRing(page, paletteRow, at('palette menu row'));
      await page.keyboard.press('Escape');

      // An entry of a tool group's flyout.
      await tools.getByRole('button', { name: /^Marquee/ }).focus();
      await page.keyboard.press('Shift+F10');
      const entry = page.getByRole('menu', { name: 'Marquee' }).getByRole('menuitemradio').first();
      await expect(entry).toBeFocused();
      await expectRing(page, entry, at('tool flyout entry'));
      await page.keyboard.press('Escape');

      // The colour picker's colour area and sliders.
      await swatch(page, 'fg').click();
      const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
      await expect(picker).toBeVisible();
      await page.keyboard.press('Shift');
      for (const slider of await picker.getByRole('slider').all()) {
        await slider.focus();
        await expect(slider).toBeFocused();
        await expectRing(page, slider, at('colour picker slider'));
      }
      await page.keyboard.press('Escape');
      await expect(picker).toHaveCount(0);
    }
  });
}

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
  await themeChoice(page, 'Light').check({ force: true });
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
