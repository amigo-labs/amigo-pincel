import { expect, test, type Page } from '@playwright/test';
import { createDocument, imageCanvas } from './helpers';
import { expectRows, openHistory } from './history-helpers';

// Image mode presents each edit once: a brush frame draws the canvas right
// after the stroke reaches the core, and the reactive redraw for the same
// revision finds the picture current instead of drawing it a second time.
// The spec counts draws on the render canvas the way the performance probe
// does (e2e-perf/probe.ts); the app itself never reads canvas pixels.

type DrawWindow = Window & { __drawTarget: HTMLCanvasElement | null; __draws: number };

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as DrawWindow;
    w.__drawTarget = null;
    w.__draws = 0;
    const proto = CanvasRenderingContext2D.prototype;
    const original = proto.drawImage as (...args: unknown[]) => void;
    Object.defineProperty(proto, 'drawImage', {
      configurable: true,
      writable: true,
      value(this: CanvasRenderingContext2D, ...args: unknown[]) {
        original.apply(this, args);
        if (this.canvas === w.__drawTarget) w.__draws += 1;
      },
    });
  });
});

/** Aims the counter at the canvas the document is drawn on. */
async function countDraws(page: Page): Promise<void> {
  await imageCanvas(page).evaluate((surface) => {
    const viewport = surface.parentElement?.parentElement;
    const canvas = viewport?.querySelector(':scope > canvas');
    if (!(canvas instanceof HTMLCanvasElement)) throw new Error('no render canvas');
    (window as unknown as DrawWindow).__drawTarget = canvas;
  });
}

function draws(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as DrawWindow).__draws);
}

/** Resolves after two animation frames, once queued draws have run. */
function frames(page: Page): Promise<void> {
  return page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      }),
  );
}

test('image: each brush frame is drawn once, and an undo still redraws', async ({ page }) => {
  await createDocument(page, 'image', { width: 256, height: 256 });
  await openHistory(page);
  await page.keyboard.press('b');
  await countDraws(page);
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const y = box.y + box.height / 2;
  const moves = 20;
  await frames(page);
  const before = await draws(page);

  await page.mouse.move(box.x + box.width * 0.1, y);
  await page.mouse.down();
  for (let i = 1; i <= moves; i++) {
    await page.mouse.move(box.x + box.width * (0.1 + (0.8 * i) / moves), y);
    await frames(page);
  }
  await page.mouse.up();
  await frames(page);
  await expectRows(page, ['New', 'Brush']);
  const stroke = (await draws(page)) - before;
  // One draw per frame that changed the picture: the press dab and the
  // moves, plus at most the release; drawn twice it would be about 2×.
  expect(stroke, `${stroke} draws for ${moves} moves`).toBeGreaterThanOrEqual(moves);
  expect(stroke, `${stroke} draws for ${moves} moves`).toBeLessThanOrEqual(moves + 2);

  // Undo changes the picture without a tool drawing it: the next frame does.
  const afterStroke = await draws(page);
  await page.keyboard.press('ControlOrMeta+z');
  await expectRows(page, ['New', 'Brush']);
  await expect.poll(() => draws(page)).toBe(afterStroke + 1);
  await frames(page);
  expect(await draws(page)).toBe(afterStroke + 1);
});
