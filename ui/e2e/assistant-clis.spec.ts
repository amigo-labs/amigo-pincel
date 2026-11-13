import { expect, test, type Page } from '@playwright/test';
import { holdCommands, reply } from './tauri-mock';
import {
  canvas,
  chatCall,
  cliStatus,
  done,
  drawCall,
  event,
  input,
  log,
  panel,
  sendMessage,
  sent,
  startDesktop,
  undoButton,
  type CliId,
} from './assistant-helpers';
import { storedPref } from './helpers';

// More agent CLIs in the Assistant panel (docs/specs/ai-chat-clis.md),
// against the mocked Tauri bridge: the test plays the CLIs' side as
// src-tauri/src/assistant does for each of them.

const ALL: CliId[] = ['claude', 'codex', 'gemini', 'opencode'];

function picker(page: Page) {
  return panel(page).getByRole('combobox', { name: 'Command-line tool' });
}

/** The panel looks the CLIs up again (it does whenever it opens). */
async function reopenPanel(page: Page): Promise<void> {
  const header = panel(page).getByRole('button', { name: 'Assistant' });
  await header.click();
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  await header.click();
  await expect(header).toHaveAttribute('aria-expanded', 'true');
}

test('AC-01 AC-04 the picker lists the CLIs in order; one too old or of unknown version is disabled', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await reply(page, 'assistant_clis', [
    cliStatus('claude', true, '2.1.322'),
    cliStatus('codex', true, '0.100.0', false),
    cliStatus('gemini', true, null),
    cliStatus('opencode', true, '1.19.5'),
  ]);
  await reopenPanel(page);
  const options = picker(page).getByRole('option');
  await expect(options).toHaveText([
    'Claude Code',
    'Codex CLI — Version 0.100.0 is too old — Pincel needs 0.122.0 or newer',
    'Gemini CLI — Version unknown',
    'opencode',
  ]);
  await expect(options.nth(1)).toBeDisabled();
  await expect(options.nth(2)).toBeDisabled();
  await expect(options.nth(3)).toBeEnabled();
  await expect(input(page)).toBeEnabled();
  // The last choice is too old now: the first usable CLI takes its place,
  // and the stored choice is kept for when it is updated.
  await picker(page).selectOption('opencode');
  await expect.poll(() => storedPref(page, 'assistantCli')).toBe('opencode');
  await reply(page, 'assistant_clis', [
    cliStatus('claude', true, '2.1.322'),
    cliStatus('opencode', true, '1.0.0', false),
  ]);
  await reopenPanel(page);
  await expect(picker(page)).toHaveValue('claude');
  expect(await storedPref(page, 'assistantCli')).toBe('opencode');
  await sendMessage(page, 'hi');
  expect((await sent(page)).cli).toBe('claude');
  await done(page, String((await sent(page)).chat));
  // Every installed CLI disabled: no message can be sent.
  await reply(page, 'assistant_clis', [
    cliStatus('claude', true, '2.0.0', false),
    cliStatus('codex', false),
    cliStatus('gemini', true, null),
    cliStatus('opencode', false),
  ]);
  await panel(page).getByRole('button', { name: 'New chat' }).click();
  await reopenPanel(page);
  await expect(picker(page).getByRole('option')).toHaveText([
    'Claude Code — Version 2.0.0 is too old — Pincel needs 2.1.69 or newer',
    'Gemini CLI — Version unknown',
  ]);
  await expect(input(page)).toBeDisabled();
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeDisabled();
});

for (const id of ['codex', 'gemini', 'opencode'] as const) {
  test(`AC-02 AC-07 a ${id} answer streams, changes the canvas in one undo step and the follow-up resumes`, async ({
    page,
  }) => {
    await startDesktop(page, 'image');
    await reply(
      page,
      'assistant_clis',
      ALL.map((c) => cliStatus(c, true)),
    );
    await reopenPanel(page);
    await picker(page).selectOption(id);
    const before = await canvas(page, 'image').screenshot();
    await sendMessage(page, 'Draw a red stroke');
    const request = await sent(page);
    expect(request.cli).toBe(id);
    const chat = String(request.chat);
    await event(page, chat, { kind: 'session', id: `${id}-session` });
    await event(page, chat, { kind: 'text', text: 'Drawing ' });
    await event(page, chat, { kind: 'text', text: 'now.' });
    await expect(log(page).getByText('Drawing now.')).toBeVisible();
    const { tool, args } = drawCall('image');
    await event(page, chat, { kind: 'toolStart', id: 't1', name: tool, args });
    const row = log(page).locator('details', { hasText: tool });
    await expect(row).toContainText('Running');
    expect((await chatCall(page, chat, tool, args)).ok).toBe(true);
    expect((await chatCall(page, chat, tool, { ...args, color: '#00ff00' })).ok).toBe(true);
    await event(page, chat, { kind: 'toolEnd', id: 't1', ok: true, text: 'ok', images: [] });
    await expect(row).toContainText('Done');
    await done(page, chat);
    await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
    // One undo step takes back the whole answer.
    await undoButton(page).click();
    await expect
      .poll(async () => (await canvas(page, 'image').screenshot()).equals(before))
      .toBe(true);
    await expect(undoButton(page)).toBeDisabled();
    // AC-07: the follow-up continues this CLI's session.
    await sendMessage(page, 'Again');
    const next = await sent(page, 1);
    expect(next.cli).toBe(id);
    expect(next.chat).toBe(chat);
    expect(next.session).toBe(`${id}-session`);
  });
}

test('AC-05 AC-06 opencode lists its models, local ones marked; a local model sees the image', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await reply(page, 'assistant_models', [
    { id: 'anthropic/claude-sonnet-5', local: false },
    { id: 'ollama/llama3.2', local: true },
    { id: 'lmstudio/qwen3-8b', local: true },
  ]);
  await reply(
    page,
    'assistant_clis',
    ALL.map((c) => cliStatus(c, true)),
  );
  await reopenPanel(page);
  await picker(page).selectOption('opencode');
  const model = panel(page).getByRole('combobox', { name: 'Model' });
  await expect(model.getByRole('option')).toHaveText([
    'Default',
    'anthropic/claude-sonnet-5',
    'ollama/llama3.2 — local',
    'lmstudio/qwen3-8b — local',
  ]);
  const imageSwitch = panel(page).getByRole('checkbox', { name: 'Agent may see the image' });
  await expect(imageSwitch).toBeVisible();
  await model.selectOption('ollama/llama3.2');
  await expect
    .poll(() => storedPref(page, 'assistantModels'))
    .toEqual({
      opencode: 'ollama/llama3.2',
    });
  // AC-06: a local model hides the switch and gets the pixel tools.
  await expect(imageSwitch).toBeHidden();
  await sendMessage(page, 'What do you see?');
  const request = await sent(page);
  expect(request.model).toBe('ollama/llama3.2');
  expect(request.imageAccess).toBe(true);
  await done(page, String(request.chat));
  // A cloud model of opencode: the switch shows, off for the new chat.
  await model.selectOption('anthropic/claude-sonnet-5');
  await page
    .getByRole('alertdialog', { name: 'Start a new chat?' })
    .getByRole('button', { name: 'Start new chat' })
    .click();
  await expect(imageSwitch).toBeVisible();
  await expect(imageSwitch).not.toBeChecked();
  await sendMessage(page, 'And now?');
  const cloud = await sent(page, 1);
  expect(cloud.model).toBe('anthropic/claude-sonnet-5');
  expect(cloud.imageAccess).toBe(false);
});

test('AC-05 a model list that fails or takes over 5 s leaves the field as free text', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await reply(page, 'assistant_models', { __error: 'opencode models failed' });
  await reply(
    page,
    'assistant_clis',
    ALL.map((c) => cliStatus(c, true)),
  );
  await reopenPanel(page);
  await picker(page).selectOption('opencode');
  const free = panel(page).getByRole('textbox', { name: 'Model' });
  await expect(free).toHaveAttribute('placeholder', 'Default');
  await free.fill('ollama/mistral');
  await free.press('Enter');
  await expect
    .poll(() => storedPref(page, 'assistantModels'))
    .toEqual({
      opencode: 'ollama/mistral',
    });
  // A hung `opencode models`: still free text after 5 s.
  await holdCommands(page, ['assistant_models']);
  await reply(page, 'assistant_models', [{ id: 'ollama/llama3.2', local: true }]);
  await reopenPanel(page);
  await page.waitForTimeout(5_500);
  await expect(free).toBeEditable();
  await expect(free).toHaveValue('ollama/mistral');
  await expect(panel(page).getByRole('combobox', { name: 'Model' })).toHaveCount(0);
});

test('AC-08 a local model whose server is down, or that cannot call tools, gets its hint', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  await reply(
    page,
    'assistant_clis',
    ALL.map((c) => cliStatus(c, true)),
  );
  await reopenPanel(page);
  await picker(page).selectOption('opencode');
  const cases = [
    [
      'localServer',
      'Cannot connect to API: connect ECONNREFUSED 127.0.0.1:11434',
      'Start Ollama / LM Studio and try again',
    ],
    [
      'noTools',
      'registry.ollama.ai/library/gemma:2b does not support tools',
      'This model cannot call tools — pick another',
    ],
  ] as const;
  for (const [i, [hint, error, text]] of cases.entries()) {
    await sendMessage(page, `try ${hint}`);
    const chat = String((await sent(page, i)).chat);
    await done(page, chat, 'failed', { text: error, hint });
    const end = log(page).getByRole('status').last();
    await expect(end).toContainText(`Failed: ${error}`);
    await expect(end).toContainText(text);
  }
});
