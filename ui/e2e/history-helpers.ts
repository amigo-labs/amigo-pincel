import { expect, type Locator, type Page } from '@playwright/test';
import { drag, pixelCanvas } from './helpers';

// Helpers for the History panel specs (docs/specs/history-panel.md).

/** The History panel of the shown editor. */
export function historyPanel(page: Page, title = 'History'): Locator {
  return page.getByRole('region', { name: title, exact: true }).filter({ visible: true });
}

/** Its step list. */
export function historyList(page: Page, title = 'History', label = 'History steps'): Locator {
  return historyPanel(page, title).getByRole('listbox', { name: label });
}

/** Its rows, start entry first. */
export function historyRows(page: Page): Locator {
  return historyList(page).getByRole('option');
}

/** Expands the History panel when it is collapsed. */
export async function openHistory(page: Page, title = 'History'): Promise<void> {
  const header = historyPanel(page, title).getByRole('button', { name: title, exact: true });
  if ((await header.getAttribute('aria-expanded')) === 'false') await header.click();
  await expect(header).toHaveAttribute('aria-expanded', 'true');
}

/** The row names, start entry first. */
export async function rowNames(page: Page): Promise<string[]> {
  return (await historyRows(page).allTextContents()).map((s) => s.trim());
}

/** Waits until the rows read `names`. */
export async function expectRows(page: Page, names: string[]): Promise<void> {
  await expect.poll(() => rowNames(page)).toEqual(names);
}

/** Waits until row `index` is the current state (and only it). */
export async function expectCurrent(page: Page, index: number): Promise<void> {
  await expect(historyRows(page).nth(index)).toHaveAttribute('aria-selected', 'true');
  await expect(historyList(page).getByRole('option', { selected: true })).toHaveCount(1);
}

/** One Pencil stroke across the Pixel canvas at height `y` (0–1). */
export async function pixelStroke(page: Page, y: number): Promise<void> {
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.1, y], [0.9, y]);
}

/** A PNG of `canvas`, once it stopped changing. */
export async function settledShot(canvas: Locator): Promise<Buffer> {
  let last = await canvas.screenshot();
  await expect
    .poll(async () => {
      const next = await canvas.screenshot();
      const same = next.equals(last);
      last = next;
      return same;
    })
    .toBe(true);
  return last;
}

/** Waits until `canvas` shows exactly `shot`. */
export async function expectShot(canvas: Locator, shot: Buffer): Promise<void> {
  await expect.poll(async () => (await canvas.screenshot()).equals(shot)).toBe(true);
}

/** `xs[i]`, which must exist. */
export function nth<T>(xs: readonly T[], i: number): T {
  const x = xs[i];
  if (x === undefined) throw new Error(`no element ${i}`);
  return x;
}
