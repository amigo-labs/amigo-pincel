import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  colorOf,
  createDocument,
  drag,
  menu,
  pixelCanvas,
  setColor,
  switchLanguage,
  zoomActual,
  zoomReadout,
  byText,
} from './helpers';

// docs/specs/tool-rail-colors.md — the colour well at the rail's end, Pan
// and Zoom in Pixel mode, one Fill group with the Gradient on N, and Pixel
// mode's Shapes without the filled variants.

type Mode = 'pixel' | 'image';
const MODES: Mode[] = ['pixel', 'image'];

const editor = (page: Page) => page.locator('[data-editor]:not([inert])');
const rail = (page: Page) => editor(page).getByRole('toolbar', { name: 'Tools', exact: true });
const list = (page: Page) => rail(page).locator('[data-tool-list]');
const well = (page: Page) => rail(page).locator('[data-color-well]');
const wellButton = (page: Page, name: string) => well(page).getByRole('button', { name, exact: true });
const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const stage = (page: Page) => editor(page).locator('[data-stage]');
const undo = (page: Page) => editor(page).locator('[data-tour="quick"]').getByRole('button', { name: 'Undo', exact: true });
const pressed = (page: Page) => list(page).locator('button[aria-pressed="true"]');
/** The rail's group buttons, by their accessible names, in order. */
const railNames = (page: Page) =>
  list(page)
    .locator('button[data-group]')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
const groupIds = (page: Page) =>
  list(page)
    .locator('button[data-group]')
    .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.group ?? ''));
/** A swatch's colour as the browser computed it. */
const shown = (page: Page, name: string) =>
  wellButton(page, name).evaluate((el) => getComputedStyle(el).getPropertyValue('--swatch').trim());

async function centreOf(target: Locator, fx: number, fy: number): Promise<[number, number]> {
  const b = await box(target);
  return [b.x + b.width * fx, b.y + b.height * fy];
}

async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 720 } });

  for (const mode of MODES) {
    test(`${mode}: the well sits at the rail's foot below the tools, which all fit at 1280 × 720 (AC-01, AC-08)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      const r = await box(rail(page));
      const w = await box(well(page));
      const l = await box(list(page));
      expect(w.y).toBeGreaterThanOrEqual(l.y + l.height - 0.5);
      expect(w.y + w.height).toBeLessThanOrEqual(r.y + r.height + 0.5);
      expect(r.y + r.height - (w.y + w.height)).toBeLessThan(16);
      // No scrolling: every tool is in view.
      expect(await list(page).evaluate((el) => el.scrollHeight <= el.clientHeight)).toBe(true);
      // Foreground over background, swap and reset.
      for (const name of [
        'Foreground color',
        'Background color',
        'Swap foreground and background colors',
        'Reset to default colors',
      ]) {
        await expect(wellButton(page, name)).toBeVisible();
      }
      const fg = await box(wellButton(page, 'Foreground color'));
      const bg = await box(wellButton(page, 'Background color'));
      expect(fg.x).toBeLessThan(bg.x);
      expect(fg.y).toBeLessThan(bg.y);
      expect(fg.x + fg.width).toBeGreaterThan(bg.x);
    });

    test(`${mode}: the well follows the colours and its swatches open the picker beside the rail (AC-02, AC-03, AC-04)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      // A change made in the Colors panel shows in the well.
      const panelFg = sidebar(page).getByRole('button', { name: 'Foreground color', exact: true });
      await panelFg.click();
      const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
      await picker.getByRole('textbox', { name: 'Hex color' }).fill('#ff0000');
      await picker.getByRole('textbox', { name: 'Hex color' }).press('Enter');
      await page.keyboard.press('Escape');
      await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(255 0 0 / 1)');

      // The well's swatch opens the picker next to the rail, inside the window.
      await wellButton(page, 'Background color').click();
      const bgPicker = page.getByRole('dialog', { name: 'Background color picker' });
      await expect(bgPicker).toBeVisible();
      const p = await box(bgPicker);
      const r = await box(rail(page));
      expect(p.x).toBeGreaterThanOrEqual(r.x + r.width - 1);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.y + p.height).toBeLessThanOrEqual(720);
      await bgPicker.getByRole('textbox', { name: 'Hex color' }).fill('#00ff00');
      await bgPicker.getByRole('textbox', { name: 'Hex color' }).press('Enter');
      await page.keyboard.press('Escape');
      await expect(bgPicker).toHaveCount(0);
      await expect(wellButton(page, 'Background color')).toBeFocused();
      await expect.poll(() => shown(page, 'Background color')).toBe('rgb(0 255 0 / 1)');
      // … and the Colors panel shows the same colour.
      expect(
        await sidebar(page)
          .getByRole('button', { name: 'Background color', exact: true })
          .evaluate((el) => getComputedStyle(el).getPropertyValue('--swatch').trim()),
      ).toBe('rgb(0 255 0 / 1)');

      // Swap and reset, by the well's controls and by the keys.
      await wellButton(page, 'Swap foreground and background colors').click();
      await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(0 255 0 / 1)');
      await expect.poll(() => shown(page, 'Background color')).toBe('rgb(255 0 0 / 1)');
      await stage(page).click({ position: { x: 5, y: 5 } });
      await page.keyboard.press('x');
      await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(255 0 0 / 1)');
      await wellButton(page, 'Reset to default colors').click();
      await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(0 0 0 / 1)');
      await expect.poll(() => shown(page, 'Background color')).toBe('rgb(255 255 255 / 1)');
    });

    test(`${mode}: the well stays with the sidebar hidden, Colors collapsed or removed; Toggle UI takes it along (AC-05)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await sidebar(page).locator('section[aria-label="Colors"] > header button[aria-expanded]').click();
      await expect(well(page)).toBeVisible();
      await page.locator('[data-panels-toggle] button').click();
      await expect(sidebar(page)).toHaveCount(0);
      await setColor(page, 'fg', '#123456');
      await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(18 52 86 / 1)');
      await page.locator('[data-panels-toggle] button').click();
      // Colors removed in View ▸ Panels.
      await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
      await page.getByRole('menu', { name: 'View', exact: true }).getByRole('menuitem', { name: 'Panels' }).click();
      await page.getByRole('menu', { name: 'Panels', exact: true }).getByRole('menuitemcheckbox', { name: 'Colors' }).click();
      await expect(sidebar(page).locator('section[aria-label="Colors"]')).toHaveCount(0);
      await wellButton(page, 'Swap foreground and background colors').click();
      await expect.poll(() => shown(page, 'Background color')).toBe('rgb(18 52 86 / 1)');
      expect(await colorOf(page, 'bg')).toBe('#123456');
      await menu(page, 'View', 'Toggle UI');
      await expect(well(page)).toHaveCount(0);
      await menu(page, 'View', 'Toggle UI');
      await expect(well(page)).toBeVisible();
    });

    test(`${mode}: the well is named, shows the keys and follows the tools in the tab order (AC-07)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await expect(wellButton(page, 'Swap foreground and background colors')).toHaveAttribute('title', /\(X\)/);
      await expect(wellButton(page, 'Reset to default colors')).toHaveAttribute('title', /\(D\)/);
      // Every control of the well comes after every tool button in the document.
      const order = await rail(page)
        .locator('button')
        .evaluateAll((els) => els.map((el) => (el.closest('[data-color-well]') ? 'well' : 'tool')));
      expect(order.filter((x) => x === 'well')).toHaveLength(4);
      expect(order.lastIndexOf('tool')).toBeLessThan(order.indexOf('well'));
      await wellButton(page, 'Foreground color').focus();
      await expect(wellButton(page, 'Foreground color')).toBeFocused();
    });

    test(`${mode}: while an effect is open the well takes no input (AC-06)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await menu(page, 'Adjustments', 'Invert Colors');
      await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
      expect(await well(page).evaluate((el) => el.closest('[inert]') !== null)).toBe(true);
    });
  }

  test('pixel: the well follows the Eyedropper, a palette swatch and D (AC-02)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // Paint a pixel red, pick it back with the Eyedropper.
    await setColor(page, 'fg', '#ff0000');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('b');
    await drag(page, pixelCanvas(page), [0.5, 0.5], [0.52, 0.5]);
    await setColor(page, 'fg', '#00ff00');
    await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(0 255 0 / 1)');
    await page.keyboard.press('i');
    await page.mouse.click(...(await centreOf(pixelCanvas(page), 0.5, 0.5)));
    await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(255 0 0 / 1)');
    // A palette swatch.
    const swatches = sidebar(page).getByRole('group', { name: 'Palette colors' }).getByRole('button');
    const first = swatches.first();
    const css = await first.evaluate((el) => getComputedStyle(el).backgroundColor);
    await first.click();
    await expect.poll(() => shown(page, 'Foreground color')).toBe(css.replace(/^rgb\((\d+), (\d+), (\d+)\)$/, 'rgb($1 $2 $3 / 1)'));
    // D: the default pair.
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('d');
    await expect.poll(() => shown(page, 'Foreground color')).toBe('rgb(0 0 0 / 1)');
    await expect.poll(() => shown(page, 'Background color')).toBe('rgb(255 255 255 / 1)');
  });

  test('pixel: Pan and Zoom follow Move, with H and Z (AC-09)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    expect((await groupIds(page)).slice(0, 3)).toEqual(['move', 'hand', 'zoom']);
    expect((await railNames(page)).slice(0, 3)).toEqual(['Move', 'Pan', 'Zoom']);
    await expect(list(page).locator('button[data-group="hand"]')).toHaveAttribute('title', 'Pan (H)');
    await expect(list(page).locator('button[data-group="zoom"]')).toHaveAttribute('title', 'Zoom (Z)');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('h');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Pan');
    await page.keyboard.press('z');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Zoom');
    // A brush key selects the brush again.
    await page.keyboard.press('b');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Pencil');
  });

  test('pixel: a Pan drag moves the view and changes nothing; a selection stays put (AC-10)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await menu(page, 'Select', 'Select All');
    await list(page).locator('button[data-group="hand"]').click();
    const before = await box(pixelCanvas(page));
    await drag(page, pixelCanvas(page), [0.5, 0.5], [0.8, 0.7]);
    const after = await box(pixelCanvas(page));
    expect(Math.round(after.x - before.x)).toBe(Math.round(before.width * 0.3));
    expect(Math.round(after.y - before.y)).toBe(Math.round(before.height * 0.2));
    expect(after.width).toBe(before.width);
    await expect(undo(page)).toBeDisabled();
  });

  test('pixel: a Zoom click zooms in around the pointer; Alt or the right button zooms out; the ends do nothing (AC-11)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await list(page).locator('button[data-group="zoom"]').click();
    const zoom = async () => Number.parseInt((await zoomReadout(page).textContent()) ?? '', 10);
    await zoomActual(page);
    expect(await zoom()).toBe(1);
    const s = await box(stage(page));
    const mid = { x: s.x + s.width / 2, y: s.y + s.height / 2 };
    for (let i = 0; i < 3; i += 1) await page.mouse.click(mid.x, mid.y);
    expect(await zoom()).toBe(8);
    // The sprite pixel under the pointer stays under it.
    const b0 = await box(pixelCanvas(page));
    const px = b0.x + b0.width * 0.25;
    const py = b0.y + b0.height * 0.75;
    await page.mouse.click(px, py);
    expect(await zoom()).toBe(16);
    const b1 = await box(pixelCanvas(page));
    expect(Math.abs((px - b1.x) / b1.width - 0.25)).toBeLessThan(0.01);
    expect(Math.abs((py - b1.y) / b1.height - 0.75)).toBeLessThan(0.01);
    await page.keyboard.down('Alt');
    await page.mouse.click(px, py);
    await page.keyboard.up('Alt');
    expect(await zoom()).toBe(8);
    await page.mouse.click(px, py, { button: 'right' });
    expect(await zoom()).toBe(4);
    // On the canvas area beside the sprite too.
    await page.mouse.click(s.x + 12, s.y + 12);
    expect(await zoom()).toBe(8);
    // The ends: 64× and 1×.
    for (let i = 0; i < 8; i += 1) await page.mouse.click(s.x + s.width / 2, s.y + s.height / 2);
    expect(await zoom()).toBe(64);
    for (let i = 0; i < 8; i += 1) await page.mouse.click(s.x + s.width / 2, s.y + s.height / 2, { button: 'right' });
    expect(await zoom()).toBe(1);
    await expect(undo(page)).toBeDisabled();
  });

  test('pixel: Pan and Zoom still work while an effect is open (AC-12)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await list(page).locator('button[data-group="zoom"]').click();
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    const zoom = async () => Number.parseInt((await zoomReadout(page).textContent()) ?? '', 10);
    const start = await zoom();
    const b = await box(pixelCanvas(page));
    await page.mouse.click(b.x + 4, b.y + b.height - 4);
    expect(await zoom()).toBe(start * 2);
    await page.getByRole('dialog', { name: 'Invert Colors' }).getByRole('button', { name: 'Cancel' }).click();
    await list(page).locator('button[data-group="hand"]').click();
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    const before = await box(pixelCanvas(page));
    await page.mouse.move(before.x + 4, before.y + before.height - 4);
    await page.mouse.down();
    await page.mouse.move(before.x + 4, before.y + before.height - 44, { steps: 4 });
    await page.mouse.up();
    expect(Math.round((await box(pixelCanvas(page))).y - before.y)).toBe(-40);
  });

  test('image: one Fill group with Fill and Gradient; G cycles (AC-13)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    const ids = await groupIds(page);
    expect(ids).toContain('fill');
    expect(ids).not.toContain('gradient');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('g');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Fill: Fill');
    await page.keyboard.press('g');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Fill: Gradient');
    await page.keyboard.press('g');
    await expect(pressed(page)).toHaveAttribute('aria-label', 'Fill: Fill');
  });

  for (const mode of MODES) {
    test(`${mode}: N selects the Gradient (AC-14)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await stage(page).click({ position: { x: 5, y: 5 } });
      await page.keyboard.press('n');
      await expect(pressed(page)).toHaveAttribute('aria-label', /: Gradient$/);
      await expect(pressed(page)).toHaveAttribute('data-group', 'fill');
      // Again: it stays the Gradient (no cycling on N).
      await page.keyboard.press('n');
      await expect(pressed(page)).toHaveAttribute('aria-label', /: Gradient$/);
    });
  }

  test('pixel: Shapes lists four shapes and no filled tool; Mode Fill draws a filled shape in the other colour (AC-15)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const shapes = list(page).locator('button[data-group="shapes"]');
    await shapes.click({ button: 'right' });
    const flyout = page.getByRole('menu', { name: 'Shapes' });
    expect(
      await flyout.getByRole('menuitemradio').evaluateAll((els) => els.map((el) => (el.textContent ?? '').trim())),
    ).toEqual(['Rectangle', 'Ellipse', 'Rounded Rectangle', 'Polygon']);
    await flyout.getByRole('menuitemradio', { name: 'Rectangle', exact: true }).click();
    await setColor(page, 'fg', '#ff0000');
    await setColor(page, 'bg', '#0000ff');
    await page.getByRole('combobox', { name: 'Shape mode' }).selectOption('fill');
    const blank = await pixelCanvas(page).screenshot();
    await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
    await expect(undo(page)).toBeEnabled();
    const filled = await pixelCanvas(page).screenshot();
    expect(filled.equals(blank)).toBe(false);
    // Inside the shape (the middle of sprite pixel 9, 9, off the grid lines)
    // is the background colour: blue.
    const centre = await page.evaluate(async (b64) => {
      const bmp = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
      const c = new OffscreenCanvas(bmp.width, bmp.height);
      const ctx = c.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bmp, 0, 0);
      return [...ctx.getImageData(Math.floor((bmp.width * 9.5) / 16), Math.floor((bmp.height * 9.5) / 16), 1, 1).data];
    }, filled.toString('base64'));
    expect(centre.join(',')).toBe('0,0,255,255');
  });

  test('pixel: the shortcut overview and the command palette list Pan, Zoom and Gradient (N), no filled shape (AC-16)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.keyboard.press('Control+k');
    const palette = page.getByRole('dialog', { name: 'Command Palette' });
    await palette.getByRole('combobox').fill('Tool:');
    const labels = await palette
      .getByRole('listbox')
      .getByRole('option')
      .evaluateAll((els) => els.map((el) => el.querySelector('span')?.textContent?.trim() ?? ''));
    expect(labels).toContain('Tool: Pan');
    expect(labels).toContain('Tool: Zoom');
    expect(labels).toContain('Tool: Gradient');
    expect(labels.filter((l) => /Filled/.test(l))).toEqual([]);
    await page.keyboard.press('Escape');

    await menu(page, 'Help', 'Keyboard Shortcuts');
    const keys = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
    await expect(keys).toBeVisible();
    const row = (name: string) => keys.getByRole('row').filter({ has: byText(page, name, { exact: true }) });
    await expect(row('Pan')).toContainText('H');
    await expect(row('Zoom').first()).toContainText('Z');
    await expect(row('Gradient')).toContainText('N');
    await expect(keys.getByText(/Filled (Rectangle|Ellipse)/)).toHaveCount(0);
  });

  test('German: the well and the view tools carry German names (AC-17)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    const w = editor(page).locator('[data-color-well]');
    for (const name of [
      'Vordergrundfarbe',
      'Hintergrundfarbe',
      'Vorder- und Hintergrundfarbe tauschen',
      'Auf Standardfarben zurücksetzen',
    ]) {
      await expect(w.getByRole('button', { name, exact: true })).toBeVisible();
    }
    const tools = editor(page).locator('[data-tool-list]');
    await expect(tools.locator('button[data-group="hand"]')).toHaveAttribute('aria-label', 'Verschieben (Ansicht)');
    await expect(tools.locator('button[data-group="zoom"]')).toHaveAttribute('aria-label', 'Zoom');
  });
});

test.describe('low window', () => {
  test.use({ viewport: { width: 1280, height: 480 } });

  test('the tool list scrolls and the well stays at the rail’s foot (AC-08)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    expect(await list(page).evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    const r = await box(rail(page));
    const w = await box(well(page));
    expect(w.y + w.height).toBeLessThanOrEqual(r.y + r.height + 0.5);
    await expect(wellButton(page, 'Foreground color')).toBeInViewport({ ratio: 1 });
    // The last tool can be scrolled into view; the well does not move.
    await list(page).locator('button[data-group]').last().scrollIntoViewIfNeeded();
    await expect(list(page).locator('button[data-group]').last()).toBeInViewport({ ratio: 1 });
    expect((await box(well(page))).y).toBe(w.y);
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  for (const mode of MODES) {
    test(`${mode}: the well stays at the end of the strip while the tools scroll sideways (AC-01, AC-08)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await expect(rail(page)).toHaveAttribute('aria-orientation', 'horizontal');
      expect(await list(page).evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
      const r = await box(rail(page));
      const l = await box(list(page));
      const w = await box(well(page));
      expect(w.x).toBeGreaterThanOrEqual(l.x + l.width - 0.5);
      expect(w.x + w.width).toBeLessThanOrEqual(r.x + r.width + 0.5);
      await list(page).locator('button[data-group]').last().scrollIntoViewIfNeeded();
      expect((await box(well(page))).x).toBe(w.x);
      // The picker opens wholly inside the window.
      await wellButton(page, 'Foreground color').click();
      const p = await box(page.getByRole('dialog', { name: 'Foreground color picker' }));
      expect(p.x).toBeGreaterThanOrEqual(0);
      expect(p.x + p.width).toBeLessThanOrEqual(390);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.y + p.height).toBeLessThanOrEqual(844);
    });
  }
});
