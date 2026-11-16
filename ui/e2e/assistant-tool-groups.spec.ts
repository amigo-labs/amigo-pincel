import { expect, test, type Page } from '@playwright/test';
import { switchLanguage } from './helpers';
import { done, event, log, panel, sendMessage, sent, startDesktop } from './assistant-helpers';

// Tool calls in a row collapse into one group row
// (docs/specs/ai-chat-panel.md AC-06, revised 2026-11-16).

function groups(page: Page) {
  return log(page).locator('[data-role="tool-group"]');
}

function plainRows(page: Page) {
  return log(page).locator(':scope > [data-role="tool"]');
}

async function start(page: Page, chat: string, id: string, name: string): Promise<void> {
  await event(page, chat, { kind: 'toolStart', id, name, args: { n: id } });
}

async function end(page: Page, chat: string, id: string, ok: boolean): Promise<void> {
  await event(page, chat, { kind: 'toolEnd', id, ok, text: `result of ${id}`, images: [] });
}

test('AC-06 tool calls in a row form one group: count, state, expanded rows', async ({ page }) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'Remove the background');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'Looking at the image.' });

  // A lone call is a plain row.
  await start(page, chat, 't1', 'doc.preview');
  await expect(plainRows(page)).toHaveCount(1);
  await expect(groups(page)).toHaveCount(0);

  // A second call in a row forms a collapsed group.
  await start(page, chat, 't2', 'doc.read_pixels');
  await expect(groups(page)).toHaveCount(1);
  await expect(plainRows(page)).toHaveCount(0);
  const group = groups(page).first();
  const summary = group.locator(':scope > summary');
  await expect(summary).toContainText('2 tool calls');
  await expect(summary).toContainText('Running');
  await expect(group).not.toHaveAttribute('open');

  // Results in another order: rows keep the call order; errors are counted.
  await end(page, chat, 't2', false);
  await expect(summary).toContainText('Running');
  await end(page, chat, 't1', true);
  await expect(summary).toContainText('1 error');

  // Expanded, it lists its rows; it stays expanded while calls join.
  await summary.click();
  await expect(group).toHaveAttribute('open');
  const rows = group.locator('[data-role="tool"]');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText('doc.preview');
  await expect(rows.nth(1)).toContainText('doc.read_pixels');
  await expect(rows.nth(1)).toContainText('Error');
  await start(page, chat, 't3', 'layer.list');
  await expect(summary).toContainText('3 tool calls');
  await expect(summary).toContainText('Running');
  await expect(group).toHaveAttribute('open');
  await expect(rows).toHaveCount(3);
  await end(page, chat, 't3', false);
  await expect(summary).toContainText('2 errors');

  // Each row still expands to its arguments and result.
  await rows.nth(1).locator('summary').click();
  await expect(rows.nth(1)).toContainText('"n": "t2"');
  await expect(rows.nth(1)).toContainText('result of t2');

  // Agent text ends the group; a group of successes reads "Done".
  await event(page, chat, { kind: 'text', text: 'Now duplicating.' });
  await start(page, chat, 't4', 'layer.duplicate');
  await expect(plainRows(page)).toHaveCount(1);
  await start(page, chat, 't5', 'layer.list');
  await expect(groups(page)).toHaveCount(2);
  await end(page, chat, 't4', true);
  await end(page, chat, 't5', true);
  await expect(groups(page).nth(1).locator(':scope > summary')).toContainText('2 tool calls');
  await expect(groups(page).nth(1).locator(':scope > summary')).toContainText('Done');
  await done(page, chat);
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
});

test('AC-06 AC-11 a group whose call runs at Stop reads "Stopped" once the answer ends', async ({
  page,
}) => {
  await startDesktop(page, 'pixel');
  await sendMessage(page, 'Draw');
  const chat = String((await sent(page)).chat);
  await start(page, chat, 't1', 'doc.info');
  await end(page, chat, 't1', true);
  await start(page, chat, 't2', 'layer.list');
  await panel(page).getByRole('button', { name: 'Stop' }).click();
  await done(page, chat, 'stopped');
  const summary = groups(page).first().locator(':scope > summary');
  await expect(summary).toContainText('2 tool calls');
  await expect(summary).toContainText('Stopped');
});

test('AC-06 the German panel names the group and its errors in German', async ({ page }) => {
  await startDesktop(page, 'image');
  await sendMessage(page, 'Remove the background');
  const chat = String((await sent(page)).chat);
  await start(page, chat, 't1', 'doc.preview');
  await start(page, chat, 't2', 'doc.preview');
  await end(page, chat, 't1', false);
  await end(page, chat, 't2', true);
  await done(page, chat);
  await expect(panel(page).getByRole('button', { name: 'Send' })).toBeVisible();
  await switchLanguage(page, 'Deutsch');
  const summary = page
    .locator('section[aria-label="Assistent"]:visible')
    .getByRole('log', { name: 'Chatnachrichten' })
    .locator('[data-role="tool-group"] > summary');
  await expect(summary).toContainText('2 Werkzeugaufrufe');
  await expect(summary).toContainText('1 Fehler');
});
