import { expect, test, type CDPSession, type Page } from '@playwright/test';
import { createDocument } from './helpers';
import { box, check, mapping, marker, pointerStatus } from './view-aids-helpers';

// image-view-aids: the pointer position in the status bar and the ruler
// markers follow a hovering mouse or pen and a pressed mouse, pen or touch,
// and hide outside the canvas area (docs/specs/image-view-aids.md AC-04,
// AC-10, AC-24).

const DOC = { width: 200, height: 100 };

/** Screen point of image coordinate (x, y). */
async function at(page: Page, x: number, y: number): Promise<[number, number]> {
  const m = await mapping(page, DOC.width);
  return [m.x + x * m.z, m.y + y * m.z];
}

/** A 200 × 100 image with the rulers on, re-fitted beside them. */
async function rulersOn(page: Page): Promise<void> {
  await createDocument(page, 'image', DOC);
  await check(page, 'View', 'Rulers');
  await expect(page.getByTestId('ruler-v')).toBeVisible();
  // Wait for the re-fit beside the rulers before mapping coordinates.
  await expect
    .poll(async () => {
      const img = await box(page.getByLabel('Image canvas'));
      const v = await box(page.getByTestId('ruler-v'));
      return img.x > v.x + v.width;
    })
    .toBe(true);
}

/** Position and both markers show image pixel (x, y) at screen (sx, sy). */
async function expectShown(page: Page, text: string, sx: number, sy: number): Promise<void> {
  await expect(pointerStatus(page)).toHaveText(text);
  const mh = await box(marker(page, 'h'));
  const mv = await box(marker(page, 'v'));
  expect(Math.abs(mh.x - sx), 'horizontal marker').toBeLessThanOrEqual(1);
  expect(Math.abs(mv.y - sy), 'vertical marker').toBeLessThanOrEqual(1);
}

async function expectHidden(page: Page): Promise<void> {
  await expect(pointerStatus(page)).toHaveCount(0);
  await expect(marker(page, 'h')).toHaveCount(0);
  await expect(marker(page, 'v')).toHaveCount(0);
}

test('a hovering mouse: position and markers inside, beside and past the image, none outside (AC-04, AC-10, AC-24)', async ({
  page,
}) => {
  await rulersOn(page);
  await expectHidden(page);
  let [sx, sy] = await at(page, 37.5, 12.5);
  await page.mouse.move(sx, sy);
  await expectShown(page, '37, 12 px', sx, sy);
  // Left of and above the image: negative values.
  [sx, sy] = await at(page, -3.5, -2.5);
  await page.mouse.move(sx, sy);
  await expectShown(page, '-4, -3 px', sx, sy);
  // At and past the width / height.
  [sx, sy] = await at(page, 200.5, 100.5);
  await page.mouse.move(sx, sy);
  await expectShown(page, '200, 100 px', sx, sy);
  [sx, sy] = await at(page, 202.5, 0.5);
  await page.mouse.move(sx, sy);
  await expectShown(page, '202, 0 px', sx, sy);
  // Over a ruler, the ruler corner or the menu bar: outside the canvas area.
  for (const id of ['ruler-h', 'ruler-v', 'ruler-corner']) {
    const r = await box(page.getByTestId(id));
    await page.mouse.move(r.x + r.width / 2, r.y + r.height / 2);
    await expectHidden(page);
    await page.mouse.move(sx, sy);
    await expect(pointerStatus(page)).toHaveText('202, 0 px');
  }
  const bar = await box(page.getByRole('menubar'));
  await page.mouse.move(bar.x + bar.width / 2, bar.y + bar.height / 2);
  await expectHidden(page);
});

test('a pressed mouse follows the drag and hides when it leaves the canvas area (AC-04, AC-24)', async ({
  page,
}) => {
  await rulersOn(page);
  let [sx, sy] = await at(page, 20.5, 30.5);
  await page.mouse.move(sx, sy);
  await page.mouse.down();
  await expectShown(page, '20, 30 px', sx, sy);
  [sx, sy] = await at(page, 60.5, 40.5);
  await page.mouse.move(sx, sy, { steps: 3 });
  await expectShown(page, '60, 40 px', sx, sy);
  // Still pressed, over the horizontal ruler: outside the canvas area.
  const h = await box(page.getByTestId('ruler-h'));
  await page.mouse.move(sx, h.y + h.height / 2, { steps: 3 });
  await expectHidden(page);
  // Back inside, still pressed.
  await page.mouse.move(sx, sy, { steps: 3 });
  await expectShown(page, '60, 40 px', sx, sy);
  // Released outside: stays hidden.
  await page.mouse.move(sx, h.y + h.height / 2, { steps: 3 });
  await page.mouse.up();
  await expectHidden(page);
});

/** One CDP pen event at screen (x, y); `buttons` 1 while the tip is down. */
async function pen(
  cdp: CDPSession,
  type: 'mousePressed' | 'mouseMoved' | 'mouseReleased',
  [x, y]: [number, number],
  buttons: 0 | 1,
): Promise<void> {
  await cdp.send('Input.dispatchMouseEvent', {
    type,
    x,
    y,
    button: type === 'mouseMoved' && buttons === 0 ? 'none' : 'left',
    buttons,
    clickCount: type === 'mouseMoved' ? 0 : 1,
    pointerType: 'pen',
    force: buttons === 1 ? 0.5 : 0,
  });
}

test('a pen: hover shows, a pressed tip follows, leaving the canvas area hides (AC-04, AC-24)', async ({
  page,
}) => {
  await rulersOn(page);
  const cdp = await page.context().newCDPSession(page);
  const h = await box(page.getByTestId('ruler-h'));
  // Hover.
  let p = await at(page, 10.5, 50.5);
  await pen(cdp, 'mouseMoved', p, 0);
  await expectShown(page, '10, 50 px', p[0], p[1]);
  p = await at(page, 150.5, 70.5);
  await pen(cdp, 'mouseMoved', p, 0);
  await expectShown(page, '150, 70 px', p[0], p[1]);
  // Hovering off over the ruler.
  await pen(cdp, 'mouseMoved', [p[0], h.y + h.height / 2], 0);
  await expectHidden(page);
  // Tip down and dragged.
  p = await at(page, 30.5, 20.5);
  await pen(cdp, 'mouseMoved', p, 0);
  await pen(cdp, 'mousePressed', p, 1);
  await expectShown(page, '30, 20 px', p[0], p[1]);
  p = await at(page, 90.5, 60.5);
  await pen(cdp, 'mouseMoved', p, 1);
  await expectShown(page, '90, 60 px', p[0], p[1]);
  // Dragged off over the ruler with the tip down.
  await pen(cdp, 'mouseMoved', [p[0], h.y + h.height / 2], 1);
  await expectHidden(page);
  await pen(cdp, 'mouseReleased', [p[0], h.y + h.height / 2], 0);
  await expectHidden(page);
  await cdp.detach();
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  type Pt = [number, number];
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

  test('a touch shows while down, follows its first contact and hides on lift (AC-04, AC-24)', async ({
    page,
  }) => {
    await rulersOn(page);
    const cdp = await page.context().newCDPSession(page);
    // One finger: down shows, move follows, lift hides.
    let p = await at(page, 40.5, 30.5);
    await touch(cdp, 'touchStart', [p]);
    await expectShown(page, '40, 30 px', p[0], p[1]);
    p = await at(page, 70.5, 35.5);
    await touch(cdp, 'touchMove', [p]);
    await expectShown(page, '70, 35 px', p[0], p[1]);
    await touch(cdp, 'touchEnd', []);
    await expectHidden(page);

    // Two fingers: only the first contact counts.
    const first = await at(page, 50.5, 50.5);
    await touch(cdp, 'touchStart', [first]);
    await expect(pointerStatus(page)).toHaveText('50, 50 px');
    const second = await at(page, 150.5, 80.5);
    await touch(cdp, 'touchStart', [first, second]);
    await expect(pointerStatus(page)).toHaveText('50, 50 px');
    const moved = await at(page, 120.5, 20.5);
    await touch(cdp, 'touchMove', [first, moved]);
    await expect(pointerStatus(page)).toHaveText('50, 50 px');
    await touch(cdp, 'touchEnd', []);
    await expectHidden(page);
    await cdp.detach();
  });
});
