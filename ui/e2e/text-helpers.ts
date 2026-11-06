import { expect, type Locator, type Page } from '@playwright/test';
import { imageCanvas } from './helpers';

// Helpers for the Image-mode live text (docs/specs/image-live-text.md).
// Positions are in document pixels of a `W` × `H` image; screenshots are
// read test-side (the app never reads canvas pixels).

/** Document size the live-text specs use. */
export const W = 200;
export const H = 120;

/** The hidden field that takes the live text's keyboard input. */
export function liveInput(page: Page): Locator {
  return page.getByRole('textbox', { name: 'Live text', exact: true });
}

/** The dashed text box. */
export function textBox(page: Page): Locator {
  return page.locator('[data-text-box]');
}

/** The caret of the live text. */
export function caret(page: Page): Locator {
  return page.locator('[data-text-caret]');
}

/** The live text's move handle. */
export function moveHandle(page: Page): Locator {
  return page.locator('[data-text-handle]');
}

/** The highlight rects of the selected characters. */
export function selectionMarks(page: Page): Locator {
  return page.locator('[data-text-selection]');
}

export function undoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Undo', exact: true });
}

export function redoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Redo', exact: true });
}

/** Picks the Text tool from the rail. */
export async function chooseText(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Text', exact: true }).click();
}

/** Sets the Text tool's font size through the options bar slider. */
export async function setFontSize(page: Page, size: number): Promise<void> {
  const slider = page.getByRole('slider', { name: 'Font size', exact: true });
  await slider.evaluate((el, v) => {
    const input = el as HTMLInputElement;
    input.value = String(v);
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, size);
}

/** CSS px per document pixel. */
export async function zoomOf(page: Page): Promise<number> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return box.width / W;
}

/** Page coordinates of document point `(x, y)`. */
export async function at(page: Page, [x, y]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / W, box.y + (y * box.height) / H];
}

/** Document coordinates of page point `(x, y)`. */
export async function docOf(page: Page, [x, y]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [((x - box.x) * W) / box.width, ((y - box.y) * H) / box.height];
}

/** Clicks the primary button at document point `p`. */
export async function clickAt(page: Page, p: [number, number]): Promise<void> {
  const [x, y] = await at(page, p);
  await page.mouse.click(x, y);
}

/** Opens a box at `p` and types `text` into it. */
export async function typeAt(page: Page, p: [number, number], text: string): Promise<void> {
  await clickAt(page, p);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type(text);
}

/** A document-space rect, `x1` / `y1` exclusive. */
export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** A box locator's rect in document pixels. */
export async function docRect(page: Page, target: Locator): Promise<Rect> {
  const b = await target.boundingBox();
  if (!b) throw new Error('not visible');
  const [x0, y0] = await docOf(page, [b.x, b.y]);
  const [x1, y1] = await docOf(page, [b.x + b.width, b.y + b.height]);
  return { x0, y0, x1, y1 };
}

/** A screenshot of the canvas with the live text's box, caret and handle
 *  hidden: what the layer holds. */
export async function canvasShot(page: Page): Promise<Buffer> {
  return imageCanvas(page).screenshot({
    // Hiding the box keeps the text field visible: a hidden field would
    // lose keyboard focus.
    style: [
      '[data-text-box], [data-text-caret], [data-text-handle], [data-text-selection],',
      '[role="group"][aria-label="Zoom"] { visibility: hidden !important; }',
    ].join(' '),
  });
}

/**
 * The bounding rect, in document pixels, of the dark (ink) pixels of the
 * canvas inside the document rows `[rowFrom, rowTo)`; `null` when there is
 * none. Reads a `canvasShot`.
 */
export async function ink(page: Page, rowFrom = 0, rowTo = H): Promise<Rect | null> {
  const png = await canvasShot(page);
  return page.evaluate(
    async ({ b64, w, h, rowFrom, rowTo }) => {
      const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
      const sx = bitmap.width / w;
      const sy = bitmap.height / h;
      let r: { x0: number; y0: number; x1: number; y1: number } | null = null;
      for (let y = Math.max(0, rowFrom); y < Math.min(h, rowTo); y += 1) {
        const py = Math.min(bitmap.height - 1, Math.floor((y + 0.5) * sy));
        for (let x = 0; x < w; x += 1) {
          const px = Math.min(bitmap.width - 1, Math.floor((x + 0.5) * sx));
          const i = (py * bitmap.width + px) * 4;
          const sum = (data[i] ?? 255) + (data[i + 1] ?? 255) + (data[i + 2] ?? 255);
          if (sum >= 384) continue;
          r =
            r === null
              ? { x0: x, y0: y, x1: x + 1, y1: y + 1 }
              : {
                  x0: Math.min(r.x0, x),
                  y0: Math.min(r.y0, y),
                  x1: Math.max(r.x1, x + 1),
                  y1: Math.max(r.y1, y + 1),
                };
        }
      }
      return r;
    },
    { b64: png.toString('base64'), w: W, h: H, rowFrom, rowTo },
  );
}

/** Undoes until nothing is left and returns how many steps there were. */
export async function undoAll(page: Page): Promise<number> {
  let n = 0;
  while (await undoButton(page).isEnabled()) {
    await undoButton(page).click();
    n += 1;
    if (n > 50) throw new Error('undo does not end');
  }
  return n;
}
