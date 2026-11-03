import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  pixelOf,
  waitForEditor,
  type ClipImage,
} from './helpers';

// Effects and adjustments in both modes (docs/specs/effects-adjustments.md):
// the shared menus and dialog, preview, cancel, the scope line and the
// refusals. Document state is read through Edit ▸ Copy (the clipboard);
// what the canvas shows is compared through screenshots.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const MODES = ['pixel', 'image'] as const;
type Mode = (typeof MODES)[number];

const RED = [255, 0, 0, 255];

const canvasOf = (page: Page, mode: Mode): Locator =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** Fills the (empty) active layer with opaque red: Bucket at the top-left. */
async function fillRed(page: Page, mode: Mode): Promise<void> {
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await page.keyboard.press('g');
  await canvasOf(page, mode).click({ position: { x: 4, y: 4 } });
}

/**
 * The active layer as a whole, read through Edit ▸ Copy; call without a
 * selection (Image mode's Deselect is an undo step of its own).
 */
async function activeLayer(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return copiedImage(page);
}

/**
 * A screenshot of a small patch left of the canvas centre, clear of the
 * effect dialog (top right) and of the pointer's brush outline.
 */
async function spot(page: Page, mode: Mode): Promise<() => Promise<Buffer>> {
  const box = await canvasOf(page, mode).boundingBox();
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

/** Waits until `shot` shows (`same = true`) or no longer shows `reference`. */
async function expectShot(shot: () => Promise<Buffer>, reference: Buffer, same: boolean) {
  await expect.poll(async () => (await shot()).equals(reference)).toBe(same);
}

const effectDialog = (page: Page) => page.getByRole('dialog');

test('image mode: effects and adjustments come from the shared catalog', async ({ page }) => {
  await createDocument(page, 'image', { width: 48, height: 32 });
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });

  await menu(page, 'Effects', 'Blur', 'Gaussian Blur…');
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('heading', { name: 'Gaussian Blur' })).toBeVisible();
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);

  await menu(page, 'Adjustments', 'Levels…');
  await expect(page.getByRole('dialog').getByLabel('Input white', { exact: true })).toHaveValue(
    '255',
  );
  await page.getByRole('dialog').getByRole('button', { name: 'OK' }).click();

  // Stroke, blur and levels are three undo steps.
  await undo.click();
  await undo.click();
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(undo).toBeDisabled();
});

test('pixel mode: the Levels adjustment applies', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await menu(page, 'Adjustments', 'Levels…');
  await page.getByRole('dialog').getByRole('button', { name: 'OK' }).click();
  await expect(page.getByText(/Levels applied/)).toBeVisible();
});

test('image mode: an effect stays inside the selection', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 48 });
  const canvas = imageCanvas(page);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  // Screenshot clips of a corner (outside the selection) and the centre.
  const clip = (fx: number, fy: number) => ({
    x: box.x + box.width * fx - 3,
    y: box.y + box.height * fy - 3,
    width: 6,
    height: 6,
  });
  // A new image is transparent; fill it so inverting shows.
  await page.keyboard.press('g');
  await canvas.click({ position: { x: 4, y: 4 } });
  const corner = () => page.screenshot({ clip: clip(0.1, 0.15) });
  const centre = () => page.screenshot({ clip: clip(0.5, 0.5) });
  const cornerBefore = await corner();
  const centreBefore = await centre();

  await page.keyboard.press('m');
  await drag(page, canvas, [0.3, 0.3], [0.7, 0.7]);
  await menu(page, 'Adjustments', 'Invert Colors');
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('selection', { exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press('Control+d');

  await expect.poll(async () => (await centre()).equals(centreBefore)).toBe(false);
  expect((await corner()).equals(cornerBefore)).toBe(true);
});

/** Effects ▸ submenus and their entries, then the Adjustments menu (AC-01). */
const EFFECT_MENUS: [string, string[]][] = [
  ['Blur', ['Gaussian Blur…', 'Box Blur…', 'Motion Blur…', 'Radial Blur…']],
  ['Sharpen', ['Sharpen', 'Unsharp Mask…']],
  ['Distort', ['Emboss…', 'Edge Detect…', 'Relief…']],
  ['Noise', ['Add Noise…', 'Reduce Noise…']],
];
const ADJUSTMENTS = [
  'Brightness / Contrast…',
  'Hue / Saturation…',
  'Curves…',
  'Levels…',
  'Color Balance…',
  'Invert Colors',
  'Grayscale…',
  'Posterize…',
  'Threshold…',
];

for (const mode of MODES) {
  test(`${mode}: Effects and Adjustments list every entry, disabled without a document (AC-01)`, async ({
    page,
  }) => {
    const bar = page.getByRole('menubar');
    const effects = page.getByRole('menu', { name: 'Effects', exact: true });
    const adjustments = page.getByRole('menu', { name: 'Adjustments', exact: true });

    // Hold the core's wasm: the editor mounts its menus, but no document
    // arrives until the request is released.
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    await page.route('**/*.wasm', async (route) => {
      await held;
      await route.continue();
    });
    await gotoStart(page);
    const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
    await page.getByRole('button', { name }).click();
    await page.getByRole('form', { name }).getByRole('button', { name: 'Create' }).click();
    await bar.getByRole('menuitem', { name: 'Effects', exact: true }).click();
    await expect(effects.getByRole('menuitem')).toHaveText(EFFECT_MENUS.map(([group]) => group));
    for (const item of await effects.getByRole('menuitem').all()) await expect(item).toBeDisabled();
    await page.keyboard.press('Escape');
    await bar.getByRole('menuitem', { name: 'Adjustments', exact: true }).click();
    await expect(adjustments.getByRole('menuitem')).toHaveText(ADJUSTMENTS);
    for (const item of await adjustments.getByRole('menuitem').all()) {
      await expect(item).toBeDisabled();
    }
    await page.keyboard.press('Escape');
    release();
    await waitForEditor(page, mode);
    await page.unroute('**/*.wasm');

    await bar.getByRole('menuitem', { name: 'Effects', exact: true }).click();
    await expect(effects.getByRole('menuitem')).toHaveText(EFFECT_MENUS.map(([group]) => group));
    for (const [group, entries] of EFFECT_MENUS) {
      await effects.getByRole('menuitem', { name: group, exact: true }).click();
      const sub = page.getByRole('menu', { name: group, exact: true }).getByRole('menuitem');
      await expect(sub).toHaveText(entries);
      for (const item of await sub.all()) await expect(item).toBeEnabled();
    }
    await page.keyboard.press('Escape');
    await bar.getByRole('menuitem', { name: 'Adjustments', exact: true }).click();
    await expect(adjustments.getByRole('menuitem')).toHaveText(ADJUSTMENTS);
    for (const item of await adjustments.getByRole('menuitem').all()) {
      await expect(item).toBeEnabled();
    }
  });
}

/** A dialog parameter: its label, kind, default and a different valid value. */
type Param =
  | { kind: 'number'; label: string; value: number; other: number }
  | { kind: 'bool'; label: string; value: boolean }
  | { kind: 'choice'; label: string; value: string };

/** A number parameter `min`–`max` with default `value`. */
const num = (label: string, value: number, min: number, max: number): Param => ({
  kind: 'number',
  label,
  value,
  other: value === max ? min : max,
});
const bool = (label: string, value: boolean): Param => ({ kind: 'bool', label, value });
const choice = (label: string, value: string): Param => ({ kind: 'choice', label, value });

/** Every effect with its menu path and the defaults of the spec's table (AC-02). */
const DIALOGS: { path: [string, ...string[]]; title: string; params: Param[] }[] = [
  {
    path: ['Effects', 'Blur', 'Gaussian Blur…'],
    title: 'Gaussian Blur',
    params: [num('Radius', 2, 0.1, 250)],
  },
  {
    path: ['Effects', 'Blur', 'Box Blur…'],
    title: 'Box Blur',
    params: [num('Width', 3, 1, 251), num('Height', 3, 1, 251)],
  },
  {
    path: ['Effects', 'Blur', 'Motion Blur…'],
    title: 'Motion Blur',
    params: [num('Distance', 6, 1, 100), num('Angle', 0, 0, 360)],
  },
  {
    path: ['Effects', 'Blur', 'Radial Blur…'],
    title: 'Radial Blur',
    params: [
      num('Amount', 10, 1, 100),
      num('Center X', 0.5, 0, 1),
      num('Center Y', 0.5, 0, 1),
      choice('Kind', 'Spin'),
    ],
  },
  { path: ['Effects', 'Sharpen', 'Sharpen'], title: 'Sharpen', params: [] },
  {
    path: ['Effects', 'Sharpen', 'Unsharp Mask…'],
    title: 'Unsharp Mask',
    params: [num('Amount %', 50, 1, 500), num('Radius', 1, 0.1, 50), num('Threshold', 0, 0, 255)],
  },
  {
    path: ['Effects', 'Distort', 'Emboss…'],
    title: 'Emboss',
    params: [num('Angle', 45, 0, 360), num('Elevation', 45, 0, 90), num('Relief', 1, 0.1, 10)],
  },
  {
    path: ['Effects', 'Distort', 'Edge Detect…'],
    title: 'Edge Detect',
    params: [choice('Algorithm', 'Sobel'), num('Amount %', 100, 1, 400)],
  },
  {
    path: ['Effects', 'Distort', 'Relief…'],
    title: 'Relief',
    params: [num('Angle', 45, 0, 360), num('Amount', 1, 0.1, 10)],
  },
  {
    path: ['Effects', 'Noise', 'Add Noise…'],
    title: 'Add Noise',
    params: [
      num('Amount', 20, 1, 100),
      choice('Distribution', 'Uniform'),
      choice('Channels', 'RGB'),
      num('Seed', 1, 0, 9999),
    ],
  },
  {
    path: ['Effects', 'Noise', 'Reduce Noise…'],
    title: 'Reduce Noise',
    params: [num('Radius', 1, 1, 10)],
  },
  {
    path: ['Adjustments', 'Brightness / Contrast…'],
    title: 'Brightness / Contrast',
    params: [
      num('Brightness', 0, -150, 150),
      num('Contrast', 0, -150, 150),
      bool('Enhanced (S-curve)', false),
    ],
  },
  {
    path: ['Adjustments', 'Hue / Saturation…'],
    title: 'Hue / Saturation',
    params: [
      num('Hue', 0, -180, 180),
      num('Saturation', 0, -100, 100),
      num('Lightness', 0, -100, 100),
      bool('Colorize', false),
    ],
  },
  {
    path: ['Adjustments', 'Curves…'],
    title: 'Curves',
    params: [
      choice('Channel', 'Composite'),
      ...[0, 64, 128, 192, 255].map((x) => num(`Out @ ${x}`, x, 0, 255)),
    ],
  },
  {
    path: ['Adjustments', 'Levels…'],
    title: 'Levels',
    params: [
      choice('Channel', 'Composite'),
      num('Input black', 0, 0, 255),
      num('Input white', 255, 0, 255),
      num('Gamma', 1, 0.1, 10),
      num('Output black', 0, 0, 255),
      num('Output white', 255, 0, 255),
    ],
  },
  {
    path: ['Adjustments', 'Color Balance…'],
    title: 'Color Balance',
    params: [
      ...['Shadows', 'Midtones', 'Highlights'].flatMap((range) =>
        ['cyan–red', 'magenta–green', 'yellow–blue'].map((axis) =>
          num(`${range} ${axis}`, 0, -100, 100),
        ),
      ),
      bool('Preserve luminosity', true),
    ],
  },
  { path: ['Adjustments', 'Invert Colors'], title: 'Invert Colors', params: [] },
  {
    path: ['Adjustments', 'Grayscale…'],
    title: 'Grayscale',
    params: [
      choice('Method', 'Luminosity'),
      num('Red weight', 0.3, 0, 1),
      num('Green weight', 0.59, 0, 1),
      num('Blue weight', 0.11, 0, 1),
    ],
  },
  {
    path: ['Adjustments', 'Posterize…'],
    title: 'Posterize',
    params: [num('Levels', 4, 2, 64)],
  },
  {
    path: ['Adjustments', 'Threshold…'],
    title: 'Threshold',
    params: [num('Threshold', 128, 0, 255)],
  },
];

/** Asserts every parameter of `dialog` shows its default, as the right control. */
async function expectDefaults(dialog: Locator, params: Param[]): Promise<void> {
  for (const p of params) {
    if (p.kind === 'number') {
      await expect(dialog.getByRole('slider', { name: p.label, exact: true })).toHaveValue(
        String(p.value),
      );
      await expect(
        dialog.getByRole('spinbutton', { name: `${p.label} value`, exact: true }),
      ).toHaveValue(String(p.value));
    } else if (p.kind === 'bool') {
      const box = dialog.getByRole('checkbox', { name: p.label, exact: true });
      if (p.value) await expect(box).toBeChecked();
      else await expect(box).not.toBeChecked();
    } else {
      const select = dialog.getByRole('combobox', { name: p.label, exact: true });
      await expect(select.locator('option:checked')).toHaveText(p.value);
    }
  }
}

/** Gives every parameter of `dialog` a value other than its default. */
async function changeAll(dialog: Locator, params: Param[]): Promise<void> {
  for (const p of params) {
    if (p.kind === 'number') {
      const field = dialog.getByRole('spinbutton', { name: `${p.label} value`, exact: true });
      await field.fill(String(p.other));
      await expect(dialog.getByRole('slider', { name: p.label, exact: true })).toHaveValue(
        String(p.other),
      );
    } else if (p.kind === 'bool') {
      const box = dialog.getByRole('checkbox', { name: p.label, exact: true });
      await box.setChecked(!p.value);
    } else {
      const select = dialog.getByRole('combobox', { name: p.label, exact: true });
      await select.selectOption({ index: 1 });
      await expect(select.locator('option:checked')).not.toHaveText(p.value);
    }
  }
}

for (const mode of MODES) {
  test(`${mode}: every effect dialog shows its parameters at their defaults; Reset restores them (AC-02)`, async ({
    page,
  }) => {
    test.slow();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = effectDialog(page);
    const noParams = dialog.getByText('This effect has no parameters.');
    for (const { path, title, params } of DIALOGS) {
      await menu(page, ...path);
      await expect(dialog.getByRole('heading', { name: title, exact: true })).toBeVisible();
      // Exactly these controls: a slider and a number field per number, a
      // dropdown per choice, a checkbox per switch (plus Preview).
      const count = (kind: Param['kind']) => params.filter((p) => p.kind === kind).length;
      await expect(dialog.getByRole('slider')).toHaveCount(count('number'));
      await expect(dialog.getByRole('spinbutton')).toHaveCount(count('number'));
      await expect(dialog.getByRole('combobox')).toHaveCount(count('choice'));
      await expect(dialog.getByRole('checkbox')).toHaveCount(count('bool') + 1);
      if (params.length === 0) {
        await expect(noParams).toBeVisible();
      } else {
        await expect(noParams).toHaveCount(0);
        await expectDefaults(dialog, params);
        await changeAll(dialog, params);
        await dialog.getByRole('button', { name: 'Reset', exact: true }).click();
        await expectDefaults(dialog, params);
      }
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
      await expect(dialog).toHaveCount(0);
    }
  });
}

/** Two dialogs to preview: one without parameters, one with a parameter set. */
const PREVIEWS: { path: [string, ...string[]]; set?: (dialog: Locator) => Promise<void> }[] = [
  { path: ['Adjustments', 'Invert Colors'] },
  {
    path: ['Adjustments', 'Hue / Saturation…'],
    set: (dialog) => dialog.getByRole('spinbutton', { name: 'Hue value', exact: true }).fill('180'),
  },
];

for (const mode of MODES) {
  test(`${mode}: the preview shows the effect and leaves the document and its history alone (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await fillRed(page, mode);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await expect(undo).toBeEnabled();
    const shot = await spot(page, mode);
    const plain = await shot();
    const dialog = effectDialog(page);
    const preview = dialog.getByRole('checkbox', { name: 'Preview', exact: true });
    for (const { path, set } of PREVIEWS) {
      await menu(page, ...path);
      await expect(preview).toBeChecked();
      await set?.(dialog);
      await expectShot(shot, plain, false);
      const previewed = await shot();
      // Under the preview the document is still the red fill, one undo step.
      expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
      if (mode === 'pixel') await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
      await expect(dialog).toBeVisible();
      // Preview off: the image without the effect; on again: the effect.
      await preview.uncheck();
      await expectShot(shot, plain, true);
      await preview.check();
      await expectShot(shot, previewed, true);
      await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
      await expect(dialog).toHaveCount(0);
    }
    // History: still exactly the fill.
    await undo.click();
    await expect(undo).toBeDisabled();
  });

  test(`${mode}: Cancel and Escape close the dialog and change nothing (AC-05)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await fillRed(page, mode);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const shot = await spot(page, mode);
    const plain = await shot();
    const dialog = effectDialog(page);
    for (const close of ['Cancel', 'Escape'] as const) {
      for (const { path, set } of PREVIEWS) {
        await menu(page, ...path);
        await set?.(dialog);
        await expectShot(shot, plain, false);
        if (close === 'Cancel') {
          await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
        } else {
          await page.keyboard.press('Escape');
        }
        await expect(dialog).toHaveCount(0);
        await expectShot(shot, plain, true);
      }
    }
    expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
    if (mode === 'pixel') await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
    await undo.click();
    await expect(undo).toBeDisabled();
  });

  test(`${mode}: the dialog names its scope, selection or whole layer (AC-08)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = effectDialog(page);
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(dialog.getByText('whole layer', { exact: true })).toBeVisible();
    await expect(dialog.getByText('selection', { exact: true })).toHaveCount(0);
    await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();

    await page.keyboard.press('m');
    await drag(page, canvasOf(page, mode), [0.3, 0.3], [0.7, 0.7]);
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(dialog.getByText('selection', { exact: true })).toBeVisible();
    await expect(dialog.getByText('whole layer', { exact: true })).toHaveCount(0);
  });

  test(`${mode}: a locked layer stays unchanged and gains no undo step (AC-12)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await fillRed(page, mode);
    const lock = page.getByRole('button', { name: /^Lock / });
    const unlock = page.getByRole('button', { name: /^Unlock / });
    await lock.click();
    await expect(unlock).toBeVisible();
    await menu(page, 'Adjustments', 'Invert Colors');
    const dialog = effectDialog(page);
    await dialog.getByRole('button', { name: 'OK', exact: true }).click();
    // Both modes close the dialog and report the refusal.
    await expect(dialog).toHaveCount(0);
    await expect(page.getByText(/^Invert Colors failed: .*locked/)).toBeVisible();
    if (mode === 'pixel') {
      await expect(page.getByText('undo 2 / redo 0')).toBeVisible();
    }
    expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
    // The refused effect left no undo step: Undo reverts the lock itself.
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(lock).toBeVisible();
  });
}

test('image: an empty selection changes nothing and says so (AC-09)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await fillRed(page, 'image');
  await page.keyboard.press('ControlOrMeta+a');
  await menu(page, 'Select', 'Invert Selection');
  await menu(page, 'Adjustments', 'Invert Colors');
  const dialog = effectDialog(page);
  await expect(dialog.getByText('selection', { exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByText('Invert Colors: the selection is empty, nothing changed'),
  ).toBeVisible();
  await page.keyboard.press('ControlOrMeta+d');
  expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
});

/** The Pixel-mode status text `undo N / redo M`. */
const depthText = (page: Page) => page.getByText(/^undo \d+ \/ redo \d+$/);

/**
 * A 16 × 16 RGBA `.aseprite` file whose only layer has one 2 × 2 blue cel
 * at (0, 0) — a cel smaller than the canvas.
 */
function asepriteWithSmallCel(): Buffer {
  const chunk = (type: number, data: Buffer) => {
    const head = Buffer.alloc(6);
    head.writeUInt32LE(6 + data.length, 0);
    head.writeUInt16LE(type, 4);
    return Buffer.concat([head, data]);
  };
  const name = Buffer.alloc(2 + 'Layer 1'.length);
  name.writeUInt16LE('Layer 1'.length, 0);
  name.write('Layer 1', 2, 'latin1');
  const layer = Buffer.alloc(18);
  layer.writeUInt16LE(3, 0); // visible | editable
  layer.writeUInt8(255, 12); // opacity
  const cel = Buffer.alloc(20);
  cel.writeUInt8(255, 6); // opacity; layer 0 at (0, 0), raw cel
  cel.writeUInt16LE(2, 16); // width
  cel.writeUInt16LE(2, 18); // height
  const blue = Buffer.from(Array.from({ length: 4 }, () => [0, 0, 255, 255]).flat());
  const chunks = [
    chunk(0x2004, Buffer.concat([layer, name])),
    chunk(0x2005, Buffer.concat([cel, blue])),
  ];
  const body = Buffer.concat(chunks);
  const frame = Buffer.alloc(16);
  frame.writeUInt32LE(16 + body.length, 0);
  frame.writeUInt16LE(0xf1fa, 4);
  frame.writeUInt16LE(chunks.length, 6);
  frame.writeUInt16LE(100, 8);
  frame.writeUInt32LE(chunks.length, 12);
  const header = Buffer.alloc(128);
  header.writeUInt32LE(128 + 16 + body.length, 0);
  header.writeUInt16LE(0xa5e0, 4);
  header.writeUInt16LE(1, 6); // frames
  header.writeUInt16LE(16, 8);
  header.writeUInt16LE(16, 10);
  header.writeUInt16LE(32, 12); // RGBA
  header.writeUInt32LE(1, 14);
  header.writeUInt16LE(100, 18);
  header.writeUInt8(1, 34); // pixel width
  header.writeUInt8(1, 35); // pixel height
  return Buffer.concat([header, frame, body]);
}

test('pixel: a selection outside the active cel changes nothing and says so (AC-09)', async ({
  page,
}) => {
  // The <input type="file"> fallback instead of the File System Access picker.
  await page.addInitScript(() => {
    delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
  });
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'small-cel.aseprite',
    mimeType: 'application/octet-stream',
    buffer: asepriteWithSmallCel(),
  });
  await waitForEditor(page, 'pixel');
  await page.keyboard.press('m');
  await drag(page, pixelCanvas(page), [0.5, 0.5], [0.8, 0.8]);
  const depth = (await depthText(page).textContent()) ?? '';
  await menu(page, 'Adjustments', 'Invert Colors');
  await effectDialog(page).getByRole('button', { name: 'OK', exact: true }).click();
  await expect(
    page.getByText('Invert Colors: selection is outside the layer, nothing changed'),
  ).toBeVisible();
  await expect(depthText(page)).toHaveText(depth);
  await page.keyboard.press('ControlOrMeta+d');
  const layer = await activeLayer(page);
  expect(pixelOf(layer, 0, 0)).toEqual([0, 0, 255, 255]);
  expect(pixelOf(layer, 1, 1)).toEqual([0, 0, 255, 255]);
});

test('pixel: a layer without a cel on the current frame refuses the effect (AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await fillRed(page, 'pixel');
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
  const depth = (await depthText(page).textContent()) ?? '';
  await menu(page, 'Adjustments', 'Invert Colors');
  await effectDialog(page).getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.getByText(/^Invert Colors failed: /)).toBeVisible();
  await expect(depthText(page)).toHaveText(depth);
  // Frame 1 still holds the red fill.
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
});
