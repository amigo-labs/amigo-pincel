import { expect, test, type Locator, type Page } from '@playwright/test';
import { solidPng } from './helpers';
import { commandCalls, emit, reply, type MockWindow } from './tauri-mock';
import {
  canvas,
  chatCall,
  done,
  drawCall,
  input,
  newTab,
  panel,
  sendMessage,
  sent,
  startDesktop,
  tabs,
  undoButton,
} from './assistant-helpers';

// The Assistant chat's `gen.*` calls (docs/specs/ai-chat-panel.md AC-08,
// AC-10, AC-11, AC-26 with docs/specs/comfyui-generate.md AC-16): bound to
// the chat's tab, part of the answer's one undo step, cancelled by Stop.
// The test plays the CLI, the MCP endpoint and the ComfyUI backend.

interface Wire {
  ok: boolean;
  json?: Record<string, unknown>;
  error?: string;
}

const MODELS = {
  checkpoints: [{ name: 'v1-5-pruned-emaonly-fp16.safetensors', family: 'sd15' }],
  loras: [],
};

let nextId = 5_000;

/** Starts the desktop app with an Image document and a ComfyUI server
 *  that answers every generation with job 5. */
async function setup(page: Page): Promise<void> {
  await startDesktop(page, 'image');
  await reply(page, 'comfyui_generation_models', MODELS);
  await reply(page, 'comfyui_generate', { job: 5, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 64, 64, '#0000ff'))]);
}

/** Emits `gen.image` from `chat` without waiting for the reply. */
async function genImage(page: Page, chat: string, args: Record<string, unknown>): Promise<number> {
  const id = nextId++;
  await emit(page, 'mcp-call', { id, tool: 'gen.image', args, chat });
  return id;
}

async function answer(page: Page, id: number): Promise<Wire> {
  await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
  return page.evaluate(
    (i) => (window as unknown as MockWindow).__mock.mcpReplies[i],
    id,
  ) as Promise<Wire>;
}

async function generations(page: Page): Promise<number> {
  return (await commandCalls(page, 'comfyui_generate')).length;
}

/** The layer rows of the shown editor; read outside an answer, while the
 *  Layers panel is not inert. */
function layerRows(page: Page): Locator {
  return page.getByRole('list', { name: 'Layer list' }).getByRole('listitem');
}

test('AC-26 a chat gen.image runs on the chat tab and refuses another document', async ({
  page,
}) => {
  await setup(page);
  await expect(input(page)).toBeEnabled();
  await newTab(page, 'image');
  await expect(input(page)).toBeEnabled();
  const chat = 'chat-1-1';
  // Without `doc`: the chat's tab, not the shown one.
  const id = await genImage(page, chat, { prompt: 'a pear' });
  await expect.poll(() => generations(page)).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 5, count: 1, error: null });
  const own = await answer(page, id);
  expect(own.ok, own.error).toBe(true);
  await expect(tabs(page).first()).toHaveAttribute('aria-selected', 'true');
  await expect(layerRows(page).filter({ hasText: 'Generated: a pear' })).toHaveCount(1);
  await tabs(page).nth(1).click();
  await expect(layerRows(page).filter({ hasText: 'Generated: a pear' })).toHaveCount(0);
  // Another tab's document: refused before anything is sent.
  const foreign = await chatCall(page, chat, 'gen.image', { prompt: 'a pear', doc: 'tab-2' });
  expect(foreign).toEqual({ ok: false, error: 'document `tab-2` is not available in the chat' });
  expect(await generations(page)).toBe(1);
});

test('AC-08 a generation of the answer lands in its one Assistant step', async ({ page }) => {
  await setup(page);
  const rows = await layerRows(page).count();
  const before = await canvas(page, 'image').screenshot();
  await sendMessage(page, 'Draw and generate');
  const chat = String((await sent(page)).chat);
  const { tool, args } = drawCall('image');
  const drawn = await chatCall(page, chat, tool, args);
  expect(drawn.ok, drawn.error).toBe(true);
  const id = await genImage(page, chat, { prompt: 'a pear' });
  await expect.poll(() => generations(page)).toBe(1);
  // The CLI ends while its generation still runs: the answer waits for it.
  await done(page, chat);
  await expect(panel(page).getByRole('button', { name: 'Stop' })).toBeVisible();
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 5, count: 1, error: null });
  const made = await answer(page, id);
  expect(made.ok, made.error).toBe(true);
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  await expect(layerRows(page)).toHaveCount(rows + 1);
  // One undo takes the stroke and the generated layer back.
  await undoButton(page).click();
  await expect(layerRows(page)).toHaveCount(rows);
  await expect(undoButton(page)).toBeDisabled();
  await expect
    .poll(async () => (await canvas(page, 'image').screenshot()).equals(before))
    .toBe(true);
});
