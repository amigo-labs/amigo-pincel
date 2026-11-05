import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu } from './helpers';
import {
  boxOf,
  dragBy,
  effectPanel,
  storedPos,
  storeLayout,
  titleBar,
  type Box,
  type Mode,
} from './effect-panel-helpers';

// Moving the effect panel (effect-panel AC-18 – AC-21): its title bar drags
// it with mouse, pen or touch and moves it with the arrow keys; the
// position, measured from the canvas area's top-right corner, is kept per
// mode and always shown inside the canvas area. The panel is a modal
// dialog whose Tab order wraps.

const canvasArea = (page: Page) => page.getByRole('main');

async function openInvert(page: Page): Promise<Locator> {
  await menu(page, 'Adjustments', 'Invert Colors');
  const panel = effectPanel(page, 'Invert Colors');
  await expect(panel).toBeVisible();
  return panel;
}

/** The panel's offset from the canvas area's top-right corner. */
async function offset(page: Page, panel: Locator): Promise<{ x: number; y: number }> {
  const area = await boxOf(canvasArea(page));
  const box = await boxOf(panel);
  return {
    x: Math.round(area.x + area.width - (box.x + box.width)),
    y: Math.round(box.y - area.y),
  };
}

/** Polls until the stored position of `mode` equals `pos` (±1 px). */
async function expectStored(page: Page, mode: Mode, pos: { x: number; y: number }): Promise<void> {
  await expect
    .poll(async () => {
      const p = (await storedPos(page, mode)) as { x?: number; y?: number } | undefined;
      return p && Math.abs((p.x ?? NaN) - pos.x) <= 1 && Math.abs((p.y ?? NaN) - pos.y) <= 1;
    })
    .toBe(true);
}

/** A CDP pointer drag on `target` with `pointerType`, from its left part. */
async function cdpDrag(
  page: Page,
  target: Locator,
  pointerType: 'pen' | 'touch',
  dx: number,
  dy: number,
  end: 'release' | 'cancel' = 'release',
): Promise<void> {
  const box = await boxOf(target);
  const x = box.x + 24;
  const y = box.y + box.height / 2;
  const cdp = await page.context().newCDPSession(page);
  if (pointerType === 'touch') {
    const at = (f: number) => [{ x: x + dx * f, y: y + dy * f, id: 1 }];
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: at(0) });
    for (const f of [0.25, 0.5, 0.75, 1]) {
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: at(f) });
    }
    await cdp.send('Input.dispatchTouchEvent', {
      type: end === 'cancel' ? 'touchCancel' : 'touchEnd',
      touchPoints: [],
    });
  } else {
    const send = (type: 'mousePressed' | 'mouseMoved' | 'mouseReleased', f: number) =>
      cdp.send('Input.dispatchMouseEvent', {
        type,
        x: x + dx * f,
        y: y + dy * f,
        button: 'left',
        buttons: type === 'mouseReleased' ? 0 : 1,
        clickCount: 1,
        pointerType,
        force: 0.5,
      });
    await send('mousePressed', 0);
    for (const f of [0.25, 0.5, 0.75, 1]) await send('mouseMoved', f);
    await send('mouseReleased', 1);
  }
  await cdp.detach();
}

const near = (a: Box, b: Box, dx: number, dy: number) =>
  Math.abs(a.x + dx - b.x) <= 1 && Math.abs(a.y + dy - b.y) <= 1;

test('the title bar drags the panel; the position is kept per mode (AC-18)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  let panel = await openInvert(page);
  const start = await boxOf(panel);

  // A press on × starts no drag (and released elsewhere, closes nothing).
  const close = panel.getByRole('button', { name: 'Close', exact: true });
  const closeBox = await boxOf(close);
  await page.mouse.move(closeBox.x + closeBox.width / 2, closeBox.y + closeBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(closeBox.x - 120, closeBox.y + 100, { steps: 4 });
  await page.mouse.up();
  await expect(panel).toBeVisible();
  expect(near(start, await boxOf(panel), 0, 0)).toBe(true);

  // The mouse drags it.
  await dragBy(page, titleBar(panel), -200, 120);
  const moved = await boxOf(panel);
  expect(near(start, moved, -200, 120)).toBe(true);
  const pos = await offset(page, panel);
  await expectStored(page, 'pixel', pos);

  // Kept for the next effect and across a reload.
  await panel.getByRole('button', { name: 'Cancel' }).click();
  panel = await openInvert(page);
  expect(await offset(page, panel)).toEqual(pos);
  await panel.getByRole('button', { name: 'Cancel' }).click();
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  panel = await openInvert(page);
  expect(await offset(page, panel)).toEqual(pos);

  // Image mode keeps its own position: still the default.
  await createDocument(page, 'image', { width: 32, height: 32 });
  panel = await openInvert(page);
  expect(await offset(page, panel)).toEqual({ x: 16, y: 16 });
});

test('a pen drag and a release outside the window keep the last position (AC-18)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const panel = await openInvert(page);
  const start = await boxOf(panel);
  await cdpDrag(page, titleBar(panel), 'pen', -150, 80);
  expect(near(start, await boxOf(panel), -150, 80)).toBe(true);
  await expectStored(page, 'image', await offset(page, panel));

  // Released beyond the window's right edge: flush with the canvas area's
  // right edge, and stored there.
  const bar = await boxOf(titleBar(panel));
  const x = bar.x + 24;
  const y = bar.y + bar.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 800, y, { steps: 6 });
  await page.mouse.move(1400, y, { steps: 2 });
  await page.mouse.up();
  expect((await offset(page, panel)).x).toBe(0);
  await expectStored(page, 'image', await offset(page, panel));
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  test('a touch drag moves the panel; a cancelled one stays where it got (AC-18)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const panel = await openInvert(page);
    const start = await boxOf(panel);
    await cdpDrag(page, titleBar(panel), 'touch', -100, 60);
    const moved = await boxOf(panel);
    expect(near(start, moved, -100, 60)).toBe(true);

    await cdpDrag(page, titleBar(panel), 'touch', -50, 40, 'cancel');
    expect(near(moved, await boxOf(panel), -50, 40)).toBe(true);
    await expectStored(page, 'pixel', await offset(page, panel));
  });
});

test('a broken or out-of-area stored position falls back silently (AC-19)', async ({ page }) => {
  await gotoStart(page);
  await storeLayout(page, 'pixel', { collapsed: {}, sidebar: true, effectPos: 'broken' });
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  let panel = await openInvert(page);
  expect(await offset(page, panel)).toEqual({ x: 16, y: 16 });
  await panel.getByRole('button', { name: 'Cancel' }).click();

  await storeLayout(page, 'pixel', {
    collapsed: {},
    sidebar: true,
    effectPos: { x: Number.NaN, y: 3 },
  });
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  panel = await openInvert(page);
  expect(await offset(page, panel)).toEqual({ x: 16, y: 16 });
  await panel.getByRole('button', { name: 'Cancel' }).click();

  // Far outside: shown at the nearest place inside (bottom-left corner),
  // the stored value stays.
  await storeLayout(page, 'pixel', {
    collapsed: {},
    sidebar: true,
    effectPos: { x: 5000, y: 5000 },
  });
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  panel = await openInvert(page);
  const area = await boxOf(canvasArea(page));
  const box = await boxOf(panel);
  expect(Math.round(box.x)).toBe(Math.round(area.x));
  expect(Math.round(box.y + box.height)).toBe(Math.round(area.y + area.height));
  await page.waitForTimeout(300);
  expect(await storedPos(page, 'pixel')).toEqual({ x: 5000, y: 5000 });
  await expect(page.getByRole('alert')).toHaveCount(0);

  // A drag past the edge stops at it.
  await dragBy(page, titleBar(panel), 0, 3000);
  expect(Math.round((await boxOf(panel)).y + (await boxOf(panel)).height)).toBe(
    Math.round(area.y + area.height),
  );
});

test('the arrow keys move the focused title bar by 16 px (AC-20)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  const panel = await openInvert(page);
  const bar = titleBar(panel);
  await bar.focus();
  await expect(bar).toBeFocused();
  expect(await offset(page, panel)).toEqual({ x: 16, y: 16 });

  await page.keyboard.press('ArrowLeft');
  expect(await offset(page, panel)).toEqual({ x: 32, y: 16 });
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  expect(await offset(page, panel)).toEqual({ x: 32, y: 48 });
  await expectStored(page, 'image', { x: 32, y: 48 });
  await page.keyboard.press('ArrowUp');
  expect(await offset(page, panel)).toEqual({ x: 32, y: 32 });

  // At the edge the panel stops.
  for (let i = 0; i < 3; i += 1) await page.keyboard.press('ArrowRight');
  expect(await offset(page, panel)).toEqual({ x: 0, y: 32 });
  await expectStored(page, 'image', { x: 0, y: 32 });
  // The document did not change (no shortcut ran).
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});

test('the panel is a modal dialog whose Tab order wraps (AC-21)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
  const panel = effectPanel(page, 'Gaussian Blur');
  await expect(panel).toBeVisible();
  await expect(panel).toHaveAttribute('aria-modal', 'true');

  const focusedInPanel = () =>
    panel.evaluate((el) => el.contains(document.activeElement) && document.activeElement !== el);
  const ok = panel.getByRole('button', { name: 'OK', exact: true });
  const bar = titleBar(panel);

  // Forward from the first control: every stop is in the panel, the title
  // bar comes round again after OK.
  await expect(panel.getByRole('slider', { name: 'Radius' })).toBeFocused();
  let sawBar = false;
  let sawOk = false;
  for (let i = 0; i < 12; i += 1) {
    await page.keyboard.press('Tab');
    expect(await focusedInPanel()).toBe(true);
    if (await ok.evaluate((el) => el === document.activeElement)) sawOk = true;
    if (sawOk && (await bar.evaluate((el) => el === document.activeElement))) sawBar = true;
  }
  expect(sawOk && sawBar).toBe(true);

  // Backward from the title bar wraps to OK.
  await bar.focus();
  await page.keyboard.press('Shift+Tab');
  await expect(ok).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(bar).toBeFocused();
});
