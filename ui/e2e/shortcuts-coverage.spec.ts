import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  colorOf,
  copiedImage,
  createDocument,
  drag,
  imageCanvas,
  menu,
  menuHint,
  pixelCanvas,
  pixelOf,
  type ClipImage,
} from './helpers';

// Default shortcuts for layers, frames, transforms, opacity, the palette and
// Toggle UI (docs/specs/power-user-shortcuts-coverage.md). Layer state is
// read from the layers panel, pixels from the clipboard (Copy), never from
// the canvas.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

type Mode = 'pixel' | 'image';

function canvasOf(page: Page, mode: Mode): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

function layerList(page: Page): Locator {
  return page.getByRole('list', { name: 'Layer list' });
}

/** Layer names in panel order (top-most first). */
async function layerNames(page: Page): Promise<string[]> {
  return layerList(page)
    .getByRole('button', { name: /^Drag to reorder / })
    .evaluateAll((els) =>
      els.map((el) => (el.getAttribute('aria-label') ?? '').replace('Drag to reorder ', '')),
    );
}

/** The name of the active layer row. */
async function activeLayer(page: Page): Promise<string> {
  // The name button: the row's other toggles carry an aria-label.
  const row = layerList(page).locator('button[aria-pressed="true"][title]:not([aria-label])');
  return (await row.getAttribute('title')) ?? '';
}

function activeTool(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
}

/** The whole active layer, read through Deselect + Copy. */
async function layerImage(page: Page): Promise<ClipImage> {
  await page.keyboard.press('ControlOrMeta+d');
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  return copiedImage(page);
}

function mirrored(img: ClipImage, axis: 'h' | 'v'): number[] {
  const out: number[] = [];
  for (let y = 0; y < img.height; y += 1) {
    for (let x = 0; x < img.width; x += 1) {
      const sx = axis === 'h' ? img.width - 1 - x : x;
      const sy = axis === 'v' ? img.height - 1 - y : y;
      out.push(...pixelOf(img, sx, sy));
    }
  }
  return out;
}

/** Dispatches a key event as a non-US layout reports it (`e.key` ≠ the printed key). */
async function layoutKey(
  page: Page,
  init: { key: string; code: string; shiftKey?: boolean; ctrlKey?: boolean; altKey?: boolean },
): Promise<void> {
  await page.evaluate((i) => {
    const target = document.activeElement ?? document.body;
    target.dispatchEvent(new KeyboardEvent('keydown', { ...i, bubbles: true, cancelable: true }));
  }, init);
}

/** Text of every `<kbd>` in the open shortcut overview. */
async function overviewChords(page: Page): Promise<string[]> {
  await page.keyboard.press('Control+/');
  const dialog = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  await expect(dialog).toBeVisible();
  const chords = await dialog.locator('kbd').allTextContents();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  return chords.map((c) => c.trim());
}

/** The tool rail, the options bar and the docked panels: what Toggle UI hides. */
function chromeParts(page: Page): Locator[] {
  return [
    page.getByRole('toolbar', { name: 'Tools' }),
    page.getByRole('toolbar', { name: 'Tool options' }),
    page.getByRole('complementary', { name: 'Panels' }),
  ];
}

async function expectChrome(page: Page, shown: boolean): Promise<void> {
  for (const part of chromeParts(page)) {
    if (shown) await expect(part).toBeVisible();
    else await expect(part).toBeHidden();
  }
  // The way back stays: menu bar and status bar.
  await expect(page.getByRole('menubar')).toBeVisible();
  await expect(page.locator('footer')).toBeVisible();
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Shift+N, Ctrl+J, Ctrl+Shift+E and Ctrl+Shift+Delete edit layers (AC-01 – AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const tool = await activeTool(page).getAttribute('aria-label');
    expect(await layerNames(page)).toHaveLength(1);

    await page.keyboard.press('Shift+N');
    await expect.poll(() => layerNames(page)).toHaveLength(2);
    // N is the Gradient group's key; Shift+N is not.
    await expect(activeTool(page)).toHaveAttribute('aria-label', tool ?? '');

    await page.keyboard.press('Control+j');
    await expect.poll(() => layerNames(page)).toHaveLength(3);

    await page.keyboard.press('Control+Shift+E');
    await expect.poll(() => layerNames(page)).toHaveLength(2);
    await expect(page.getByRole('dialog', { name: 'Export sprite sheet' })).toBeHidden();

    await page.keyboard.press('Control+Shift+Delete');
    await expect.poll(() => layerNames(page)).toHaveLength(1);
  });

  test(`${mode} mode: a layer key whose menu item is disabled does nothing (AC-16)`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await createDocument(page, mode, { width: 16, height: 16 });
    const names = await layerNames(page);
    // Merge down on the bottom layer, delete on the only layer.
    await page.keyboard.press('Control+Shift+E');
    await page.keyboard.press('Control+Shift+Delete');
    await page.keyboard.press('Alt+ArrowDown');
    expect(await layerNames(page)).toEqual(names);
    await expect(page.locator('footer')).not.toContainText(/failed/i);
    expect(errors).toEqual([]);
  });

  test(`${mode} mode: Alt+↑ / Alt+↓ select the layer above / below (AC-05)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Shift+N');
    await page.keyboard.press('Shift+N');
    await expect.poll(() => layerNames(page)).toHaveLength(3);
    const names = await layerNames(page);
    await expect.poll(() => activeLayer(page)).toBe(names[0]);

    // Top of the stack: nothing changes.
    await page.keyboard.press('Alt+ArrowUp');
    await expect.poll(() => activeLayer(page)).toBe(names[0]);
    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => activeLayer(page)).toBe(names[1]);
    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => activeLayer(page)).toBe(names[2]);
    // Bottom: nothing changes.
    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => activeLayer(page)).toBe(names[2]);
    await page.keyboard.press('Alt+ArrowUp');
    await expect.poll(() => activeLayer(page)).toBe(names[1]);
  });

  test(`${mode} mode: Shift+H / Shift+V flip the active layer, the tool stays (AC-07)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('b');
    const tool = await activeTool(page).getAttribute('aria-label');
    // An L-shaped mark in the top-left quarter: no axis of symmetry.
    await drag(page, canvasOf(page, mode), [0.1, 0.1], [0.4, 0.1]);
    await drag(page, canvasOf(page, mode), [0.1, 0.1], [0.1, 0.3]);
    const before = await layerImage(page);

    await page.keyboard.press('Shift+H');
    const flippedH = await layerImage(page);
    expect(flippedH.data).not.toEqual(before.data);
    expect(flippedH.data).toEqual(mirrored(before, 'h'));
    await expect(activeTool(page)).toHaveAttribute('aria-label', tool ?? '');

    await page.keyboard.press('Shift+V');
    const flippedV = await layerImage(page);
    expect(flippedV.data).toEqual(mirrored(flippedH, 'v'));
    await expect(activeTool(page)).toHaveAttribute('aria-label', tool ?? '');
  });

  test(`${mode} mode: Tab on the canvas stage and View ▸ Toggle UI hide the tool rail, options and panels (AC-09, AC-17)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await expectChrome(page, true);

    // Focus on the page body: Tab moves focus, the UI stays.
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await page.keyboard.press('Tab');
    await expectChrome(page, true);
    expect(await page.evaluate(() => document.activeElement !== document.body)).toBe(true);

    // A press on the stage gives it focus; Tab there toggles.
    await canvasOf(page, mode).click({ position: { x: 2, y: 2 } });
    await page.keyboard.press('Tab');
    await expectChrome(page, false);
    await page.keyboard.press('Tab');
    await expectChrome(page, true);

    await menu(page, 'View', 'Toggle UI');
    await expectChrome(page, false);
    await menu(page, 'View', 'Toggle UI');
    await expectChrome(page, true);
  });

  test(`${mode} mode: the menus show the new shortcuts (AC-12)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    expect(await menuHint(page, 'Layer', 'New Layer')).toBe('Shift+N');
    expect(await menuHint(page, 'Layer', 'Duplicate Layer')).toBe('Ctrl+J');
    expect(await menuHint(page, 'Layer', 'Delete Layer')).toBe('Ctrl+Shift+Del');
    expect(await menuHint(page, 'Layer', 'Merge Down')).toBe('Ctrl+Shift+E');
    expect(await menuHint(page, 'Layer', 'Flip Horizontal')).toBe('Shift+H');
    expect(await menuHint(page, 'Layer', 'Flip Vertical')).toBe('Shift+V');
    expect(await menuHint(page, 'View', 'Toggle UI')).toBe('Tab');
    // The keys flip the layer, not the image; the rotations are desktop-only.
    expect(await menuHint(page, 'Image', 'Flip Horizontal')).toBe('');
    expect(await menuHint(page, 'Image', 'Rotate 90° CW')).toBe('');
    if (mode === 'pixel') expect(await menuHint(page, 'Frame', 'New Frame')).toBe('Alt+N');
  });

  test(`${mode} mode: no two shortcuts share a chord, none is browser-reserved (AC-13, AC-14)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const chords = await overviewChords(page);
    expect(chords.length).toBeGreaterThan(30);
    const repeated = chords.filter((c, i) => chords.indexOf(c) !== i);
    expect(repeated).toEqual([]);
    const reserved = [
      'Ctrl+N',
      'Ctrl+Shift+N',
      'Ctrl+T',
      'Ctrl+Shift+T',
      'Ctrl+W',
      'Ctrl+Shift+W',
      'Ctrl+Tab',
      'Ctrl+Shift+Tab',
      'Ctrl+R',
      'Ctrl+Shift+R',
      'F5',
    ];
    expect(chords.filter((c) => reserved.includes(c))).toEqual([]);
  });

  test(`${mode} mode: a focused text field gets the keys (AC-15)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const names = await layerNames(page);
    const name = names[0] ?? '';
    await layerList(page).locator(`button[title="${name}"]`).dblclick();
    const field = page.getByRole('textbox', { name: `Rename ${name}` });
    await expect(field).toBeFocused();
    await page.keyboard.press('End');
    await page.keyboard.press('Shift+N');
    await page.keyboard.press('Shift+H');
    await page.keyboard.press('Tab');
    await expect(field).toBeHidden();
    expect(await layerNames(page)).toEqual([`${name}NH`]);
    await expectChrome(page, true);
  });
}

test('pixel mode: Alt+N adds a frame (AC-06)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(page.getByRole('button', { name: 'Frame 2', exact: true })).toHaveCount(0);
  await page.keyboard.press('Alt+n');
  await expect(page.getByRole('button', { name: 'Frame 2', exact: true })).toBeVisible();
});

test('image mode: Shift+digits set the brush opacity, bare digits still zoom (AC-10)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('b');
  const opacity = page.getByRole('toolbar', { name: 'Tool options' }).getByRole('slider', {
    name: /^Opacity/,
  });
  const zoom = page.getByRole('group', { name: 'Zoom' }).getByRole('button', {
    name: 'Fit to window',
  });

  await page.keyboard.press('Shift+Digit1');
  await expect(opacity).toHaveValue('10');
  await page.keyboard.press('Shift+Digit9');
  await expect(opacity).toHaveValue('90');
  await page.keyboard.press('Shift+Digit0');
  await expect(opacity).toHaveValue('100');
  await page.keyboard.press('Shift+Digit5');
  await expect(opacity).toHaveValue('50');

  // German layout: Shift+0 types `=`, also the zoom-in key.
  const zoomBefore = await zoom.textContent();
  await layoutKey(page, { key: '=', code: 'Digit0', shiftKey: true });
  await expect(opacity).toHaveValue('100');
  expect(await zoom.textContent()).toBe(zoomBefore);

  await page.keyboard.press('1');
  await expect(zoom).toHaveText('100%');
});

test('pixel mode: Alt+] / Alt+[ step through the palette, [ / ] still size the brush (AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('d');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#000000');
  // DB32: #000000, #222034, #45283c, …
  await page.keyboard.press('Alt+BracketRight');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#222034');
  await page.keyboard.press('Alt+BracketRight');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#45283c');
  await page.keyboard.press('Alt+BracketLeft');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#222034');
  await page.keyboard.press('Alt+BracketLeft');
  await page.keyboard.press('Alt+BracketLeft');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#000000');

  await page.keyboard.press('b');
  const size = page.getByRole('spinbutton', { name: 'Brush size' });
  await expect(size).toHaveValue('1');
  await page.keyboard.press(']');
  await expect(size).toHaveValue('2');
  await expect.poll(() => colorOf(page, 'fg')).toBe('#000000');
});

test('pixel mode: Ctrl+Alt+E opens the sprite sheet export, Ctrl+Shift+E does not (AC-19)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  expect(await menuHint(page, 'File', 'Export', 'Sprite Sheet…')).toBe('Ctrl+Alt+E');
  const chords = await overviewChords(page);
  expect(chords).toContain('Ctrl+Alt+E');

  await page.keyboard.press('Control+Shift+E');
  await expect(page.getByRole('dialog', { name: 'Export PNG' })).toBeHidden();
  await expect(dialog).toBeHidden();

  await page.keyboard.press('Control+Alt+e');
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();

  // German Windows: Ctrl+Alt is AltGr, and AltGr+E types `€`.
  await layoutKey(page, { key: '€', code: 'KeyE', ctrlKey: true, altKey: true });
  await expect(dialog).toBeVisible();
});

test('image mode: Ctrl+Alt+E does nothing (AC-19)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.keyboard.press('Control+Alt+e');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('a reload shows the full UI again (AC-18)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'View', 'Toggle UI');
  await expectChrome(page, false);
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expectChrome(page, true);
});
