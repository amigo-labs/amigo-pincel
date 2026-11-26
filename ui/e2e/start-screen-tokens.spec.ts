import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { gotoStart } from './helpers';

const here = dirname(fileURLToPath(import.meta.url));

// docs/specs/start-screen-tokens.md — the start screen's New | Open columns
// and quiet footer; radii, text sizes and stacking levels as tokens.

async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

const newPixel = (page: Page) => page.getByRole('button', { name: 'New pixel sprite' });
const open = (page: Page) => page.getByRole('button', { name: /^Open…/ });
const footer = (page: Page) => page.locator('[data-start-footer]');
const header = (page: Page) => page.locator('#start-title').locator('xpath=../..');

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 800 } });

  test('New on the left, Open on the right, top edges level (AC-01)', async ({ page }) => {
    await gotoStart(page);
    const n = await box(newPixel(page));
    const o = await box(open(page));
    expect(o.x).toBeGreaterThan(n.x + n.width);
    expect(Math.abs(o.y - n.y)).toBeLessThanOrEqual(1);
    const sample = await box(page.getByRole('button', { name: /sample/i }).first());
    expect(sample.x).toBeGreaterThanOrEqual(o.x - 1);
  });

  test('the title row holds the title only; the utilities are quiet controls in the footer (AC-02)', async ({ page }) => {
    await gotoStart(page);
    await expect(header(page).getByRole('button')).toHaveCount(0);
    await expect(header(page).getByRole('combobox')).toHaveCount(0);
    for (const name of ['Tour', 'Settings', 'About Pincel']) {
      const b = footer(page).getByRole('button', { name, exact: true });
      await expect(b).toBeVisible();
      await expect(b).toHaveClass(/am-btn--ghost/);
    }
    const lang = footer(page).getByRole('combobox', { name: 'Language' });
    await expect(lang).toBeVisible();
    const f = await box(footer(page));
    const l = await box(lang);
    expect(f.x + f.width - (l.x + l.width)).toBeLessThanOrEqual(1);
    // The footer works: Settings opens.
    await footer(page).getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(page.getByRole('dialog', { name: 'Settings' })).toBeVisible();
  });

  test('the footer sits at the window’s bottom while the content is shorter (AC-03)', async ({ page }) => {
    await gotoStart(page);
    const f = await box(footer(page));
    expect(800 - (f.y + f.height)).toBeLessThanOrEqual(32);
    // Taller content: the footer follows it.
    await page.setViewportSize({ width: 1280, height: 400 });
    const g = await box(footer(page));
    const o = await box(page.getByRole('button', { name: /sample/i }).first());
    expect(g.y).toBeGreaterThan(o.y + o.height);
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 800, height: 900 } });

  test('below 960 px New comes first and Open below it, each full width (AC-04)', async ({ page }) => {
    await gotoStart(page);
    const n = await box(newPixel(page));
    const o = await box(open(page));
    expect(o.y).toBeGreaterThan(n.y + n.height);
    const section = await box(page.locator('section[aria-labelledby="start-title"]'));
    expect(o.width).toBeGreaterThan(section.width - 4);
  });
});

/** Every `.svelte` file under `dir`. */
function svelteFiles(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...svelteFiles(p));
    else if (name.endsWith('.svelte')) out.push(p);
  }
  return out;
}

test('no component writes a radius, font size or z-index as a literal (AC-05)', () => {
  const literal = /(?<![\w-])(rounded-\[[^\]]+\]|text-\[[\d.]+(px|rem)\]|z-\[\d+\]|z-\d+(?![\w-]))/;
  const offenders: string[] = [];
  for (const file of svelteFiles(join(here, '..', 'src'))) {
    readFileSync(file, 'utf8')
      .split('\n')
      .forEach((line, i) => {
        if (literal.test(line)) offenders.push(`${file}:${i + 1}: ${line.trim()}`);
      });
  }
  expect(offenders).toEqual([]);
});

test('the stacking tokens keep today’s values, lowest first (AC-05)', () => {
  const css = readFileSync(join(here, '..', 'src', 'app.css'), 'utf8');
  const levels = [...css.matchAll(/--pc-z-([a-z-]+):\s*(\d+);/g)].map((m) => [m[1], Number(m[2])] as const);
  expect(Object.fromEntries(levels)).toEqual({
    raised: 10,
    overlay: 20,
    'overlay-control': 21,
    drawer: 30,
    toast: 34,
    float: 35,
    'float-control': 36,
    menu: 40,
    'menu-lifted': 46,
    dialog: 50,
    popover: 60,
    'popover-control': 61,
    tooltip: 100,
  });
  const values = levels.map(([, v]) => v);
  expect([...values].sort((a, b) => a - b)).toEqual(values);
});
