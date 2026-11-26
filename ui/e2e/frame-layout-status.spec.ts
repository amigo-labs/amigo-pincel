import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, pixelCanvas, switchLanguage } from './helpers';
import { statusDrop } from '../src/lib/components/layout/status';

// docs/specs/frame-layout.md — the status bar both modes share, the zoom
// control with its menu, and the About dialog's version and backend.

const WIDE = { width: 1280, height: 800 };
const NARROW = { width: 390, height: 844 };

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

async function box(locator: Locator): Promise<Box> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

const status = (page: Page) => page.locator('[data-frame="status"]');
const statusText = (page: Page) => status(page).locator('[data-status-text]');
const position = (page: Page) => status(page).getByTestId('status-pointer');
const sizeItem = (page: Page) => status(page).locator('[data-status-size]');
const frameItem = (page: Page) => status(page).locator('[data-status-frame]');
const zoomGroup = (page: Page) => page.getByRole('group', { name: 'Zoom', exact: true });
const zoomValue = (page: Page) => zoomGroup(page).getByRole('button', { name: /^Zoom level / });
const zoomIn = (page: Page) => zoomGroup(page).getByRole('button', { name: 'Zoom in' });
const zoomOut = (page: Page) => zoomGroup(page).getByRole('button', { name: 'Zoom out' });
const zoomMenu = (page: Page) => page.getByRole('menu', { name: 'Zoom', exact: true });
const canvasOf = (page: Page, mode: 'pixel' | 'image') =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** The labels of the open zoom menu's entries, top to bottom. */
async function menuLabels(page: Page): Promise<string[]> {
  return zoomMenu(page)
    .locator('[role^="menuitem"] .am-menu-label')
    .evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim()));
}

test.describe('status bar drop rule (AC-07)', () => {
  test('everything fits: nothing is left out', () => {
    expect(statusDrop(600, 200, 80, 150)).toBe(0);
  });
  test('too narrow: the pointer position goes first, the probe readout next', () => {
    // 30 fixed + 200 tail + 92 position + 162 probe = 484.
    expect(statusDrop(483, 200, 80, 150)).toBe(1);
    // Without the position: 392.
    expect(statusDrop(392, 200, 80, 150)).toBe(1);
    expect(statusDrop(391, 200, 80, 150)).toBe(2);
  });
  test('absent items take no room', () => {
    expect(statusDrop(230, 200, 0, 0)).toBe(0);
    expect(statusDrop(322, 200, 80, 0)).toBe(0);
    expect(statusDrop(321, 200, 80, 0)).toBe(1);
  });
});

for (const mode of ['pixel', 'image'] as const) {
  test.describe(`${mode}, wide`, () => {
    test.use({ viewport: WIDE });

    test('text, size, frame and zoom in one order; the position comes and goes without moving them (AC-06, AC-07, AC-08, AC-11)', async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      const bar = status(page);
      // Nothing of the old, per-mode content.
      await expect(bar).not.toContainText(/canvas2d|webgpu|sRGB|undo|\bdev\b/i);
      await expect(sizeItem(page)).toHaveText(/^16 × 16 px$/);
      if (mode === 'pixel') await expect(frameItem(page)).toHaveText('Frame 1/1');
      else await expect(frameItem(page)).toHaveCount(0);

      // The text region: its tooltip is its full text.
      const text = (await statusText(page).textContent())?.trim() ?? '';
      expect(text.length).toBeGreaterThan(0);
      await expect(statusText(page)).toHaveAttribute('title', text);
      if (mode === 'image') expect(text).toBe('Brush');

      // Left to right: text, size, (frame,) zoom — the zoom control last,
      // inside the bar, none over the canvas.
      const order = [statusText(page), sizeItem(page)];
      if (mode === 'pixel') order.push(frameItem(page));
      order.push(zoomOut(page), zoomValue(page), zoomIn(page));
      let last = -1;
      for (const item of order) {
        const b = await box(item);
        expect(b.x).toBeGreaterThan(last);
        last = b.x;
      }
      const barBox = await box(bar);
      const zin = await box(zoomIn(page));
      expect(zin.x + zin.width).toBeGreaterThan(barBox.x + barBox.width - 16);
      await expect(page.locator('[data-stage] [data-zoom-control]')).toHaveCount(0);
      await expect(page.locator('[data-zoom-control]:visible')).toHaveCount(1);

      // No pointer over the canvas: no position.
      await page.mouse.move(5, 5);
      await expect(position(page)).toHaveCount(0);
      const before = [await box(sizeItem(page)), await box(zoomValue(page))];
      const canvas = await box(canvasOf(page, mode));
      await page.mouse.move(canvas.x + (5.5 / 16) * canvas.width, canvas.y + (3.5 / 16) * canvas.height);
      await expect(position(page)).toHaveText(mode === 'pixel' ? 'x 5, y 3' : /^5, 3 px$/);
      const pos = await box(position(page));
      expect(pos.x + pos.width).toBeLessThanOrEqual(before[0]!.x);
      const after = [await box(sizeItem(page)), await box(zoomValue(page))];
      expect(after[0]!.x).toBeCloseTo(before[0]!.x, 0);
      expect(after[1]!.x).toBeCloseTo(before[1]!.x, 0);

      // Off the canvas again: gone.
      await page.mouse.move(barBox.x + 20, barBox.y + barBox.height / 2);
      await expect(position(page)).toHaveCount(0);
    });

    test('the zoom value opens a menu of Fit to Window and the steps of the mode (AC-09, AC-10)', async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      const group = zoomGroup(page);
      await expect(group.getByRole('button')).toHaveCount(3);
      await expect(page.getByRole('button', { name: 'Actual size' })).toHaveCount(0);
      await expect(zoomIn(page)).toHaveAttribute('title', /^Zoom in \(\+\)$/);
      await expect(zoomOut(page)).toHaveAttribute('title', /^Zoom out \(-\)$/);

      // A click on the value changes no zoom: it opens the menu.
      const label = await zoomValue(page).textContent();
      await zoomValue(page).click();
      await expect(zoomMenu(page)).toBeVisible();
      await expect(zoomValue(page)).toHaveText(label ?? '');
      const steps =
        mode === 'pixel'
          ? ['1×', '2×', '4×', '8×', '16×', '32×', '64×']
          : ['25%', '50%', '100%', '200%', '400%', '800%', '1600%', '3200%', '6400%'];
      expect(await menuLabels(page)).toEqual(['Fit to Window', ...steps]);
      // Shortcut hints: Fit to Window, and Actual Size on the factor-1 entry.
      await expect(zoomMenu(page).getByRole('menuitem', { name: 'Fit to Window' }).locator('kbd')).toHaveText('0');
      const one = mode === 'pixel' ? '1×' : '100%';
      await expect(
        zoomMenu(page).getByRole('menuitemcheckbox', { name: one, exact: true }).locator('kbd'),
      ).toHaveText('1');

      // Choosing a step sets it, closes the menu, returns the focus.
      const four = mode === 'pixel' ? '4×' : '400%';
      await zoomMenu(page).getByRole('menuitemcheckbox', { name: four, exact: true }).click();
      await expect(zoomMenu(page)).toHaveCount(0);
      await expect(zoomValue(page)).toHaveText(four);
      await expect(zoomValue(page)).toBeFocused();

      // The step that equals the zoom is checked, no other.
      await zoomValue(page).click();
      const checked = zoomMenu(page).getByRole('menuitemcheckbox', { checked: true });
      await expect(checked).toHaveCount(1);
      await expect(checked).toHaveAccessibleName(four);

      // The zoom changes by shortcut while the menu is open: it stays open
      // and the check mark follows.
      await page.keyboard.press('1');
      await expect(zoomMenu(page)).toBeVisible();
      await expect(zoomValue(page)).toHaveText(one);
      await expect(zoomMenu(page).getByRole('menuitemcheckbox', { checked: true })).toHaveAccessibleName(one);

      // Escape: closed, nothing changed, focus back on the value.
      await page.keyboard.press('Escape');
      await expect(zoomMenu(page)).toHaveCount(0);
      await expect(zoomValue(page)).toHaveText(one);
      await expect(zoomValue(page)).toBeFocused();

      // A click outside closes it and does nothing else.
      await zoomValue(page).click();
      const canvas = await box(canvasOf(page, mode));
      await page.mouse.click(canvas.x + canvas.width / 2, canvas.y + canvas.height / 2);
      await expect(zoomMenu(page)).toHaveCount(0);
      await expect(zoomValue(page)).toHaveText(one);
      await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();

      // Fit to Window does what View ▸ Fit to Window does.
      await zoomValue(page).click();
      await zoomMenu(page).getByRole('menuitem', { name: 'Fit to Window' }).click();
      const fitted = await zoomValue(page).textContent();
      expect(fitted).not.toBe(one);
      await page.keyboard.press('1');
      await menu(page, 'View', 'Fit to Window');
      await expect(zoomValue(page)).toHaveText(fitted ?? '');
    });

    test('keyboard: Space opens, arrows move, Enter chooses; the limits disable (AC-09, AC-19)', async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await zoomValue(page).focus();
      await page.keyboard.press('Space');
      await expect(zoomMenu(page)).toBeVisible();
      const first = zoomMenu(page).getByRole('menuitem', { name: 'Fit to Window' });
      await expect(first).toBeFocused();
      // Down past Fit to Window onto the smallest step; End onto 64 / 6400.
      await page.keyboard.press('End');
      await page.keyboard.press('Enter');
      await expect(zoomMenu(page)).toHaveCount(0);
      await expect(zoomValue(page)).toHaveText(mode === 'pixel' ? '64×' : '6400%');
      await expect(zoomIn(page)).toBeDisabled();
      await expect(zoomOut(page)).toBeEnabled();

      await zoomValue(page).focus();
      await page.keyboard.press('Space');
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('Enter');
      const smallest = mode === 'pixel' ? '1×' : '25%';
      await expect(zoomValue(page)).toHaveText(smallest);
      if (mode === 'pixel') await expect(zoomOut(page)).toBeDisabled();

      // Zoom In by Space.
      await zoomIn(page).focus();
      await page.keyboard.press('Space');
      await expect(zoomValue(page)).not.toHaveText(smallest);
    });
  });
}

test.describe('wide', () => {
  test.use({ viewport: WIDE });

  test('image: a step from the menu keeps the centre of the canvas area in place (AC-10)', async ({ page }) => {
    await createDocument(page, 'image', { width: 400, height: 300 });
    const stage = await box(page.locator('[data-stage]'));
    // Rest the pointer on the centre of the canvas area, then pan away so
    // the centre is not the image's centre.
    const cx = stage.x + stage.width / 2;
    const cy = stage.y + stage.height / 2;
    await page.keyboard.press('1');
    await page.keyboard.down('Space');
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx + 60, cy + 40, { steps: 4 });
    await page.mouse.up();
    await page.keyboard.up('Space');
    await page.mouse.move(cx, cy);
    await expect(position(page)).toHaveText(/px$/);
    const before = await position(page).textContent();

    // By keyboard, so the pointer stays where it is.
    await zoomValue(page).focus();
    await page.keyboard.press('Space');
    await zoomMenu(page).getByRole('menuitemcheckbox', { name: '400%', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(zoomValue(page)).toHaveText('400%');
    // The pointer has not moved; a move away and back refreshes the readout.
    await page.mouse.move(cx + 3, cy);
    await page.mouse.move(cx, cy);
    await expect(position(page)).toHaveText(before ?? '');
  });

  test('pixel: a step from the menu keeps the centre of the canvas area in place (AC-10)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 64, height: 64 });
    const stage = await box(page.locator('[data-stage]'));
    const cx = stage.x + stage.width / 2;
    const cy = stage.y + stage.height / 2;
    await zoomValue(page).click();
    await zoomMenu(page).getByRole('menuitemcheckbox', { name: '4×', exact: true }).click();
    // Pan with Space-drag.
    await page.keyboard.down('Space');
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx + 42, cy + 26, { steps: 4 });
    await page.mouse.up();
    await page.keyboard.up('Space');
    await page.mouse.move(cx, cy);
    await expect(position(page)).toHaveText(/^x \d+, y \d+$/);
    const before = await position(page).textContent();

    await zoomValue(page).focus();
    await page.keyboard.press('Space');
    await zoomMenu(page).getByRole('menuitemcheckbox', { name: '8×', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(zoomValue(page)).toHaveText('8×');
    // The pointer has not moved; a move away and back refreshes the readout.
    await page.mouse.move(cx + 3, cy);
    await page.mouse.move(cx, cy);
    await expect(position(page)).toHaveText(before ?? '');
  });

  test('the zoom menu closes when the tab changes or the window becomes narrow (AC-10)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'New document', exact: true }).click();
    await page.getByRole('dialog', { name: 'New document' }).getByRole('button', { name: 'Create' }).click();
    await expect(page.getByRole('tab')).toHaveCount(2);
    await zoomValue(page).click();
    await expect(zoomMenu(page)).toBeVisible();
    await page.keyboard.press('Alt+PageDown');
    await expect(zoomMenu(page)).toHaveCount(0);

    await zoomValue(page).click();
    await expect(zoomMenu(page)).toBeVisible();
    await page.setViewportSize({ width: 600, height: 800 });
    await expect(zoomMenu(page)).toHaveCount(0);
    // The control now floats over the canvas (AC-11).
    await expect(page.locator('[data-stage] [data-zoom-control]:visible')).toHaveCount(1);
  });

  test('while an effect is open the zoom control works; the rest of the status bar, the tabs, New and search are inert (AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    const inert = (locator: Locator) => locator.evaluate((el) => el.closest('[inert]') !== null);
    expect(await inert(statusText(page))).toBe(true);
    expect(await inert(page.getByRole('tablist', { name: 'Open documents' }))).toBe(true);
    expect(await inert(page.locator('[data-search]'))).toBe(true);
    expect(await inert(page.locator('[data-zoom-control]:visible'))).toBe(false);
    expect(await page.locator('[data-zoom-control]:visible').evaluate((el) => el.closest('[data-dimmed]') !== null)).toBe(false);

    const before = await zoomValue(page).textContent();
    await zoomIn(page).click();
    await expect(zoomValue(page)).not.toHaveText(before ?? '');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    // The New button sits inside the inert tab bar.
    expect(await inert(page.getByRole('button', { name: 'New document', exact: true }))).toBe(true);
  });

  test('the About dialog shows the version, and the render backend for a Pixel tab only (AC-17)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await menu(page, 'Help', 'About Pincel');
    const about = page.getByRole('dialog', { name: 'About Pincel' });
    await expect(about).toContainText(/Version \S+/);
    await expect(about.locator('[data-about-renderer]')).toHaveText(/^Renderer: (canvas2d|webgpu)$/);
    await about.getByRole('button', { name: 'Close' }).click();

    // An Image tab: no backend.
    await page.getByRole('button', { name: 'New document', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'New document' });
    await dialog.getByRole('radio', { name: /Image/ }).click();
    await dialog.getByRole('button', { name: 'Create' }).click();
    await expect(page.getByRole('tab')).toHaveCount(2);
    await expect(imageCanvas(page)).toHaveAttribute('data-ready', 'true', { timeout: 30_000 });
    await menu(page, 'Help', 'About Pincel');
    await expect(about).toContainText(/Version \S+/);
    await expect(about.locator('[data-about-renderer]')).toHaveCount(0);
  });

  test('German: the zoom menu starts with "An Fenster anpassen" (AC-20)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    await page.getByRole('group', { name: 'Zoom', exact: true }).getByRole('button', { name: /^Zoomstufe / }).click();
    await expect(page.getByRole('menu', { name: 'Zoom', exact: true }).getByRole('menuitem').first()).toHaveText(
      /^An Fenster anpassen/,
    );
    await expect(status(page).getByTestId('status-pointer')).toHaveCount(0);
  });

  test('pixel: Enter chooses the focused zoom entry and starts no playback (AC-19)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await zoomValue(page).focus();
    await page.keyboard.press('Space');
    await zoomMenu(page).getByRole('menuitemcheckbox', { name: '2×', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(zoomValue(page)).toHaveText('2×');
    await expect(page.getByRole('button', { name: 'Pause' })).toHaveCount(0);
  });
});

test.describe('640 px', () => {
  test.use({ viewport: { width: 640, height: 800 } });

  test('the items right of the text region stay fully visible (AC-07)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const canvas = await box(pixelCanvas(page));
    await page.mouse.move(canvas.x + canvas.width / 2, canvas.y + canvas.height / 2);
    await expect(position(page)).toBeVisible();
    for (const item of [position(page), sizeItem(page), frameItem(page), zoomOut(page), zoomValue(page), zoomIn(page)]) {
      await expect(item).toBeInViewport({ ratio: 1 });
      const b = await box(item);
      expect(b.x + b.width).toBeLessThanOrEqual(640.5);
    }
  });
});

test.describe('narrow', () => {
  test.use({ viewport: NARROW });

  test('the zoom control floats over the canvas corner and stays clear of the panel sheet (AC-11)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const control = page.locator('[data-stage] [data-zoom-control]');
    await expect(control).toBeVisible();
    const stage = await box(page.locator('[data-stage]'));
    const c = await box(control);
    expect(c.x + c.width).toBeGreaterThan(stage.x + stage.width - 24);
    expect(c.y + c.height).toBeGreaterThan(stage.y + stage.height - 24);

    await page.getByRole('button', { name: 'Panels', exact: true }).click();
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await expect(sheet).toBeVisible();
    const s = await box(sheet);
    await expect
      .poll(async () => {
        const now = await box(control);
        return now.y + now.height;
      })
      .toBeLessThanOrEqual(s.y);
    // It can be operated: the menu opens and a step applies.
    await zoomValue(page).click();
    await zoomMenu(page).getByRole('menuitemcheckbox', { name: '2×', exact: true }).click();
    await expect(zoomValue(page)).toHaveText('2×');
  });
});
