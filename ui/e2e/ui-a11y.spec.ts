import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';

// Accessibility floor of the shared theme (app.css): non-text contrast >= 3:1
// (WCAG 1.4.11), checked states visible in forced-colors mode, and options-bar
// hints that stay reachable by scrolling on phones.

interface Rgba {
  readonly r: number;
  readonly g: number;
  readonly b: number;
  readonly a: number;
}

const num = (v: string | undefined, fallback: number): number => (v === undefined ? fallback : Number(v));

/** Parses a computed color: `#rrggbb`, `rgb(…)`, `rgba(…)` or `color(srgb r g b / a)`. */
function parseColor(css: string): Rgba {
  const value = css.trim();
  const srgb = /^color\(srgb ([\d.e-]+) ([\d.e-]+) ([\d.e-]+)(?: \/ ([\d.e-]+))?\)$/.exec(value);
  if (srgb) {
    return { r: num(srgb[1], 0) * 255, g: num(srgb[2], 0) * 255, b: num(srgb[3], 0) * 255, a: num(srgb[4], 1) };
  }
  const rgb = /^rgba?\(([\d.]+),? ([\d.]+),? ([\d.]+)(?:,? \/? ?([\d.]+))?\)$/.exec(value);
  if (rgb) return { r: num(rgb[1], 0), g: num(rgb[2], 0), b: num(rgb[3], 0), a: num(rgb[4], 1) };
  const hex = /^#([0-9a-f]{6})$/i.exec(value);
  if (hex) {
    const n = parseInt(hex[1] ?? '0', 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255, a: 1 };
  }
  throw new Error(`unparsed color: ${css}`);
}

/** `top` composited over the opaque `bottom`. */
function over(top: Rgba, bottom: Rgba): Rgba {
  const mix = (t: number, u: number): number => t * top.a + u * (1 - top.a);
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 1 };
}

function luminance({ r, g, b }: Rgba): number {
  const lin = (v: number): number => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(x: Rgba, y: Rgba): number {
  const lx = luminance(x);
  const ly = luminance(y);
  return (Math.max(lx, ly) + 0.05) / (Math.min(lx, ly) + 0.05);
}

async function themeColor(page: Page, token: string): Promise<Rgba> {
  const value = await page.evaluate(
    (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim(),
    `--color-pc-${token}`,
  );
  return parseColor(value);
}

test('the slider rail reaches 3:1 against the panel', async ({ page }) => {
  await createDocument(page, 'pixel');
  const panel = await themeColor(page, 'panel');
  const track = await themeColor(page, 'track');
  expect(contrast(track, panel)).toBeGreaterThanOrEqual(3);
});

test('the current timeline frame border reaches 3:1 against panel and fill', async ({ page }) => {
  await createDocument(page, 'pixel');
  const panel = await themeColor(page, 'panel');
  const cell = page.locator('.frame-current').first();
  await expect(cell).toBeVisible();
  const { border, fill } = await cell.evaluate((el) => {
    const s = getComputedStyle(el);
    return { border: s.borderTopColor, fill: s.backgroundColor };
  });
  const filled = over(parseColor(fill), panel);
  const edge = over(parseColor(border), filled);
  expect(contrast(over(parseColor(border), panel), panel)).toBeGreaterThanOrEqual(3);
  expect(contrast(edge, filled)).toBeGreaterThanOrEqual(3);
});

test('checkboxes and radios fall back to native rendering in forced-colors mode', async ({ page }) => {
  await createDocument(page, 'pixel');
  await page.emulateMedia({ forcedColors: 'active' });
  const appearances = await page.evaluate(() =>
    ['checkbox', 'radio'].map((type) => {
      const input = document.createElement('input');
      input.type = type;
      input.checked = true;
      document.body.append(input);
      const value = getComputedStyle(input).appearance;
      input.remove();
      return value;
    }),
  );
  expect(appearances).toEqual(['auto', 'auto']);
});

test.describe('on a phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('options-bar hints are not clipped', async ({ page }) => {
    await createDocument(page, 'pixel');
    await page.getByTitle('Pencil (B)').click();
    const hints = page.locator('[role="toolbar"] .opt-hint');
    // Attached, not visible: a hint squeezed to zero width counts as hidden.
    await expect(hints.first()).toBeAttached();
    const clipped = await hints.evaluateAll((els) =>
      els.filter((el) => el.scrollWidth > el.clientWidth + 1).map((el) => el.textContent?.trim()),
    );
    expect(clipped).toEqual([]);
  });
});
