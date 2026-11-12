// Live agent calls in the desktop app (docs/specs/mcp-live-desktop.md): the
// MCP endpoint in the Tauri process forwards each tool call as an
// `mcp-call` event; this module runs it on the document open in a tab —
// through the mounted editor's `agentCall` (one undo step, the editor
// refreshed as after any command) or, for opening, creating, closing and
// saving documents, through the shell's own flows — and answers through
// `mcp_call_result`.
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { dialogOpen, strokeDone, watchStrokes } from './busy';
import { docTool, isDocTool, type DocShell } from './doc-tools';
import { activateTab, session, tabByKey } from '../shell/session.svelte';
import { chatTabKey } from '../assistant/chat-tabs';
import type { AgentFile } from '../shell/types';
import { docIdOf, runOn, targetTab } from './tab-calls';
import { genTool, isGenTool } from './gen-tools';
import { callFonts, fontList } from './fonts';
import {
  USER_CANCELLED,
  baseName,
  fail,
  fromBase64,
  pickOpenPath,
  pickSavePath,
  readPath,
  writePath,
  type Wire,
} from './wire';

/** A forwarded call (`ForwardedCall` in src-tauri/src/mcp/endpoint.rs). */
export interface LiveCall {
  id: number;
  tool: string;
  args: unknown;
  /** The Assistant chat that made the call; `null` for an external client. */
  chat: string | null;
}

/** Tools that read a file the user picks in the open dialog (AC-16). */
const READS = new Set(['layer.import', 'palette.import']);

/** A document tool on the targeted tab, which becomes active so the user
 *  sees the change (AC-07). */
async function documentTool(call: LiveCall, args: Record<string, unknown>): Promise<Wire> {
  // The app's font list needs no document, like doc.list
  // (docs/specs/system-fonts-mcp.md AC-11).
  if (call.tool === 'font.list') return fontList(args);
  const tab = targetTab(args);
  if (typeof tab === 'string') return fail(tab);
  if (session.active !== tab) activateTab(tab);
  let file: AgentFile | null = null;
  if (READS.has(call.tool) && typeof args.path === 'string') {
    const picked = await pickOpenPath(call.id, args.path);
    if (picked === null) return fail(USER_CANCELLED);
    file = { path: args.path, bytes: await readPath(picked) };
  }
  const fonts = await callFonts(call.tool, args);
  const wire = runOn(tab, call.tool, args, file, fonts);
  if (!wire.ok || !wire.files || wire.files.length === 0) return wire;
  // Writing tools: every file goes where the user confirms, or none does.
  const files = wire.files;
  const picked: string[] = [];
  for (const f of files) {
    const path = await pickSavePath(call.id, baseName(f.path));
    if (path === null) return fail(USER_CANCELLED);
    picked.push(path);
  }
  for (const [i, f] of files.entries()) await writePath(picked[i] ?? f.path, fromBase64(f.data));
  const json = typeof wire.json === 'object' && wire.json !== null ? wire.json : {};
  return { ok: true, json: { ...json, saved_to: picked }, ...(wire.png ? { png: wire.png } : {}) };
}

/** Tools an Assistant chat is not offered (ai-chat-panel AC-26); the
 *  endpoint refuses them too (`CHAT_HIDDEN` in src-tauri/src/mcp). */
const CHAT_HIDDEN = new Set([
  'doc.open',
  'doc.new',
  'doc.close',
  'doc.list',
  'doc.undo',
  'doc.redo',
]);

/** A chat's arguments bound to the chat's own tab, whatever `doc` says,
 *  or why the call is refused (ai-chat-panel AC-26). */
function chatArgs(
  call: LiveCall,
  chat: string,
  args: Record<string, unknown>,
): Record<string, unknown> | string {
  const key = chatTabKey(chat);
  const tab = key === null ? null : tabByKey(key);
  if (tab === null || CHAT_HIDDEN.has(call.tool)) {
    return `\`${call.tool}\` is not available in the chat`;
  }
  const id = docIdOf(tab);
  if (args.doc !== undefined && args.doc !== id) {
    return `document \`${String(args.doc)}\` is not available in the chat`;
  }
  return { ...args, doc: id };
}

/** A chat's call: on the chat's own tab (ai-chat-panel AC-26). */
function chatCall(
  call: LiveCall,
  chat: string,
  args: Record<string, unknown>,
  shell: DocShell,
): Promise<Wire> | Wire {
  const bound = chatArgs(call, chat, args);
  if (typeof bound === 'string') return fail(bound);
  if (isDocTool(call.tool)) return docTool(call, bound, shell);
  return documentTool(call, bound);
}

function argsOf(call: LiveCall): Record<string, unknown> {
  return typeof call.args === 'object' && call.args !== null && !Array.isArray(call.args)
    ? (call.args as Record<string, unknown>)
    : {};
}

async function run(call: LiveCall, shell: DocShell): Promise<Wire> {
  const args = argsOf(call);
  await strokeDone();
  if (call.chat !== null) return chatCall(call, call.chat, args, shell);
  if (isDocTool(call.tool)) return docTool(call, args, shell);
  return documentTool(call, args);
}

async function answer(id: number, reply: Wire): Promise<void> {
  try {
    await invoke('mcp_call_result', { id, reply });
  } catch (err) {
    console.error('mcp_call_result failed', err);
  }
}

/** The calls taken so far, one after another. */
let chain: Promise<void> = Promise.resolve();

/** A running `gen.*` call: it runs beside the chain (comfyui-generate
 *  AC-16). */
interface Generation {
  readonly abort: AbortController;
  /** The Assistant chat that made the call; `null` for an external client. */
  readonly chat: string | null;
  /** Settles once the call has answered. */
  readonly done: Promise<void>;
}

/** Running `gen.*` calls by call id. */
const generating = new Map<number, Generation>();

/** Resolves once every call taken so far — and every call taken while
 *  waiting — has answered, `chat`'s generations included
 *  (ai-chat-panel AC-08, AC-11: a call already running counts towards the
 *  answer). Another client's generation does not hold the answer up. */
export async function liveIdle(chat: string): Promise<void> {
  for (;;) {
    const current = chain;
    const own = [...generating.values()].filter((g) => g.chat === chat).map((g) => g.done);
    await Promise.all([current, ...own]);
    if (current === chain && own.length === 0) return;
  }
}

/** Stop (ai-chat-panel AC-11): `chat`'s running generations are
 *  cancelled on the server; nothing is added. */
export function cancelChatGenerations(chat: string): void {
  for (const g of generating.values()) if (g.chat === chat) g.abort.abort();
}

/** Takes the endpoint's calls; returns the function that stops. Calls run
 *  one after another; a call that arrives while a dialog is open fails at
 *  once (AC-18), one during a stroke waits for pointerup (AC-17). */
export function startLiveHost(shell: DocShell): () => void {
  const stopStrokes = watchStrokes();
  let stopped = false;
  let unlisten: (() => void) | null = null;
  let unlistenExpired: (() => void) | null = null;
  listen<LiveCall>('mcp-call', (e) => {
    const call = e.payload;
    if (dialogOpen()) {
      void answer(call.id, fail('editor busy: a dialog is open'));
      return;
    }
    if (isGenTool(call.tool)) {
      // A generation takes minutes: it runs beside the queue of calls
      // (comfyui-generate AC-16); a chat's one on the chat's tab (AC-26).
      const args = call.chat === null ? argsOf(call) : chatArgs(call, call.chat, argsOf(call));
      if (typeof args === 'string') {
        void answer(call.id, fail(args));
        return;
      }
      const abort = new AbortController();
      const done = genTool(call.tool, args, abort.signal)
        .catch((err: unknown) => fail(err instanceof Error ? err.message : String(err)))
        .then(async (reply) => {
          generating.delete(call.id);
          await answer(call.id, reply);
        });
      generating.set(call.id, { abort, chat: call.chat, done });
      return;
    }
    chain = chain.then(async () => {
      let reply: Wire;
      try {
        reply = await run(call, shell);
      } catch (err) {
        reply = fail(err instanceof Error ? err.message : String(err));
      }
      await answer(call.id, reply);
    });
  })
    .then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    })
    .catch((err: unknown) => console.error('mcp-call listen failed', err));
  // A call that timed out: its generation is cancelled (comfyui-generate AC-20).
  listen<number>('mcp-call-expired', (e) => generating.get(e.payload)?.abort.abort())
    .then((fn) => {
      if (stopped) fn();
      else unlistenExpired = fn;
    })
    .catch((err: unknown) => console.error('mcp-call-expired listen failed', err));
  return () => {
    stopped = true;
    stopStrokes();
    unlisten?.();
    unlistenExpired?.();
    for (const g of generating.values()) g.abort.abort();
  };
}
