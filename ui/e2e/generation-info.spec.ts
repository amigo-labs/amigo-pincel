import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  fakeFileHandles,
  fakeFs,
  gotoStart,
  hidePanel,
  newDocument,
  showPanel,
  solidPng,
  switchLanguage,
  waitForEditor,
} from './helpers';
import { commandCalls, emit, installTauriMock, launchDesktop, reply } from './tauri-mock';

// Generation info on generated layers (docs/specs/comfyui-generation-info.md):
// recorded when a variant becomes a layer, shown in the layers panel, copied
// and loaded back into the Generate panel. The file formats and the undo /
// merge rules are proven in Rust (pincel-core, pincel-image-core).

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

const PROMPT = 'a red apple on a table: Seed: 5';

const generatePanel = (page: Page): Locator =>
  page.getByRole('region', { name: 'Generate', exact: true });
const section = (page: Page): Locator => page.getByRole('group', { name: 'Generation', exact: true });
const layerList = (page: Page): Locator => page.getByRole('list', { name: 'Layer list' });
const nameButton = (page: Page, name: string): Locator =>
  layerList(page).getByRole('button', { name, exact: true });
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page): Locator => page.getByRole('button', { name: 'Redo', exact: true });

/** The desktop app with a usable server and a 32² document. */
async function setup(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await installTauriMock(page);
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await launchDesktop(page);
  await reply(page, 'comfyui_connect', CONNECTED);
  await reply(page, 'comfyui_generation_models', MODELS);
  await reply(page, 'comfyui_generate', { job: 1, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 64, 64, '#0000ff'))]);
  await newDocument(page, mode, { width: 32, height: 32 });
}

async function openPanel(page: Page): Promise<void> {
  await showPanel(page, 'Generate');
  await expect(generatePanel(page).getByRole('combobox').first()).toBeVisible();
}

async function collapsePanel(page: Page): Promise<void> {
  await hidePanel(page, 'Generate');
}

const field = (page: Page, name: string): Locator =>
  generatePanel(page).getByRole('textbox', { name, exact: true });
const select = (page: Page, name: string): Locator =>
  generatePanel(page).getByRole('combobox', { name, exact: true });

async function requests(page: Page): Promise<Record<string, unknown>[]> {
  return (await commandCalls(page, 'comfyui_generate')).map(
    (c) => c.args.request as Record<string, unknown>,
  );
}

/** The text in the clipboard, line breaks as LF (Gotchas: Windows CRLF). */
async function clipboard(page: Page): Promise<string> {
  const text = await page.evaluate(() => navigator.clipboard.readText());
  return text.replace(/\r\n/g, '\n');
}

/** The Generation section's value for `label`. */
function value(page: Page, label: string): Locator {
  return section(page)
    .getByRole('term')
    .filter({ hasText: new RegExp(`^${label}$`) })
    .locator('xpath=following-sibling::dd[1]');
}

test('AC-01 – AC-05 AC-07 Apply records the variant; the panel shows, copies and reuses it', async ({
  page,
}) => {
  await setup(page, 'image');
  await openPanel(page);
  await select(page, 'Model').selectOption('v1-5-pruned-emaonly-fp16.safetensors');
  await select(page, 'LoRA').selectOption('pixel.safetensors');
  await field(page, 'Negative prompt').fill('blurry, text');
  await field(page, 'Prompt').fill(PROMPT);
  await generatePanel(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 4, error: null });
  const strip = generatePanel(page).getByRole('listbox', { name: 'Generated variants' });
  await expect(strip.getByRole('option')).toHaveCount(4);
  // The third variant: seed 42 + 2.
  await strip.getByRole('option').nth(2).click();
  await generatePanel(page).getByRole('button', { name: 'Apply' }).click();

  const layer = `Generated: ${Array.from(PROMPT).slice(0, 30).join('')}`;
  await expect(nameButton(page, layer)).toBeVisible();
  // AC-02: the generated layer is active and shows its section.
  await expect(section(page)).toBeVisible();
  await expect(value(page, 'Prompt')).toHaveText(PROMPT);
  await expect(value(page, 'Negative prompt')).toHaveText('blurry, text');
  await expect(value(page, 'Model')).toHaveText('v1-5-pruned-emaonly-fp16.safetensors');
  await expect(value(page, 'LoRA')).toHaveText('pixel.safetensors');
  await expect(value(page, 'LoRA strength')).toHaveText('1');
  await expect(value(page, 'Strength')).toHaveText('100%');
  await expect(value(page, 'Seed')).toHaveText('44');
  // AC-03: the row's tooltip names the layer and the full prompt.
  await expect(nameButton(page, layer)).toHaveAttribute('title', `${layer}\n${PROMPT}`);

  // AC-04: Copy writes the text form in English.
  await section(page).getByRole('button', { name: 'Copy', exact: true }).click();
  await expect
    .poll(() => clipboard(page))
    .toBe(
      `Prompt: ${PROMPT}\nNegative prompt: blurry, text\nModel: v1-5-pruned-emaonly-fp16.safetensors\n` +
        'LoRA: pixel.safetensors\nLoRA strength: 1\nStrength: 100%\nSeed: 44',
    );
  // A refused write says so, and nothing else changes.
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new DOMException('denied', 'NotAllowedError'));
  });
  await section(page).getByRole('button', { name: 'Copy', exact: true }).click();
  await expect(section(page).getByRole('status')).toHaveText('Could not copy');

  // AC-02: another layer shows no section.
  await nameButton(page, 'Layer 1').click();
  await expect(section(page)).toHaveCount(0);
  await nameButton(page, layer).click();
  await expect(section(page).getByRole('status')).toHaveCount(0);

  // AC-07: undo removes the layer, redo brings it back with its info.
  await undoButton(page).click();
  await expect(nameButton(page, layer)).toHaveCount(0);
  await redoButton(page).click();
  await nameButton(page, layer).click();
  await expect(value(page, 'Seed')).toHaveText('44');

  // AC-05: Use in Generate panel restores the fields and expands the panel;
  // the variant count stays.
  await field(page, 'Prompt').fill('something else');
  await field(page, 'Negative prompt').fill('');
  await select(page, 'LoRA').selectOption({ index: 0 });
  await field(page, 'Seed').fill('');
  const count = await generatePanel(page).getByRole('spinbutton', { name: 'Variants' }).inputValue();
  await collapsePanel(page);
  await section(page).getByRole('button', { name: 'Use in Generate panel' }).click();
  await expect(page.getByRole('tab', { name: 'Generate', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(generatePanel(page)).toBeVisible();
  await expect(field(page, 'Prompt')).toHaveValue(PROMPT);
  await expect(field(page, 'Negative prompt')).toHaveValue('blurry, text');
  await expect(select(page, 'Model')).toHaveValue('v1-5-pruned-emaonly-fp16.safetensors');
  await expect(select(page, 'LoRA')).toHaveValue('pixel.safetensors');
  await expect(field(page, 'Seed')).toHaveValue('44');
  await expect(generatePanel(page).getByRole('spinbutton', { name: 'Variants' })).toHaveValue(count);
});

test('AC-05 a recorded model the server no longer has falls back with its notice', async ({ page }) => {
  await setup(page, 'image');
  await openPanel(page);
  await select(page, 'Model').selectOption('my-model.safetensors');
  await field(page, 'Prompt').fill('a house');
  await generatePanel(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 1, error: null });
  await generatePanel(page).getByRole('button', { name: 'Apply' }).click();
  await expect(section(page)).toBeVisible();
  await collapsePanel(page);
  await reply(page, 'comfyui_generation_models', {
    checkpoints: [MODELS.checkpoints[0]],
    loras: MODELS.loras,
  });
  await section(page).getByRole('button', { name: 'Use in Generate panel' }).click();
  await expect(generatePanel(page).getByText('my-model.safetensors is no longer installed')).toBeVisible();
  await expect(select(page, 'Model')).toHaveValue('v1-5-pruned-emaonly-fp16.safetensors');
  await expect(field(page, 'Prompt')).toHaveValue('a house');
});

// The `mcp-call` round trip of the agent tools (comfyui-generate-tools.spec.ts).
type Wire = { ok: boolean; json?: unknown; error?: string };
type MockWindow = { __mock: { mcpReplies: Record<number, Wire> } };

async function agentCall(page: Page, tool: string, args: Record<string, unknown>): Promise<Wire> {
  await emit(page, 'mcp-call', { id: 77, tool, args, chat: null });
  await page.waitForFunction(() => 77 in (window as unknown as MockWindow).__mock.mcpReplies);
  const wire = await page.evaluate(() => (window as unknown as MockWindow).__mock.mcpReplies[77]);
  if (wire === undefined) throw new Error('no reply to the agent call');
  return wire;
}

test('AC-01 AC-02 AC-13 gen.image records its generation on a Pixel layer, shown in German', async ({
  page,
}) => {
  await setup(page, 'pixel');
  await reply(page, 'comfyui_generate', { job: 5, seed: 7 });
  const answered = agentCall(page, 'gen.image', {
    prompt: 'a small green slime',
    negative: 'text',
    model: 'my-model.safetensors',
    lora: 'pixel.safetensors',
    lora_strength: 0.5,
    seed: 7,
  });
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 5, count: 1, error: null });
  expect((await answered).ok).toBe(true);
  await nameButton(page, 'Generated: a small green slime').click();
  await expect(value(page, 'Prompt')).toHaveText('a small green slime');
  await expect(value(page, 'Negative prompt')).toHaveText('text');
  await expect(value(page, 'Model')).toHaveText('my-model.safetensors');
  await expect(value(page, 'LoRA')).toHaveText('pixel.safetensors');
  await expect(value(page, 'LoRA strength')).toHaveText('0.5');
  await expect(value(page, 'Strength')).toHaveText('100%');
  await expect(value(page, 'Seed')).toHaveText('7');

  // AC-13: the German UI.
  await switchLanguage(page, 'Deutsch');
  const de = page.getByRole('group', { name: 'Generierung', exact: true });
  await expect(de.getByRole('term').filter({ hasText: /^Negativer Prompt$/ })).toBeVisible();
  await expect(de.getByRole('button', { name: 'Kopieren', exact: true })).toBeVisible();
  await expect(de.getByRole('button', { name: 'Im Panel „Generieren“ verwenden' })).toBeVisible();
});

// A stored OpenRaster file with one generated layer, built here: the
// writer itself is tested in pincel-image-core.
function crc32(bytes: Buffer): number {
  let c = ~0;
  for (const b of bytes) {
    c ^= b;
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return ~c >>> 0;
}

function storedZip(files: [string, Buffer][]): Buffer {
  const locals: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of files) {
    const n = Buffer.from(name);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(n.length, 26);
    locals.push(local, n, data);
    const dir = Buffer.alloc(46);
    dir.writeUInt32LE(0x02014b50, 0);
    dir.writeUInt16LE(20, 4);
    dir.writeUInt16LE(20, 6);
    dir.writeUInt32LE(crc, 16);
    dir.writeUInt32LE(data.length, 20);
    dir.writeUInt32LE(data.length, 24);
    dir.writeUInt16LE(n.length, 28);
    dir.writeUInt32LE(offset, 42);
    central.push(dir, n);
    offset += 30 + n.length + data.length;
  }
  const dirBytes = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(dirBytes.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, dirBytes, end]);
}

test('AC-06 the browser app shows the section without "Use in Generate panel"', async ({ page }) => {
  await gotoStart(page);
  const png = Buffer.from(await solidPng(page, 8, 8, '#00ff00'));
  const stack =
    "<?xml version='1.0' encoding='UTF-8'?><image w='8' h='8' xmlns:pincel='urn:amigo-labs:pincel:ora'>" +
    "<stack><layer name='Generated: a pear' src='data/layer0.png' pincel:prompt='a pear' " +
    "pincel:negative='' pincel:model='m.safetensors' pincel:lora='' pincel:lora-strength='1' " +
    "pincel:strength='100' pincel:seed='3'/></stack></image>";
  const ora = storedZip([
    ['mimetype', Buffer.from('image/openraster')],
    ['stack.xml', Buffer.from(stack)],
    ['data/layer0.png', png],
  ]);
  await fakeFileHandles(page, { 'pear.ora': ora });
  await gotoStart(page);
  await fakeFs(page, { pick: 'pear.ora' });
  await page.getByRole('button', { name: 'Open…' }).click();
  await waitForEditor(page, 'image');
  await expect(value(page, 'Prompt')).toHaveText('a pear');
  await expect(section(page).getByRole('button', { name: 'Copy', exact: true })).toBeVisible();
  await expect(section(page).getByRole('button', { name: 'Use in Generate panel' })).toHaveCount(0);
});
