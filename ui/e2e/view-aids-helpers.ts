import { expect, type Locator, type Page } from '@playwright/test';
import { imageCanvas, menu } from './helpers';

// Shared steps of the image-view-aids specs (docs/specs/image-view-aids.md):
// menu checkboxes, the zoom readout, the image → screen mapping and the
// ruler ticks read from the DOM.

export type Box = { x: number; y: number; width: number; height: number };

export async function box(l: Locator): Promise<Box> {
  const b = await l.boundingBox();
  if (!b) throw new Error('not visible');
  return b;
}

/** Opens View (and a submenu) and reads whether `name` is checked. */
export async function checked(
  page: Page,
  name: string,
  submenu?: string,
  top = 'View',
): Promise<boolean> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  if (submenu) await page.getByRole('menuitem', { name: submenu }).click();
  const state = await page
    .getByRole('menuitemcheckbox', { name, exact: true })
    .getAttribute('aria-checked');
  await page.keyboard.press('Escape');
  if (submenu) await page.keyboard.press('Escape');
  return state === 'true';
}

/** Chooses a checkbox entry of a top menu, through its submenus. */
export async function check(page: Page, top: string, ...path: string[]): Promise<void> {
  const last = path.pop() ?? '';
  await menu(page, top, ...path);
  await page
    .getByRole('menu', { name: path.at(-1) ?? top, exact: true })
    .getByRole('menuitemcheckbox', { name: last, exact: true })
    .click();
}

export const area = (page: Page): Locator => page.getByTestId('canvas-area');
export const pointerStatus = (page: Page): Locator => page.getByTestId('status-pointer');
export const marker = (page: Page, axis: 'h' | 'v'): Locator =>
  page.getByTestId(`ruler-${axis}-marker`);

/** Image coordinate → screen: the surface covers the image exactly. */
export async function mapping(
  page: Page,
  width: number,
): Promise<{ x: number; y: number; z: number }> {
  const b = await box(imageCanvas(page));
  return { x: b.x, y: b.y, z: b.width / width };
}

/** The zoom readout in percent. */
export async function zoomPercent(page: Page): Promise<number> {
  const text = (await page.getByRole('button', { name: 'Fit to window' }).textContent()) ?? '';
  return Number(text.replace('%', ''));
}

/** Zooms in with `+` (×1.25) until the zoom is at least `percent`. */
export async function zoomTo(page: Page, percent: number): Promise<void> {
  while ((await zoomPercent(page)) < percent) {
    const before = await zoomPercent(page);
    await page.keyboard.press('+');
    await expect.poll(() => zoomPercent(page)).not.toBe(before);
  }
}

/** One labelled tick as the page lays it out. */
export interface ShownTick {
  value: number;
  /** Screen x (horizontal ruler) or y (vertical ruler) of the tick. */
  tick: number;
  label: Box;
  text: string;
}

/** Every labelled tick of one ruler, read in one go. */
export async function shownTicks(page: Page, axis: 'h' | 'v'): Promise<ShownTick[]> {
  return page.evaluate((a) => {
    const ticks = [...document.querySelectorAll(`[data-testid="ruler-${a}-tick"]`)];
    const labels = [...document.querySelectorAll(`[data-testid="ruler-${a}-label"]`)];
    if (ticks.length !== labels.length) throw new Error('a tick without its label');
    return ticks.map((t, i) => {
      const r = t.getBoundingClientRect();
      const label = labels[i];
      if (!label) throw new Error('missing label');
      const l = label.getBoundingClientRect();
      return {
        value: Number(t.getAttribute('data-value')),
        tick: a === 'h' ? r.x : r.y,
        label: { x: l.x, y: l.y, width: l.width, height: l.height },
        text: label.textContent ?? '',
      };
    });
  }, axis);
}

/** How a ruler unit maps and prints: image pixels per unit, label text. */
export interface RulerUnit {
  readonly ppu: number;
  readonly format: (v: number) => string;
}

export const PIXELS: RulerUnit = { ppu: 1, format: (v) => String(v) };

/** Inches or centimeters at 96 DPI (or `dpi`), labels with two decimals. */
export function decimalUnit(unit: 'in' | 'cm', dpi = 96, comma = false): RulerUnit {
  return {
    ppu: unit === 'in' ? dpi : dpi / 2.54,
    format: (v) => {
      const s = v.toFixed(2).replace(/^-0\.00$/, '0.00');
      return comma ? s.replace('.', ',') : s;
    },
  };
}

/**
 * Both rulers: every tick within 1 px of where its value lies on screen,
 * its label 0–4 px past it and reading that value, steps of 1 / 2 / 5 × 10ⁿ
 * at least 40 px apart, labels never overlapping (AC-02, AC-03). Returns
 * the values of the horizontal ruler.
 */
export async function expectTicksAligned(
  page: Page,
  docWidth: number,
  unit: RulerUnit = PIXELS,
): Promise<number[]> {
  const m = await mapping(page, docWidth);
  let hValues: number[] = [];
  for (const axis of ['h', 'v'] as const) {
    const ticks = await shownTicks(page, axis);
    expect(ticks.length, axis).toBeGreaterThan(1);
    const step = (ticks[1]?.value ?? 0) - (ticks[0]?.value ?? 0);
    const mantissa = step / 10 ** Math.floor(Math.log10(step) + 1e-9);
    expect([1, 2, 5]).toContain(Number(mantissa.toFixed(6)));
    ticks.forEach((t, i) => {
      const at = (axis === 'h' ? m.x : m.y) + t.value * unit.ppu * m.z;
      expect(Math.abs(t.tick - at), `${axis} tick ${t.value}`).toBeLessThanOrEqual(1);
      const gap = axis === 'h' ? t.label.x - t.tick : t.label.y - t.tick;
      expect(gap).toBeGreaterThanOrEqual(0);
      expect(gap).toBeLessThanOrEqual(4);
      expect(t.text, `${axis} label at ${t.value}`).toBe(unit.format(t.value));
      const prev = ticks[i - 1];
      if (!prev) return;
      expect(t.value - prev.value).toBeCloseTo(step, 9);
      expect(t.tick - prev.tick).toBeGreaterThanOrEqual(40 - 1);
      expectApart(axis, prev.label, t.label);
    });
    if (axis === 'h') hValues = ticks.map((t) => t.value);
  }
  return hValues;
}

/** `b` starts past the end of `a` along the ruler's axis. */
export function expectApart(axis: 'h' | 'v', a: Box, b: Box): void {
  const end = axis === 'h' ? a.x + a.width : a.y + a.height;
  expect(axis === 'h' ? b.x : b.y, `labels overlap on ruler ${axis}`).toBeGreaterThan(end);
}
