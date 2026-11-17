import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, pixelCanvas } from './helpers';

// Regression: wheel-zooming about a point beside the document pushed it out
// of the canvas area for good — zooming in over empty space moved it far
// off screen, and zooming out again at another spot never brought it back.
// The view now keeps part of the document in view after every zoom and pan.

type Mode = 'pixel' | 'image';
const DOC = { width: 64, height: 48 };
/** CSS px of the document that stay in view (or all of it when smaller). */
const KEEP = 48;

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
const stageOf = (page: Page, mode: Mode) => canvasOf(page, mode).locator('xpath=../..');

async function boxOf(target: Locator) {
  const box = await target.boundingBox();
  if (!box) throw new Error('element is not visible');
  return box;
}

/** How many CSS px of the document overlap the canvas area, per axis. */
async function overlap(page: Page, mode: Mode): Promise<{ x: number; y: number }> {
  const doc = await boxOf(canvasOf(page, mode));
  const stage = await boxOf(stageOf(page, mode));
  const along = (a0: number, a1: number, b0: number, b1: number) =>
    Math.max(0, Math.min(a1, b1) - Math.max(a0, b0));
  return {
    x: along(doc.x, doc.x + doc.width, stage.x, stage.x + stage.width),
    y: along(doc.y, doc.y + doc.height, stage.y, stage.y + stage.height),
  };
}

async function expectInView(page: Page, mode: Mode): Promise<void> {
  const doc = await boxOf(canvasOf(page, mode));
  const seen = await overlap(page, mode);
  expect(seen.x).toBeGreaterThanOrEqual(Math.min(KEEP, doc.width) - 1);
  expect(seen.y).toBeGreaterThanOrEqual(Math.min(KEEP, doc.height) - 1);
}

async function wheelAt(page: Page, x: number, y: number, deltaY: number, times: number) {
  await page.mouse.move(x, y);
  for (let i = 0; i < times; i++) {
    await page.mouse.wheel(0, deltaY);
    // One frame per step, so each zoom reads the box the last one left.
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => r(null))));
  }
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: zooming about a point beside the document keeps it in view`, async ({ page }) => {
    await createDocument(page, mode, DOC);
    const stage = await boxOf(stageOf(page, mode));
    // Zoom out about the canvas area's top-left corner (the document shrinks
    // into it), in about the bottom-right corner (empty space), then out
    // about the top-left corner again.
    await wheelAt(page, stage.x + 3, stage.y + 3, 300, 40);
    await expectInView(page, mode);
    await wheelAt(page, stage.x + stage.width - 3, stage.y + stage.height - 3, -300, 20);
    await expectInView(page, mode);
    await wheelAt(page, stage.x + 3, stage.y + 3, 300, 40);
    await expectInView(page, mode);
  });
}
