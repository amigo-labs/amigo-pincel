import { crc32, deflateSync } from 'node:zlib';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, pixelCanvas, waitForEditor } from './helpers';

// Tool-variant flyout on the tool rail (docs/specs/tool-variant-flyout.md):
// a right-click, a long press or Shift+F10 on a group button lists the
// group's variants. The browser's own context menu is invisible to
// automation; "it does not open" is the cancelled `contextmenu` event.

const PIXEL_SHAPES = [
  'Rectangle',
  'Filled Rectangle',
  'Ellipse',
  'Filled Ellipse',
  'Rounded Rectangle',
  'Polygon',
];
const IMAGE_SHAPES = ['Rectangle', 'Rounded Rectangle', 'Ellipse', 'Polygon'];

function rail(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Tools' });
}

function groupButton(page: Page, id: string): Locator {
  return rail(page).locator(`[data-group="${id}"]`);
}

function flyout(page: Page, name = 'Shapes'): Locator {
  return page.getByRole('menu', { name, exact: true });
}

function activeTool(page: Page): Locator {
  return rail(page).locator('[aria-pressed="true"]');
}

/** Records `defaultPrevented` of every `contextmenu` event reaching the window. */
async function trackContextMenus(page: Page): Promise<() => Promise<boolean[]>> {
  await page.evaluate(() => {
    const w = window as unknown as { ctxLog: boolean[] };
    w.ctxLog = [];
    window.addEventListener('contextmenu', (e) => w.ctxLog.push(e.defaultPrevented));
  });
  return () => page.evaluate(() => (window as unknown as { ctxLog: boolean[] }).ctxLog);
}

async function center(target: Locator): Promise<{ x: number; y: number }> {
  const box = await target.boundingBox();
  if (!box) throw new Error('target is not visible');
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: a right-click on a group button lists its variants (AC-01, AC-12)`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const log = await trackContextMenus(page);
    const before = await activeTool(page).getAttribute('aria-label');

    await groupButton(page, 'shapes').click({ button: 'right' });
    const menu = flyout(page);
    await expect(menu).toBeVisible();
    const entries = menu.getByRole('menuitemradio');
    await expect(entries).toHaveText(mode === 'pixel' ? PIXEL_SHAPES : IMAGE_SHAPES);
    for (const entry of await entries.all()) {
      await expect(entry.locator('svg')).toHaveCount(1);
      await expect(entry).toHaveAttribute('aria-checked', 'false');
    }
    expect(await log()).toEqual([true]);
    await expect(activeTool(page)).toHaveAttribute('aria-label', before ?? '');
  });
}

test('pixel mode: single buttons, separators and the empty rail open nothing (AC-02)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const log = await trackContextMenus(page);
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');

  await groupButton(page, 'move').click({ button: 'right' });
  await rail(page).getByRole('separator').first().click({ button: 'right', force: true });
  // The disabled Tile Stamp (no tile picked) and the space after the last button.
  const stamp = await center(groupButton(page, 'stamp'));
  await page.mouse.click(stamp.x, stamp.y, { button: 'right' });
  const box = await rail(page).boundingBox();
  if (!box) throw new Error('rail is not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height - 4, { button: 'right' });

  // A long press on a single button selects nothing.
  const move = await center(groupButton(page, 'move'));
  await page.mouse.move(move.x, move.y);
  await page.mouse.down();
  await page.waitForTimeout(800);
  await page.mouse.up();

  await expect(page.getByRole('menu')).toHaveCount(0);
  expect(await log()).toEqual([true, true, true, true]);
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
});

test('pixel mode: a mouse hold opens the flyout, release over an entry chooses it (AC-03, AC-06)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const shapes = await center(groupButton(page, 'shapes'));
  await page.mouse.move(shapes.x, shapes.y);
  await page.mouse.down();
  const menu = flyout(page);
  await expect(menu).toBeVisible();
  // Release on the button itself: the flyout stays, nothing is chosen.
  await page.mouse.up();
  await page.waitForTimeout(300);
  await expect(menu).toBeVisible();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
  await page.keyboard.press('Escape');

  // Hold, slide onto an entry, release.
  await page.mouse.down();
  await expect(menu).toBeVisible();
  const ellipse = await center(menu.getByRole('menuitemradio', { name: 'Ellipse', exact: true }));
  await page.mouse.move(ellipse.x, ellipse.y, { steps: 5 });
  await page.mouse.up();
  await expect(menu).toBeHidden();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Ellipse');
  await expect(
    page.getByRole('radiogroup', { name: 'Shapes variant' }).getByRole('radio', { name: 'Ellipse', exact: true }),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(groupButton(page, 'shapes')).toBeFocused();
});

test('pixel mode: a pen hold opens the flyout and chooses on release (AC-03)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const shapes = await center(groupButton(page, 'shapes'));
  const cdp = await page.context().newCDPSession(page);
  const pen = (type: 'mousePressed' | 'mouseMoved' | 'mouseReleased', at: { x: number; y: number }) =>
    cdp.send('Input.dispatchMouseEvent', {
      type,
      ...at,
      button: 'left',
      buttons: type === 'mouseReleased' ? 0 : 1,
      clickCount: 1,
      pointerType: 'pen',
      force: 0.5,
    });
  await pen('mousePressed', shapes);
  const menu = flyout(page);
  await expect(menu).toBeVisible();
  const polygon = await center(menu.getByRole('menuitemradio', { name: 'Polygon', exact: true }));
  await pen('mouseMoved', polygon);
  await pen('mouseReleased', polygon);
  await cdp.detach();
  await expect(menu).toBeHidden();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Polygon');
});

test('pixel mode: a short press, a drag or a moved press selects the shown variant (AC-04)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const shapes = await center(groupButton(page, 'shapes'));
  await page.mouse.click(shapes.x, shapes.y);
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Rectangle');

  // A press that moves more than 10 px, held past 500 ms, opens nothing;
  // released over the button it is a plain click.
  await page.keyboard.press('b');
  await page.mouse.move(shapes.x - 6, shapes.y);
  await page.mouse.down();
  await page.mouse.move(shapes.x + 8, shapes.y, { steps: 3 });
  await page.waitForTimeout(800);
  await expect(page.getByRole('menu')).toHaveCount(0);
  await page.mouse.up();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Rectangle');
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  test('pixel mode: a touch hold opens the flyout; a cancelled or two-finger press does not (AC-03, AC-04)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const shapes = await center(groupButton(page, 'shapes'));
    const cdp = await page.context().newCDPSession(page);
    const touch = (type: 'touchStart' | 'touchMove' | 'touchEnd' | 'touchCancel', points: { x: number; y: number }[]) =>
      cdp.send('Input.dispatchTouchEvent', {
        type,
        touchPoints: points.map((p, id) => ({ ...p, id, radiusX: 2, radiusY: 2, force: 1 })),
      });
    const menu = flyout(page);

    // Second finger before 500 ms.
    await touch('touchStart', [shapes]);
    await touch('touchStart', [shapes, { x: shapes.x + 200, y: shapes.y }]);
    await page.waitForTimeout(800);
    await expect(page.getByRole('menu')).toHaveCount(0);
    await touch('touchEnd', []);

    // Cancelled press.
    await touch('touchStart', [shapes]);
    await touch('touchCancel', []);
    await page.waitForTimeout(800);
    await expect(page.getByRole('menu')).toHaveCount(0);
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');

    // Hold, slide onto an entry, lift.
    await touch('touchStart', [shapes]);
    await expect(menu).toBeVisible();
    const fill = await center(menu.getByRole('menuitemradio', { name: 'Filled Rectangle', exact: true }));
    await touch('touchMove', [fill]);
    await touch('touchEnd', []);
    await cdp.detach();
    await expect(menu).toBeHidden();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Filled Rectangle');
  });

  test('pixel mode: a quick tap selects the shown variant (AC-04)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    await groupButton(page, 'lasso').tap();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Lasso');
    await expect(page.getByRole('menu')).toHaveCount(0);
  });
});

test('pixel mode: header, checked entry and keyboard choice (AC-05, AC-06, AC-08)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await page.keyboard.press('u');
  await page.keyboard.press('u');
  await page.keyboard.press('u');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Ellipse');

  await groupButton(page, 'shapes').click({ button: 'right' });
  const menu = flyout(page);
  await expect(menu.locator('[data-flyout-header]')).toHaveText('Shapes (U)');
  const entry = (name: string) => menu.getByRole('menuitemradio', { name, exact: true });
  await expect(entry('Ellipse')).toHaveAttribute('aria-checked', 'true');
  await expect(menu.getByRole('menuitemradio', { checked: true })).toHaveCount(1);
  // Focus starts on the checked entry.
  await expect(entry('Ellipse')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(entry('Filled Ellipse')).toBeFocused();
  await page.keyboard.press('End');
  await expect(entry('Polygon')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(entry('Rectangle')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(entry('Polygon')).toBeFocused();
  await page.keyboard.press('Home');
  await expect(entry('Rectangle')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(menu).toBeHidden();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Filled Rectangle');
  await expect(groupButton(page, 'shapes')).toBeFocused();

  // Space chooses too; with the active tool outside the group, focus
  // starts on the first entry.
  await page.keyboard.press('b');
  await groupButton(page, 'select').click({ button: 'right' });
  const marquee = flyout(page, 'Marquee');
  await expect(marquee.locator('[data-flyout-header]')).toHaveText('Marquee (M)');
  await expect(marquee.getByRole('menuitemradio', { checked: true })).toHaveCount(0);
  await expect(marquee.getByRole('menuitemradio', { name: 'Rectangle Select' })).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press(' ');
  await expect(marquee).toBeHidden();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Marquee: Ellipse Select');
});

test('pixel mode: Escape, Tab, outside clicks and other buttons close the flyout (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const shapes = groupButton(page, 'shapes');
  const menu = flyout(page);

  await shapes.click({ button: 'right' });
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await expect(shapes).toBeFocused();

  await shapes.click({ button: 'right' });
  await page.keyboard.press('Tab');
  await expect(menu).toBeHidden();
  await expect(groupButton(page, 'text')).toBeFocused();

  await shapes.click({ button: 'right' });
  await page.keyboard.press('Shift+Tab');
  await expect(menu).toBeHidden();
  await expect(groupButton(page, 'line')).toBeFocused();

  await shapes.click({ button: 'right' });
  const zoomIn = page.getByRole('button', { name: 'Zoom in' });
  await zoomIn.click();
  await expect(menu).toBeHidden();
  await expect(zoomIn).toBeFocused();

  await shapes.click({ button: 'right' });
  await shapes.click({ button: 'right' });
  await expect(menu).toBeHidden();

  await shapes.click({ button: 'right' });
  await groupButton(page, 'lasso').click({ button: 'right' });
  await expect(menu).toBeHidden();
  const lasso = flyout(page, 'Lasso');
  await expect(lasso).toBeVisible();
  await expect(lasso.getByRole('menuitemradio', { name: 'Lasso', exact: true })).toBeFocused();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
});

test('pixel mode: Shift+F10 and the context-menu key open the flyout (AC-09)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const log = await trackContextMenus(page);
  const shapes = groupButton(page, 'shapes');
  const menu = flyout(page);

  await shapes.focus();
  await page.keyboard.press('Shift+F10');
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('menuitemradio', { name: 'Rectangle', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');

  await page.keyboard.press('ContextMenu');
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('menuitemradio', { name: 'Rectangle', exact: true })).toBeFocused();
  // Whatever `contextmenu` events the keys fired were cancelled.
  expect((await log()).every(Boolean)).toBe(true);
});

test.describe('shortcuts while open', () => {
  test.beforeEach(async ({ page }) => {
    // Save falls back to a download the test can observe.
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  test('pixel mode: the open flyout blocks editor keys, Ctrl and ⌘ alike (AC-10)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
    await drag(page, pixelCanvas(page), [0.2, 0.8], [0.8, 0.2]);
    await expect(undo).toBeEnabled();
    let downloads = 0;
    page.on('download', () => downloads++);

    const menu = flyout(page);
    await groupButton(page, 'shapes').click({ button: 'right' });
    for (const key of ['b', 'e', 'Control+z', 'Meta+z', 'Control+s', 'Meta+s']) await page.keyboard.press(key);
    await page.waitForTimeout(500);
    await expect(menu).toBeVisible();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
    expect(downloads).toBe(0);
    // Undo depth unchanged: two undos are still possible.
    await page.keyboard.press('Escape');
    await expect(menu).toBeHidden();

    // Closed: the same keys act again.
    await page.keyboard.press('e');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eraser');
    await page.keyboard.press('Meta+z');
    await expect(undo).toBeEnabled();
    await page.keyboard.press('Control+z');
    await expect(undo).toBeDisabled();
    const download = page.waitForEvent('download');
    await page.keyboard.press('Meta+s');
    await download;
  });
});

test('pixel mode: the flyout sits right of the vertical rail (AC-11)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const shapes = groupButton(page, 'shapes');
  await shapes.click({ button: 'right' });
  const fly = await flyout(page).boundingBox();
  const button = await shapes.boundingBox();
  const strip = await rail(page).boundingBox();
  if (!fly || !button || !strip) throw new Error('not visible');
  expect(Math.abs(fly.y - button.y)).toBeLessThanOrEqual(1);
  expect(fly.x).toBeGreaterThanOrEqual(strip.x + strip.width);
});

test.describe('phone strip', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('pixel mode: the flyout sits above the strip, inside the window (AC-11)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    await expect(rail(page)).toHaveAttribute('aria-orientation', 'horizontal');
    const select = groupButton(page, 'select');
    await select.click({ button: 'right' });
    const fly = await flyout(page, 'Marquee').boundingBox();
    const button = await select.boundingBox();
    const strip = await rail(page).boundingBox();
    if (!fly || !button || !strip) throw new Error('not visible');
    expect(fly.y + fly.height).toBeLessThanOrEqual(strip.y);
    expect(Math.abs(fly.x - button.x)).toBeLessThanOrEqual(1);
    await page.keyboard.press('Escape');

    // The last group button sits at the right edge: the flyout shifts in.
    const shapes = groupButton(page, 'shapes');
    await shapes.scrollIntoViewIfNeeded();
    await rail(page).evaluate((el) => el.scrollTo({ left: el.scrollWidth }));
    await shapes.click({ button: 'right' });
    const right = await flyout(page).boundingBox();
    if (!right) throw new Error('not visible');
    expect(right.x).toBeGreaterThanOrEqual(0);
    expect(right.x + right.width).toBeLessThanOrEqual(390);
  });
});

test.describe('short window', () => {
  test.use({ viewport: { width: 1000, height: 200 } });

  test('pixel mode: a flyout taller than the window is limited to it and scrolls (AC-11)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const shapes = groupButton(page, 'shapes');
    await shapes.scrollIntoViewIfNeeded();
    await shapes.click({ button: 'right' });
    const menu = flyout(page);
    await expect(menu).toBeVisible();
    const fly = await menu.boundingBox();
    if (!fly) throw new Error('not visible');
    expect(fly.y).toBeGreaterThanOrEqual(0);
    expect(fly.y + fly.height).toBeLessThanOrEqual(200);
    expect(await menu.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
  });
});

test.describe('German UI', () => {
  test('pixel mode: header and accessible name are German (AC-13)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Language' }).click();
    await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
    await page.getByRole('toolbar', { name: 'Werkzeuge' }).locator('[data-group="shapes"]').click({ button: 'right' });
    const menu = page.getByRole('menu', { name: 'Formen', exact: true });
    await expect(menu).toBeVisible();
    await expect(menu.locator('[data-flyout-header]')).toHaveText('Formen (U)');
    await expect(menu.getByRole('menuitemradio', { name: 'Gefülltes Rechteck' })).toBeVisible();
  });
});

/** A 1×1 PNG, to open a document through the shell. */
function tinyPng(): Buffer {
  const chunk = (type: string, data: Buffer) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length, 0);
    head.write(type, 4, 'latin1');
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
    return Buffer.concat([head, data, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(1, 0);
  ihdr.writeUInt32BE(1, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.from([0, 255, 0, 0, 255]))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A 1×1 24-bit BMP: a file only Image mode opens. */
function tinyBmp(): Buffer {
  const b = Buffer.alloc(58);
  b.write('BM', 0, 'latin1');
  b.writeUInt32LE(58, 2);
  b.writeUInt32LE(54, 10);
  b.writeUInt32LE(40, 14);
  b.writeInt32LE(1, 18);
  b.writeInt32LE(1, 22);
  b.writeUInt16LE(1, 26);
  b.writeUInt16LE(24, 28);
  b.writeUInt32LE(4, 34);
  b.set([0, 0, 255, 0], 54);
  return b;
}

test.describe('closing from outside', () => {
  test.use({ viewport: { width: 1280, height: 500 } });

  test('pixel mode: resize, rail scroll and an outside tool change close the flyout (AC-14)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const shapes = groupButton(page, 'shapes');
    const menu = flyout(page);

    await shapes.click({ button: 'right' });
    await page.setViewportSize({ width: 1200, height: 500 });
    await expect(menu).toBeHidden();

    await shapes.click({ button: 'right' });
    await rail(page).evaluate((el) => (el.scrollTop = el.scrollTop > 0 ? 0 : 40));
    await expect(menu).toBeHidden();

    // The options bar changes the tool without a press outside the flyout.
    await page.keyboard.press('u');
    await shapes.click({ button: 'right' });
    await expect(menu).toBeVisible();
    await page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name: 'Polygon' })
      .evaluate((el) => (el as HTMLElement).click());
    await expect(menu).toBeHidden();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Polygon');
  });

  test('pixel mode: replacing the document closes the flyout (AC-14)', async ({ page }) => {
    await page.addInitScript(() => {
      delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
    });
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    await page.getByRole('menu', { name: 'File' }).getByRole('menuitem', { name: 'Open…' }).click();
    const picker = await chooser;
    // The picker is still open when the flyout opens from the keyboard.
    await groupButton(page, 'shapes').focus();
    await page.keyboard.press('Shift+F10');
    await expect(flyout(page)).toBeVisible();
    await picker.setFiles({ name: 'dot.png', mimeType: 'image/png', buffer: tinyPng() });
    // Pixel mode opens a PNG as a new sprite in place.
    await expect(page.getByText('opened dot.png · 1×1')).toBeVisible();
    await expect(page.getByRole('menu')).toHaveCount(0);
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
  });

  test('pixel mode: closing the editor closes its flyout (AC-14)', async ({ page }) => {
    await page.addInitScript(() => {
      delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
    });
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    await page.getByRole('menu', { name: 'File' }).getByRole('menuitem', { name: 'Open…' }).click();
    const picker = await chooser;
    await groupButton(page, 'shapes').focus();
    await page.keyboard.press('Shift+F10');
    await expect(flyout(page)).toBeVisible();
    // A BMP belongs to Image mode: it opens as an Image tab and the Pixel
    // editor goes into the background (document-tabs AC-21).
    await picker.setFiles({ name: 'dot.bmp', mimeType: 'image/bmp', buffer: tinyBmp() });
    await waitForEditor(page, 'image');
    await expect(pixelCanvas(page)).toBeHidden();
    await expect(page.getByRole('menu')).toHaveCount(0);
    // Back on the Pixel tab the flyout stays closed.
    await page.getByRole('tab', { name: /pincel\.aseprite/ }).click();
    await expect(pixelCanvas(page)).toBeVisible();
    await expect(page.getByRole('menu')).toHaveCount(0);
  });
});

test.describe('macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  });

  test('pixel mode: a Control-click opens and closes the flyout like a right-click (AC-01, AC-07)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const shapes = groupButton(page, 'shapes');
    const menu = flyout(page);
    // macOS turns a Control-click into a left-button press plus a
    // `contextmenu` event; the OS step cannot be driven from here.
    const controlClick = () =>
      shapes.evaluate((el) => {
        const at = { bubbles: true, cancelable: true, composed: true, button: 0, ctrlKey: true };
        const pointer = { ...at, isPrimary: true, pointerId: 1, pointerType: 'mouse' };
        el.dispatchEvent(new PointerEvent('pointerdown', { ...pointer, buttons: 1 }));
        el.dispatchEvent(new MouseEvent('contextmenu', { ...at, buttons: 1 }));
        el.dispatchEvent(new PointerEvent('pointerup', { ...pointer, buttons: 0 }));
      });
    await controlClick();
    await expect(menu).toBeVisible();
    await page.waitForTimeout(700);
    await expect(menu).toBeVisible();
    await controlClick();
    await expect(menu).toBeHidden();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Pencil');
  });
});
