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
  setColor,
  type ClipImage,
  waitForEditor,
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
  await setColor(page, 'fg', '#ff0000');
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

  await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
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

/** An effect parameter: its label, kind, default and a different valid value. */
type Param =
  | { kind: 'number'; label: string; value: number; other: number }
  | { kind: 'bool'; label: string; value: boolean }
  | { kind: 'choice'; label: string; value: string }
  | { kind: 'angle'; label: string; value: number }
  | { kind: 'point'; label: string; x: string; y: string; value: number }
  | { kind: 'seed'; label: string; value: number; other: number }
  | { kind: 'pair'; first: string; second: string; value: number; other: number };

/** A number parameter `min`–`max` with default `value`. */
const num = (label: string, value: number, min: number, max: number): Param => ({
  kind: 'number',
  label,
  value,
  other: value === max ? min : max,
});
const bool = (label: string, value: boolean): Param => ({ kind: 'bool', label, value });
const choice = (label: string, value: string): Param => ({ kind: 'choice', label, value });
const angle = (label: string, value: number): Param => ({ kind: 'angle', label, value });
/** The Center point, X / Y in percent of the image (effect-catalog-v2). */
const center: Param = { kind: 'point', label: 'Center', x: 'Center X', y: 'Center Y', value: 50 };
const seed = (label: string, value: number, other: number): Param => ({
  kind: 'seed',
  label,
  value,
  other,
});

/** Every effect of the Technical notes table, with its menu path in the
 *  current catalog (effect-catalog-v2), the modes that list it and its
 *  defaults as the later effect specs set them (effects-blurs,
 *  effects-object-stylize-color-noise, adjustments-paintnet) (AC-02). */
const IMAGE_ONLY = ['image'] as const;
const BOTH_MODES = ['pixel', 'image'] as const;
const DIALOGS: {
  path: [string, ...string[]];
  title: string;
  modes: readonly Mode[];
  params: Param[];
}[] = [
  {
    path: ['Effects', 'Blurs', 'Gaussian Blur…'],
    title: 'Gaussian Blur',
    modes: IMAGE_ONLY,
    params: [num('Radius', 2, 0.1, 250)],
  },
  {
    path: ['Effects', 'Blurs', 'Square Blur…'],
    title: 'Square Blur',
    modes: IMAGE_ONLY,
    params: [{ kind: 'pair', first: 'Width', second: 'Height', value: 3, other: 251 }],
  },
  {
    path: ['Effects', 'Blurs', 'Motion Blur…'],
    title: 'Motion Blur',
    modes: IMAGE_ONLY,
    params: [num('Distance', 6, 1, 100), angle('Angle', 0), bool('Centered', true)],
  },
  {
    path: ['Effects', 'Blurs', 'Radial Blur…'],
    title: 'Radial Blur',
    modes: IMAGE_ONLY,
    params: [num('Amount', 10, 1, 100), center],
  },
  { path: ['Effects', 'Photo', 'Sharpen'], title: 'Sharpen', modes: IMAGE_ONLY, params: [] },
  {
    path: ['Effects', 'Photo', 'Unsharp Mask…'],
    title: 'Unsharp Mask',
    modes: IMAGE_ONLY,
    params: [num('Amount %', 50, 1, 500), num('Radius', 1, 0.1, 50), num('Threshold', 0, 0, 255)],
  },
  {
    path: ['Effects', 'Stylize', 'Emboss…'],
    title: 'Emboss',
    modes: IMAGE_ONLY,
    params: [angle('Angle', 45), num('Elevation', 45, 0, 90), num('Relief', 1, 0.1, 10)],
  },
  {
    path: ['Effects', 'Stylize', 'Edge Detect…'],
    title: 'Edge Detect',
    modes: IMAGE_ONLY,
    params: [choice('Algorithm', 'Sobel'), num('Amount %', 100, 1, 400)],
  },
  {
    path: ['Effects', 'Stylize', 'Relief…'],
    title: 'Relief',
    modes: IMAGE_ONLY,
    params: [angle('Angle', 45), num('Amount', 1, 0.1, 10)],
  },
  {
    path: ['Effects', 'Noise', 'Add Noise…'],
    title: 'Add Noise',
    modes: BOTH_MODES,
    params: [
      num('Amount', 20, 1, 100),
      choice('Distribution', 'Uniform'),
      choice('Channels', 'RGB'),
      seed('Seed', 1, 9999),
    ],
  },
  {
    path: ['Effects', 'Noise', 'Reduce Noise…'],
    title: 'Reduce Noise',
    modes: IMAGE_ONLY,
    params: [num('Radius', 6, 1, 200), num('Strength', 40, 0, 100)],
  },
  {
    path: ['Adjustments', 'Brightness / Contrast…'],
    title: 'Brightness / Contrast',
    modes: BOTH_MODES,
    params: [
      num('Brightness', 0, -150, 150),
      num('Contrast', 0, -150, 150),
      bool('Enhanced (S-curve)', false),
    ],
  },
  {
    path: ['Adjustments', 'Hue / Saturation…'],
    title: 'Hue / Saturation',
    modes: BOTH_MODES,
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
    modes: BOTH_MODES,
    params: [
      choice('Channel', 'Composite'),
      ...[0, 64, 128, 192, 255].map((x) => num(`Out @ ${x}`, x, 0, 255)),
    ],
  },
  {
    path: ['Adjustments', 'Levels…'],
    title: 'Levels',
    modes: BOTH_MODES,
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
    modes: BOTH_MODES,
    params: [
      ...['Shadows', 'Midtones', 'Highlights'].flatMap((range) =>
        ['cyan–red', 'magenta–green', 'yellow–blue'].map((axis) =>
          num(`${range} ${axis}`, 0, -100, 100),
        ),
      ),
      bool('Preserve luminosity', true),
    ],
  },
  {
    path: ['Adjustments', 'Invert Colors'],
    title: 'Invert Colors',
    modes: BOTH_MODES,
    params: [],
  },
  {
    path: ['Adjustments', 'Black and White…'],
    title: 'Black and White',
    modes: BOTH_MODES,
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
    modes: BOTH_MODES,
    params: [num('Levels', 4, 2, 64)],
  },
  {
    path: ['Adjustments', 'Threshold…'],
    title: 'Threshold',
    modes: BOTH_MODES,
    params: [num('Threshold', 128, 0, 255)],
  },
];

const valueField = (panel: Locator, label: string) =>
  panel.getByRole('spinbutton', { name: `${label} value`, exact: true });

/** Asserts every parameter of `panel` shows its default, as the right control. */
async function expectDefaults(panel: Locator, params: Param[]): Promise<void> {
  for (const p of params) {
    switch (p.kind) {
      case 'number':
        await expect(panel.getByRole('slider', { name: p.label, exact: true })).toHaveValue(
          String(p.value),
        );
        await expect(valueField(panel, p.label)).toHaveValue(String(p.value));
        break;
      case 'bool': {
        const box = panel.getByRole('checkbox', { name: p.label, exact: true });
        if (p.value) await expect(box).toBeChecked();
        else await expect(box).not.toBeChecked();
        break;
      }
      case 'choice': {
        const select = panel.getByRole('combobox', { name: p.label, exact: true });
        await expect(select.locator('option:checked')).toHaveText(p.value);
        break;
      }
      case 'angle':
        await expect(
          panel.getByRole('slider', { name: `${p.label} dial`, exact: true }),
        ).toHaveAttribute('aria-valuenow', String(p.value));
        await expect(
          panel.getByRole('spinbutton', { name: `${p.label} in degrees`, exact: true }),
        ).toHaveValue(String(p.value));
        break;
      case 'point':
        for (const axis of [p.x, p.y]) {
          await expect(
            panel.getByRole('spinbutton', { name: `${axis} in percent`, exact: true }),
          ).toHaveValue(String(p.value));
        }
        break;
      case 'seed':
        await expect(valueField(panel, p.label)).toHaveValue(String(p.value));
        await expect(panel.getByRole('button', { name: 'Reseed', exact: true })).toBeVisible();
        break;
      case 'pair':
        for (const label of [p.first, p.second]) {
          await expect(panel.getByRole('slider', { name: label, exact: true })).toHaveValue(
            String(p.value),
          );
          await expect(valueField(panel, label)).toHaveValue(String(p.value));
        }
        await expect(
          panel.getByRole('checkbox', { name: `Link ${p.first} and ${p.second}`, exact: true }),
        ).toBeChecked();
        break;
    }
  }
}

/** Gives every parameter of `panel` a value other than its default. */
async function changeAll(panel: Locator, params: Param[]): Promise<void> {
  for (const p of params) {
    switch (p.kind) {
      case 'number':
        await valueField(panel, p.label).fill(String(p.other));
        await expect(panel.getByRole('slider', { name: p.label, exact: true })).toHaveValue(
          String(p.other),
        );
        break;
      case 'bool':
        await panel.getByRole('checkbox', { name: p.label, exact: true }).setChecked(!p.value);
        break;
      case 'choice': {
        const select = panel.getByRole('combobox', { name: p.label, exact: true });
        await select.selectOption({ index: 1 });
        await expect(select.locator('option:checked')).not.toHaveText(p.value);
        break;
      }
      case 'angle': {
        const field = panel.getByRole('spinbutton', { name: `${p.label} in degrees`, exact: true });
        await field.fill(String((p.value + 90) % 360));
        await field.press('Tab');
        await expect(
          panel.getByRole('slider', { name: `${p.label} dial`, exact: true }),
        ).toHaveAttribute('aria-valuenow', String((p.value + 90) % 360));
        break;
      }
      case 'point':
        for (const axis of [p.x, p.y]) {
          await panel
            .getByRole('spinbutton', { name: `${axis} in percent`, exact: true })
            .fill('20');
        }
        break;
      case 'seed':
        await valueField(panel, p.label).fill(String(p.other));
        break;
      case 'pair':
        // Linked: moving the first moves the second too.
        await valueField(panel, p.first).fill(String(p.other));
        await expect(valueField(panel, p.second)).toHaveValue(String(p.other));
        await panel
          .getByRole('checkbox', { name: `Link ${p.first} and ${p.second}`, exact: true })
          .setChecked(false);
        break;
    }
  }
}

/** The controls each parameter kind adds, for the exact-count check. */
function controlCounts(params: Param[]): { sliders: number; fields: number; boxes: number } {
  let sliders = 0;
  let fields = 0;
  let boxes = 0;
  for (const p of params) {
    if (p.kind === 'number') [sliders, fields] = [sliders + 1, fields + 1];
    else if (p.kind === 'angle') [sliders, fields] = [sliders + 1, fields + 1];
    else if (p.kind === 'point') fields += 2;
    else if (p.kind === 'seed') fields += 1;
    else if (p.kind === 'pair') [sliders, fields, boxes] = [sliders + 2, fields + 2, boxes + 1];
    else if (p.kind === 'bool') boxes += 1;
  }
  return { sliders, fields, boxes };
}

for (const mode of MODES) {
  test(`${mode}: every effect panel shows its parameters at their defaults; Reset restores them (AC-02)`, async ({
    page,
  }) => {
    test.slow();
    await createDocument(page, mode, { width: 16, height: 16 });
    const panel = effectDialog(page);
    const noParams = panel.getByText('This effect has no parameters.');
    for (const { path, title, params } of DIALOGS.filter((d) => d.modes.includes(mode))) {
      await menu(page, ...path);
      await expect(panel.getByRole('heading', { name: title, exact: true })).toBeVisible();
      // Exactly these controls: per number a slider and a number field, per
      // angle a dial and a degree field, per point two percent fields, per
      // seed a field and Reseed, per linked pair two sliders, two fields and
      // the link; a dropdown per choice, a checkbox per switch (plus Preview).
      const counts = controlCounts(params);
      await expect(panel.getByRole('slider')).toHaveCount(counts.sliders);
      await expect(panel.getByRole('spinbutton')).toHaveCount(counts.fields);
      await expect(panel.getByRole('combobox')).toHaveCount(
        params.filter((p) => p.kind === 'choice').length,
      );
      await expect(panel.getByRole('checkbox')).toHaveCount(counts.boxes + 1);
      if (params.length === 0) {
        await expect(noParams).toBeVisible();
      } else {
        await expect(noParams).toHaveCount(0);
        await expectDefaults(panel, params);
        await changeAll(panel, params);
        await panel.getByRole('button', { name: 'Reset', exact: true }).click();
        await expectDefaults(panel, params);
      }
      await panel.getByRole('button', { name: 'Cancel', exact: true }).click();
      await expect(panel).toHaveCount(0);
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
    // Both modes report the refusal and keep the panel open with its values
    // (effect-panel AC-16); Cancel then closes it.
    await expect(page.getByText(/^Invert Colors failed: .*locked/)).toBeVisible();
    await expect(dialog.getByRole('heading', { name: 'Invert Colors', exact: true })).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(dialog).toHaveCount(0);
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
  const panel = effectDialog(page);
  await panel.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.getByText(/^Invert Colors failed: /)).toBeVisible();
  await expect(depthText(page)).toHaveText(depth);
  // The failed effect keeps its panel (effect-panel AC-16), which keeps the
  // timeline inert (AC-03) until it closes.
  await expect(panel.getByRole('heading', { name: 'Invert Colors', exact: true })).toBeVisible();
  await panel.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(panel).toHaveCount(0);
  // Frame 1 still holds the red fill.
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  expect(pixelOf(await activeLayer(page), 8, 8)).toEqual(RED);
});
