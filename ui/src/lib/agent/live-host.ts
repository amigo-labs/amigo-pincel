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
import { activateTab, session } from '../shell/session.svelte';
import type { AgentFile } from '../shell/types';
import { runOn, targetTab } from './tab-calls';
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
  const tab = targetTab(args);
  if (typeof tab === 'string') return fail(tab);
  if (session.active !== tab) activateTab(tab);
  let file: AgentFile | null = null;
  if (READS.has(call.tool) && typeof args.path === 'string') {
    const picked = await pickOpenPath(call.id, args.path);
    if (picked === null) return fail(USER_CANCELLED);
    file = { path: args.path, bytes: await readPath(picked) };
  }
  const wire = runOn(tab, call.tool, args, file);
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

async function run(call: LiveCall, shell: DocShell): Promise<Wire> {
  const args =
    typeof call.args === 'object' && call.args !== null && !Array.isArray(call.args)
      ? (call.args as Record<string, unknown>)
      : {};
  await strokeDone();
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

/** Takes the endpoint's calls; returns the function that stops. Calls run
 *  one after another; a call that arrives while a dialog is open fails at
 *  once (AC-18), one during a stroke waits for pointerup (AC-17). */
export function startLiveHost(shell: DocShell): () => void {
  const stopStrokes = watchStrokes();
  let chain: Promise<void> = Promise.resolve();
  let stopped = false;
  let unlisten: (() => void) | null = null;
  listen<LiveCall>('mcp-call', (e) => {
    const call = e.payload;
    if (dialogOpen()) {
      void answer(call.id, fail('editor busy: a dialog is open'));
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
  return () => {
    stopped = true;
    stopStrokes();
    unlisten?.();
  };
}
