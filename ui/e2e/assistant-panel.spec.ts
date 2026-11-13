import { expect, test } from '@playwright/test';
import { createDocument, storedPref } from './helpers';
import { commandCalls, reply } from './tauri-mock';
import {
  canvas,
  chatCall,
  claude,
  done,
  drawCall,
  event,
  input,
  log,
  newDoc,
  newTab,
  panel,
  redoButton,
  sendMessage,
  sent,
  startDesktop,
  undoButton,
} from './assistant-helpers';

// The Assistant panel (docs/specs/ai-chat-panel.md): where it shows, the
// CLI picker and model, and one answer from Send to its undo step.

test('AC-01 the browser app shows no Assistant panel in either editor', async ({ page }) => {
  for (const mode of ['pixel', 'image'] as const) {
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(page.getByRole('region', { name: 'Layers' }).first()).toBeVisible();
    await expect(page.locator('section[aria-label="Assistant"]')).toHaveCount(0);
  }
});

test('AC-02 AC-03 AC-04 both editors list the panel; no CLI found; picker, model and their prefs', async ({
  page,
}) => {
  await startDesktop(page, 'image', false);
  // AC-03: nothing installed.
  await expect(panel(page).getByText('No supported AI command-line tool found')).toBeVisible();
  await expect(
    panel(page).getByText('Install Claude Code: https://docs.claude.com/en/docs/claude-code'),
  ).toBeVisible();
  await expect(input(page)).toBeDisabled();
  // Installed meanwhile: found the next time the panel opens.
  await reply(page, 'assistant_clis', [claude(true)]);
  const header = panel(page).getByRole('button', { name: 'Assistant' });
  await header.click();
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  await header.click();
  await expect(input(page)).toBeEnabled();
  // AC-04: the picker lists Claude Code, the model field says Default.
  const picker = panel(page).getByRole('combobox', { name: 'Command-line tool' });
  await expect(picker.getByRole('option')).toHaveText(['Claude Code']);
  const model = panel(page).getByRole('textbox', { name: 'Model' });
  await expect(model).toHaveAttribute('placeholder', 'Default');
  await model.fill('opus');
  await model.press('Enter');
  await expect.poll(() => storedPref(page, 'assistantModels')).toEqual({ claude: 'opus' });
  await expect.poll(() => storedPref(page, 'assistantCli')).toBe('claude');
  // AC-02: collapsed, and still collapsed after a restart.
  await header.click();
  await expect
    .poll(async () => {
      const layout = (await storedPref(page, 'layout:image')) as {
        collapsed?: Record<string, boolean>;
      };
      return layout?.collapsed?.assistant;
    })
    .toBe(true);
  await page.reload();
  await reply(page, 'assistant_clis', [claude(true)]);
  await page.getByRole('button', { name: 'Got it' }).click();
  await newDoc(page, 'image');
  await expect(panel(page).getByRole('button', { name: 'Assistant' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  await panel(page).getByRole('button', { name: 'Assistant' }).click();
  await expect(panel(page).getByRole('textbox', { name: 'Model' })).toHaveValue('opus');
  // The Pixel editor lists the panel too.
  await newTab(page, 'pixel');
  await expect(panel(page)).toHaveCount(1);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`AC-05 – AC-08 AC-12 AC-15 AC-21 a ${mode} answer streams, changes the canvas and undoes in one step`, async ({
    page,
  }) => {
    await startDesktop(page, mode);
    const send = panel(page).getByRole('button', { name: 'Send' });
    await expect(send).toBeDisabled();
    await input(page).fill('   ');
    await expect(send).toBeDisabled();
    const before = await canvas(page, mode).screenshot();
    await sendMessage(page, 'Draw a red square');
    await expect(log(page).getByText('Draw a red square')).toBeVisible();
    const request = await sent(page);
    const chat = String(request.chat);
    expect(chat).toMatch(/^chat-\d+-1$/);
    expect(request.cli).toBe('claude');
    expect(request.model).toBeNull();
    expect(request.session).toBeNull();
    expect(request.imageAccess).toBe(false);
    // AC-15: the editor-context block in front of the text.
    const prompt = String(request.prompt);
    expect(prompt).toMatch(
      /^<editor-context>\ndocument: .+ \((Pixel|Image)\), 16×16\nselection: none\n/,
    );
    expect(prompt).toContain('recent steps: none');
    expect(prompt.endsWith('</editor-context>\n\nDraw a red square')).toBe(true);
    expect(String(request.systemPrompt)).toContain('PixelAssist');
    // AC-10: Stop instead of Send.
    await expect(panel(page).getByRole('button', { name: 'Stop' })).toBeVisible();
    // AC-05, AC-21: streamed text, HTML shown as text.
    await event(page, chat, { kind: 'session', id: 'session-1' });
    await event(page, chat, { kind: 'text', text: 'On it: <img src=x onerror="window.__xss=1">' });
    await event(page, chat, {
      kind: 'text',
      text: ' and `draw.rect` next.\n```\n<b>code</b>\n```',
    });
    await expect(log(page).getByText('On it: <img src=x onerror="window.__xss=1">')).toBeVisible();
    await expect(log(page).locator('code', { hasText: 'draw.rect' })).toBeVisible();
    await expect(log(page).locator('pre', { hasText: '<b>code</b>' })).toBeVisible();
    await expect(log(page).locator('img, b')).toHaveCount(0);
    expect(
      await page.evaluate(() => (window as unknown as { __xss?: number }).__xss),
    ).toBeUndefined();
    // AC-06, AC-07: a tool row, the call itself changes the canvas.
    const { tool, args } = drawCall(mode);
    await event(page, chat, { kind: 'toolStart', id: 't1', name: tool, args });
    const row = log(page).locator('details', { hasText: tool });
    await expect(row).toContainText('Running');
    const r1 = await chatCall(page, chat, tool, args);
    expect(r1.ok, r1.error).toBe(true);
    await expect
      .poll(async () => (await canvas(page, mode).screenshot()).equals(before))
      .toBe(false);
    const r2 = await chatCall(page, chat, tool, { ...args, color: '#0000ff' });
    expect(r2.ok, r2.error).toBe(true);
    await event(page, chat, {
      kind: 'toolEnd',
      id: 't1',
      ok: true,
      text: 'drawn',
      images: [[16, 16]],
    });
    await expect(row).toContainText('Done');
    await row.locator('summary').click();
    await expect(row).toContainText('"color": "#ff0000"');
    await expect(row).toContainText('drawn');
    await expect(row).toContainText('image (16×16)');
    const after = await canvas(page, mode).screenshot();
    // AC-08: one step named "Assistant".
    await done(page, chat);
    await expect(send).toBeVisible();
    await expect
      .poll(async () => (await commandCalls(page, 'mcp_chat_answering')).map((c) => c.args))
      .toEqual([{ chat, running: false }]);
    const history = page.getByRole('region', { name: 'History' });
    await history.getByRole('button', { name: 'History' }).click();
    await expect(history.getByText('Assistant', { exact: true })).toBeVisible();
    await undoButton(page).click();
    await expect
      .poll(async () => (await canvas(page, mode).screenshot()).equals(before))
      .toBe(true);
    await expect(undoButton(page)).toBeDisabled();
    await redoButton(page).click();
    await expect.poll(async () => (await canvas(page, mode).screenshot()).equals(after)).toBe(true);
    await expect(redoButton(page)).toBeDisabled();
    // AC-12: the follow-up continues the session; AC-15 names the step.
    await sendMessage(page, 'Now make it blue');
    const next = await sent(page, 1);
    expect(next.chat).toBe(chat);
    expect(next.session).toBe('session-1');
    expect(String(next.prompt)).toContain('recent steps: Assistant');
  });
}

test('AC-09 an answer that changes nothing adds no undo step', async ({ page }) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'What is on the canvas?');
  const chat = String((await sent(page)).chat);
  const info = await chatCall(page, chat, 'doc.info');
  expect(info.ok).toBe(true);
  await event(page, chat, { kind: 'text', text: 'An empty canvas.' });
  await done(page, chat);
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  await expect(undoButton(page)).toBeDisabled();
});

test('AC-11 Stop ends the answer: open rows and the answer read "Stopped", its change is one step', async ({
  page,
}) => {
  await startDesktop(page, 'pixel');
  const before = await canvas(page, 'pixel').screenshot();
  await sendMessage(page, 'Draw');
  const chat = String((await sent(page)).chat);
  const { tool, args } = drawCall('pixel');
  await event(page, chat, { kind: 'toolStart', id: 't1', name: tool, args });
  await panel(page).getByRole('button', { name: 'Stop' }).click();
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_stop')).map((c) => c.args))
    .toEqual([{ chat }]);
  // The call already under way still runs and counts towards the answer.
  expect((await chatCall(page, chat, tool, args)).ok).toBe(true);
  await done(page, chat, 'stopped');
  await expect(log(page).locator('details', { hasText: tool })).toContainText('Stopped');
  await expect(log(page).getByRole('status')).toHaveText('Stopped');
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  await undoButton(page).click();
  await expect
    .poll(async () => (await canvas(page, 'pixel').screenshot()).equals(before))
    .toBe(true);
  await expect(undoButton(page)).toBeDisabled();
});

test('AC-20 AC-27 a failed answer shows the CLI error, its hint, and marks open rows "Failed"', async ({
  page,
}) => {
  await startDesktop(page, 'image');
  const cases = [
    ['signIn', 'Run `claude` once in a terminal to sign in'],
    ['offline', 'No internet connection'],
    ['permission', 'Claude Code asked for a permission Pincel does not grant'],
    ['unexpected', 'Claude Code stopped unexpectedly'],
  ] as const;
  for (const [i, [hint, text]] of cases.entries()) {
    await sendMessage(page, `try ${hint}`);
    const chat = String((await sent(page, i)).chat);
    await event(page, chat, { kind: 'toolStart', id: `t${i}`, name: 'doc.info', args: {} });
    await done(page, chat, 'failed', { text: `cli says ${hint}`, hint });
    const end = log(page).getByRole('status').last();
    await expect(end).toContainText(`Failed: cli says ${hint}`);
    await expect(end).toContainText(text);
    await expect(log(page).locator('details').last()).toContainText('Failed');
  }
  // Unknown model: names the model.
  const model = panel(page).getByRole('textbox', { name: 'Model' });
  await model.fill('nope-1');
  await model.press('Enter');
  await page
    .getByRole('alertdialog', { name: 'Start a new chat?' })
    .getByRole('button', { name: 'Start new chat' })
    .click();
  await sendMessage(page, 'hi');
  const chat = String((await sent(page, cases.length)).chat);
  expect((await sent(page, cases.length)).model).toBe('nope-1');
  await done(page, chat, 'failed', { text: 'model not found', hint: 'unknownModel' });
  await expect(log(page).getByRole('status').last()).toContainText(
    'Claude Code does not know the model nope-1',
  );
});

test('AC-28 a chat that cannot start shows the reason and adds nothing', async ({ page }) => {
  await startDesktop(page, 'pixel');
  await reply(page, 'assistant_send', { __error: 'the endpoint cannot bind a port' });
  await sendMessage(page, 'Draw');
  await expect(log(page).getByRole('status')).toHaveText(
    'The chat could not start: the endpoint cannot bind a port',
  );
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  await expect(undoButton(page)).toBeDisabled();
});
