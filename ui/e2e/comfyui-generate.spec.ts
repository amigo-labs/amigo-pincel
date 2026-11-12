import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  imageCanvas,
  menu,
  newDocument,
  setColor,
  solidPng,
  storedPref,
  switchLanguage,
} from './helpers';
import { historyRows, openHistory, rowNames } from './history-helpers';
import { commandCalls, emit, installTauriMock, launchDesktop, reply } from './tauri-mock';

// The Generate panel (docs/specs/comfyui-generate.md). The generation runs
// in the backend (src-tauri/src/comfyui/generate.rs, tested in Rust against
// a fake ComfyUI); here the mocked Tauri bridge answers its commands and
// the test emits its `comfyui:generation` events.

const SIZE = 64;
const PROMPT = 'a red apple on a table, photorealistic, warm light';

function connected(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    kind: 'connected',
    server: {
      url: 'http://127.0.0.1:8188',
      version: '0.3.40',
      gpu: null,
      checkpoints: ['v1-5-pruned-emaonly-fp16.safetensors', 'my-model.safetensors'],
      loras: ['pixel.safetensors'],
      missingNodes: [],
      started: false,
      ...over,
    },
  };
}

const MODELS = {
  checkpoints: [
    { name: 'v1-5-pruned-emaonly-fp16.safetensors', family: 'sd15' },
    { name: 'my-model.safetensors', family: null },
  ],
  loras: ['pixel.safetensors'],
};

const panel = (page: Page): Locator => page.getByRole('region', { name: 'Generate', exact: true });
const generateButton = (page: Page): Locator =>
  panel(page).locator('div').getByRole('button', { name: 'Generate', exact: true });
const strip = (page: Page): Locator => panel(page).getByRole('listbox', { name: 'Generated variants' });
const preview = (page: Page): Locator => page.getByTestId('generate-preview');
const layerRows = (page: Page): Locator =>
  page.getByRole('list', { name: 'Layer list' }).getByRole('listitem');
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });

/** The desktop app with a usable server and an Image document. */
async function setup(page: Page, connect: unknown = connected()): Promise<void> {
  await installTauriMock(page);
  await launchDesktop(page);
  await reply(page, 'comfyui_connect', connect);
  await reply(page, 'comfyui_generation_models', MODELS);
  await reply(page, 'comfyui_generate', { job: 1, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 64, 64, '#0000ff'))]);
  await newDocument(page, 'image', { width: SIZE, height: SIZE });
  await whiteLayer(page);
}

/** Fills the transparent layer white, so it has something to refine. */
async function whiteLayer(page: Page): Promise<void> {
  await setColor(page, 'fg', '#ffffff');
  await page.keyboard.press('g');
  await page.mouse.click(...(await point(page, 1, 1)));
  await expect(undoButton(page)).toBeEnabled();
}

/** Expands the Generate panel (it starts collapsed). */
async function openPanel(page: Page): Promise<void> {
  const header = panel(page).locator('header').getByRole('button');
  if ((await header.getAttribute('aria-expanded')) !== 'true') await header.click();
  await expect(panel(page).getByRole('combobox').first()).toBeVisible();
}

async function typePrompt(page: Page, text = PROMPT): Promise<void> {
  await panel(page).getByRole('textbox', { name: 'Prompt', exact: true }).fill(text);
}

/** The `comfyui_generate` requests so far. */
async function requests(page: Page): Promise<Record<string, unknown>[]> {
  return (await commandCalls(page, 'comfyui_generate')).map((c) => c.args.request as Record<string, unknown>);
}

/** Screen point of document pixel `(x, y)`'s center. */
async function point(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + ((x + 0.5) * box.width) / SIZE, box.y + ((y + 0.5) * box.height) / SIZE];
}

/** Rectangle Select of the document rect `(x, y, w, h)`. */
async function select(page: Page, x: number, y: number, w: number, h: number): Promise<void> {
  await page.getByRole('button', { name: 'Marquee: Rectangle Select' }).click();
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  const at = (dx: number, dy: number): [number, number] => [
    box.x + (dx * box.width) / SIZE,
    box.y + (dy * box.height) / SIZE,
  ];
  await page.mouse.move(...at(x, y));
  await page.mouse.down();
  await page.mouse.move(...at(x + w, y + h), { steps: 4 });
  await page.mouse.up();
}

/** RGB of document pixel `(x, y)` as the screen shows it. */
async function shownPixel(page: Page, x: number, y: number): Promise<number[]> {
  const [sx, sy] = await point(page, x, y);
  const shot = await page.screenshot({ clip: { x: sx, y: sy, width: 1, height: 1 } });
  return page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bmp = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(1, 1);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    ctx.drawImage(bmp, 0, 0);
    return Array.from(ctx.getImageData(0, 0, 1, 1).data.slice(0, 3));
  }, shot.toString('base64'));
}

/** Runs a generation to its end with `count` variants. */
async function finishedGeneration(page: Page, count = 4): Promise<void> {
  await generateButton(page).click();
  await expect.poll(async () => (await requests(page)).length).toBeGreaterThan(0);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count, error: null });
  await expect(strip(page).getByRole('option')).toHaveCount(count);
}

test.describe('browser', () => {
  test('the PWA has no Generate panel (AC-01)', async ({ page }) => {
    await createDocument(page, 'image');
    await expect(page.getByRole('region', { name: 'Layers', exact: true })).toBeVisible();
    await expect(panel(page)).toHaveCount(0);
  });
});

test.describe('desktop', () => {
  test('the desktop app has a collapsed Generate panel (AC-01)', async ({ page }) => {
    await setup(page);
    await expect(panel(page)).toBeVisible();
    await expect(panel(page).locator('header').getByRole('button')).toHaveAttribute('aria-expanded', 'false');
    // Collapsed, it needs no server (comfyui-connect AC-03).
    expect(await commandCalls(page, 'comfyui_connect')).toHaveLength(0);
  });

  test('without a usable server the panel shows the status and Set up… (AC-02)', async ({ page }) => {
    await setup(page, { kind: 'notFound' });
    await openPanel(page);
    await expect(panel(page).getByRole('status')).toHaveText('No ComfyUI server found');
    await typePrompt(page);
    await expect(generateButton(page)).toBeDisabled();
    await reply(page, 'comfyui_settings', {
      server: 'automatic',
      existingDir: null,
      customUrl: '',
      managedDir: null,
    });
    await panel(page).getByRole('button', { name: 'Set up…' }).click();
    await expect(page.getByRole('dialog', { name: 'Image generation' })).toBeVisible();
    // The other states read as comfyui-connect names them.
    await emit(page, 'comfyui:status', connected({ missingNodes: ['LoadImageMask'] }));
    await page.getByRole('dialog', { name: 'Image generation' }).getByRole('button', { name: 'Close' }).click();
    await expect(panel(page).getByRole('status')).toHaveText('This ComfyUI is too old — update it');
    await emit(page, 'comfyui:status', connected({ checkpoints: [] }));
    await expect(panel(page).getByRole('status')).toHaveText('No model installed');
    await emit(page, 'comfyui:status', { kind: 'starting' });
    await expect(panel(page).getByRole('status')).toHaveText('Starting ComfyUI…');
    await emit(page, 'comfyui:status', { kind: 'refused', status: 401 });
    await expect(panel(page).getByRole('status')).toHaveText('The server refused access (401)');
    await emit(page, 'comfyui:status', { kind: 'unreachable', url: 'http://x' });
    await expect(panel(page).getByRole('status')).toHaveText('Not reachable');
    await emit(page, 'comfyui:status', { kind: 'notConfigured' });
    await expect(panel(page).getByRole('status')).toHaveText('ComfyUI is not set up');
    await expect(generateButton(page)).toBeDisabled();
  });

  test('the panel offers models, LoRA, strength, variants and seed, and keeps them (AC-03)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    const p = panel(page);
    const model = p.getByRole('combobox', { name: 'Model', exact: true });
    await expect(model).toHaveValue('v1-5-pruned-emaonly-fp16.safetensors');
    await expect(model.getByRole('option')).toHaveText([
      'v1-5-pruned-emaonly-fp16.safetensors',
      'my-model.safetensors',
    ]);
    // A catalog model has its family; no Model type choice.
    await expect(p.getByRole('combobox', { name: 'Model type' })).toHaveCount(0);
    const lora = p.getByRole('combobox', { name: 'LoRA', exact: true });
    await expect(lora).toHaveValue('');
    await expect(lora.getByRole('option')).toHaveText(['None', 'pixel.safetensors']);
    await expect(p.getByRole('slider', { name: 'LoRA strength' })).toHaveValue('1');
    await expect(p.getByRole('slider', { name: 'Strength', exact: true })).toHaveValue('100');
    await expect(p.getByRole('spinbutton', { name: 'Variants' })).toHaveValue('4');
    await expect(p.getByRole('textbox', { name: 'Seed' })).toHaveValue('');
    // Outside the catalog: Model type, SDXL by default.
    await model.selectOption('my-model.safetensors');
    const type = p.getByRole('combobox', { name: 'Model type' });
    await expect(type).toHaveValue('sdxl');
    await type.selectOption('flux');
    await expect(p.getByRole('textbox', { name: 'Negative prompt' })).toBeDisabled();
    await type.selectOption('sd15');
    await expect(p.getByRole('textbox', { name: 'Negative prompt' })).toBeEnabled();
    await lora.selectOption('pixel.safetensors');
    await p.getByRole('slider', { name: 'LoRA strength' }).fill('1.5');
    await p.getByRole('slider', { name: 'Strength', exact: true }).fill('40');
    await p.getByRole('spinbutton', { name: 'Variants' }).fill('6');
    await p.getByRole('spinbutton', { name: 'Variants' }).blur();
    await p.getByRole('textbox', { name: 'Negative prompt' }).fill('blurry');
    await p.getByRole('textbox', { name: 'Negative prompt' }).blur();
    await expect
      .poll(() => storedPref(page, 'comfyGenerate'))
      .toMatchObject({
        model: 'my-model.safetensors',
        lora: 'pixel.safetensors',
        loraStrength: 1.5,
        strength: 40,
        count: 6,
        negative: 'blurry',
        families: { 'my-model.safetensors': 'sd15' },
      });
    // After a restart; the server no longer has the LoRA.
    await page.reload();
    await page.getByRole('button', { name: 'Got it' }).click();
    await reply(page, 'comfyui_connect', connected());
    await reply(page, 'comfyui_generation_models', { ...MODELS, loras: [] });
    await newDocument(page, 'image', { width: SIZE, height: SIZE });
    await openPanel(page);
    await expect(p.getByRole('combobox', { name: 'Model', exact: true })).toHaveValue('my-model.safetensors');
    await expect(p.getByRole('combobox', { name: 'Model type' })).toHaveValue('sd15');
    await expect(p.getByRole('slider', { name: 'Strength', exact: true })).toHaveValue('40');
    await expect(p.getByRole('spinbutton', { name: 'Variants' })).toHaveValue('6');
    await expect(p.getByRole('textbox', { name: 'Negative prompt' })).toHaveValue('blurry');
    await expect(p.getByRole('combobox', { name: 'LoRA', exact: true })).toHaveValue('');
    await expect(p.getByRole('note')).toHaveText('pixel.safetensors is no longer installed');
  });

  test('Generate needs a prompt (AC-04)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await expect(generateButton(page)).toBeDisabled();
    await typePrompt(page, '   ');
    await expect(generateButton(page)).toBeDisabled();
    await typePrompt(page);
    await expect(generateButton(page)).toBeEnabled();
  });

  test('text to image, image to image and inpainting send what the state says (AC-05 – AC-07)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    // No selection, 100 %: text to image at SD 1.5's size for a square canvas.
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    const [t2i] = await requests(page);
    expect(t2i).toMatchObject({
      model: 'v1-5-pruned-emaonly-fp16.safetensors',
      family: 'sd15',
      prompt: PROMPT,
      count: 4,
      strength: 1,
      width: 512,
      height: 512,
      input: null,
      mask: null,
      seed: null,
    });
    await panel(page).getByRole('button', { name: 'Cancel' }).click();
    await emit(page, 'comfyui:generation', { kind: 'cancelled', job: 1 });
    // Below 100 %: the active layer (white background) is the input.
    await panel(page).getByRole('slider', { name: 'Strength', exact: true }).fill('50');
    await panel(page).getByRole('textbox', { name: 'Seed' }).fill('123');
    await reply(page, 'comfyui_generate', { job: 2, seed: 123 });
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(2);
    const i2i = (await requests(page))[1];
    expect(i2i).toMatchObject({ strength: 0.5, seed: 123, mask: null, width: 512, height: 512 });
    expect(typeof i2i?.input).toBe('string');
    await panel(page).getByRole('button', { name: 'Cancel' }).click();
    await emit(page, 'comfyui:generation', { kind: 'cancelled', job: 2 });
    // A wide selection: inpainting with a mask, at a 2:1 generation size.
    await panel(page).getByRole('slider', { name: 'Strength', exact: true }).fill('100');
    await select(page, 8, 16, 32, 16);
    await reply(page, 'comfyui_generate', { job: 3, seed: 7 });
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(3);
    const inpaint = (await requests(page))[2];
    expect(inpaint).toMatchObject({ strength: 1, width: 704, height: 384 });
    expect(typeof inpaint?.input).toBe('string');
    expect(typeof inpaint?.mask).toBe('string');
  });

  test('a waiting or running generation shows its place and progress; the editor stays usable (AC-08)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    await emit(page, 'comfyui:generation', { kind: 'queued', job: 1, ahead: 2 });
    await expect(page.getByTestId('generate-progress')).toHaveText('Queued, 2 ahead');
    await emit(page, 'comfyui:generation', { kind: 'running', job: 1 });
    await emit(page, 'comfyui:generation', { kind: 'progress', job: 1, step: 12, total: 30 });
    await expect(page.getByTestId('generate-progress')).toHaveText('Step 12 / 30');
    const jpeg = (await solidPng(page, 8, 8, '#ff0000')).toString('base64');
    await emit(page, 'comfyui:generation', { kind: 'preview', job: 1, image: `data:image/png;base64,${jpeg}` });
    await expect(panel(page).getByRole('img', { name: 'Live preview' })).toBeVisible();
    // Drawing goes on: a selection and its undo.
    await select(page, 4, 4, 8, 8);
    await expect(undoButton(page)).toBeEnabled();
    await expect(page.getByTestId('generate-progress')).toHaveText('Step 12 / 30');
  });

  test('Cancel removes the job and adds nothing (AC-09)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    await panel(page).getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_generation_cancel')).map((c) => c.args.job)).toEqual([1]);
    await emit(page, 'comfyui:generation', { kind: 'cancelled', job: 1 });
    await expect(generateButton(page)).toBeEnabled();
    await expect(layerRows(page)).toHaveCount(1);
    await expect(preview(page)).toHaveCount(0);
  });

  test('finished variants show in the strip and on the canvas; the document stays (AC-10, AC-11)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await reply(
      page,
      'comfyui_variant',
      [...(await solidPng(page, 64, 64, '#0000ff'))],
      [...(await solidPng(page, 64, 64, '#00ff00'))],
    );
    await openHistory(page);
    const steps = await rowNames(page);
    await finishedGeneration(page, 2);
    await expect(strip(page).getByRole('option', { name: 'Variant 1' })).toHaveAttribute('aria-selected', 'true');
    await expect(preview(page)).toBeVisible();
    await expect.poll(() => shownPixel(page, 32, 32)).toEqual([0, 0, 255]);
    await expect(layerRows(page)).toHaveCount(1);
    expect(await rowNames(page)).toEqual(steps);
    await strip(page).getByRole('option', { name: 'Variant 2' }).click();
    await expect(preview(page)).toHaveAttribute('data-variant', '1');
    await expect.poll(() => shownPixel(page, 32, 32)).toEqual([0, 255, 0]);
    expect(await rowNames(page)).toEqual(steps);
  });

  test('Apply adds one named layer over the target box, masked, as one undo step (AC-07, AC-12)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await select(page, 16, 16, 16, 16);
    const before = await layerRows(page).count();
    await finishedGeneration(page, 1);
    // The preview already matches the result: blue inside, white outside.
    await expect.poll(() => shownPixel(page, 20, 20)).toEqual([0, 0, 255]);
    await expect.poll(() => shownPixel(page, 8, 8)).toEqual([255, 255, 255]);
    await openHistory(page);
    const steps = await historyRows(page).count();
    await panel(page).getByRole('button', { name: 'Apply' }).click();
    await expect(layerRows(page)).toHaveCount(before + 1);
    await expect(historyRows(page)).toHaveCount(steps + 1);
    await expect(historyRows(page).last()).toContainText('Generate Image');
    await expect(layerRows(page).filter({ hasText: /Generated: a red apple on a table, photor(?!e)/ })).toHaveCount(1);
    await expect(strip(page)).toHaveCount(0);
    await expect(preview(page)).toHaveCount(0);
    await expect.poll(() => shownPixel(page, 20, 20)).toEqual([0, 0, 255]);
    await expect.poll(() => shownPixel(page, 8, 8)).toEqual([255, 255, 255]);
    await expect.poll(() => shownPixel(page, 40, 40)).toEqual([255, 255, 255]);
    await page.keyboard.press('Control+z');
    await expect(layerRows(page)).toHaveCount(before);
    await expect.poll(() => shownPixel(page, 20, 20)).toEqual([255, 255, 255]);
  });

  test('Discard and Generate again drop the variants (AC-13)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await finishedGeneration(page, 2);
    await panel(page).getByRole('button', { name: 'Discard' }).click();
    await expect(strip(page)).toHaveCount(0);
    await expect(preview(page)).toHaveCount(0);
    await expect(layerRows(page)).toHaveCount(1);
    await reply(page, 'comfyui_generate', { job: 2, seed: 1 });
    await generateButton(page).click();
    await emit(page, 'comfyui:generation', { kind: 'finished', job: 2, count: 2, error: null });
    await expect(strip(page).getByRole('option')).toHaveCount(2);
    await reply(page, 'comfyui_generate', { job: 3, seed: 1 });
    await generateButton(page).click();
    await expect(strip(page)).toHaveCount(0);
    await expect(preview(page)).toHaveCount(0);
    expect((await commandCalls(page, 'comfyui_generation_release')).map((c) => c.args.job)).toContain(2);
  });

  test('a generation and its variants stay with their tab (AC-14)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    await emit(page, 'comfyui:generation', { kind: 'progress', job: 1, step: 3, total: 30 });
    await expect(page.getByTestId('generate-progress')).toHaveText('Step 3 / 30');
    await menu(page, 'File', 'New…');
    await page.getByRole('dialog', { name: 'New document' }).getByRole('button', { name: 'Create' }).click();
    await expect(page.getByRole('tab')).toHaveCount(2);
    await expect(page.getByTestId('generate-progress')).toHaveCount(0);
    await expect(generateButton(page)).toBeEnabled();
    await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 1, error: null });
    await expect(strip(page)).toHaveCount(0);
    await expect(preview(page)).toHaveCount(0);
    await page.getByRole('tab').first().click();
    await expect(strip(page).getByRole('option')).toHaveCount(1);
    await expect(preview(page)).toBeVisible();
  });

  test('errors show in the panel; variants that arrived stay usable (AC-15)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    await emit(page, 'comfyui:generation', { kind: 'failed', job: 1, error: { kind: 'outOfMemory' } });
    await expect(panel(page).getByRole('alert')).toHaveText(
      'Not enough GPU memory — try fewer variants, a smaller model or a smaller selection',
    );
    await expect(generateButton(page)).toBeEnabled();
    await reply(page, 'comfyui_generate', { job: 2, seed: 1 });
    await generateButton(page).click();
    await expect(panel(page).getByRole('alert')).toHaveCount(0);
    await emit(page, 'comfyui:generation', {
      kind: 'finished',
      job: 2,
      count: 1,
      error: { kind: 'fewer', got: 1, wanted: 4 },
    });
    await expect(panel(page).getByRole('alert')).toHaveText('Only 1 of 4 images came back');
    await expect(strip(page).getByRole('option')).toHaveCount(1);
    await panel(page).getByRole('button', { name: 'Apply' }).click();
    await expect(layerRows(page)).toHaveCount(2);
    await reply(page, 'comfyui_generate', { job: 3, seed: 1 });
    await generateButton(page).click();
    await emit(page, 'comfyui:generation', { kind: 'failed', job: 3, error: { kind: 'unreachable' } });
    await expect(panel(page).getByRole('alert')).toHaveText('ComfyUI stopped answering');
    // A refused start (no server any more) reads the same way.
    await reply(page, 'comfyui_generate', { __error: 'gone' });
    await generateButton(page).click();
    await expect(panel(page).getByRole('alert')).toHaveText('The generation failed: Error: gone');
    await expect(generateButton(page)).toBeEnabled();
  });

  test('closing a tab cancels its generation (AC-21)', async ({ page }) => {
    await setup(page);
    await openPanel(page);
    await typePrompt(page);
    await generateButton(page).click();
    await expect.poll(async () => (await requests(page)).length).toBe(1);
    await menu(page, 'File', 'Close');
    await page.getByRole('button', { name: "Don't Save" }).click();
    await expect(page.getByRole('tab')).toHaveCount(0);
    await expect.poll(async () => (await commandCalls(page, 'comfyui_generation_cancel')).map((c) => c.args.job)).toEqual([1]);
  });

  test('the panel and its messages are German in the German UI (AC-22)', async ({ page }) => {
    await setup(page, { kind: 'notFound' });
    await switchLanguage(page, 'Deutsch');
    const p = page.getByRole('region', { name: 'Generieren', exact: true });
    await p.locator('header').getByRole('button').click();
    await expect(p.getByRole('status')).toHaveText('Kein ComfyUI-Server gefunden');
    await expect(p.getByRole('button', { name: 'Einrichten…' })).toBeVisible();
    await expect(p.getByRole('textbox', { name: 'Negativer Prompt' })).toBeVisible();
    await expect(p.getByRole('slider', { name: 'Stärke', exact: true })).toBeVisible();
    await expect(p.getByRole('spinbutton', { name: 'Varianten' })).toBeVisible();
  });
});
