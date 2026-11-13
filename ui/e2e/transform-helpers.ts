import { expect, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, setColor } from './helpers';

// Helpers for the Move tools' transform-handle specs
// (docs/specs/image-transform-handles.md).

/** The box outline (an SVG); `data-transform-box` is `idle` or `live`. */
export const box = (page: Page): Locator => page.locator('[data-transform-box]');

/** The handle `id` (`corner0` … `corner3`, `n`, `e`, `s`, `w`). */
export const handle = (page: Page, id: string): Locator =>
  page.locator(`[data-transform-handle="${id}"]`);

/** The pivot marker. */
export const pivot = (page: Page): Locator => page.locator('[data-transform-pivot]');

/** The options bar's position / size / angle readout. */
export const readout = (page: Page): Locator => page.locator('[data-transform-readout]');

/** The toolbar's Undo button. */
export const undo = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });

/** On the open Image document: a black stroke, its middle selected, and
 *  Move Selected Pixels picked. */
export async function paintScene(page: Page): Promise<void> {
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.3, 0.3], [0.7, 0.7]);
  await page.keyboard.press('v');
  await page.keyboard.press('v');
  await expect(page.getByRole('button', { name: 'Move: Move Selected Pixels' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
}

/** A 64 × 64 image with `paintScene`. */
export async function scene(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await paintScene(page);
}

/** The centre of `l` on the page. */
export async function centre(l: Locator): Promise<[number, number]> {
  const b = await l.boundingBox();
  if (!b) throw new Error('not visible');
  return [b.x + b.width / 2, b.y + b.height / 2];
}

/** Drags from `from` by `(dx, dy)` screen px with `button`, in steps. */
export async function dragBy(
  page: Page,
  from: [number, number],
  dx: number,
  dy: number,
  button: 'left' | 'right' = 'left',
): Promise<void> {
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down({ button });
  for (let i = 1; i <= 6; i++)
    await page.mouse.move(from[0] + (dx * i) / 6, from[1] + (dy * i) / 6);
  await page.mouse.up({ button });
}

type DiagnosticsWindow = Window & {
  __pincelDiagnostics?: { calls?: Record<string, number> };
};

/** How often the editor has sent a transform's box to the core. */
export const applied = (page: Page): Promise<number> =>
  page.evaluate(
    () => (window as DiagnosticsWindow).__pincelDiagnostics?.calls?.['image.applyTransform'] ?? 0,
  );

const BOX_PARTS = '[data-transform-box], [data-transform-handle], [data-transform-pivot]';

/** The canvas pixels: an element screenshot also captures what floats over
 *  it, so the selection's marching ants and the box are hidden for it. */
export const shot = (page: Page): Promise<Buffer> =>
  imageCanvas(page).screenshot({
    style: `[data-testid="selection-overlay"], ${BOX_PARTS} { visibility: hidden !important; }`,
  });

/** The selection outline alone: the overlay with every other canvas (the
 *  pixels and a drag's preview are drawn on a viewport-sized one) and the
 *  box hidden. Take it with reduced motion, so the marching ants stand
 *  still between two shots. */
export const outlineShot = (page: Page): Promise<Buffer> =>
  page.getByTestId('selection-overlay').screenshot({
    style: `canvas:not([data-testid="selection-overlay"]), ${BOX_PARTS} { visibility: hidden !important; }`,
  });

/** The angle the readout shows, in degrees. */
export async function readoutAngle(page: Page): Promise<number> {
  const text = (await readout(page).textContent()) ?? '';
  const m = /(-?\d+(?:\.\d+)?)°/.exec(text);
  if (!m?.[1]) throw new Error(`no angle in "${text}"`);
  return Number(m[1]);
}

/** The box outline's corners on the screen (top-left, top-right,
 *  bottom-right, bottom-left of the box). */
export async function boxCorners(page: Page): Promise<[number, number][]> {
  const points = (await box(page).locator('polygon').getAttribute('points')) ?? '';
  return points
    .trim()
    .split(/\s+/)
    .map((p) => {
      const [x, y] = p.split(',').map(Number);
      if (x === undefined || y === undefined) throw new Error(`bad point "${p}"`);
      return [x, y];
    });
}
