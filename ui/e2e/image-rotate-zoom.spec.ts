import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  dragFiles,
  gotoStart,
  imageCanvas,
  menu,
  waitForEditor,
  type ClipImage,
} from './helpers';

// Layer ▸ Rotate / Zoom in Image mode
// (docs/specs/image-transform-handles-rotate-zoom.md): the dialog, its live
// preview, Apply as one undo step, the identity, the locked-layer refusal
// and typed values. The resampling math is pinned in Rust
// (crates/pincel-effects/src/transform/tests.rs,
// crates/pincel-image-core/tests/rotate_zoom.rs); here Nearest at 90° is
// compared with Layer ▸ Rotate 90° CW through the clipboard.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const SIZE = 8;

/** Opens an 8 × 8 PNG with a distinct opaque colour per pixel (dropped on
 *  the start screen), so any turn or shift changes the layer. */
async function openGrid(page: Page): Promise<void> {
  await gotoStart(page);
  const bytes = await page.evaluate(async (n) => {
    const canvas = new OffscreenCanvas(n, n);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    const img = ctx.createImageData(n, n);
    for (let y = 0; y < n; y += 1) {
      for (let x = 0; x < n; x += 1) {
        img.data.set([x * 30 + 20, y * 30 + 20, (x * 7 + y * 13) % 256, 255], (y * n + x) * 4);
      }
    }
    ctx.putImageData(img, 0, 0);
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  }, SIZE);
  await dragFiles(page.getByRole('main'), 'drop', [{ name: 'grid.png', bytes }]);
  await waitForEditor(page, 'image');
}

/** The active layer, read through Edit ▸ Copy (no selection may exist). */
async function activeLayer(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

/** A screenshot of a patch left of the canvas centre, clear of the dialog. */
async function spot(page: Page): Promise<() => Promise<Buffer>> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  const clip = {
    x: box.x + box.width * 0.3 - 3,
    y: box.y + box.height * 0.5 - 3,
    width: 6,
    height: 6,
  };
  return async () => {
    await page.mouse.move(1, 1);
    return page.screenshot({ clip });
  };
}

async function expectShot(shot: () => Promise<Buffer>, reference: Buffer, same: boolean) {
  await expect.poll(async () => (await shot()).equals(reference)).toBe(same);
}

const dialog = (page: Page): Locator => page.getByRole('dialog');
const field = (page: Page, label: string): Locator =>
  dialog(page).getByRole('spinbutton', { name: `${label} value`, exact: true });
const quality = (page: Page): Locator =>
  dialog(page).getByRole('combobox', { name: 'Quality', exact: true });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });

async function openRotateZoom(page: Page): Promise<void> {
  await menu(page, 'Layer', 'Rotate / Zoom…');
  await expect(dialog(page).getByRole('heading', { name: 'Rotate / Zoom' })).toBeVisible();
}

/** Types `value` into the field of `label` and commits it (Tab). */
async function typeValue(page: Page, label: string, value: string): Promise<void> {
  await field(page, label).fill(value);
  await field(page, label).press('Tab');
}

test('Layer ▸ Rotate / Zoom… opens the dialog with its controls and defaults (AC-01)', async ({
  page,
}) => {
  await openGrid(page);
  await openRotateZoom(page);
  const ranges: [string, string, string, string, string][] = [
    ['Angle', '0', '-180', '180', '0.1'],
    ['Pan X', '0', '-100', '100', '0.1'],
    ['Pan Y', '0', '-100', '100', '0.1'],
    ['Zoom', '1', '0.1', '10', '0.01'],
  ];
  for (const [label, value, min, max, step] of ranges) {
    const input = field(page, label);
    await expect(input).toHaveValue(value);
    await expect(input).toHaveAttribute('min', min);
    await expect(input).toHaveAttribute('max', max);
    await expect(input).toHaveAttribute('step', step);
  }
  await expect(quality(page).locator('option')).toHaveText(['Nearest', 'Bilinear', 'Bicubic']);
  await expect(quality(page).locator('option:checked')).toHaveText('Bilinear');
  // Rotate / Zoom lives in the Layer menu only, not among the effects.
  await page.keyboard.press('Escape');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Effects', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Distort', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'Rotate / Zoom…' })).toHaveCount(0);
});

test('without a document the item cannot be chosen (AC-01)', async ({ page }) => {
  // Hold the core's wasm: the editor gets no document until released.
  let release = () => {};
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route('**/*.wasm', async (route) => {
    await held;
    await route.continue();
  });
  await gotoStart(page);
  await page.getByRole('button', { name: 'New image' }).click();
  await page
    .getByRole('form', { name: 'New image' })
    .getByRole('button', { name: 'Create' })
    .click();
  await expect(page.getByText('Loading the Image editor…')).toBeVisible();
  await expect(page.getByRole('menubar')).toHaveCount(0);
  release();
  await waitForEditor(page, 'image');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Layer', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'Rotate / Zoom…' })).toBeEnabled();
});

test('the preview turns the layer; Cancel and Escape leave document and history (AC-02)', async ({
  page,
}) => {
  await openGrid(page);
  const original = await activeLayer(page);
  const shot = await spot(page);
  const plain = await shot();
  // A redo step that a new undo step would discard.
  await menu(page, 'Layer', 'Rotate 90° CW');
  await expectShot(shot, plain, false);
  const turned = await shot();
  await undoButton(page).click();
  await expectShot(shot, plain, true);
  await expect(undoButton(page)).toBeDisabled();
  await expect(redoButton(page)).toBeEnabled();

  for (const close of ['Cancel', 'Escape'] as const) {
    await openRotateZoom(page);
    await typeValue(page, 'Angle', '90');
    await quality(page).selectOption('Nearest');
    await expectShot(shot, turned, true);
    await typeValue(page, 'Pan X', '25');
    await typeValue(page, 'Zoom', '0.5');
    await expectShot(shot, turned, false);
    if (close === 'Cancel') {
      await dialog(page).getByRole('button', { name: 'Cancel', exact: true }).click();
    } else {
      await page.keyboard.press('Escape');
    }
    await expect(dialog(page)).toHaveCount(0);
    await expectShot(shot, plain, true);
    await expect(undoButton(page)).toBeDisabled();
    await expect(redoButton(page)).toBeEnabled();
  }
  expect(await activeLayer(page)).toEqual(original);
});

test('Apply keeps the preview as one undo step; Nearest 90° equals Rotate 90° CW (AC-03, AC-06)', async ({
  page,
}) => {
  await openGrid(page);
  const original = await activeLayer(page);
  await menu(page, 'Layer', 'Rotate 90° CW');
  const turned = await activeLayer(page);
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
  expect(turned).not.toEqual(original);

  const shot = await spot(page);
  const plain = await shot();
  await openRotateZoom(page);
  await typeValue(page, 'Angle', '90');
  await quality(page).selectOption('Nearest');
  await expectShot(shot, plain, false);
  const previewed = await shot();
  await dialog(page).getByRole('button', { name: 'OK', exact: true }).click();
  await expect(dialog(page)).toHaveCount(0);
  await expectShot(shot, previewed, true);
  expect(await activeLayer(page)).toEqual(turned);

  // Exactly one step: one Undo restores every pixel and empties the history.
  await expect(undoButton(page)).toBeEnabled();
  await undoButton(page).click();
  expect(await activeLayer(page)).toEqual(original);
  await expect(undoButton(page)).toBeDisabled();
});

test('the identity changes no pixel and adds no undo step (AC-05)', async ({ page }) => {
  await openGrid(page);
  const original = await activeLayer(page);
  await menu(page, 'Layer', 'Rotate 90° CW');
  await undoButton(page).click();
  await expect(redoButton(page)).toBeEnabled();
  for (const q of ['Nearest', 'Bilinear', 'Bicubic']) {
    await openRotateZoom(page);
    await quality(page).selectOption(q);
    await dialog(page).getByRole('button', { name: 'OK', exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    // No step: Undo stays off and the redo step survives; no notice.
    await expect(undoButton(page)).toBeDisabled();
    await expect(redoButton(page)).toBeEnabled();
    await expect(page.getByText(/nothing changed/)).toHaveCount(0);
  }
  expect(await activeLayer(page)).toEqual(original);
});

/** The names in the layers panel, top first. */
async function layerNames(page: Page): Promise<string[]> {
  return page
    .getByRole('list', { name: 'Layer list' })
    .getByRole('button', { name: /^Drag to reorder / })
    .evaluateAll((els) =>
      els.map((el) => (el.getAttribute('aria-label') ?? '').replace('Drag to reorder ', '')),
    );
}

function layerRow(page: Page, name: string): Locator {
  return page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });
}

test('a locked layer or a group refuses with the locked-layer notice (AC-08)', async ({ page }) => {
  await openGrid(page);
  const original = await activeLayer(page);
  const [name] = (await layerNames(page)) as [string];
  const refused = async (who: string) => {
    await menu(page, 'Layer', 'Rotate / Zoom…');
    await expect(page.getByText(`“${who}” is locked`)).toBeVisible();
    await expect(dialog(page)).toHaveCount(0);
  };
  // The layer locked itself.
  await page.getByRole('button', { name: `Lock ${name}`, exact: true }).click();
  await refused(name);
  await page.getByRole('button', { name: `Unlock ${name}`, exact: true }).click();

  // A group as the active layer.
  await menu(page, 'Layer', 'New Group');
  await refused('Group 1');

  // The layer inside a locked group.
  const grip = page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true });
  const a = await grip.boundingBox();
  const b = await layerRow(page, 'Group 1').boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height * 0.5, { steps: 6 });
  await page.mouse.up();
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  await page.getByRole('button', { name: 'Lock Group 1', exact: true }).click();
  await layerRow(page, name)
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click();
  await refused(name);

  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy Merged');
  expect(await copiedImage(page)).toEqual(original);
});

test('typed values outside a range become the nearest bound; text keeps the value (AC-09)', async ({
  page,
}) => {
  await openGrid(page);
  await openRotateZoom(page);
  const cases: [string, string, string][] = [
    ['Angle', '500', '180'],
    ['Angle', '-999', '-180'],
    ['Pan X', '250', '100'],
    ['Pan Y', '-101', '-100'],
    ['Zoom', '50', '10'],
    ['Zoom', '0.01', '0.1'],
  ];
  for (const [label, typed, bound] of cases) {
    await typeValue(page, label, typed);
    await expect(field(page, label)).toHaveValue(bound);
    await expect(dialog(page).getByRole('slider', { name: label, exact: true })).toHaveValue(bound);
  }
  // A non-numeric entry ("-" alone, "e") keeps the previous value.
  await typeValue(page, 'Pan X', '12.5');
  for (const junk of ['-', 'e']) {
    await field(page, 'Pan X').fill('');
    await field(page, 'Pan X').pressSequentially(junk);
    await field(page, 'Pan X').press('Tab');
    await expect(field(page, 'Pan X')).toHaveValue('12.5');
  }
  // The bound is what Apply uses: 180° equals Image ▸ Rotate 180° here.
  await typeValue(page, 'Pan X', '0');
  await typeValue(page, 'Pan Y', '0');
  await typeValue(page, 'Zoom', '1');
  await typeValue(page, 'Angle', '720');
  await quality(page).selectOption('Nearest');
  await dialog(page).getByRole('button', { name: 'OK', exact: true }).click();
  await expect(dialog(page)).toHaveCount(0);
  const applied = await activeLayer(page);
  await undoButton(page).click();
  await menu(page, 'Image', 'Rotate 180°');
  expect(applied).toEqual(await activeLayer(page));
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the item, the dialog and its controls are German (AC-01, AC-03)', async ({ page }) => {
    await openGrid(page);
    await menu(page, 'Ebene', 'Drehen / Zoomen…');
    const panel = dialog(page);
    await expect(panel.getByRole('heading', { name: 'Drehen / Zoomen' })).toBeVisible();
    for (const label of ['Winkel', 'Verschieben X', 'Verschieben Y', 'Zoom']) {
      await expect(panel.getByRole('slider', { name: label, exact: true })).toBeVisible();
    }
    const select = panel.getByRole('combobox', { name: 'Qualität', exact: true });
    await expect(select.locator('option')).toHaveText([
      'Nächster Nachbar',
      'Bilinear',
      'Bikubisch',
    ]);
  });
});
