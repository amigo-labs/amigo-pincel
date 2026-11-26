import { expect, test, type Page } from '@playwright/test';
import { reply } from './tauri-mock';
import {
  cliStatus,
  done,
  log,
  newTab,
  panel,
  sendMessage,
  sent,
  startDesktop,
  tabs,
  type CliId,
} from './assistant-helpers';
import { hidePanel, showPanel, storedPref } from './helpers';

// The effort field beside the Assistant's model field
// (docs/specs/ai-chat-clis.md AC-10 – AC-15), against the mocked Tauri
// bridge: the test plays the CLIs' side as src-tauri/src/assistant does.

const ALL: CliId[] = ['claude', 'codex', 'gemini', 'opencode'];

function picker(page: Page) {
  return panel(page).getByRole('combobox', { name: 'Command-line tool' });
}

function effortSelect(page: Page) {
  return panel(page).getByRole('combobox', { name: 'Effort' });
}

function effortText(page: Page) {
  return panel(page).getByRole('textbox', { name: 'Effort' });
}

/** The panel looks the CLIs up again (it does whenever it opens). */
async function reopenPanel(page: Page): Promise<void> {
  await hidePanel(page, 'Assistant');
  await showPanel(page, 'Assistant');
}

/** Opens the desktop app with every CLI installed (`statuses` overrides). */
async function startWithAll(page: Page, statuses?: Record<string, unknown>[]): Promise<void> {
  await startDesktop(page, 'image');
  await reply(page, 'assistant_clis', statuses ?? ALL.map((c) => cliStatus(c, true)));
  await reopenPanel(page);
}

test('AC-10 each CLI offers its own levels; Gemini CLI and a version without the option offer none', async ({
  page,
}) => {
  await startWithAll(page);
  await expect(effortSelect(page).getByRole('option')).toHaveText([
    'Default',
    'low',
    'medium',
    'high',
    'xhigh',
    'max',
  ]);
  await expect(effortSelect(page)).toHaveValue('');
  await picker(page).selectOption('codex');
  await expect(effortSelect(page).getByRole('option')).toHaveText([
    'Default',
    'minimal',
    'low',
    'medium',
    'high',
    'xhigh',
  ]);
  await picker(page).selectOption('opencode');
  await expect(effortSelect(page)).toHaveCount(0);
  await expect(effortText(page)).toHaveAttribute('placeholder', 'Default');
  await picker(page).selectOption('gemini');
  await expect(effortSelect(page)).toHaveCount(0);
  await expect(effortText(page)).toHaveCount(0);

  // Claude Code older than its effort option: no field, no effort sent.
  await reply(page, 'assistant_clis', [cliStatus('claude', true, '2.1.70', true, false)]);
  await reopenPanel(page);
  await picker(page).selectOption('claude');
  await expect(effortSelect(page)).toHaveCount(0);
  await sendMessage(page, 'Hello');
  expect((await sent(page)).effort).toBeNull();
});

test('AC-11 AC-12 an effort goes with the next message; changing it keeps the chat and its session', async ({
  page,
}) => {
  await startWithAll(page);
  await effortSelect(page).selectOption('high');
  await sendMessage(page, 'First');
  const first = await sent(page);
  expect(first.effort).toBe('high');
  const chat = String(first.chat);
  await done(page, chat);

  await effortSelect(page).selectOption('max');
  await expect(page.getByRole('alertdialog', { name: 'Start a new chat?' })).toHaveCount(0);
  await expect(log(page)).toContainText('First');
  await sendMessage(page, 'Second');
  const second = await sent(page, 1);
  expect(second.chat).toBe(chat);
  expect(second.effort).toBe('max');
  await done(page, chat);

  await effortSelect(page).selectOption('');
  await sendMessage(page, 'Third');
  const third = await sent(page, 2);
  expect(third.chat).toBe(chat);
  expect(third.effort).toBeNull();
});

test('AC-13 the last effort per CLI seeds new chats across restarts, not other open chats', async ({
  page,
}) => {
  await startWithAll(page);
  await picker(page).selectOption('codex');
  await effortSelect(page).selectOption('low');
  await expect.poll(() => storedPref(page, 'assistantEfforts')).toEqual({ codex: 'low' });
  // Another CLI starts with its own memory: none yet.
  await picker(page).selectOption('claude');
  await expect(effortSelect(page)).toHaveValue('');
  await effortSelect(page).selectOption('high');
  await sendMessage(page, 'In the first tab');
  const first = await sent(page);
  expect(first.effort).toBe('high');
  await done(page, String(first.chat));

  // A new chat in another tab starts with the remembered effort; changing
  // it there leaves the first tab's chat as it was.
  await newTab(page, 'image');
  await expect(effortSelect(page)).toHaveValue('high');
  await effortSelect(page).selectOption('low');
  await tabs(page).first().click();
  await expect(effortSelect(page)).toHaveValue('high');

  // After a restart, each CLI comes back with its last effort.
  await expect
    .poll(() => storedPref(page, 'assistantEfforts'))
    .toEqual({ codex: 'low', claude: 'low' });
  await startWithAll(page);
  await expect(effortSelect(page)).toHaveValue('low');
  await picker(page).selectOption('codex');
  await expect(effortSelect(page)).toHaveValue('low');
});

test('AC-14 an effort the CLI rejects shows its error and stays selected', async ({ page }) => {
  await startWithAll(page);
  await effortSelect(page).selectOption('max');
  await sendMessage(page, 'Think hard');
  const chat = String((await sent(page)).chat);
  const error = 'API Error: 400 effort level max is not supported by this model';
  await done(page, chat, 'failed', { text: error, hint: 'unexpected' });
  await expect(log(page).getByRole('status').last()).toContainText(`Failed: ${error}`);
  await expect(effortSelect(page)).toHaveValue('max');
});

test('AC-15 opencode takes a variant name; other text is marked and cannot be sent', async ({
  page,
}) => {
  await startWithAll(page);
  await picker(page).selectOption('opencode');
  const send = panel(page).getByRole('button', { name: 'Send' });
  const field = effortText(page);
  await panel(page).getByRole('textbox', { name: 'Message' }).fill('Hi');
  for (const bad of ['a b', 'hi"gh', 'x'.repeat(33)]) {
    await field.fill(bad);
    await expect(field).toHaveAttribute('aria-invalid', 'true');
    await expect(panel(page).getByText('Use letters, digits, - or _')).toBeVisible();
    await expect(send).toBeDisabled();
  }
  await field.fill(' high ');
  await expect(field).not.toHaveAttribute('aria-invalid', 'true');
  await expect(send).toBeEnabled();
  await send.click();
  expect((await sent(page)).effort).toBe('high');
  await done(page, String((await sent(page)).chat));

  await field.fill('');
  await sendMessage(page, 'Again');
  expect((await sent(page, 1)).effort).toBeNull();
});
