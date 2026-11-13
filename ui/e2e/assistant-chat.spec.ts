import { expect, test } from '@playwright/test';
import { drag, storedPref, switchLanguage } from './helpers';
import { closeWindow, commandCalls, destroyed, reply } from './tauri-mock';
import {
  canvas,
  chatCall,
  claude,
  done,
  drawCall,
  event,
  input,
  isInert,
  log,
  newDoc,
  newTab,
  panel,
  sendMessage,
  sent,
  startDesktop,
  tabs,
  undoButton,
} from './assistant-helpers';

// The Assistant chat across tabs and settings (docs/specs/ai-chat-panel.md):
// the inert editor while answering, chats per tab, New chat, the CLI /
// model question, the image switch, the system prompt, calls bound to the
// chat's tab and closing the app during an answer.

test('AC-10 while an answer runs only zoom, pan and the chat stay usable', async ({ page }) => {
  await startDesktop(page, 'pixel');
  await sendMessage(page, 'Draw');
  const chat = String((await sent(page)).chat);
  const stop = panel(page).getByRole('button', { name: 'Stop' });
  await expect(stop).toBeVisible();
  // Rail, tabs, quick actions, menus but View, the other panels.
  expect(await isInert(page.getByRole('toolbar', { name: 'Tool options' }))).toBe(true);
  expect(await isInert(tabs(page).first())).toBe(true);
  expect(await isInert(undoButton(page))).toBe(true);
  expect(await isInert(page.locator('[data-menu-trigger="edit"]'))).toBe(true);
  expect(await isInert(page.locator('[data-menu-trigger="file"]'))).toBe(true);
  expect(await isInert(page.locator('[data-menu-trigger="view"]'))).toBe(false);
  expect(await isInert(page.getByRole('region', { name: 'Layers' }))).toBe(true);
  // The chat's own controls.
  await expect(panel(page).getByRole('button', { name: 'New chat' })).toBeDisabled();
  await expect(panel(page).getByRole('button', { name: 'Edit system prompt' })).toBeDisabled();
  await expect(panel(page).getByRole('combobox', { name: 'Command-line tool' })).toBeDisabled();
  await expect(panel(page).getByRole('textbox', { name: 'Model' })).toBeDisabled();
  await expect(
    panel(page).getByRole('checkbox', { name: 'Agent may see the image' }),
  ).toBeDisabled();
  expect(await isInert(stop)).toBe(false);
  // Drawing and document shortcuts do nothing; zoom works.
  const before = await canvas(page, 'pixel').screenshot();
  await drag(page, canvas(page, 'pixel'), [0.2, 0.2], [0.8, 0.8]);
  await canvas(page, 'pixel').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await page.keyboard.press('Control+n');
  await expect(page.getByRole('dialog', { name: 'New document' })).toHaveCount(0);
  expect((await canvas(page, 'pixel').screenshot()).equals(before)).toBe(true);
  const zoom = page
    .getByRole('group', { name: 'Zoom' })
    .getByRole('button', { name: 'Fit to window' });
  const zoomBefore = await zoom.textContent();
  await page.keyboard.press('+');
  await expect(zoom).not.toHaveText(zoomBefore ?? '');
  // Everything comes back once the answer ends.
  await done(page, chat);
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  expect(await isInert(page.locator('[data-menu-trigger="edit"]'))).toBe(false);
  expect(await isInert(tabs(page).first())).toBe(false);
});

test('AC-13 AC-14 each tab has its own chat; New chat starts over; closing a tab ends its chat', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'first tab message');
  const first = String((await sent(page)).chat);
  await event(page, first, { kind: 'session', id: 'session-a' });
  await done(page, first);
  await newTab(page, 'pixel');
  await expect(log(page).getByText('first tab message')).toHaveCount(0);
  await sendMessage(page, 'second tab message');
  const second = String((await sent(page, 1)).chat);
  expect(second).not.toBe(first);
  await done(page, second);
  await tabs(page).first().click();
  await expect(log(page).getByText('first tab message')).toBeVisible();
  // AC-13: New chat.
  await panel(page).getByRole('button', { name: 'New chat' }).click();
  await expect(log(page).getByText('first tab message')).toHaveCount(0);
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_end')).map((c) => c.args.chat))
    .toEqual([first]);
  await sendMessage(page, 'fresh start');
  const fresh = await sent(page, 2);
  expect(fresh.chat).toBe(first.replace(/-1$/, '-2'));
  expect(fresh.session).toBeNull();
  await done(page, String(fresh.chat));
  // AC-14: closing the Pixel tab ends its chat.
  await tabs(page).nth(1).click();
  await page.keyboard.press('Alt+w');
  await expect(tabs(page)).toHaveCount(1);
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_end')).map((c) => c.args.chat))
    .toContain(second);
});

test('AC-25 changing the model of a chat with messages asks first', async ({ page }) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'hello');
  const chat = String((await sent(page)).chat);
  await done(page, chat);
  const model = panel(page).getByRole('textbox', { name: 'Model' });
  const question = page.getByRole('alertdialog', { name: 'Start a new chat?' });
  await model.fill('sonnet');
  await model.press('Enter');
  await question.getByRole('button', { name: 'Cancel' }).click();
  await expect(model).toHaveValue('');
  await expect(log(page).getByText('hello')).toBeVisible();
  await model.fill('sonnet');
  await model.press('Enter');
  await question.getByRole('button', { name: 'Start new chat' }).click();
  await expect(log(page).getByText('hello')).toHaveCount(0);
  await expect(model).toHaveValue('sonnet');
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_end')).map((c) => c.args.chat))
    .toEqual([chat]);
  await sendMessage(page, 'again');
  expect((await sent(page, 1)).model).toBe('sonnet');
});

test('AC-17 the image switch is off for a new chat and warns when turned on', async ({ page }) => {
  await startDesktop(page, 'image');
  const toggle = panel(page).getByRole('checkbox', { name: 'Agent may see the image' });
  await expect(toggle).not.toBeChecked();
  await expect(panel(page).getByText("The image is sent to the model's provider")).toHaveCount(0);
  await toggle.check();
  await expect(panel(page).getByText("The image is sent to the model's provider")).toBeVisible();
  await expect
    .poll(async () => (await commandCalls(page, 'mcp_chat_image_access')).map((c) => c.args.on))
    .toEqual([true]);
  await sendMessage(page, 'look');
  const request = await sent(page);
  expect(request.imageAccess).toBe(true);
  await done(page, String(request.chat));
  await panel(page).getByRole('button', { name: 'New chat' }).click();
  await expect(toggle).not.toBeChecked();
});

test('AC-18 AC-19 AC-23 the system prompt: default per language, edited, saved, reset', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await panel(page).getByRole('button', { name: 'Edit system prompt' }).click();
  const dialog = page.getByRole('dialog', { name: 'System prompt' });
  const field = dialog.getByRole('textbox');
  await expect(field).toHaveValue(/^You are "PixelAssist"/);
  await field.fill('  ');
  await expect(dialog.getByRole('button', { name: 'Save' })).toBeDisabled();
  await field.fill('Be brief.');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => storedPref(page, 'assistantPrompt')).toBe('Be brief.');
  await sendMessage(page, 'hi');
  const first = await sent(page);
  expect(first.systemPrompt).toBe('Be brief.');
  await done(page, String(first.chat));
  // Kept after a restart.
  await page.reload();
  await reply(page, 'assistant_clis', [claude(true)]);
  await page.getByRole('button', { name: 'Got it' }).click();
  await newDoc(page, 'image');
  await sendMessage(page, 'hi again');
  const second = await sent(page);
  expect(second.systemPrompt).toBe('Be brief.');
  await done(page, String(second.chat));
  // Reset to default, in German (AC-18, AC-23).
  await switchLanguage(page, 'Deutsch');
  const de = page.locator('section[aria-label="Assistent"]:visible');
  await de.getByRole('button', { name: 'Systemprompt bearbeiten' }).click();
  const deDialog = page.getByRole('dialog', { name: 'Systemprompt' });
  await deDialog.getByRole('button', { name: 'Auf Standard zurücksetzen' }).click();
  await expect(deDialog.getByRole('textbox')).toHaveValue(/^Du bist „PixelAssist“/);
  await deDialog.getByRole('button', { name: 'Abbrechen' }).click();
  await expect.poll(() => storedPref(page, 'assistantPrompt')).toBeNull();
  await de.getByRole('textbox', { name: 'Nachricht' }).fill('Hallo');
  await de.getByRole('button', { name: 'Senden' }).click();
  expect(String((await sent(page, 1)).systemPrompt)).toMatch(/^Du bist „PixelAssist“/);
  await expect(de.getByRole('button', { name: 'Stopp' })).toBeVisible();
});

test('AC-26 the chat gets no document-level tools and acts on its own tab only', async ({
  page,
}) => {
  await startDesktop(page, 'pixel');
  await expect(input(page)).toBeEnabled();
  await newTab(page, 'image');
  await expect(input(page)).toBeEnabled();
  // Tab 1's chat calls while tab 2 is shown.
  const chat = 'chat-1-1';
  for (const tool of ['doc.open', 'doc.new', 'doc.close', 'doc.list', 'doc.undo', 'doc.redo']) {
    const r = await chatCall(page, chat, tool);
    expect(r.ok, tool).toBe(false);
    expect(r.error).toBe(`\`${tool}\` is not available in the chat`);
  }
  const foreign = await chatCall(page, chat, 'doc.info', { doc: 'tab-2' });
  expect(foreign.ok).toBe(false);
  expect(foreign.error).toContain('not available in the chat');
  const own = await chatCall(page, chat, 'doc.info');
  expect(own.ok, own.error).toBe(true);
  expect(own.json?.doc).toBe('tab-1');
  expect(own.json?.mode).toBe('pixel');
  // A chat that ended has no tab.
  const ended = await chatCall(page, 'chat-9-1', drawCall('pixel').tool, drawCall('pixel').args);
  expect(ended.ok).toBe(false);
});

test('AC-22 closing the window during an answer stops it before the close flow', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'Draw');
  const chat = String((await sent(page)).chat);
  await closeWindow(page);
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_stop')).map((c) => c.args.chat))
    .toEqual([chat]);
  expect(await destroyed(page)).toBe(0);
  await done(page, chat, 'stopped');
  await expect.poll(() => destroyed(page)).toBe(1);
});
