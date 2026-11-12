// The Assistant chats of the desktop app (docs/specs/ai-chat-panel.md): one
// chat per document tab, kept until the tab closes (AC-14). An answer runs
// the selected CLI in the Tauri process (src-tauri/src/assistant), which
// streams its progress back as `assistant-event`s; the CLI reaches the
// document through the live MCP endpoint with the chat's token, and the
// answer's changes fold into one undo step (AC-08, AC-09).
import { untrack } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { liveIdle } from '../agent/live-host';
import { docIdOf } from '../agent/tab-calls';
import { parseWire } from '../agent/wire';
import { historyStepName } from '../history/names';
import { locale, t } from '../i18n/index.svelte';
import { getPref, setPref } from '../idb/prefs';
import { session, tabByKey, type ShellTab } from '../shell/session.svelte';
import { actionFor, type ShortcutAction } from '../shell/shortcuts';
import { applyEvent, type ChatEvent, type ChatItem } from './chat-events';
import { bindChatTab, unbindChatTab } from './chat-tabs';
import { withContext, type EditorContext } from './context';
import { defaultPrompt } from './prompts';

/** A supported CLI as `assistant_clis` reports it (AC-03). */
export interface CliStatus {
  id: string;
  name: string;
  installUrl: string;
  installed: boolean;
}

/** One tab's chat. */
export interface Chat {
  /** The tab's key (`ShellTab.key`). */
  readonly key: number;
  /** `chat-<key>-<n>`: a new id per New chat, so the endpoint gives it a
   *  new token (AC-13). */
  id: string;
  n: number;
  items: ChatItem[];
  /** The CLI session the next answer continues (AC-12). */
  session: string | null;
  answering: boolean;
  /** "Agent may see the image" (AC-17): off for every new chat. */
  imageAccess: boolean;
  /** The CLI and model the chat's messages went to; `null` while it has
   *  none (AC-25). */
  cli: string | null;
  model: string;
}

const PREF_CLI = 'assistantCli';
const PREF_MODELS = 'assistantModels';
const PREF_PROMPT = 'assistantPrompt';

/** How long closing the app waits for stopped answers to end (AC-22). */
const STOP_BUDGET_MS = 5_000;

class AssistantState {
  /** Chats by tab key. */
  chats = $state<Record<number, Chat>>({});
  /** The supported CLIs, `null` until the first check (AC-03). */
  clis = $state<CliStatus[] | null>(null);
  /** The last chosen CLI (AC-04). */
  cli = $state<string | null>(null);
  /** The last model per CLI; empty is the CLI's default (AC-04). */
  models = $state<Record<string, string>>({});
  /** The user's system prompt; `null` is the default (AC-18, AC-19). */
  prompt = $state<string | null>(null);
}

export const assistant = new AssistantState();

/** `true` while any chat's answer runs: the editor is inert then (AC-10). */
export function assistantBusy(): boolean {
  return Object.values(assistant.chats).some((c) => c.answering);
}

/** Shortcuts that keep working while an answer runs: zoom and view
 *  (AC-10). */
const VIEW_ACTIONS: ReadonlySet<ShortcutAction> = new Set<ShortcutAction>([
  'zoomIn',
  'zoomOut',
  'zoomFit',
  'zoomActual',
  'toggleGrid',
  'togglePixelGrid',
  'toggleRulers',
  'toggleUi',
  'shortcuts',
]);

/** `true` when an answer runs and `e` is not a zoom or view shortcut: the
 *  editors ignore it then (AC-10). */
export function keyBlockedByAssistant(e: KeyboardEvent): boolean {
  if (!assistantBusy()) return false;
  const action = actionFor(e);
  return action === null || !VIEW_ACTIONS.has(action);
}

/** The chat of the tab with `key`, created on first use. */
export function chatOf(key: number): Chat {
  const existing = assistant.chats[key];
  if (existing) return existing;
  assistant.chats[key] = {
    key,
    id: `chat-${key}-1`,
    n: 1,
    items: [],
    session: null,
    answering: false,
    imageAccess: false,
    cli: null,
    model: '',
  };
  bindChatTab(`chat-${key}-1`, key);
  return assistant.chats[key];
}

/** The installed supported CLIs. */
export function installedClis(): CliStatus[] {
  return (assistant.clis ?? []).filter((c) => c.installed);
}

/** The CLI a send in `chat` uses: the chat's own once it has messages,
 *  else the last chosen one if installed, else the first installed. */
export function chatCli(chat: Chat): string | null {
  if (chat.cli !== null) return chat.cli;
  const installed = installedClis();
  return installed.find((c) => c.id === assistant.cli)?.id ?? installed[0]?.id ?? null;
}

/** The model a send in `chat` uses; empty is the CLI's default. */
export function chatModel(chat: Chat): string {
  if (chat.cli !== null) return chat.model;
  const cli = chatCli(chat);
  return cli === null ? '' : (assistant.models[cli] ?? '');
}

/** The system prompt every chat uses (AC-18, AC-19). */
export function systemPrompt(): string {
  return assistant.prompt ?? defaultPrompt(locale());
}

/** Looks the CLIs up again (AC-03: whenever the panel or picker opens). */
export async function refreshClis(): Promise<void> {
  try {
    const clis = await invoke<CliStatus[] | null>('assistant_clis');
    assistant.clis = Array.isArray(clis) ? clis : [];
  } catch (err) {
    console.error('assistant_clis failed', err);
    assistant.clis = [];
  }
}

/** Remembers the CLI and model choice (AC-04). Changing either while
 *  `chat` has messages starts a new chat with it (AC-25); the caller asks
 *  first. */
export function choose(chat: Chat, cli: string, model: string): void {
  if (chat.answering) return;
  if (chat.items.length > 0) newChat(chat);
  assistant.cli = cli;
  assistant.models = { ...assistant.models, [cli]: model.trim() };
  void savePref(PREF_CLI, cli);
  void savePref(PREF_MODELS, $state.snapshot(assistant.models));
}

/** Saves an edited system prompt (AC-19); `null` resets to the default. */
export function setSystemPrompt(text: string | null): void {
  assistant.prompt = text;
  void savePref(PREF_PROMPT, text);
}

/** "Agent may see the image" for `chat` (AC-17). */
export function setImageAccess(chat: Chat, on: boolean): void {
  if (chat.answering) return;
  chat.imageAccess = on;
  invoke('mcp_chat_image_access', { chat: chat.id, on }).catch((err: unknown) =>
    console.error('mcp_chat_image_access failed', err),
  );
}

/** New chat (AC-13): the list empties, the chat's token ends, the next
 *  message starts a new session; the document is left alone. */
export function newChat(chat: Chat): void {
  if (chat.answering) return;
  endChat(chat.id);
  chat.n += 1;
  chat.id = `chat-${chat.key}-${chat.n}`;
  chat.items = [];
  chat.session = null;
  chat.imageAccess = false;
  chat.cli = null;
  chat.model = '';
  bindChatTab(chat.id, chat.key);
}

/** Stop (AC-11): the CLI ends; the `done` event finishes the answer. */
export function stopAnswer(chat: Chat): void {
  if (!chat.answering) return;
  invoke('assistant_stop', { chat: chat.id }).catch((err: unknown) =>
    console.error('assistant_stop failed', err),
  );
}

/** Sends `text` in the chat of `tab` (AC-05, AC-15). */
export async function send(tab: ShellTab, text: string): Promise<void> {
  const chat = chatOf(tab.key);
  const cli = chatCli(chat);
  if (text.trim() === '' || chat.answering || cli === null || assistantBusy()) return;
  const model = chatModel(chat);
  const prompt = withContext(editorContext(tab), text);
  chat.cli = cli;
  chat.model = model;
  chat.items = [...chat.items, { kind: 'user', text }];
  chat.answering = true;
  tab.doc.assistantBegin();
  try {
    await invoke('assistant_send', {
      request: {
        chat: chat.id,
        cli,
        model: model === '' ? null : model,
        prompt,
        systemPrompt: systemPrompt(),
        session: chat.session,
        imageAccess: chat.imageAccess,
      },
    });
  } catch (err) {
    // Nothing started (AC-28): the reason shows, no step is added.
    const reason = err instanceof Error ? err.message : String(err);
    chat.items = [...chat.items, { kind: 'error', text: reason }];
    tab.doc.assistantEnd();
    chat.answering = false;
  }
}

/** The block the agent receives in front of the user's text (AC-15). */
function editorContext(tab: ShellTab): EditorContext {
  const id = docIdOf(tab);
  const info = parseWire(tab.doc.agentCall(id, 'doc.info', '{}', null));
  const list = parseWire(tab.doc.agentCall(id, 'layer.list', '{}', null));
  const json = (info.ok && isRecord(info.json) ? info.json : {}) as Record<string, unknown>;
  const rows =
    list.ok && isRecord(list.json) && Array.isArray(list.json.layers) ? list.json.layers : [];
  const sel = json.selection;
  return {
    name: tab.doc.name(),
    mode: tab.mode,
    width: Number(json.width ?? 0),
    height: Number(json.height ?? 0),
    selection:
      isRecord(sel) && ['x', 'y', 'w', 'h'].every((k) => typeof sel[k] === 'number')
        ? { x: Number(sel.x), y: Number(sel.y), w: Number(sel.w), h: Number(sel.h) }
        : null,
    layers: rows.filter(isRecord).map((l) => ({
      name: String(l.name ?? ''),
      active: l.id !== undefined && l.id === json.active_layer,
    })),
    steps: tab.doc.undoKinds().map((k) => historyStepName(k, t)),
  };
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** One event of `chat`'s answer. */
async function onEvent(chatId: string, event: ChatEvent): Promise<void> {
  const chat = Object.values(assistant.chats).find((c) => c.id === chatId);
  if (!chat) return;
  if (event.kind === 'session') {
    chat.session = event.id;
    return;
  }
  chat.items = applyEvent(chat.items, event);
  if (event.kind !== 'done') return;
  // A call already running in the editor — a generation of the chat
  // included — completes and counts towards the answer (AC-11); then the
  // answer becomes one undo step (AC-08).
  await liveIdle(chat.id);
  tabByKey(chat.key)?.doc.assistantEnd();
  invoke('mcp_chat_answering', { chat: chat.id, running: false }).catch((err: unknown) =>
    console.error('mcp_chat_answering failed', err),
  );
  chat.answering = false;
}

function endChat(id: string): void {
  unbindChatTab(id);
  invoke('assistant_end', { chat: id }).catch((err: unknown) =>
    console.error('assistant_end failed', err),
  );
}

/** Closing the app (AC-22): running answers stop and finish first,
 *  bounded so a stuck CLI never keeps the app open. */
export async function stopAllAnswers(): Promise<void> {
  const running = Object.values(assistant.chats).filter((c) => c.answering);
  if (running.length === 0) return;
  for (const chat of running) stopAnswer(chat);
  const deadline = Date.now() + STOP_BUDGET_MS;
  while (assistantBusy() && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

async function savePref(key: string, value: unknown): Promise<void> {
  try {
    await setPref(key, value);
  } catch (err) {
    console.error(`setPref ${key} failed`, err);
  }
}

async function loadPrefs(): Promise<void> {
  try {
    const [cli, models, prompt] = await Promise.all([
      getPref(PREF_CLI),
      getPref(PREF_MODELS),
      getPref(PREF_PROMPT),
    ]);
    if (typeof cli === 'string') assistant.cli = cli;
    if (isRecord(models)) {
      assistant.models = Object.fromEntries(
        Object.entries(models).filter((e): e is [string, string] => typeof e[1] === 'string'),
      );
    }
    if (typeof prompt === 'string' && prompt.trim() !== '') assistant.prompt = prompt;
  } catch (err) {
    console.error('loading the Assistant prefs failed', err);
  }
}

/** Starts the chats (desktop app only): the CLI's events, the prefs, and
 *  a closed tab ends its chat (AC-14). Returns the function that stops. */
export function startAssistant(): () => void {
  let stopped = false;
  let unlisten: (() => void) | null = null;
  void loadPrefs();
  listen<{ chat: string; event: ChatEvent }>('assistant-event', (e) => {
    void onEvent(e.payload.chat, e.payload.event);
  })
    .then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    })
    .catch((err: unknown) => console.error('assistant-event listen failed', err));
  const stopTabs = $effect.root(() => {
    $effect(() => {
      const open = session.tabs.map((t) => t.key);
      untrack(() => {
        for (const chat of Object.values(assistant.chats)) {
          if (open.includes(chat.key)) continue;
          stopAnswer(chat);
          endChat(chat.id);
          delete assistant.chats[chat.key];
        }
      });
    });
  });
  return () => {
    stopped = true;
    unlisten?.();
    stopTabs();
  };
}
