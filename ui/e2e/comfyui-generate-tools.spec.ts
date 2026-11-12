import { expect, test, type Locator, type Page } from '@playwright/test';
import { menu, newDocument, solidPng, storedPref } from './helpers';
import { commandCalls, emit, installTauriMock, launchDesktop, reply, type MockWindow } from './tauri-mock';

// The desktop host tools `gen.image` and `gen.models`
// (docs/specs/comfyui-generate.md AC-16 – AC-20), against the mocked Tauri
// bridge: the test plays the MCP endpoint (forwarded `mcp-call` events,
// `mcp-call-expired`) and the ComfyUI backend (command answers and
// `comfyui:generation` events).

interface Wire {
  ok: boolean;
  json?: Record<string, unknown>;
  error?: string;
}

const MODELS = {
  checkpoints: [
    { name: 'v1-5-pruned-emaonly-fp16.safetensors', family: 'sd15' },
    { name: 'my-model.safetensors', family: null },
  ],
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

let nextId = 1;

async function send(page: Page, tool: string, args: Record<string, unknown> = {}): Promise<number> {
  const id = nextId++;
  await emit(page, 'mcp-call', { id, tool, args, chat: null });
  return id;
}

async function answer(page: Page, id: number): Promise<Wire> {
  await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
  return page.evaluate((i) => (window as unknown as MockWindow).__mock.mcpReplies[i], id) as Promise<Wire>;
}

async function call(page: Page, tool: string, args: Record<string, unknown> = {}): Promise<Wire> {
  return answer(page, await send(page, tool, args));
}

const layerRows = (page: Page): Locator =>
  page.getByRole('list', { name: 'Layer list' }).getByRole('listitem');
const panel = (page: Page): Locator => page.getByRole('region', { name: 'Generate', exact: true });

async function setup(page: Page, mode: 'pixel' | 'image' = 'image'): Promise<void> {
  await installTauriMock(page);
  await launchDesktop(page);
  await reply(page, 'comfyui_connect', CONNECTED);
  await reply(page, 'comfyui_generation_models', MODELS);
  await reply(page, 'comfyui_generate', { job: 5, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 64, 64, '#0000ff'))]);
  await newDocument(page, mode, { width: 32, height: 32 });
}

async function requests(page: Page): Promise<Record<string, unknown>[]> {
  return (await commandCalls(page, 'comfyui_generate')).map((c) => c.args.request as Record<string, unknown>);
}

test('gen.models lists the names gen.image accepts (AC-18)', async ({ page }) => {
  await setup(page);
  const r = await call(page, 'gen.models');
  expect(r).toEqual({
    ok: true,
    json: { checkpoints: ['v1-5-pruned-emaonly-fp16.safetensors', 'my-model.safetensors'], loras: ['pixel.safetensors'] },
  });
});

test('without a usable server gen.* fail with the status (AC-19)', async ({ page }) => {
  await setup(page);
  await reply(page, 'comfyui_generation_models', { __error: 'no ComfyUI server found' });
  expect(await call(page, 'gen.models')).toEqual({ ok: false, error: 'no ComfyUI server found' });
  expect(await call(page, 'gen.image', { prompt: 'a pear' })).toEqual({
    ok: false,
    error: 'no ComfyUI server found',
  });
  expect(await requests(page)).toHaveLength(0);
});

test('bad names and values fail before anything is sent (AC-17)', async ({ page }) => {
  await setup(page);
  const bad = async (args: Record<string, unknown>): Promise<string> => {
    const r = await call(page, 'gen.image', { prompt: 'a pear', ...args });
    expect(r.ok).toBe(false);
    return r.error ?? '';
  };
  expect(await bad({ model: 'nope.safetensors' })).toBe(
    'unknown model "nope.safetensors"; valid models: "v1-5-pruned-emaonly-fp16.safetensors", "my-model.safetensors"',
  );
  expect(await bad({ lora: 'gone.safetensors' })).toBe(
    'unknown LoRA "gone.safetensors"; valid LoRAs: "pixel.safetensors"',
  );
  expect(await bad({ strength: 0 })).toBe('`strength` must be a number from 0.01 to 1.0');
  expect(await bad({ lora_strength: 3 })).toBe('`lora_strength` must be a number from 0 to 2');
  expect(await bad({ seed: -1 })).toBe('`seed` must be an integer of at least 0');
  expect(await bad({ prompt: '' })).toBe('`prompt` must be a non-empty string');
  expect(await requests(page)).toHaveLength(0);
});

test('gen.image generates one variant and applies it; the panel keeps its variants (AC-16)', async ({ page }) => {
  await setup(page);
  // The user's own variants in the strip, with the second model chosen.
  await panel(page).locator('header').getByRole('button').click();
  await panel(page).getByRole('combobox', { name: 'Model', exact: true }).selectOption('my-model.safetensors');
  await expect.poll(async () => ((await storedPref(page, 'comfyGenerate')) as { model?: string } | undefined)?.model).toBe(
    'my-model.safetensors',
  );
  await panel(page).getByRole('textbox', { name: 'Prompt', exact: true }).fill('a house');
  await reply(page, 'comfyui_generate', { job: 1, seed: 1 }, { job: 5, seed: 42 });
  await panel(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 2, error: null });
  const strip = panel(page).getByRole('listbox', { name: 'Generated variants' });
  await expect(strip.getByRole('option')).toHaveCount(2);

  const id = await send(page, 'gen.image', { prompt: 'a green pear on a plate', seed: 42 });
  await expect.poll(async () => (await requests(page)).length).toBe(2);
  const request = (await requests(page))[1];
  expect(request).toMatchObject({
    model: 'my-model.safetensors',
    family: 'sdxl',
    lora: null,
    loraStrength: 1,
    prompt: 'a green pear on a plate',
    seed: 42,
    count: 1,
    strength: 1,
  });
  await emit(page, 'comfyui:generation', { kind: 'progress', job: 5, step: 2, total: 30 });
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 5, count: 1, error: null });
  expect(await answer(page, id)).toEqual({
    ok: true,
    json: { layer: 'Generated: a green pear on a plate', seed: 42, model: 'my-model.safetensors' },
  });
  await expect(layerRows(page).filter({ hasText: 'Generated: a green pear on a plate' })).toHaveCount(1);
  await expect(strip.getByRole('option')).toHaveCount(2);
});

test('an expired call or a closed tab cancels the job and adds nothing (AC-20)', async ({ page }) => {
  await setup(page);
  const before = await layerRows(page).count();
  const id = await send(page, 'gen.image', { prompt: 'a pear' });
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await emit(page, 'mcp-call-expired', id);
  await expect.poll(async () => (await commandCalls(page, 'comfyui_generation_cancel')).map((c) => c.args.job)).toEqual([5]);
  await emit(page, 'comfyui:generation', { kind: 'cancelled', job: 5 });
  expect((await answer(page, id)).ok).toBe(false);
  await expect(layerRows(page)).toHaveCount(before);

  // The tab closes while its generation runs.
  await reply(page, 'comfyui_generate', { job: 6, seed: 1 });
  const id2 = await send(page, 'gen.image', { prompt: 'a pear' });
  await expect.poll(async () => (await requests(page)).length).toBe(2);
  await menu(page, 'File', 'Close');
  await expect(page.getByRole('tab')).toHaveCount(0);
  await expect.poll(async () => (await commandCalls(page, 'comfyui_generation_cancel')).map((c) => c.args.job)).toEqual([5, 6]);
  await emit(page, 'comfyui:generation', { kind: 'cancelled', job: 6 });
  expect((await answer(page, id2)).ok).toBe(false);
});

test('a generation error fails the call in English (AC-16, AC-19)', async ({ page }) => {
  await setup(page);
  const id = await send(page, 'gen.image', { prompt: 'a pear' });
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'failed', job: 5, error: { kind: 'outOfMemory' } });
  expect(await answer(page, id)).toEqual({
    ok: false,
    error: 'Not enough GPU memory — try fewer variants, a smaller model or a smaller selection',
  });
});

test('gen.image on a Pixel document applies one variant as a new layer (comfyui-generate-pixel AC-08)', async ({
  page,
}) => {
  await setup(page, 'pixel');
  const id = await send(page, 'gen.image', { prompt: 'a small green slime monster', seed: 42 });
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  expect((await requests(page))[0]).toMatchObject({ count: 1, seed: 42, width: 512, height: 512, input: null });
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 5, count: 1, error: null });
  expect(await answer(page, id)).toEqual({
    ok: true,
    json: { layer: 'Generated: a small green slime monster', seed: 42, model: 'v1-5-pruned-emaonly-fp16.safetensors' },
  });
  await expect(
    page.getByRole('list', { name: 'Layer list' }).getByRole('button', { name: /^Drag to reorder / }),
  ).toHaveCount(2);
});
