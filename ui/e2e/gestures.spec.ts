import { expect, test, type CDPSession, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  imageCanvas,
  pixelCanvas,
  setColor,
  storedPref,
  switchLanguage,
} from './helpers';

// Tablet gestures (docs/specs/tablet-gestures.md): two- / three-finger taps
// undo / redo, and a pen's eraser end erases without changing the tool.

test.use({ hasTouch: true });

type Mode = 'pixel' | 'image';
type Pt = [number, number];

const canvasOf = (page: Page, mode: Mode): Locator =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page): Locator => page.getByRole('button', { name: 'Redo', exact: true });

async function touch(cdp: CDPSession, type: 'touchStart' | 'touchMove' | 'touchEnd', points: Pt[]) {
  await cdp.send('Input.dispatchTouchEvent', {
    type,
    touchPoints: points.map(([x, y], i) => ({ x, y, id: i + 1 })),
  });
}

/** Page points at fractional canvas positions. */
async function points(target: Locator, fs: Pt[]): Promise<Pt[]> {
  const b = await target.boundingBox();
  if (!b) throw new Error('canvas not visible');
  return fs.map(([fx, fy]) => [b.x + b.width * fx, b.y + b.height * fy]);
}

/** A quick tap with `n` fingers. */
async function tap(page: Page, target: Locator, n: 2 | 3): Promise<void> {
  const cdp = await page.context().newCDPSession(page);
  const pts = await points(target, [[0.3, 0.3], [0.6, 0.6], [0.4, 0.7]].slice(0, n) as Pt[]);
  await touch(cdp, 'touchStart', pts);
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
}

/** Two strokes, two undo steps. */
async function twoStrokes(page: Page, mode: Mode): Promise<void> {
  await createDocument(page, mode, { width: 32, height: 32 });
  await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.2]);
  await drag(page, canvasOf(page, mode), [0.2, 0.8], [0.8, 0.8]);
  await expect(undoButton(page)).toBeEnabled();
  await expect(redoButton(page)).toBeDisabled();
}

/** The color shown at a fractional canvas position (screen read). */
async function shown(page: Page, target: Locator, fx: number, fy: number): Promise<string> {
  const png = await target.screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const c = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = c.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const x = Math.min(bitmap.width - 1, Math.floor(bitmap.width * fx));
      const y = Math.min(bitmap.height - 1, Math.floor(bitmap.height * fy));
      const [r = 0, g = 0, b = 0] = ctx.getImageData(x, y, 1, 1).data;
      return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
    },
    { b64: png.toString('base64'), fx, fy },
  );
}

/** A pen stroke across the middle with the eraser end (button 5, bit 32 of
 *  `buttons`), dispatched as pointer events: CDP has no eraser button. The
 *  mouse's pointer id keeps pointer capture valid. */
async function eraserStroke(
  target: Locator,
  force = 0.8,
  opts: { button?: number; end?: 'pointerup' | 'pointercancel' } = {},
): Promise<void> {
  const { button = 5, end = 'pointerup' } = opts;
  await target.evaluate((el, [force, button, end]) => {
    const r = el.getBoundingClientRect();
    const at = (fx: number) => ({ clientX: r.left + r.width * fx, clientY: r.top + r.height * 0.52 });
    const fire = (type: string, fx: number, buttons: number) =>
      (document.elementFromPoint(at(fx).clientX, at(fx).clientY) ?? el).dispatchEvent(
        new PointerEvent(type, {
          ...at(fx),
          pointerId: 1,
          pointerType: 'pen',
          button: type === 'pointermove' ? -1 : (button as number),
          buttons,
          pressure: buttons ? (force as number) : 0,
          bubbles: true,
          cancelable: true,
          composed: true,
        }),
      );
    // Inside the sprite also in Pixel mode, whose canvas spans the stage.
    fire('pointerdown', 0.42, 32);
    for (const fx of [0.46, 0.5, 0.54, 0.58]) fire('pointermove', fx, 32);
    fire(end as string, 0.58, 0);
  }, [force, button, end] as const);
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a two-finger tap undoes, a three-finger tap redoes (AC-01, AC-02, AC-04, AC-06)`, async ({
    page,
  }) => {
    await twoStrokes(page, mode);
    const target = canvasOf(page, mode);
    await tap(page, target, 2);
    await expect(redoButton(page)).toBeEnabled();
    await expect(undoButton(page)).toBeEnabled();
    await tap(page, target, 3);
    await expect(redoButton(page)).toBeDisabled();
    // Undo both strokes, then a tap with nothing to undo changes nothing.
    await undoButton(page).click();
    await undoButton(page).click();
    await expect(undoButton(page)).toBeDisabled();
    await tap(page, target, 2);
    await expect(undoButton(page)).toBeDisabled();
    // No finger of a tap left a mark: redo is the only way back.
    await tap(page, target, 3);
    await expect(undoButton(page)).toBeEnabled();
  });

  test(`${mode}: a slow second finger or a pinch does not undo (AC-03)`, async ({ page }) => {
    await twoStrokes(page, mode);
    const target = canvasOf(page, mode);
    const cdp = await page.context().newCDPSession(page);
    const [a, b] = await points(target, [
      [0.3, 0.3],
      [0.6, 0.6],
    ]);
    if (!a || !b) throw new Error('points');
    await touch(cdp, 'touchStart', [a]);
    await page.waitForTimeout(220);
    await touch(cdp, 'touchStart', [a, b]);
    await touch(cdp, 'touchEnd', []);
    // A pinch: the fingers spread.
    await touch(cdp, 'touchStart', [a, b]);
    await touch(cdp, 'touchMove', [
      [a[0] - 30, a[1] - 30],
      [b[0] + 30, b[1] + 30],
    ]);
    await touch(cdp, 'touchEnd', []);
    await cdp.detach();
    // Both strokes are still there: two Undos empty the history.
    await undoButton(page).click();
    await expect(undoButton(page)).toBeEnabled();
    await undoButton(page).click();
    await expect(undoButton(page)).toBeDisabled();
  });

  test(`${mode}: a tap whose first finger began a stroke undoes that stroke and the one before (AC-05)`, async ({
    page,
  }) => {
    await twoStrokes(page, mode);
    const target = canvasOf(page, mode);
    const cdp = await page.context().newCDPSession(page);
    const [a, b] = await points(target, [
      [0.3, 0.5],
      [0.6, 0.6],
    ]);
    if (!a || !b) throw new Error('points');
    await touch(cdp, 'touchStart', [a]);
    await touch(cdp, 'touchMove', [[a[0] + 4, a[1]]]);
    await touch(cdp, 'touchStart', [[a[0] + 4, a[1]], b]);
    await touch(cdp, 'touchEnd', []);
    await cdp.detach();
    // Only stroke A is left: one more Undo empties the history.
    await expect(redoButton(page)).toBeEnabled();
    await undoButton(page).click();
    await expect(undoButton(page)).toBeDisabled();
  });

  test(`${mode}: the pen's eraser end erases one step and keeps the tool (AC-07, AC-08, AC-10)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await setColor(page, 'fg', '#ff0000');
    await page.keyboard.press('g');
    await canvasOf(page, mode).click();
    const target = canvasOf(page, mode);
    const filled = await shown(page, target, 0.5, 0.52);
    await page.keyboard.press('b');
    const rail = page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
    const toolBefore = await rail.getAttribute('aria-label');
    await eraserStroke(target);
    await expect.poll(() => shown(page, target, 0.5, 0.52)).not.toBe(filled);
    expect(await shown(page, target, 0.5, 0.3)).toBe(filled);
    expect(await rail.getAttribute('aria-label')).toBe(toolBefore);
    await undoButton(page).click();
    await expect.poll(() => shown(page, target, 0.5, 0.52)).toBe(filled);
  });
}

test('image: the eraser end follows pen pressure like the Eraser tool (AC-09)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await setColor(page, 'fg', '#ff0000');
  await page.keyboard.press('g');
  await imageCanvas(page).click();
  const target = imageCanvas(page);
  // Three pixels beside the stroke: inside the full-pressure brush only.
  const beside = () => shown(page, target, 0.5, 0.52 + 3 / 32);
  await expect.poll(beside).toBe('#ff0000');
  await page.keyboard.press('b');
  await eraserStroke(target, 0.05);
  await expect.poll(() => shown(page, target, 0.5, 0.52)).not.toBe('#ff0000');
  expect(await beside()).toBe('#ff0000');
  await undoButton(page).click();
  await eraserStroke(target, 1);
  await expect.poll(beside).not.toBe('#ff0000');
});

/** Settings ▸ Touch gestures (docs/specs/settings-dialog.md); set it to `on`
 *  unless `on` is undefined, and return the checkbox's state before. */
async function touchGestures(page: Page, on?: boolean): Promise<boolean> {
  await page.keyboard.press('Control+,');
  const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
  const box = dialog.getByRole('checkbox', { name: 'Touch gestures' });
  const before = await box.isChecked();
  if (on !== undefined) await box.setChecked(on);
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();
  return before;
}

test('Settings ▸ Touch gestures turns the taps off and is kept after a reload (AC-11 – AC-13)', async ({
  page,
}) => {
  await twoStrokes(page, 'image');
  expect(await touchGestures(page, false)).toBe(true);
  await tap(page, imageCanvas(page), 2);
  await expect(redoButton(page)).toBeDisabled();
  await expect.poll(() => storedPref(page, 'touchGestures')).toBe(false);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await touchGestures(page)).toBe(false);
});

test('the Touch gestures setting is German in the German UI (AC-14)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await switchLanguage(page, 'Deutsch');
  await page.keyboard.press('Control+,');
  await expect(
    page
      .getByRole('dialog', { name: 'Einstellungen', exact: true })
      .getByRole('checkbox', { name: 'Touch-Gesten' }),
  ).toBeVisible();
});

/** Two fingers that land together, wait `holdMs`, then lift. */
async function slowTap(page: Page, target: Locator, holdMs: number): Promise<void> {
  const cdp = await page.context().newCDPSession(page);
  const pts = await points(target, [
    [0.3, 0.3],
    [0.6, 0.6],
  ]);
  await touch(cdp, 'touchStart', pts);
  await page.waitForTimeout(holdMs);
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
}

test('a two-finger touch still down after 300 ms does not undo (AC-03)', async ({ page }) => {
  await twoStrokes(page, 'image');
  await slowTap(page, imageCanvas(page), 380);
  await expect(redoButton(page)).toBeDisabled();
});

test('an eraser reported only by bit 32 of `buttons` erases; a cancelled one keeps the tool (AC-07, AC-08)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await setColor(page, 'fg', '#ff0000');
  await page.keyboard.press('g');
  await imageCanvas(page).click();
  const target = imageCanvas(page);
  await expect.poll(() => shown(page, target, 0.5, 0.52)).toBe('#ff0000');
  await page.keyboard.press('b');
  const rail = page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
  const tool = await rail.getAttribute('aria-label');
  await eraserStroke(target, 0.8, { button: 0 });
  await expect.poll(() => shown(page, target, 0.5, 0.52)).not.toBe('#ff0000');
  await undoButton(page).click();
  await eraserStroke(target, 0.8, { end: 'pointercancel' });
  expect(await rail.getAttribute('aria-label')).toBe(tool);
  // The Brush still paints after the cancelled eraser stroke.
  await drag(page, target, [0.1, 0.1], [0.3, 0.1]);
  await expect(undoButton(page)).toBeEnabled();
});

test('with Touch Gestures off, pinch zoom and the eraser end keep working (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await touchGestures(page, false);
  const target = imageCanvas(page);
  const zoom = page.getByRole('button', { name: 'Fit to window' });
  const before = await zoom.textContent();
  const cdp = await page.context().newCDPSession(page);
  const [a, b] = await points(target, [
    [0.45, 0.5],
    [0.55, 0.5],
  ]);
  if (!a || !b) throw new Error('points');
  await touch(cdp, 'touchStart', [a, b]);
  for (let i = 1; i <= 5; i += 1) {
    await touch(cdp, 'touchMove', [
      [a[0] - 20 * i, a[1]],
      [b[0] + 20 * i, b[1]],
    ]);
  }
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
  await expect(zoom).not.toHaveText(before ?? '');

  await setColor(page, 'fg', '#ff0000');
  await page.getByRole('button', { name: 'Fit to window' }).click();
  await page.keyboard.press('g');
  await target.click();
  await expect.poll(() => shown(page, target, 0.5, 0.52)).toBe('#ff0000');
  await page.keyboard.press('b');
  await eraserStroke(target);
  await expect.poll(() => shown(page, target, 0.5, 0.52)).not.toBe('#ff0000');
});
