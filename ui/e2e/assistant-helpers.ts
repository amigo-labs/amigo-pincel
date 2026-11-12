import { expect, type Locator, type Page } from '@playwright/test';
import { imageCanvas, pixelCanvas, waitForEditor } from './helpers';
import { commandCalls, emit, installTauriMock, reply, type MockWindow } from './tauri-mock';

// Helpers of the Assistant chat specs (docs/specs/ai-chat-panel.md), run
// against the mocked Tauri bridge: the test plays the CLI side, answering
// `assistant_clis` / `assistant_send` and emitting `assistant-event`s as
// src-tauri/src/assistant does, and the MCP endpoint, emitting `mcp-call`s
// with the chat's id as the CLI's tool calls arrive there.

export type Mode = 'pixel' | 'image';

/** Claude Code as `assistant_clis` reports it. */
export function claude(installed: boolean): Record<string, unknown> {
  return {
    id: 'claude',
    name: 'Claude Code',
    installUrl: 'https://docs.claude.com/en/docs/claude-code',
    installed,
  };
}

export const RECT = { x0: 1, y0: 1, x1: 8, y1: 8, color: '#ff0000', filled: true };
export const STROKE = {
  points: [
    [2, 2],
    [13, 13],
  ],
  size: 4,
  color: '#ff0000',
};

/** A document-changing tool call for `mode`. */
export function drawCall(mode: Mode): { tool: string; args: Record<string, unknown> } {
  return mode === 'pixel'
    ? { tool: 'draw.rect', args: RECT }
    : { tool: 'draw.stroke', args: STROKE };
}

/** Opens the desktop app (mocked) with a 16×16 document of `mode`;
 *  Claude Code counts as installed unless `installed` is false. */
export async function startDesktop(page: Page, mode: Mode, installed = true): Promise<void> {
  await installTauriMock(page);
  await page.goto('./');
  await reply(page, 'assistant_clis', [claude(installed)]);
  await page.getByRole('button', { name: 'Got it' }).click();
  await newDoc(page, mode);
}

/** Creates a 16×16 document of `mode` through the start screen's card. */
export async function newDoc(page: Page, mode: Mode): Promise<void> {
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

/** File ▸ New… as a new tab of `mode`. */
export async function newTab(page: Page, mode: Mode): Promise<void> {
  await page.keyboard.press('Control+n');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: mode === 'image' ? /^Image/ : /^Pixel/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(dialog).toBeHidden();
  await waitForEditor(page, mode);
}

/** The tabs of the tab bar. */
export function tabs(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
}

export function canvas(page: Page, mode: Mode): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

/** The Assistant panel of the shown editor. */
export function panel(page: Page): Locator {
  return page.locator('section[aria-label="Assistant"]:visible');
}

export function input(page: Page): Locator {
  return panel(page).getByRole('textbox', { name: 'Message' });
}

export function log(page: Page): Locator {
  return panel(page).getByRole('log', { name: 'Chat messages' });
}

/** Types `text` and sends it with the Send button. */
export async function sendMessage(page: Page, text: string): Promise<void> {
  await input(page).fill(text);
  await panel(page).getByRole('button', { name: 'Send' }).click();
}

/** The `request` of the `n`-th `assistant_send` call (0-based), once made. */
export async function sent(page: Page, n = 0): Promise<Record<string, unknown>> {
  await expect
    .poll(async () => (await commandCalls(page, 'assistant_send')).length)
    .toBeGreaterThan(n);
  const calls = await commandCalls(page, 'assistant_send');
  return calls[n]?.args.request as Record<string, unknown>;
}

/** Emits one `assistant-event` of `chat`. */
export async function event(page: Page, chat: string, ev: Record<string, unknown>): Promise<void> {
  await emit(page, 'assistant-event', { chat, event: ev });
}

/** Ends the answer of `chat`. */
export async function done(
  page: Page,
  chat: string,
  outcome: 'finished' | 'stopped' | 'failed' = 'finished',
  error: { text: string; hint: string } | null = null,
): Promise<void> {
  await event(page, chat, { kind: 'done', outcome, error });
}

let nextCall = 1_000;

/** The agent's tool call `tool` from `chat` reaching the editor; returns
 *  the UI's reply. */
export async function chatCall(
  page: Page,
  chat: string | null,
  tool: string,
  args: Record<string, unknown> = {},
): Promise<{ ok: boolean; json?: Record<string, unknown>; error?: string }> {
  const id = nextCall++;
  await emit(page, 'mcp-call', { id, tool, args, chat });
  await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
  return page.evaluate(
    (i) => (window as unknown as MockWindow).__mock.mcpReplies[i],
    id,
  ) as Promise<{
    ok: boolean;
    json?: Record<string, unknown>;
    error?: string;
  }>;
}

/** `true` when `loc` sits inside an inert subtree. */
export async function isInert(loc: Locator): Promise<boolean> {
  return loc.evaluate((el) => el.closest('[inert]') !== null);
}

/** The header Undo / Redo buttons. */
export function undoButton(page: Page): Locator {
  return page.locator('[data-tour="quick"]').getByRole('button', { name: 'Undo', exact: true });
}

export function redoButton(page: Page): Locator {
  return page.locator('[data-tour="quick"]').getByRole('button', { name: 'Redo', exact: true });
}
