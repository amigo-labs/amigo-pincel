import { expect, test, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, solidPng, switchLanguage } from './helpers';
import {
  claude,
  done,
  event,
  input,
  isInert,
  log,
  newDoc,
  panel as assistant,
  sendMessage,
  sent,
} from './assistant-helpers';
import { commandCalls, emit, installTauriMock, reply } from './tauri-mock';

// docs/specs/sidebar-tab-groups.md — the desktop app's Generate | Assistant
// group (mocked Tauri bridge): no server before the Generate tab is chosen
// (AC-11), a panel keeps its content across a tab switch (AC-16), and an
// answer or a generation keeps running behind the other tab (AC-17, AC-18).

const AI = 'Generate, Assistant';
const DOCUMENT = 'Layers, History';

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const tablist = (page: Page, name: string) => sidebar(page).getByRole('tablist', { name, exact: true });
const tabOf = (page: Page, group: string, name: string) =>
  tablist(page, group).getByRole('tab', { name, exact: true });
const collapse = (page: Page, group: string) => sidebar(page).getByRole('button', { name: group, exact: true });
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

test.use({ viewport: { width: 1280, height: 900 } });

test('pixel: three groups, Generate | Assistant last with the Assistant active (AC-01, AC-07)', async ({ page }) => {
  await start(page, 'pixel');
  const lists = sidebar(page).getByRole('tablist');
  await expect(lists).toHaveCount(3);
  expect(await lists.evaluateAll((els) => els.map((el) => el.getAttribute('aria-label')))).toEqual([
    DOCUMENT,
    'Preview, Tilesets, Slices',
    AI,
  ]);
  await expect(tabOf(page, AI, 'Assistant')).toHaveAttribute('aria-selected', 'true');
  await expect(collapse(page, AI)).toHaveAttribute('aria-expanded', 'true');
  await expect(assistant(page)).toBeVisible();
  await expect(generate(page)).toHaveCount(0);
});

test('image: Layers | History and Generate | Assistant share the height in equal parts (AC-02, AC-07, AC-13)', async ({
  page,
}) => {
  await start(page, 'image');
  await expect(sidebar(page).getByRole('tablist')).toHaveCount(2);
  const group = (id: string) => sidebar(page).locator(`[data-panel-group="${id}"]`);
  const doc = await group('document').boundingBox();
  const ai = await group('ai').boundingBox();
  if (!doc || !ai) throw new Error('groups not laid out');
  expect(Math.abs(doc.height - ai.height)).toBeLessThanOrEqual(2);
  // A collapsed one takes only its header; the other takes the rest.
  await collapse(page, AI).click();
  const collapsedAi = await group('ai').boundingBox();
  const grownDoc = await group('document').boundingBox();
  if (!collapsedAi || !grownDoc) throw new Error('groups not laid out');
  expect(collapsedAi.height).toBeLessThan(40);
  expect(grownDoc.height).toBeGreaterThan(doc.height + ai.height - 60);
});

test('no ComfyUI server is asked for while the Generate tab is not active or its group is collapsed (AC-11)', async ({
  page,
}) => {
  await start(page, 'image');
  // First use: the Assistant is the active tab.
  await expect(assistant(page)).toBeVisible();
  expect(await connects(page)).toBe(0);
  // Collapsing and expanding the group on the Assistant asks for none either.
  await collapse(page, AI).click();
  await collapse(page, AI).click();
  await expect(assistant(page)).toBeVisible();
  expect(await connects(page)).toBe(0);

  // The Generate tab connects.
  await tabOf(page, AI, 'Generate').click();
  await expect(prompt(page)).toBeVisible();
  await expect.poll(() => connects(page)).toBeGreaterThan(0);
  const after = await connects(page);

  // Back on the Assistant, and with the group collapsed on Generate: no further call.
  await tabOf(page, AI, 'Assistant').click();
  await expect(generate(page)).toHaveCount(0);
  await tabOf(page, AI, 'Generate').click();
  await expect.poll(() => connects(page)).toBeGreaterThan(after);
  const shown = await connects(page);
  await collapse(page, AI).click();
  await expect(generate(page)).toHaveCount(0);
  await page.waitForTimeout(400);
  expect(await connects(page)).toBe(shown);
});

test('a panel shows what it showed before a tab switch: conversation, draft, prompt, history, layers (AC-16)', async ({
  page,
}) => {
  await start(page, 'image');
  // A finished exchange and a draft in the Assistant.
  await sendMessage(page, 'first question');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'first answer' });
  await done(page, chat);
  await expect(log(page)).toContainText('first answer');
  await input(page).fill('half a thought');

  // A prompt in Generate.
  await tabOf(page, AI, 'Generate').click();
  await expect(assistant(page)).toHaveCount(0);
  await prompt(page).fill('a red apple');

  // Settings in Generate, and a result it shows.
  await generate(page).getByRole('textbox', { name: 'Seed', exact: true }).fill('1234');
  const variants = generate(page).getByRole('spinbutton', { name: 'Variants', exact: true });
  await variants.fill('3');
  await variants.press('Tab');
  await generate(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await expect.poll(async () => (await commandCalls(page, 'comfyui_generate')).length).toBe(1);
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 1, error: null });
  const results = generate(page).getByRole('listbox', { name: 'Generated variants' }).getByRole('option');
  await expect(results).toHaveCount(1);

  // Back: the conversation and the draft are there.
  await tabOf(page, AI, 'Assistant').click();
  await expect(log(page)).toContainText('first question');
  await expect(log(page)).toContainText('first answer');
  await expect(input(page)).toHaveValue('half a thought');
  // … and the prompt.
  await tabOf(page, AI, 'Generate').click();
  await expect(prompt(page)).toHaveValue('a red apple');
  // … with the settings and the result shown before.
  await expect(generate(page).getByRole('textbox', { name: 'Seed', exact: true })).toHaveValue('1234');
  await expect(variants).toHaveValue('3');
  await expect(results).toHaveCount(1);
  await tabOf(page, AI, 'Assistant').click();
  // Collapsing the group keeps both too.
  await collapse(page, AI).click();
  await collapse(page, AI).click();
  await expect(input(page)).toHaveValue('half a thought');

  // History lists the steps made while Layers was shown; Layers keeps its rows.
  await page.getByRole('button', { name: 'Add layer' }).click();
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  const rows = section(page, 'Layers').getByRole('list', { name: 'Layer list' }).getByRole('listitem');
  await expect(rows).toHaveCount(2);
  await tabOf(page, DOCUMENT, 'History').click();
  const steps = section(page, 'History').getByRole('option');
  const count = await steps.count();
  expect(count).toBeGreaterThanOrEqual(3);
  await tabOf(page, DOCUMENT, 'Layers').click();
  await expect(rows).toHaveCount(2);
  await page.keyboard.press('Control+z');
  await tabOf(page, DOCUMENT, 'History').click();
  await expect(steps).toHaveCount(count);
  await expect(section(page, 'History').getByRole('option', { selected: true })).toHaveCount(1);
});

test('while an answer runs the tabs can be chosen; the answer goes on behind Generate, which takes no input (AC-17)', async ({
  page,
}) => {
  await start(page, 'image');
  await sendMessage(page, 'draw something');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'Working' });
  await expect(assistant(page).getByRole('button', { name: 'Stop' })).toBeVisible();

  // The tab row is live, the Generate panel is inert.
  expect(await isInert(tabOf(page, AI, 'Generate'))).toBe(false);
  expect(await isInert(collapse(page, AI))).toBe(false);
  await tabOf(page, AI, 'Generate').click();
  await expect(generate(page)).toBeVisible();
  expect(await isInert(generate(page))).toBe(true);
  expect(await commandCalls(page, 'assistant_stop')).toHaveLength(0);

  // The answer goes on and ends while Generate is shown.
  await event(page, chat, { kind: 'text', text: ' on it. Done.' });
  await done(page, chat);
  await tabOf(page, AI, 'Assistant').click();
  await expect(log(page)).toContainText('Working on it. Done.');
  await expect(assistant(page).getByRole('button', { name: 'Stop' })).toHaveCount(0);
  await expect(assistant(page).getByRole('button', { name: 'Send' })).toBeVisible();
  // Generate takes input again.
  await tabOf(page, AI, 'Generate').click();
  expect(await isInert(generate(page))).toBe(false);
});

test('an answer that is still running shows as far as it has come on return (AC-17)', async ({ page }) => {
  await start(page, 'image');
  await sendMessage(page, 'draw something');
  const chat = String((await sent(page)).chat);
  await tabOf(page, AI, 'Generate').click();
  await event(page, chat, { kind: 'text', text: 'Half way' });
  await tabOf(page, AI, 'Assistant').click();
  await expect(log(page)).toContainText('Half way');
  await expect(assistant(page).getByRole('button', { name: 'Stop' })).toBeVisible();
  await done(page, chat);
});

test('a running generation is not stopped by the Assistant tab; on return Generate shows its progress, then its result (AC-18)', async ({
  page,
}) => {
  await start(page, 'image');
  await tabOf(page, AI, 'Generate').click();
  await prompt(page).fill('a red apple');
  await generate(page).locator('div').getByRole('button', { name: 'Generate', exact: true }).click();
  await expect.poll(async () => (await commandCalls(page, 'comfyui_generate')).length).toBe(1);
  const cancel = generate(page).getByRole('button', { name: 'Cancel' });
  await expect(cancel).toBeVisible();

  await tabOf(page, AI, 'Assistant').click();
  await expect(generate(page)).toHaveCount(0);
  expect(await commandCalls(page, 'comfyui_generation_cancel')).toHaveLength(0);

  // Still running on return.
  await tabOf(page, AI, 'Generate').click();
  await expect(cancel).toBeVisible();
  // It finishes behind the Assistant tab; the result is there on return.
  await tabOf(page, AI, 'Assistant').click();
  await emit(page, 'comfyui:generation', { kind: 'finished', job: 1, count: 1, error: null });
  await tabOf(page, AI, 'Generate').click();
  await expect(generate(page).getByRole('listbox', { name: 'Generated variants' }).getByRole('option')).toHaveCount(1);
  await expect(cancel).toHaveCount(0);
  expect(await commandCalls(page, 'comfyui_generation_cancel')).toHaveLength(0);
});

for (const language of ['English', 'Deutsch'] as const) {
  test(`240 px sidebar, ${language}: all three groups fit; every tab can be brought into view and chosen (AC-14)`, async ({
    page,
  }) => {
    await start(page, 'pixel');
    if (language === 'Deutsch') await switchLanguage(page, 'Deutsch');
    // The narrowest sidebar: three 16 px steps of the splitter from 288 px.
    const splitter = page.locator('[data-editor]:not([inert]) [role="separator"][aria-valuenow]');
    await splitter.focus();
    for (let i = 0; i < 3; i += 1) await page.keyboard.press('ArrowRight');
    const side = page.locator('[data-editor]:not([inert]) aside');
    await expect.poll(async () => Math.round((await side.boundingBox())?.width ?? 0)).toBe(240);
    const noSidewaysScroll = () =>
      side.evaluate((el) => {
        const scroller = el.firstElementChild as HTMLElement;
        return scroller.scrollWidth <= scroller.clientWidth && el.scrollWidth <= el.clientWidth;
      });
    expect(await noSidewaysScroll()).toBe(true);
    const tabs = side.getByRole('tab');
    await expect(tabs).toHaveCount(7);
    for (let i = 0; i < 7; i += 1) {
      const tab = tabs.nth(i);
      await tab.scrollIntoViewIfNeeded();
      await expect(tab).toBeInViewport({ ratio: 1 });
      const b = await tab.boundingBox();
      const s = await side.boundingBox();
      if (!b || !s) throw new Error('tab not laid out');
      expect(b.x).toBeGreaterThanOrEqual(s.x);
      expect(b.x + b.width).toBeLessThanOrEqual(s.x + s.width + 0.5);
      await tab.click();
      await expect(tab).toHaveAttribute('aria-selected', 'true');
      expect(await noSidewaysScroll()).toBe(true);
    }
  });
}
