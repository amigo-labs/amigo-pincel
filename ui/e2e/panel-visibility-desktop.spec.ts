import { expect, test, type Locator, type Page } from '@playwright/test';
import { solidPng } from './helpers';
import { claude, done, event, input, newDoc, panel as assistant, sendMessage, sent } from './assistant-helpers';
import { commandCalls, emit, installTauriMock, reply } from './tauri-mock';

// docs/specs/panel-visibility.md — View ▸ Panels in the desktop app (mocked
// Tauri bridge): the Generate and Assistant entries (AC-01), a removed
// Generate panel starts no server (AC-13), a removed panel keeps its content
// (AC-14), running work keeps its panel (AC-15), the group heights (AC-16).

const AI = 'Generate, Assistant';

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const tabOf = (page: Page, group: string, name: string) =>
  sidebar(page).getByRole('tablist', { name: group, exact: true }).getByRole('tab', { name, exact: true });
const section = (page: Page, name: string) => sidebar(page).locator(`section[aria-label="${name}"]`);
const generate = (page: Page): Locator => section(page, 'Generate');
const prompt = (page: Page): Locator => generate(page).getByRole('textbox', { name: 'Prompt', exact: true });
const connects = async (page: Page) => (await commandCalls(page, 'comfyui_connect')).length;

const CONNECTED = {
  kind: 'connected',
  server: {
    url: 'http://127.0.0.1:8188',
    version: '0.3.40',
    gpu: null,
    checkpoints: ['v1-5-pruned-emaonly-fp16.safetensors'],
    loras: [],
    missingNodes: [],
    started: false,
  },
};

/** The desktop app with a usable CLI and ComfyUI server and a 16×16 document. */
async function start(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await installTauriMock(page);
  await page.goto('./');
  await reply(page, 'assistant_clis', [claude(true)]);
  await reply(page, 'comfyui_connect', CONNECTED);
  await reply(page, 'comfyui_generation_models', {
    checkpoints: [{ name: 'v1-5-pruned-emaonly-fp16.safetensors', family: 'sd15' }],
    loras: [],
  });
  await reply(page, 'comfyui_generate', { job: 1, seed: 42 });
  await reply(page, 'comfyui_variant', [...(await solidPng(page, 16, 16, '#0000ff'))]);
  await page.getByRole('button', { name: 'Got it' }).click();
  await newDoc(page, mode);
}

async function openPanels(page: Page): Promise<Locator> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menu', { name: 'View', exact: true }).getByRole('menuitem', { name: 'Panels', exact: true }).click();
  const sub = page.getByRole('menu', { name: 'Panels', exact: true });
  await expect(sub).toBeVisible();
  return sub;
}

async function closeMenus(page: Page): Promise<void> {
  for (let i = 0; i < 3 && (await page.getByRole('menu').count()) > 0; i += 1) {
    await page.keyboard.press('Escape');
  }
  await expect(page.getByRole('menu')).toHaveCount(0);
}

async function choose(page: Page, name: string): Promise<void> {
  const sub = await openPanels(page);
  await sub.getByRole('menuitemcheckbox', { name, exact: true }).click();
  await expect(page.getByRole('menu')).toHaveCount(0);
}

test.use({ viewport: { width: 1280, height: 900 } });

test('the entries follow the sidebar order, Generate and Assistant included, the Timeline last (AC-01)', async ({
  page,
}) => {
  await start(page, 'pixel');
  const sub = await openPanels(page);
  expect(
    await sub
      .getByRole('menuitemcheckbox')
      .evaluateAll((els) => els.map((el) => (el.querySelector('.am-menu-label')?.textContent ?? '').trim())),
  ).toEqual([
    'Colors',
    'Palette',
    'Layers',
    'History',
    'Preview',
    'Tilesets',
    'Slices',
    'Generate',
    'Assistant',
    'Timeline',
  ]);
  await closeMenus(page);
});

test('a removed Generate panel starts no ComfyUI server; enabling it shows it and connects (AC-06, AC-13)', async ({
  page,
}) => {
  await start(page, 'image');
  await choose(page, 'Generate');
  await expect(sidebar(page).getByRole('tablist', { name: AI })).toHaveCount(0);
  // The Assistant is drawn like a single panel.
  await expect(assistant(page)).toBeVisible();
  await page.waitForTimeout(400);
  expect(await connects(page)).toBe(0);

  await choose(page, 'Generate');
  await expect(tabOf(page, AI, 'Generate')).toHaveAttribute('aria-selected', 'true');
  await expect(prompt(page)).toBeVisible();
  await expect.poll(() => connects(page)).toBeGreaterThan(0);
  // Removed while it was the shown tab: no further call.
  const shown = await connects(page);
  await choose(page, 'Generate');
  await expect(generate(page)).toHaveCount(0);
  await page.waitForTimeout(400);
  expect(await connects(page)).toBe(shown);
});

test('the Assistant removed and enabled again keeps its conversation and draft; Generate keeps its prompt (AC-14)', async ({
  page,
}) => {
  await start(page, 'image');
  await sendMessage(page, 'first question');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'first answer' });
  await done(page, chat);
  await input(page).fill('half a thought');
  await tabOf(page, AI, 'Generate').click();
  await prompt(page).fill('a red apple');

  await choose(page, 'Assistant');
  await choose(page, 'Generate');
  await expect(sidebar(page).locator('[data-panel-group="ai"]')).toBeHidden();
  await choose(page, 'Assistant');
  await expect(assistant(page)).toContainText('first answer');
  await expect(input(page)).toHaveValue('half a thought');
  await choose(page, 'Generate');
  await expect(prompt(page)).toHaveValue('a red apple');
});

test('while an answer runs the Assistant entry and Reset Panels are disabled (AC-15)', async ({ page }) => {
  await start(page, 'image');
  await sendMessage(page, 'draw something');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'Working' });
  await expect(assistant(page).getByRole('button', { name: 'Stop' })).toBeVisible();
  let sub = await openPanels(page);
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Assistant', exact: true })).toBeDisabled();
  await expect(sub.getByRole('menuitem', { name: 'Reset Panels', exact: true })).toBeDisabled();
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Generate', exact: true })).toBeEnabled();
  await closeMenus(page);
  await done(page, chat);
  await expect(assistant(page).getByRole('button', { name: 'Stop' })).toHaveCount(0);
  sub = await openPanels(page);
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Assistant', exact: true })).toBeEnabled();
  await expect(sub.getByRole('menuitem', { name: 'Reset Panels', exact: true })).toBeEnabled();
  await closeMenus(page);
});

test('while a generation runs the Generate entry and Reset Panels are disabled (AC-15)', async ({ page }) => {
  await start(page, 'image');
  await tabOf(page, AI, 'Generate').click();
  await prompt(page).fill('a red apple');
  await generate(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await expect(generate(page).getByRole('button', { name: 'Cancel' })).toBeVisible();
  let sub = await openPanels(page);
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Generate', exact: true })).toBeDisabled();
  await expect(sub.getByRole('menuitem', { name: 'Reset Panels', exact: true })).toBeDisabled();
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Assistant', exact: true })).toBeEnabled();
  await closeMenus(page);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 1, error: null });
  await expect(generate(page).getByRole('button', { name: 'Cancel' })).toHaveCount(0);
  sub = await openPanels(page);
  await expect(sub.getByRole('menuitemcheckbox', { name: 'Generate', exact: true })).toBeEnabled();
  await expect(sub.getByRole('menuitem', { name: 'Reset Panels', exact: true })).toBeEnabled();
  await closeMenus(page);
});

test('without Layers | History the Generate | Assistant group takes the shared height alone (AC-16)', async ({ page }) => {
  await start(page, 'image');
  const group = (id: string) => sidebar(page).locator(`[data-panel-group="${id}"]`);
  const doc = await group('document').boundingBox();
  const ai = await group('ai').boundingBox();
  if (!doc || !ai) throw new Error('groups not laid out');
  await choose(page, 'Layers');
  await choose(page, 'History');
  await expect(group('document')).toBeHidden();
  const alone = await group('ai').boundingBox();
  if (!alone) throw new Error('group not laid out');
  expect(Math.abs(alone.height - (doc.height + ai.height))).toBeLessThanOrEqual(2);
});
