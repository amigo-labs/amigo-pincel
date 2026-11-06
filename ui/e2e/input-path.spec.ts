import { expect, test, type Locator, type Page } from '@playwright/test';
import { colorOf, createDocument, imageCanvas, pixelCanvas, setColor } from './helpers';

// Input path (docs/specs/performance-input-path.md): coalesced pointer
// samples reach the Pixel stroke, and a touch drag on either canvas never
// scrolls, zooms or reloads the page.

test.use({ hasTouch: true });

const N = 16;
type At = (px: number, py: number) => readonly [number, number];

/** Sprite pixel → page point on the Pixel canvas of an N×N sprite. */
async function pixelAt(page: Page): Promise<At> {
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return (px, py) =>
    [box.x + ((px + 0.5) * box.width) / N, box.y + ((py + 0.5) * box.height) / N] as const;
}

/** Sample sprite pixel (x, y) with the Pixel Eyedropper; `#000000` is transparent. */
async function pick(page: Page, at: At, x: number, y: number): Promise<string> {
  await setColor(page, 'fg', '#000000');
  await page.getByTitle(/Eyedropper/).click();
  await page.mouse.click(...at(x, y));
  return colorOf(page, 'fg');
}

test('pixel: a move event with several coalesced samples paints through every sample (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: N, height: N });
  const at = await pixelAt(page);
  await page.getByTitle('Pencil (B)').click();
  await setColor(page, 'fg', '#00ff00');
  await page.mouse.move(...at(1, 1));
  await page.mouse.down();
  // One `pointermove` at (12, 12) that carries the samples a fast pen
  // reports in between: a straight line from (1, 1) would miss them.
  // Playwright cannot synthesize coalesced events, so the event is built
  // in the page with its `getCoalescedEvents` answering the samples.
  const samples = [at(5, 1), at(5, 8), at(12, 8), at(12, 12)];
  await pixelCanvas(page).evaluate((canvas, pts) => {
    const init = (x: number, y: number): PointerEventInit => ({
      bubbles: true,
      cancelable: true,
      composed: true,
      pointerId: 1,
      pointerType: 'mouse',
      isPrimary: true,
      buttons: 1,
      clientX: x,
      clientY: y,
    });
    const coalesced = pts.map(([x, y]) => new PointerEvent('pointermove', init(x, y)));
    const last = pts[pts.length - 1];
    if (!last) throw new Error('no samples');
    const ev = new PointerEvent('pointermove', init(last[0], last[1]));
    Object.defineProperty(ev, 'getCoalescedEvents', { value: () => coalesced });
    canvas.dispatchEvent(ev);
  }, samples);
  await page.mouse.up();
  for (const [x, y] of [
    [1, 1],
    [3, 1],
    [5, 1],
    [5, 4],
    [5, 8],
    [9, 8],
    [12, 8],
    [12, 10],
    [12, 12],
  ] as const) {
    expect(await pick(page, at, x, y), `pixel (${x}, ${y})`).toBe('#00ff00');
  }
  // Off the samples' path: the straight jump (1, 1) → (12, 12) was not taken.
  expect(await pick(page, at, 6, 6), 'pixel (6, 6)').toBe('#000000');
});

/** `touch-action` of the element under `target`'s centre and its ancestors. */
async function touchActions(target: Locator): Promise<string[]> {
  const box = await target.boundingBox();
  if (!box) throw new Error('canvas not visible');
  return target.page().evaluate(
    ([x, y]) => {
      const out: string[] = [];
      for (let el = document.elementFromPoint(x, y); el; el = el.parentElement) {
        out.push(getComputedStyle(el).touchAction);
      }
      return out;
    },
    [box.x + box.width / 2, box.y + box.height / 2] as const,
  );
}

/** A one-finger touch drag across `target`'s middle row, from 20 % to 80 %. */
async function touchDrag(page: Page, target: Locator): Promise<void> {
  const box = await target.boundingBox();
  if (!box) throw new Error('canvas not visible');
  const y = box.y + box.height / 2;
  const at = (f: number) => [{ x: box.x + box.width * f, y, id: 1 }];
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: at(0.2) });
  for (const f of [0.3, 0.4, 0.5, 0.6, 0.7, 0.8]) {
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: at(f) });
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await cdp.detach();
}

/** Page scroll, pinch zoom and navigation state a touch gesture could change. */
async function pageState(page: Page) {
  return page.evaluate(() => ({
    scrollX: window.scrollX,
    scrollY: window.scrollY,
    scale: window.visualViewport?.scale ?? 1,
    navigations: performance.getEntriesByType('navigation').length,
    marker: (window as unknown as { __inputPathMarker?: boolean }).__inputPathMarker === true,
  }));
}

const MODES = {
  pixel: {
    canvas: pixelCanvas,
    brush: (page: Page) => page.getByTitle('Pencil (B)').click(),
  },
  image: { canvas: imageCanvas, brush: (page: Page) => page.keyboard.press('b') },
} as const;

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a touch drag on the canvas paints and never scrolls, zooms or reloads (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: N, height: N });
    const canvas = MODES[mode].canvas(page);
    // The browser hands the gesture to the page only when the element under
    // the finger (or an ancestor up to the scroller) disallows panning.
    expect(await touchActions(canvas)).toContain('none');
    await MODES[mode].brush(page);
    await setColor(page, 'fg', '#ff0000');
    await page.evaluate(() => {
      (window as unknown as { __inputPathMarker?: boolean }).__inputPathMarker = true;
    });
    const before = await pageState(page);
    await touchDrag(page, canvas);
    // A double tap is the browser's zoom gesture.
    await touchDrag(page, canvas);
    expect(await pageState(page)).toEqual(before);
    // The drag reached the tool as one continuous stroke: the middle of the
    // row is painted, which a cancelled (scrolled) pointer would not do.
    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas not visible');
    if (mode === 'pixel') {
      await setColor(page, 'fg', '#000000');
      await page.getByTitle(/Eyedropper/).click();
      await page.mouse.click(box.x + box.width * 0.5, box.y + box.height / 2);
    } else {
      await setColor(page, 'fg', '#000000');
      await page.keyboard.press('i');
      await page.mouse.click(box.x + box.width * 0.5, box.y + box.height / 2);
    }
    await expect.poll(() => colorOf(page, 'fg')).toBe('#ff0000');
  });
}
