import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  setColor,
} from './helpers';

// The Clone Stamp in Image mode (docs/specs/clone-stamp.md). The composite
// is read through Edit ▸ Copy Merged (the clipboard), the source crosshair
// through its DOM box.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** Document side in pixels. */
const N = 32;
const RED = [255, 0, 0, 255];
const BLUE = [0, 0, 255, 255];

function activeTool(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
}

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });
const crosshair = (page: Page) => page.getByTestId('clone-crosshair');
const setSource = (page: Page) => page.getByRole('button', { name: 'Set source' });
const NO_SOURCE = 'Ctrl+click to set a source first';

/** Screen position of document point `(x, y)` (canvas pixels). */
async function screen(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / N, box.y + (y * box.height) / N];
}

/** Screen pixels per document pixel. */
async function scale(page: Page): Promise<number> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return box.width / N;
}

/** Presses at the centre of pixel `(px, py)`, with Ctrl when `ctrl`, ⌘ when `'Meta'`. */
async function press(
  page: Page,
  px: number,
  py: number,
  ctrl: boolean | 'Meta' = false,
): Promise<void> {
  const [x, y] = await screen(page, px + 0.5, py + 0.5);
  const key = ctrl === 'Meta' ? 'Meta' : 'Control';
  if (ctrl) await page.keyboard.down(key);
  await page.mouse.click(x, y);
  if (ctrl) await page.keyboard.up(key);
}

/** Drags from pixel `a` to pixel `b` (centres). */
async function stroke(page: Page, a: [number, number], b: [number, number]): Promise<void> {
  const [x0, y0] = await screen(page, a[0] + 0.5, a[1] + 0.5);
  const [x1, y1] = await screen(page, b[0] + 0.5, b[1] + 0.5);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
}

/** The composite, RGBA per pixel. */
async function composite(page: Page): Promise<(x: number, y: number) => number[]> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy Merged');
  const img = await copiedImage(page);
  return (x, y) => pixelOf(img, x, y);
}

/** Centre of the crosshair's box in screen pixels, and its size. */
async function crosshairAt(page: Page): Promise<{ x: number; y: number; w: number; h: number }> {
  const box = await crosshair(page).boundingBox();
  if (!box) throw new Error('no crosshair');
  return { x: box.x + box.width / 2, y: box.y + box.height / 2, w: box.width, h: box.height };
}

/** Expects the crosshair centred on document point `(x, y)`. */
async function expectCrosshairAt(page: Page, x: number, y: number): Promise<void> {
  await expect(crosshair(page)).toBeVisible();
  await expect
    .poll(async () => {
      const c = await crosshairAt(page);
      const [sx, sy] = await screen(page, x, y);
      return Math.max(Math.abs(c.x - sx), Math.abs(c.y - sy));
    })
    .toBeLessThan(1);
}

/**
 * A 32×32 image: a red dab around pixel (8, 16) and a blue one around
 * (8, 5), size 8, on the white background. Ends with the Clone Stamp.
 */
async function paintedDocument(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: N, height: N });
  await page.keyboard.press('b');
  await setColor(page, 'fg', '#ff0000ff');
  await press(page, 8, 16);
  await setColor(page, 'fg', '#0000ffff');
  await press(page, 8, 5);
  await expect(undoButton(page)).toBeEnabled();
  await page.keyboard.press('c');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Clone Stamp');
}

/** Undo steps on the stack, counted by undoing until Undo is disabled and
 *  redoing them all again. */
async function undoDepth(page: Page): Promise<number> {
  let n = 0;
  while (await undoButton(page).isEnabled()) {
    await undoButton(page).click();
    n += 1;
  }
  for (let i = 0; i < n; i += 1) await redoButton(page).click();
  return n;
}

test('the Clone Stamp is a rail tool with key C in Image mode only (AC-01)', async ({ page }) => {
  await createDocument(page, 'image', { width: N, height: N });
  const rail = page.getByRole('toolbar', { name: 'Tools' });
  const labels = await rail
    .locator('button[aria-pressed]')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label')));
  expect(labels.indexOf('Clone Stamp')).toBe(labels.indexOf('Smudge') + 1);
  await expect(rail.getByRole('button', { name: 'Clone Stamp' })).toHaveAttribute(
    'title',
    'Clone Stamp (C)',
  );
  // The paint section keeps its separators: navigate / select / paint /
  // color / draw.
  await expect(rail.getByRole('separator')).toHaveCount(4);
  await page.keyboard.press('c');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Clone Stamp');

  await menu(page, 'Help', 'Keyboard Shortcuts');
  const dialog = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  const row = dialog
    .getByRole('row')
    .filter({ has: page.getByRole('rowheader', { name: 'Clone Stamp', exact: true }) });
  await expect(row.locator('kbd')).toHaveText(['C']);
  await page.keyboard.press('Escape');

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(pixelCanvas(page)).toBeVisible();
  const before = await activeTool(page).getAttribute('aria-label');
  await page.keyboard.press('c');
  await expect(activeTool(page)).toHaveAttribute('aria-label', before ?? '');
  await expect(rail.getByRole('button', { name: 'Clone Stamp' })).toHaveCount(0);
  await menu(page, 'Help', 'Keyboard Shortcuts');
  await expect(
    page
      .getByRole('dialog', { name: 'Keyboard Shortcuts' })
      .getByRole('rowheader', { name: 'Clone Stamp', exact: true }),
  ).toHaveCount(0);
});

test('the options bar shares the Brush tip and adds Sample and Set source (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: N, height: N });
  await page.keyboard.press('c');
  const size = page.getByRole('slider', { name: /^Size/ });
  const opacity = page.getByRole('slider', { name: /^Opacity/ });
  const shape = page.getByRole('combobox', { name: /^Shape/ });
  const sample = page.getByRole('combobox', { name: /^Sample/ });
  for (const control of [size, opacity, shape, sample]) await expect(control).toBeVisible();
  await expect(page.getByRole('checkbox', { name: 'Size', exact: true })).toBeVisible();
  await expect(page.getByRole('checkbox', { name: 'Opacity', exact: true })).toBeVisible();
  await expect(sample).toHaveValue('current_layer');
  await expect(setSource(page)).toHaveAttribute('aria-pressed', 'false');

  // A tip change with the Clone Stamp shows in the Brush, and back.
  await size.fill('23');
  await shape.selectOption('flat');
  await expect(page.getByRole('slider', { name: /^Hardness/ })).toBeVisible();
  await expect(page.getByRole('slider', { name: /^Angle/ })).toBeVisible();
  await expect(page.getByRole('checkbox', { name: 'Tilt turns the nib' })).toBeVisible();
  await sample.selectOption('all_layers');
  await page.keyboard.press('b');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Brush');
  await expect(size).toHaveValue('23');
  await expect(shape).toHaveValue('flat');
  await expect(sample).toHaveCount(0);
  await opacity.fill('40');
  await page.keyboard.press('c');
  await expect(opacity).toHaveValue('40');
  await expect(sample).toHaveValue('all_layers');
  // Sample is the Clone Stamp's own: the Paint Bucket keeps its own.
  await page.keyboard.press('g');
  await expect(sample).toHaveValue('current_layer');
  await page.keyboard.press('c');
  await expect(sample).toHaveValue('all_layers');

  // [ and ] change the size as for the Brush.
  await page.keyboard.press(']');
  await expect(size).toHaveValue('28');
  await page.keyboard.press('[');
  await page.keyboard.press('[');
  await expect(size).toHaveValue('18');
});

test('Ctrl+click sets the source; strokes copy at the fixed offset (AC-03, AC-05, AC-07, AC-12)', async ({
  page,
}) => {
  await paintedDocument(page);
  const start = await composite(page);
  const steps = await undoDepth(page);

  // Ctrl+click: no pixel, no undo step.
  await press(page, 8, 16, true);
  await expectCrosshairAt(page, 8.5, 16.5);
  expect(await undoDepth(page)).toBe(steps);
  const afterSource = await composite(page);
  for (const [x, y] of [
    [8, 16],
    [24, 16],
  ] as const) {
    expect(afterSource(x, y)).toEqual(start(x, y));
  }

  // The first stroke fixes the offset (-16, 0): red lands at (24, 16).
  expect(start(24, 16)).not.toEqual(RED);
  await stroke(page, [24, 16], [26, 16]);
  let px = await composite(page);
  expect(px(24, 16)).toEqual(RED);
  expect(await undoDepth(page)).toBe(steps + 1);

  // A later stroke at q copies from q + offset, not from the source.
  await stroke(page, [24, 5], [25, 5]);
  px = await composite(page);
  expect(px(24, 5)).toEqual(BLUE);
  expect(await undoDepth(page)).toBe(steps + 2);

  // Undo restores the pixels, redo the stroke; neither moves the offset.
  await undoButton(page).click();
  px = await composite(page);
  expect(px(24, 5)).toEqual(start(24, 5));
  expect(px(24, 16)).toEqual(RED);
  await redoButton(page).click();
  expect((await composite(page))(24, 5)).toEqual(BLUE);
  await undoButton(page).click();
  await stroke(page, [25, 5], [26, 5]);
  expect((await composite(page))(25, 5)).toEqual(BLUE);

  // A new source: the next stroke fixes a new offset (blue → (20, 24)).
  await press(page, 8, 5, true);
  await expectCrosshairAt(page, 8.5, 5.5);
  await stroke(page, [20, 24], [21, 24]);
  expect((await composite(page))(20, 24)).toEqual(BLUE);
});

test('Set source makes the next press set the source and switches off (AC-04)', async ({
  page,
}) => {
  await paintedDocument(page);
  const steps = await undoDepth(page);
  await setSource(page).click();
  await expect(setSource(page)).toHaveAttribute('aria-pressed', 'true');
  await press(page, 8, 16);
  await expect(setSource(page)).toHaveAttribute('aria-pressed', 'false');
  await expectCrosshairAt(page, 8.5, 16.5);
  expect(await undoDepth(page)).toBe(steps);
  await stroke(page, [24, 16], [25, 16]);
  expect((await composite(page))(24, 16)).toEqual(RED);
});

test('a stroke without a source paints nothing and says how to set one (AC-13)', async ({
  page,
}) => {
  await paintedDocument(page);
  const start = await composite(page);
  const steps = await undoDepth(page);
  await stroke(page, [20, 16], [26, 16]);
  await expect(page.getByText(NO_SOURCE)).toBeVisible();
  expect(await undoDepth(page)).toBe(steps);
  expect((await composite(page))(24, 16)).toEqual(start(24, 16));
  // Setting the source clears the notice.
  await press(page, 8, 16, true);
  await expect(page.getByText(NO_SOURCE)).toHaveCount(0);
});

test('a locked layer refuses the stroke with the Brush notice; the source can still be set (AC-14)', async ({
  page,
}) => {
  await paintedDocument(page);
  const start = await composite(page);
  const name = (
    (await page.getByRole('button', { name: /^Lock / }).getAttribute('aria-label')) ?? ''
  ).replace('Lock ', '');
  const lock = page.getByRole('button', { name: `Lock ${name}`, exact: true });
  await lock.click();
  // Locking is an undo step of its own; count from here.
  const steps = await undoDepth(page);
  const locked = page.getByText(`“${name}” is locked`);

  // No source: the locked notice wins.
  await stroke(page, [20, 16], [26, 16]);
  await expect(locked).toBeVisible();
  await expect(page.getByText(NO_SOURCE)).toHaveCount(0);

  // Setting the source works on the locked layer; painting still does not.
  await press(page, 8, 16, true);
  await expectCrosshairAt(page, 8.5, 16.5);
  await stroke(page, [24, 16], [26, 16]);
  await expect(locked).toBeVisible();
  expect(await undoDepth(page)).toBe(steps);
  expect((await composite(page))(24, 16)).toEqual(start(24, 16));

  await page.getByRole('button', { name: `Unlock ${name}`, exact: true }).click();
  await stroke(page, [24, 16], [26, 16]);
  expect((await composite(page))(24, 16)).toEqual(RED);
});

test('the crosshair marks the source at a constant size, then follows the pointer (AC-15)', async ({
  page,
}) => {
  await paintedDocument(page);
  await expect(crosshair(page)).toHaveCount(0);
  await press(page, 8, 16, true);
  await expectCrosshairAt(page, 8.5, 16.5);
  const small = await crosshairAt(page);
  expect([Math.round(small.w), Math.round(small.h)]).toEqual([16, 16]);

  // Another zoom: the same on-screen size, still on the source.
  const before = await scale(page);
  await page.getByRole('button', { name: 'Zoom out' }).click();
  await expect.poll(() => scale(page)).not.toBe(before);
  await expectCrosshairAt(page, 8.5, 16.5);
  const zoomed = await crosshairAt(page);
  expect([Math.round(zoomed.w), Math.round(zoomed.h)]).toEqual([16, 16]);
  await page.getByRole('button', { name: 'Fit to window' }).click();

  // Once a stroke fixed the offset (-16, 0): pointer plus offset.
  await stroke(page, [24, 16], [25, 16]);
  const [hx, hy] = await screen(page, 27.25, 10.75);
  await page.mouse.move(hx, hy);
  await expectCrosshairAt(page, 11.25, 10.75);
  // While painting, too.
  await page.mouse.down();
  const [mx, my] = await screen(page, 29.5, 12.5);
  await page.mouse.move(mx, my, { steps: 3 });
  await expectCrosshairAt(page, 13.5, 12.5);
  await page.mouse.up();

  // Off the canvas while hovering: hidden. Another tool: hidden.
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.move(box.x - 10, box.y + box.height / 2);
  await expect(crosshair(page)).toHaveCount(0);
  await page.mouse.move(hx, hy);
  await expect(crosshair(page)).toBeVisible();
  await page.keyboard.press('b');
  await expect(crosshair(page)).toHaveCount(0);
});

test('each tab keeps its own source through tool, layer and tab switches (AC-16)', async ({
  page,
}) => {
  await paintedDocument(page);
  await press(page, 8, 16, true);
  await expectCrosshairAt(page, 8.5, 16.5);

  // Tool and layer switches keep it.
  await page.keyboard.press('b');
  await expect(crosshair(page)).toHaveCount(0);
  await page.keyboard.press('c');
  await expectCrosshairAt(page, 8.5, 16.5);
  await menu(page, 'Layer', 'New Layer');
  await expectCrosshairAt(page, 8.5, 16.5);

  // A second document has none of its own.
  await menu(page, 'File', 'New…');
  await page
    .getByRole('dialog', { name: 'New document' })
    .getByRole('button', { name: 'Create' })
    .click();
  const tabs = page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
  await expect(tabs).toHaveCount(2);
  await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(imageCanvas(page)).toHaveAttribute('data-ready', 'true');
  await expect(crosshair(page)).toHaveCount(0);
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(page.getByText(NO_SOURCE)).toBeVisible();

  // Back in the first tab the source is there again; closing the second
  // tab leaves it alone.
  await tabs.nth(0).click();
  await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
  await expectCrosshairAt(page, 8.5, 16.5);
  await page
    .getByRole('tablist', { name: 'Open documents' })
    .getByRole('button', { name: /^Close / })
    .nth(1)
    .click();
  await expect(tabs).toHaveCount(1);
  await expectCrosshairAt(page, 8.5, 16.5);
});

test('Ctrl+press or Set source beside the canvas changes nothing (AC-17)', async ({ page }) => {
  await paintedDocument(page);
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const beside: [number, number] = [box.x - 12, box.y + box.height / 2];
  const ctrlClick = async (): Promise<void> => {
    await page.keyboard.down('Control');
    await page.mouse.click(...beside);
    await page.keyboard.up('Control');
  };
  // No source yet: none afterwards either.
  await ctrlClick();
  await expect(crosshair(page)).toHaveCount(0);
  await setSource(page).click();
  await page.mouse.click(...beside);
  await expect(crosshair(page)).toHaveCount(0);
  await expect(setSource(page)).toHaveAttribute('aria-pressed', 'true');
  await setSource(page).click();

  // With a source, it stays where it was.
  const start = await composite(page);
  await press(page, 8, 16, true);
  await ctrlClick();
  await expectCrosshairAt(page, 8.5, 16.5);
  const px = await composite(page);
  for (const [x, y] of [
    [0, 16],
    [8, 16],
  ] as const) {
    expect(px(x, y)).toEqual(start(x, y));
  }
});

test('Sample All Layers copies the visible composite into the active layer (AC-09)', async ({
  page,
}) => {
  await paintedDocument(page);
  await menu(page, 'Layer', 'New Layer');
  const steps = await undoDepth(page);
  await press(page, 8, 16, true);
  // Current Layer reads the empty new layer: nothing changes.
  await stroke(page, [24, 16], [25, 16]);
  expect(await undoDepth(page)).toBe(steps);
  await page.getByRole('combobox', { name: /^Sample/ }).selectOption('all_layers');
  await stroke(page, [24, 16], [25, 16]);
  expect((await composite(page))(24, 16)).toEqual(RED);
  expect(await undoDepth(page)).toBe(steps + 1);
});

test('in German the tool, its options and its notice are translated (AC-18)', async ({ page }) => {
  await createDocument(page, 'image', { width: N, height: N });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await expect(page.getByRole('button', { name: 'Vergrößern' })).toBeVisible();
  await page.keyboard.press('c');
  const button = page.getByRole('button', { name: 'Kopierstempel', exact: true });
  await expect(button).toHaveAttribute('aria-pressed', 'true');
  await expect(button).toHaveAttribute('title', 'Kopierstempel (C)');
  await expect(page.getByRole('button', { name: 'Quelle festlegen' })).toHaveAttribute(
    'title',
    'Der nächste Klick auf die Leinwand legt den Quellpunkt fest',
  );
  await expect(page.getByText('Aufnehmen von')).toBeVisible();
  await stroke(page, [10, 10], [12, 10]);
  await expect(page.getByText('Erst mit Strg+Klick eine Quelle festlegen')).toBeVisible();
});

test.describe('on macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
  });

  test('⌘+click sets the source, Ctrl+click does not, and the hint names ⌘ (AC-03, AC-13)', async ({
    page,
  }) => {
    const MAC_HINT = '⌘+click to set a source first';
    await paintedDocument(page);
    const steps = await undoDepth(page);

    // Ctrl+click is the secondary click on macOS: no source.
    await press(page, 8, 16, true);
    await expect(page.getByText(MAC_HINT)).toBeVisible();
    await expect(crosshair(page)).toHaveCount(0);
    // A stroke without a source names ⌘, not Ctrl.
    await stroke(page, [20, 16], [26, 16]);
    await expect(page.getByText(MAC_HINT)).toBeVisible();
    await expect(page.getByText(NO_SOURCE)).toHaveCount(0);
    expect(await undoDepth(page)).toBe(steps);

    // ⌘+click sets it, without an undo step.
    await press(page, 8, 16, 'Meta');
    await expectCrosshairAt(page, 8.5, 16.5);
    await expect(page.getByText(MAC_HINT)).toHaveCount(0);
    expect(await undoDepth(page)).toBe(steps);
  });
});
