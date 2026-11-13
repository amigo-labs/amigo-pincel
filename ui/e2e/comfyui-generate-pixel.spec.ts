import { expect, test, type Locator, type Page } from '@playwright/test';
import { menu, newDocument, pixelCanvas, setColor, solidPng } from './helpers';
import { commandCalls, emit, installTauriMock, launchDesktop, reply } from './tauri-mock';

// The Generate panel in Pixel mode (docs/specs/comfyui-generate-pixel.md).
// The panel, its job and its variants are the ones of Image mode
// (comfyui-generate.spec.ts); here the mocked Tauri bridge answers the
// backend's commands and the test emits its `comfyui:generation` events,
// while the sprite side — the target box, the hard-edged input, the
// variants scaled to the box and the new layer — runs in the real core.
// Scaling and palette mapping are pinned in Rust
// (crates/pincel-core/src/generate.rs): indexed and grayscale sprites
// cannot be opened in the editor yet.

const SIZE = 32;
const PROMPT = 'a small green slime monster';

const MODELS = {
  checkpoints: [{ name: 'v1-5-pruned-emaonly-fp16.safetensors', family: 'sd15' }],
  loras: ['pixel.safetensors'],
};

const CONNECTED = {
  kind: 'connected',
  server: {
    url: 'http://127.0.0.1:8188',
    version: '0.3.40',
    gpu: null,
    checkpoints: MODELS.checkpoints.map((c) => c.name),
    loras: MODELS.loras,
    missingNodes: [],
    started: false,
  },
};

const panel = (page: Page): Locator => page.getByRole('region', { name: 'Generate', exact: true });
const generateButton = (page: Page): Locator =>
  panel(page).locator('div').getByRole('button', { name: 'Generate', exact: true });
const strip = (page: Page): Locator => panel(page).getByRole('listbox', { name: 'Generated variants' });
const preview = (page: Page): Locator => page.getByTestId('generate-preview');
const layerList = (page: Page): Locator => page.getByRole('list', { name: 'Layer list' });
const layerRows = (page: Page): Locator =>
  layerList(page).getByRole('button', { name: /^Drag to reorder / });
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });

/** The desktop app with a usable server and a `size`² Pixel document. */
async function setup(page: Page, size = SIZE): Promise<void> {
  await installTauriMock(page);
  await launchDesktop(page);
  await reply(page, 'comfyui_connect', CONNECTED);
  await reply(page, 'comfyui_generation_models', MODELS);
  await reply(page, 'comfyui_generate', { job: 1, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 64, 64, '#0000ff'))]);
  await newDocument(page, 'pixel', { width: size, height: size });
}

async function openPanel(page: Page): Promise<void> {
  const header = panel(page).locator('header').getByRole('button');
  if ((await header.getAttribute('aria-expanded')) !== 'true') await header.click();
  await expect(panel(page).getByRole('combobox').first()).toBeVisible();
  await panel(page).getByRole('textbox', { name: 'Prompt', exact: true }).fill(PROMPT);
}

async function requests(page: Page): Promise<Record<string, unknown>[]> {
  return (await commandCalls(page, 'comfyui_generate')).map((c) => c.args.request as Record<string, unknown>);
}

/** Screen point at the centre of sprite pixel `(x, y)`. */
async function at(page: Page, x: number, y: number, size = SIZE): Promise<[number, number]> {
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + ((x + 0.5) * box.width) / size, box.y + ((y + 0.5) * box.height) / size];
}

/** Marquee from pixel `(x0, y0)` to `(x1, y1)`, both inclusive. */
async function select(page: Page, x0: number, y0: number, x1: number, y1: number): Promise<void> {
  await page.keyboard.press('m');
  await page.mouse.move(...(await at(page, x0, y0)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, x1, y1)), { steps: 4 });
  await page.mouse.up();
}

/** RGB of sprite pixel `(x, y)` as the screen shows it. */
async function shownPixel(page: Page, x: number, y: number): Promise<number[]> {
  const [sx, sy] = await at(page, x, y);
  const shot = await page.screenshot({ clip: { x: sx, y: sy, width: 1, height: 1 } });
  return page.evaluate(async (b64) => {
    const bmp = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
    const canvas = new OffscreenCanvas(1, 1);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bmp, 0, 0);
    return Array.from(ctx.getImageData(0, 0, 1, 1).data.slice(0, 3));
  }, shot.toString('base64'));
}

/** Width, height and the RGBA of pixel `(x, y)` of a base64 PNG. */
async function pngPixel(page: Page, b64: string, x: number, y: number): Promise<number[]> {
  return page.evaluate(
    async ([data, px, py]) => {
      const bmp = await createImageBitmap(await (await fetch(`data:image/png;base64,${data}`)).blob());
      const canvas = new OffscreenCanvas(bmp.width, bmp.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bmp, 0, 0);
      return [bmp.width, bmp.height, ...Array.from(ctx.getImageData(px, py, 1, 1).data)];
    },
    [b64, x, y] as const,
  );
}

/** Runs a generation to its end with `count` variants. */
async function finished(page: Page, count = 2): Promise<void> {
  await generateButton(page).click();
  await expect.poll(async () => (await requests(page)).length).toBeGreaterThan(0);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count, error: null });
  await expect(strip(page).getByRole('option')).toHaveCount(count);
}

/** Natural sizes of the strip's images. */
async function thumbSizes(page: Page): Promise<number[][]> {
  return strip(page)
    .locator('img')
    .evaluateAll((imgs) => imgs.map((i) => [(i as HTMLImageElement).naturalWidth, (i as HTMLImageElement).naturalHeight]));
}

test('the desktop Pixel editor has a collapsed Generate panel beside the Assistant (AC-01)', async ({ page }) => {
  await setup(page);
  await expect(panel(page)).toBeVisible();
  await expect(panel(page).locator('header').getByRole('button')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByRole('region', { name: 'Assistant', exact: true })).toBeVisible();
  // Collapsed, it needs no server (comfyui-connect AC-03).
  expect(await commandCalls(page, 'comfyui_connect')).toHaveLength(0);
});

test('variants show at the target box size on the strip and the canvas; the document stays (AC-01, AC-02)', async ({
  page,
}) => {
  await setup(page);
  await select(page, 4, 6, 11, 13);
  await openPanel(page);
  await finished(page);
  // Text to image at SD 1.5's size: nothing on the layer to inpaint around.
  expect((await requests(page))[0]).toMatchObject({ width: 512, height: 512, strength: 1, input: null, mask: null });
  // Averaged down from 64² to the 8×8 selection.
  expect(await thumbSizes(page)).toEqual([
    [8, 8],
    [8, 8],
  ]);
  await expect(preview(page)).toHaveAttribute('data-variant', '0');
  const canvas = await pixelCanvas(page).boundingBox();
  const shown = await preview(page).boundingBox();
  if (!canvas || !shown) throw new Error('not visible');
  expect(shown.x - canvas.x).toBeCloseTo((4 * canvas.width) / SIZE, 0);
  expect(shown.width).toBeCloseTo((8 * canvas.width) / SIZE, 0);
  expect(await shownPixel(page, 6, 8)).toEqual([0, 0, 255]);
  await expect(panel(page).getByRole('note')).toHaveCount(0);
  await strip(page).getByRole('option').nth(1).click();
  await expect(preview(page)).toHaveAttribute('data-variant', '1');
  // A preview is not an edit: the selection is the only undo step.
  await expect(layerRows(page)).toHaveCount(1);
});

test('a target larger than the generation is enlarged with hard edges and noted (AC-02)', async ({ page }) => {
  await setup(page, 640);
  await openPanel(page);
  await finished(page, 1);
  expect((await requests(page))[0]).toMatchObject({ width: 512, height: 512 });
  expect(await thumbSizes(page)).toEqual([[640, 640]]);
  await expect(panel(page).getByRole('note')).toHaveText("Larger than the model's output");
});

test('Apply adds a layer above the active one with the variant in the box, as one undo step (AC-06)', async ({
  page,
}) => {
  await setup(page);
  await select(page, 4, 6, 11, 13);
  await openPanel(page);
  await finished(page);
  await panel(page).getByRole('button', { name: 'Apply' }).click();
  await expect(layerRows(page)).toHaveCount(2);
  await expect(layerList(page)).toContainText(`Generated: ${PROMPT.slice(0, 30)}`);
  await expect(preview(page)).toHaveCount(0);
  await expect(strip(page)).toHaveCount(0);
  await page.keyboard.press('ControlOrMeta+d');
  expect(await shownPixel(page, 6, 8)).toEqual([0, 0, 255]);
  expect(await shownPixel(page, 2, 2)).not.toEqual([0, 0, 255]);
  // One step: the selection is no undo step in Pixel mode.
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(1);
  await expect(undoButton(page)).toBeDisabled();
});

test('strength below 100 % sends the active layer enlarged with hard edges (AC-07)', async ({ page }) => {
  await setup(page);
  await page.getByTitle('Pencil (B)').click();
  await setColor(page, 'fg', '#ff0000');
  await page.mouse.click(...(await at(page, 1, 1)));
  await expect(undoButton(page)).toBeEnabled();
  await openPanel(page);
  await panel(page).getByRole('slider', { name: 'Strength', exact: true }).fill('50');
  await generateButton(page).click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const request = (await requests(page))[0];
  expect(request).toMatchObject({ strength: 0.5, width: 512, height: 512, mask: null });
  const input = request?.input;
  if (typeof input !== 'string') throw new Error('no input');
  // 32 → 512: each sprite pixel is a 16×16 block with hard edges.
  expect(await pngPixel(page, input, 16, 16)).toEqual([512, 512, 255, 0, 0, 255]);
  expect(await pngPixel(page, input, 31, 31)).toEqual([512, 512, 255, 0, 0, 255]);
  expect((await pngPixel(page, input, 15, 15))[5]).toBe(0);
  expect((await pngPixel(page, input, 32, 32))[5]).toBe(0);
});

test('closing a Pixel tab cancels its generation (AC-01)', async ({ page }) => {
  await setup(page);
  await openPanel(page);
  await generateButton(page).click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await menu(page, 'File', 'Close');
  await expect(page.getByRole('tab')).toHaveCount(0);
  await expect
    .poll(async () => (await commandCalls(page, 'comfyui_generation_cancel')).map((c) => c.args.job))
    .toEqual([1]);
});
