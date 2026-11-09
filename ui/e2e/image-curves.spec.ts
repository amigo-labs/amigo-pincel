import { expect, test, type CDPSession, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, setColor } from './helpers';

// The Line / Curve tool in the editor (docs/specs/image-shapes-curves.md):
// control points on a live line, caps, the new dash patterns, the move grip.

const handles = (page: Page): Locator => page.locator('[data-live-handle]');
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page): Locator => page.getByRole('button', { name: 'Redo', exact: true });
const options = (page: Page): Locator => page.getByRole('toolbar', { name: 'Tool options' });

/** Page coordinates of a fractional canvas position. */
async function at(page: Page, [fx, fy]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + box.width * fx, box.y + box.height * fy];
}

async function handleAt(page: Page, id: string): Promise<[number, number]> {
  const box = await page.locator(`[data-live-handle="${id}"]`).boundingBox();
  if (!box) throw new Error(`no handle ${id}`);
  return [box.x + box.width / 2, box.y + box.height / 2];
}

/** The color shown at a page point, from a screenshot of the canvas. */
async function shownAt(page: Page, [px, py]: [number, number]): Promise<string> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const bitmap = await createImageBitmap(
        await (await fetch(`data:image/png;base64,${b64}`)).blob(),
      );
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const x = Math.min(bitmap.width - 1, Math.floor(bitmap.width * fx));
      const y = Math.min(bitmap.height - 1, Math.floor(bitmap.height * fy));
      const [r = 0, g = 0, b = 0] = ctx.getImageData(x, y, 1, 1).data;
      return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
    },
    { b64: png.toString('base64'), fx: (px - box.x) / box.width, fy: (py - box.y) / box.height },
  );
}

async function mouseDrag(
  page: Page,
  from: [number, number],
  to: [number, number],
  shift = false,
): Promise<void> {
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  if (shift) await page.keyboard.down('Shift');
  await page.mouse.move(to[0], to[1], { steps: 6 });
  await page.mouse.up();
  if (shift) await page.keyboard.up('Shift');
}

/** A white 200 × 120 image with a live blue line, width 4 (or `width`),
 *  across it. */
async function liveLine(page: Page, width = 4): Promise<void> {
  await createDocument(page, 'image', { width: 200, height: 120 });
  await page.keyboard.press('l');
  await setColor(page, 'fg', '#0000ff');
  await options(page)
    .getByRole('slider', { name: /^Stroke/ })
    .fill(String(width));
  await drag(page, imageCanvas(page), [0.1, 0.7], [0.9, 0.7]);
  await expect(handles(page)).toHaveCount(3);
}

const BLUE = '#0000ff';

/** Whether `hex` is the line's blue or its anti-aliased edge. */
function blueish(hex: string): boolean {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return (b ?? 0) > 200 && (r ?? 255) < 120 && (g ?? 255) < 120;
}

/** CSS px per document pixel of the 200-wide document. */
async function zoomOf(page: Page): Promise<number> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return box.width / 200;
}

test('the options bar offers caps and five dash patterns; the tool reads Line / Curve (AC-17)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  await page.keyboard.press('l');
  const bar = options(page);
  for (const name of ['Start cap', 'End cap']) {
    const select = bar.getByRole('combobox', { name });
    await expect(select).toBeVisible();
    await expect(select.getByRole('option')).toHaveText(['Round', 'Flat', 'Arrow']);
  }
  await expect(bar.getByRole('combobox', { name: 'Dash' }).getByRole('option')).toHaveText([
    'Solid',
    'Dashed',
    'Dotted',
    'Dash-Dot',
    'Dash-Dot-Dot',
  ]);
  const rail = page.getByRole('toolbar', { name: 'Tools' });
  await expect(rail.getByRole('button', { name: /^Line \/ Curve/ })).toHaveCount(1);
  // Box shapes take the dash patterns, not the caps.
  await page.keyboard.press('u');
  await expect(bar.getByRole('combobox', { name: 'Dash' }).getByRole('option')).toHaveCount(5);
  await expect(bar.getByRole('combobox', { name: 'Start cap' })).toHaveCount(0);
  // The shortcut overview names the tool too, and German has every label.
  await menu(page, 'Help', 'Keyboard Shortcuts');
  const overview = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  await expect(
    overview.getByRole('rowheader', { name: 'Line / Curve', exact: true }),
  ).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await page.keyboard.press('l');
  const leiste = page.getByRole('toolbar', { name: 'Werkzeugoptionen' });
  await expect(
    leiste.getByRole('combobox', { name: 'Linienanfang' }).getByRole('option'),
  ).toHaveText(['Rund', 'Flach', 'Pfeil']);
  await expect(
    page
      .getByRole('toolbar', { name: 'Werkzeuge' })
      .getByRole('button', { name: /^Linie \/ Kurve/ }),
  ).toHaveCount(1);
});

test('a drag on the stroke adds a control point the curve passes through; a press alone adds nothing (AC-02)', async ({
  page,
}) => {
  await liveLine(page);
  const depth = async () => ((await undoButton(page).isEnabled()) ? 1 : 0);
  // A press on the stroke without movement: no point, no step, still live.
  const mid = await at(page, [0.5, 0.7]);
  await page.mouse.click(mid[0], mid[1]);
  await expect(handles(page)).toHaveCount(3);
  await undoButton(page).click(); // the line itself
  await expect(handles(page)).toHaveCount(0);
  await redoButton(page).click();
  await expect(handles(page)).toHaveCount(3);
  expect(await depth()).toBe(1);

  // A drag inserts a control point that follows the pointer.
  const target = await at(page, [0.5, 0.3]);
  await mouseDrag(page, mid, target);
  await expect(handles(page)).toHaveCount(4);
  const ctrl = await handleAt(page, 'ctrl0');
  expect(Math.abs(ctrl[0] - target[0])).toBeLessThan(2);
  expect(Math.abs(ctrl[1] - target[1])).toBeLessThan(2);
  // The curve passes through it (just below the handle square).
  expect(blueish(await shownAt(page, [ctrl[0], ctrl[1] + 6]))).toBe(true);
  // The whole drag is one step: Undo removes the point, the line stays live.
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(3);
  await expect(page.locator('[data-live-handle="ctrl0"]')).toHaveCount(0);
});

test('control points and endpoints drag; Shift snaps an end against its neighbour (AC-03)', async ({
  page,
}) => {
  await liveLine(page);
  await mouseDrag(page, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.3]));
  await expect(handles(page)).toHaveCount(4);
  // Drag the control point: it follows; one step.
  const to = await at(page, [0.35, 0.2]);
  await mouseDrag(page, await handleAt(page, 'ctrl0'), to);
  const ctrl = await handleAt(page, 'ctrl0');
  expect(Math.hypot(ctrl[0] - to[0], ctrl[1] - to[1])).toBeLessThan(2);
  expect(blueish(await shownAt(page, [ctrl[0], ctrl[1] + 6]))).toBe(true);
  await undoButton(page).click();
  const back = await handleAt(page, 'ctrl0');
  const was = await at(page, [0.5, 0.3]);
  expect(Math.hypot(back[0] - was[0], back[1] - was[1])).toBeLessThan(2);

  // Shift on the end snaps the segment from the control point to 45°.
  const c = await handleAt(page, 'ctrl0');
  const off: [number, number] = [c[0] + 120, c[1] + 100];
  await mouseDrag(page, await handleAt(page, 'end1'), off, true);
  const e = await handleAt(page, 'end1');
  const angle = (Math.atan2(e[1] - c[1], e[0] - c[0]) * 180) / Math.PI;
  expect(Math.abs(angle - 45)).toBeLessThan(1);
});

test('a double-click removes a control point as one step; on an endpoint it does nothing (AC-04)', async ({
  page,
}) => {
  await liveLine(page);
  await mouseDrag(page, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.3]));
  await expect(handles(page)).toHaveCount(4);
  const end = await handleAt(page, 'end0');
  await page.mouse.dblclick(end[0], end[1]);
  await expect(handles(page)).toHaveCount(4);
  expect(await handleAt(page, 'end0')).toEqual(end);

  const ctrl = await handleAt(page, 'ctrl0');
  await page.mouse.dblclick(ctrl[0], ctrl[1]);
  await expect(handles(page)).toHaveCount(3);
  // The curve is straight again.
  expect(blueish(await shownAt(page, await at(page, [0.5, 0.7])))).toBe(true);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(4);
  // That Undo took back the removal only: one more removes the point's drag.
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(3);
  await expect(undoButton(page)).toBeEnabled();
});

test('caps and dash changes show on the live line at once; the commit folds into one step (AC-12, AC-13)', async ({
  page,
}) => {
  await liveLine(page);
  const end = await handleAt(page, 'end1');
  // Arrow head at the end (16 px long, as wide as long): 12 px behind the
  // tip and 4.5 px beside the axis lies in the head, outside the 4 px stroke.
  const z = await zoomOf(page);
  const besideHead: [number, number] = [end[0] - 12 * z, end[1] - 4.5 * z];
  expect(await shownAt(page, besideHead)).not.toBe(BLUE);
  await options(page).getByRole('combobox', { name: 'End cap' }).selectOption('arrow');
  await expect.poll(() => shownAt(page, besideHead)).toBe(BLUE);
  await options(page).getByRole('combobox', { name: 'Dash' }).selectOption('dash_dot');
  await expect(handles(page)).toHaveCount(3);
  // The head stays solid on a dashed line.
  expect(await shownAt(page, besideHead)).toBe(BLUE);

  // Enter commits: no handles; one Undo restores the white canvas.
  await page.keyboard.press('Enter');
  await expect(handles(page)).toHaveCount(0);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(0);
  expect(await shownAt(page, besideHead)).not.toBe(BLUE);
  await expect(undoButton(page)).toBeDisabled();
  await redoButton(page).click();
  expect(await shownAt(page, besideHead)).toBe(BLUE);
  await expect(handles(page)).toHaveCount(0);

  // A new line, a control point, then Escape: nothing of it remains.
  await drag(page, imageCanvas(page), [0.1, 0.3], [0.9, 0.3]);
  await mouseDrag(page, await at(page, [0.5, 0.3]), await at(page, [0.5, 0.1]));
  await page.keyboard.press('Escape');
  await expect(handles(page)).toHaveCount(0);
  expect(await shownAt(page, await at(page, [0.3, 0.3]))).not.toBe(BLUE);
  await undoButton(page).click();
  expect(await shownAt(page, besideHead)).not.toBe(BLUE);
});

test('the grip sits beside the curve’s midpoint and moves the whole line as one step (AC-22)', async ({
  page,
}) => {
  await liveLine(page);
  const straightGrip = await handleAt(page, 'grip');
  await mouseDrag(page, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.15]));
  const grip = await handleAt(page, 'grip');
  const ctrl = await handleAt(page, 'ctrl0');
  // The grip follows the curve's midpoint (near the control point here),
  // not the straight segment's.
  expect(Math.hypot(grip[0] - ctrl[0], grip[1] - ctrl[1])).toBeLessThan(30);
  expect(Math.abs(grip[1] - straightGrip[1])).toBeGreaterThan(50);
  const before = await Promise.all(['end0', 'end1', 'ctrl0'].map((id) => handleAt(page, id)));
  await mouseDrag(page, grip, [grip[0] + 30, grip[1] + 20]);
  const after = await Promise.all(['end0', 'end1', 'ctrl0'].map((id) => handleAt(page, id)));
  after.forEach((p, i) => {
    expect(Math.abs(p[0] - before[i]![0] - 30)).toBeLessThan(2);
    expect(Math.abs(p[1] - before[i]![1] - 20)).toBeLessThan(2);
  });
  await undoButton(page).click();
  const undone = await Promise.all(['end0', 'end1', 'ctrl0'].map((id) => handleAt(page, id)));
  expect(undone).toEqual(before);
});

test('History names a committed curve after the Line tool', async ({ page }) => {
  await liveLine(page);
  await mouseDrag(page, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.3]));
  await page.keyboard.press('Enter');
  await menu(page, 'Edit', 'Fill Selection');
  const header = page
    .getByRole('region', { name: 'History', exact: true })
    .getByRole('button', { name: 'History', exact: true });
  if ((await header.getAttribute('aria-expanded')) === 'false') await header.click();
  await expect(
    page.getByRole('listbox', { name: 'History steps' }).getByRole('option').nth(1),
  ).toHaveText('Line');
});

test('a line with 64 points takes no more; moving points still works (AC-15)', async ({ page }) => {
  test.setTimeout(90_000);
  await createDocument(page, 'image', { width: 1200, height: 100 });
  await page.keyboard.press('l');
  await setColor(page, 'fg', '#0000ff');
  await drag(page, imageCanvas(page), [0.02, 0.5], [0.98, 0.5]);
  await expect(handles(page)).toHaveCount(3);
  const [x0, y] = await handleAt(page, 'end0');
  const [x1] = await handleAt(page, 'end1');
  const step = (x1 - x0 - 60) / 64;
  expect(step).toBeGreaterThan(10); // presses stay out of the handles' reach
  // 62 control points along the line, each nudged along it: the line stays
  // straight, every point a handle of its own.
  for (let i = 0; i < 62; i += 1) {
    const x = x0 + 30 + step * (i + 1);
    await mouseDrag(page, [x, y], [x + 3, y]);
  }
  await expect(handles(page)).toHaveCount(65);
  // The 64-point line: a drag on its stroke adds nothing, the line is as it was.
  const free = x0 + 30 + step * 63.5;
  await mouseDrag(page, [free, y], [free, y - 30]);
  await expect(handles(page)).toHaveCount(65);
  await expect(page.locator('[data-live-handle="ctrl62"]')).toHaveCount(0);
  // Moving a point still works.
  const c = await handleAt(page, 'ctrl10');
  await mouseDrag(page, c, [c[0], c[1] - 20]);
  expect((await handleAt(page, 'ctrl10'))[1]).toBeLessThan(c[1] - 15);
});

test('the stroke reaches 8 CSS px, or half the stroke width when wider (AC-02)', async ({
  page,
}) => {
  // A thin line: 8 CSS px.
  await liveLine(page, 1);
  const mid = await at(page, [0.5, 0.7]);
  await mouseDrag(page, [mid[0], mid[1] - 7], [mid[0], mid[1] - 40]);
  await expect(page.locator('[data-live-handle="ctrl0"]')).toHaveCount(1);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(3);
  // 10 CSS px off misses the stroke: a press there commits the line.
  await page.mouse.click(mid[0] + 40, mid[1] - 10);
  await expect(handles(page)).toHaveCount(0);

  // A wide line at this zoom: half its width is more than 8 CSS px.
  await liveLine(page, 4);
  const z = await zoomOf(page);
  const half = 2 * z;
  expect(half).toBeGreaterThan(8.5);
  const m = await at(page, [0.5, 0.7]);
  await mouseDrag(page, [m[0], m[1] - (half - 0.5)], [m[0], m[1] - 60]);
  await expect(page.locator('[data-live-handle="ctrl0"]')).toHaveCount(1);
});

test('a drag on the stroke between two points inserts the new point between them (AC-02)', async ({
  page,
}) => {
  await liveLine(page);
  // A point near the right end first …
  await mouseDrag(page, await at(page, [0.8, 0.7]), await at(page, [0.8, 0.4]));
  const right = await handleAt(page, 'ctrl0');
  // … then one on the stroke between the start and that point: the middle
  // of the blue run in the column at x = 0.25.
  const [cx, top] = await at(page, [0.25, 0.35]);
  const [, bottom] = await at(page, [0.25, 0.85]);
  const blue: number[] = [];
  for (let y = top; y <= bottom; y += 2) {
    if (blueish(await shownAt(page, [cx, y]))) blue.push(y);
  }
  expect(blue.length).toBeGreaterThan(2);
  const onCurve: [number, number] = [cx, blue[Math.floor(blue.length / 2)]!];
  const target = await at(page, [0.25, 0.95]);
  await mouseDrag(page, onCurve, target);
  await expect(handles(page)).toHaveCount(5);
  const first = await handleAt(page, 'ctrl0');
  const second = await handleAt(page, 'ctrl1');
  expect(Math.hypot(first[0] - target[0], first[1] - target[1])).toBeLessThan(2);
  expect(Math.hypot(second[0] - right[0], second[1] - right[1])).toBeLessThan(2);
  // The curve passes through both.
  expect(blueish(await shownAt(page, [first[0], first[1] + 6]))).toBe(true);
  expect(blueish(await shownAt(page, [second[0], second[1] + 6]))).toBe(true);
});

test('Shift does not snap a control point (AC-03)', async ({ page }) => {
  await liveLine(page);
  await mouseDrag(page, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.4]));
  const c = await handleAt(page, 'ctrl0');
  const to: [number, number] = [c[0] + 37, c[1] + 11];
  await mouseDrag(page, c, to, true);
  const moved = await handleAt(page, 'ctrl0');
  expect(Math.hypot(moved[0] - to[0], moved[1] - to[1])).toBeLessThan(2);
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  async function touchDrag(
    cdp: CDPSession,
    from: [number, number],
    to: [number, number],
  ): Promise<void> {
    const pt = ([x, y]: [number, number]) => [{ x, y, id: 1 }];
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: pt(from) });
    for (let i = 1; i <= 6; i += 1) {
      const p: [number, number] = [
        from[0] + ((to[0] - from[0]) * i) / 6,
        from[1] + ((to[1] - from[1]) * i) / 6,
      ];
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: pt(p) });
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  }

  async function tap(cdp: CDPSession, [x, y]: [number, number]): Promise<void> {
    await cdp.send('Input.dispatchTouchEvent', {
      type: 'touchStart',
      touchPoints: [{ x, y, id: 1 }],
    });
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  }

  test('a double-tap removes a control point as one step; on an endpoint it does nothing (AC-04)', async ({
    page,
  }) => {
    await liveLine(page);
    const cdp = await page.context().newCDPSession(page);
    await touchDrag(cdp, await at(page, [0.5, 0.7]), await at(page, [0.5, 0.35]));
    await expect(handles(page)).toHaveCount(4);
    const end = await handleAt(page, 'end0');
    await tap(cdp, end);
    await tap(cdp, end);
    await expect(handles(page)).toHaveCount(4);
    expect(await handleAt(page, 'end0')).toEqual(end);
    const ctrl = await handleAt(page, 'ctrl0');
    await tap(cdp, ctrl);
    await tap(cdp, ctrl);
    await expect(handles(page)).toHaveCount(3);
    await undoButton(page).click();
    await expect(handles(page)).toHaveCount(4);
  });

  test('a touch on the stroke reaches 16 CSS px (AC-02)', async ({ page }) => {
    await liveLine(page, 1);
    const cdp = await page.context().newCDPSession(page);
    const mid = await at(page, [0.5, 0.7]);
    await touchDrag(cdp, [mid[0], mid[1] - 14], [mid[0], mid[1] - 50]);
    await expect(page.locator('[data-live-handle="ctrl0"]')).toHaveCount(1);
  });
});
