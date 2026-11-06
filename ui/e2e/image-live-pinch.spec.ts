import { expect, test, type CDPSession, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas } from './helpers';
import {
  H,
  W,
  at,
  caret,
  chooseText,
  docRect,
  ink,
  liveInput,
  setFontSize,
  textBox,
  typeAt,
  undoAll,
  zoomOf,
} from './text-helpers';

// A two-finger pinch while an Image-mode live object is open
// (docs/specs/image-live-text.md AC-12 and its pinch edge case,
// docs/specs/image-live-objects-ui.md): the first finger reaches the tool
// code as an ordinary press, so it must not commit the object, open a new
// text box or move the caret before it is clear that no second finger
// follows. Real (CDP) touch points, as in `pinch.spec.ts`.

test.use({ hasTouch: true });

type Pt = [number, number];

/** One CDP touch event; the points get ids 1, 2, …; `touchEnd` with no
 *  points lifts every finger. */
async function touch(
  cdp: CDPSession,
  type: 'touchStart' | 'touchMove' | 'touchEnd',
  points: Pt[],
): Promise<void> {
  await cdp.send('Input.dispatchTouchEvent', {
    type,
    touchPoints: points.map(([x, y], i) => ({ x, y, id: i + 1 })),
  });
}

/**
 * A pinch around document point `centre`: one finger lands first, the
 * second follows, both spread apart and lift. The fingers stay at least
 * `spread0` CSS px from the centre.
 */
async function pinchAround(page: Page, centre: Pt, spread0 = 12): Promise<void> {
  const [cx, cy] = await at(page, centre);
  const cdp = await page.context().newCDPSession(page);
  const points = (s: number): Pt[] => [
    [cx - s, cy],
    [cx + s, cy],
  ];
  const [first] = points(spread0);
  if (!first) throw new Error('no first finger');
  await touch(cdp, 'touchStart', [first]);
  await touch(cdp, 'touchStart', points(spread0));
  for (const s of [spread0 + 4, spread0 + 8, spread0 + 12]) {
    await touch(cdp, 'touchMove', points(s));
  }
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
}

/** A one-finger tap at document point `p`. */
async function tapAt(page: Page, p: Pt): Promise<void> {
  const [x, y] = await at(page, p);
  const cdp = await page.context().newCDPSession(page);
  await touch(cdp, 'touchStart', [[x, y]]);
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
}

test('a pinch beside the live text keeps it open with its caret', async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await chooseText(page);
  await setFontSize(page, 24);
  await typeAt(page, [20, 20], 'Hello');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  const box = await docRect(page, textBox(page));
  const at0 = await docRect(page, caret(page));
  const zoom0 = await zoomOf(page);

  // Both fingers well outside the box (it ends near x 80, y 50).
  await pinchAround(page, [140, 90]);
  await expect.poll(() => zoomOf(page)).toBeGreaterThan(zoom0);

  await expect(textBox(page)).toHaveCount(1);
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  const moved = await docRect(page, textBox(page));
  expect(moved.x0).toBeCloseTo(box.x0, 0);
  expect(moved.y0).toBeCloseTo(box.y0, 0);
  const at1 = await docRect(page, caret(page));
  expect((at1.x0 + at1.x1) / 2).toBeCloseTo((at0.x0 + at0.x1) / 2, 0);
  expect(at1.y0).toBeCloseTo(at0.y0, 0);

  // The pinch added no step: typing goes on at the caret as its own burst.
  await page.keyboard.type('X');
  // begin "H", the burst "ello", the burst "X".
  expect(await undoAll(page)).toBe(3);
});

test('a one-finger tap beside the live text commits it and focuses the new box', async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await chooseText(page);
  await setFontSize(page, 24);
  await typeAt(page, [20, 20], 'Hello');
  // The press waits for the finger to lift, then commits and opens a new
  // box there (AC-11).
  await tapAt(page, [120, 80]);
  await expect(textBox(page)).toHaveCount(1);
  const fresh = await docRect(page, textBox(page));
  expect(fresh.x0).toBeCloseTo(120, 0);
  expect(fresh.y0).toBeCloseTo(80, 0);
  // The tap's compatibility mousedown, after the lift, keeps the focus there.
  await expect(liveInput(page)).toBeFocused();
  expect(await ink(page)).not.toBeNull();
  // The commit is one step; the new box is none.
  expect(await undoAll(page)).toBe(1);
});

test('a pinch beside a live shape keeps it live; a touch drag beside it commits', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: W, height: H });
  await page.keyboard.press('u');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name: 'Rectangle', exact: true })
    .click();
  await drag(page, imageCanvas(page), [20 / W, 20 / H], [60 / W, 50 / H]);
  const handles = page.locator('[data-live-handle]');
  await expect(handles.first()).toBeVisible();
  const count = await handles.count();
  const zoom0 = await zoomOf(page);

  await pinchAround(page, [140, 90]);
  await expect.poll(() => zoomOf(page)).toBeGreaterThan(zoom0);
  // Still live, with every handle, and nothing to redo or undo beyond it.
  await expect(handles).toHaveCount(count);
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled();

  // A one-finger drag beside it commits it and drags out the next shape.
  const [cx, cy] = await at(page, [120, 70]);
  const [dx, dy] = await at(page, [150, 100]);
  const cdp = await page.context().newCDPSession(page);
  await touch(cdp, 'touchStart', [[cx, cy]]);
  for (const k of [1, 2, 3, 4]) {
    await touch(cdp, 'touchMove', [[cx + ((dx - cx) * k) / 4, cy + ((dy - cy) * k) / 4]]);
  }
  await touch(cdp, 'touchEnd', []);
  await cdp.detach();
  await expect(handles).toHaveCount(count);
  // The first shape's commit and the second, live shape.
  expect(await undoAll(page)).toBe(2);
});
