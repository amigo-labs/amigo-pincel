import { expect, test, type CDPSession, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  pixelCanvas,
} from './helpers';

// Two-finger pinch and pan (docs/specs/pen-input.md AC-01 – AC-06): real
// (CDP) touch points; the view is read from the canvas's on-screen box.

test.use({ hasTouch: true, permissions: ['clipboard-read', 'clipboard-write'] });

type Mode = 'pixel' | 'image';
const DOC = { width: 64, height: 48 };

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

type Pt = [number, number];

/**
 * One CDP touch event; the points get ids `firstId`, `firstId + 1`, ….
 * `touchEnd` lifts the listed points, or every finger when none is listed.
 */
async function touch(
  cdp: CDPSession,
  type: 'touchStart' | 'touchMove' | 'touchEnd',
  points: Pt[],
  firstId = 1,
): Promise<void> {
  await cdp.send('Input.dispatchTouchEvent', {
    type,
    touchPoints: points.map(([x, y], i) => ({ x, y, id: i + firstId })),
  });
}

async function boxOf(target: Locator) {
  const box = await target.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  return box;
}

/** Document coordinate under the client point `(x, y)`. */
async function docAt(target: Locator, [x, y]: Pt): Promise<Pt> {
  const box = await boxOf(target);
  return [((x - box.x) / box.width) * DOC.width, ((y - box.y) / box.height) * DOC.height];
}

/** Spread two real (CDP) touch points apart over `target`'s centre. */
async function pinchOut(page: Page, target: Locator): Promise<void> {
  const box = await boxOf(target);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  const cdp = await page.context().newCDPSession(page);
  const points = (spread: number): Pt[] => [
    [cx - spread, cy],
    [cx + spread, cy],
  ];
  await touch(cdp, 'touchStart', points(10));
  for (const spread of [20, 40, 60, 80]) await touch(cdp, 'touchMove', points(spread));
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
}

/** The zoom readout as a number (`8×` or `125%`). */
async function zoomOf(page: Page): Promise<number> {
  const text = await page.getByRole('button', { name: 'Fit to window' }).textContent();
  return Number(text?.replace(/[×%]/, ''));
}

test('pixel mode: two-finger pinch zooms in and paints nothing', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const readout = page.getByText(/^\d+×$/);
  const before = Number((await readout.textContent())?.replace('×', ''));
  await pinchOut(page, pixelCanvas(page));
  await expect
    .poll(async () => Number((await readout.textContent())?.replace('×', '')))
    .toBeGreaterThan(before);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});

test('image mode: two-finger pinch zooms in and paints nothing', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const readout = page.getByRole('button', { name: 'Fit to window' });
  const before = Number((await readout.textContent())?.replace('%', ''));
  await pinchOut(page, imageCanvas(page));
  await expect
    .poll(async () => Number((await readout.textContent())?.replace('%', '')))
    .toBeGreaterThan(before);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: two fingers pan by their midpoint and zoom about it`, async ({ page }) => {
    await createDocument(page, mode, DOC);
    const canvas = canvasOf(page, mode);
    const cdp = await page.context().newCDPSession(page);

    // Both fingers move together by (40, 24): the view pans by that much.
    const b0 = await boxOf(canvas);
    const zoom0 = await zoomOf(page);
    const cx = b0.x + b0.width / 2;
    const cy = b0.y + b0.height / 2;
    await touch(cdp, 'touchStart', [
      [cx - 30, cy],
      [cx + 30, cy],
    ]);
    for (const k of [1, 2, 3, 4]) {
      await touch(cdp, 'touchMove', [
        [cx - 30 + 10 * k, cy + 6 * k],
        [cx + 30 + 10 * k, cy + 6 * k],
      ]);
    }
    await touch(cdp, 'touchEnd', []);
    // The view redraws on the next frame: wait for the box to arrive.
    await expect.poll(async () => (await boxOf(canvas)).x - b0.x).toBeCloseTo(40, 0);
    const b1 = await boxOf(canvas);
    expect(await zoomOf(page)).toBe(zoom0);
    expect(b1.width).toBeCloseTo(b0.width, 0);
    expect(b1.y - b0.y).toBeCloseTo(24, 0);

    // Spreading the fingers to twice their distance about an off-centre
    // midpoint zooms in, and the document point under it stays put.
    const mid: Pt = [b1.x + b1.width * 0.3, b1.y + b1.height * 0.6];
    const before = await docAt(canvas, mid);
    const around = (spread: number): Pt[] => [
      [mid[0] - spread, mid[1]],
      [mid[0] + spread, mid[1]],
    ];
    await touch(cdp, 'touchStart', around(15));
    for (const spread of [20, 25, 30]) await touch(cdp, 'touchMove', around(spread));
    await touch(cdp, 'touchEnd', []);
    await expect.poll(async () => (await boxOf(canvas)).width).toBeGreaterThan(b1.width * 1.9);
    expect(await zoomOf(page)).toBeGreaterThan(zoom0 * 1.9);
    const after = await docAt(canvas, mid);
    expect(Math.abs(after[0] - before[0])).toBeLessThan(0.25);
    expect(Math.abs(after[1] - before[1])).toBeLessThan(0.25);
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    await cdp.detach();
  });

  test(`${mode} mode: the finger left after a pinch neither paints nor selects`, async ({
    page,
  }) => {
    await createDocument(page, mode, DOC);
    const canvas = canvasOf(page, mode);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const cdp = await page.context().newCDPSession(page);
    const b = await boxOf(canvas);
    const at = (fx: number, fy: number): Pt => [b.x + b.width * fx, b.y + b.height * fy];

    /** Pinch, lift the second finger, drag the first one on, lift it. */
    const pinchThenDrag = async () => {
      await touch(cdp, 'touchStart', [at(0.4, 0.5), at(0.6, 0.5)]);
      await touch(cdp, 'touchMove', [at(0.38, 0.5), at(0.62, 0.5)]);
      await touch(cdp, 'touchEnd', [at(0.62, 0.5)], 2);
      for (const fy of [0.3, 0.2, 0.1]) await touch(cdp, 'touchMove', [at(0.2, fy)]);
      await touch(cdp, 'touchEnd', []);
    };

    // Pencil: the remaining finger's drag paints nothing.
    await page.keyboard.press('b');
    await pinchThenDrag();
    await expect(undo).toBeDisabled();

    // Once every finger has lifted, one finger paints again.
    await touch(cdp, 'touchStart', [at(0.2, 0.2)]);
    for (const fx of [0.4, 0.6, 0.8]) await touch(cdp, 'touchMove', [at(fx, 0.2)]);
    await touch(cdp, 'touchEnd', []);
    await expect(undo).toBeEnabled();
    await undo.click();
    await expect(undo).toBeDisabled();

    // Rectangle select: the remaining finger's drag selects nothing, so
    // Copy still takes the whole layer.
    await page.keyboard.press('m');
    await pinchThenDrag();
    await clearClipboard(page);
    await page.keyboard.press('ControlOrMeta+c');
    const copied = await copiedImage(page);
    expect([copied.width, copied.height]).toEqual([DOC.width, DOC.height]);
    await cdp.detach();
  });

  test(`${mode} mode: a mouse and a pen down together never pinch`, async ({ page }) => {
    await createDocument(page, mode, DOC);
    const canvas = canvasOf(page, mode);
    await page.keyboard.press('b');
    const zoom0 = await zoomOf(page);
    const b = await boxOf(canvas);
    const y = b.y + b.height / 2;
    const cdp = await page.context().newCDPSession(page);
    const send = (
      type: 'mousePressed' | 'mouseMoved' | 'mouseReleased',
      x: number,
      pointerType: 'mouse' | 'pen',
    ) =>
      cdp.send('Input.dispatchMouseEvent', {
        type,
        x,
        y,
        button: 'left',
        buttons: type === 'mouseReleased' ? 0 : 1,
        clickCount: 1,
        pointerType,
        force: 1,
      });
    const cx = b.x + b.width / 2;
    // Mouse and pen down at once, then moving apart: a touch pinch would
    // roll back the first stroke and zoom by the growing distance.
    await send('mousePressed', cx - 40, 'mouse');
    await send('mousePressed', cx + 40, 'pen');
    for (const d of [50, 60, 80]) {
      await send('mouseMoved', cx - d, 'mouse');
      await send('mouseMoved', cx + d, 'pen');
    }
    await send('mouseReleased', cx + 80, 'pen');
    await send('mouseReleased', cx - 80, 'mouse');
    await cdp.detach();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
    expect(await zoomOf(page)).toBe(zoom0);
  });
}
