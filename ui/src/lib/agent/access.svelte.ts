// View ▸ AI agents… (docs/specs/mcp-live-desktop.md AC-01 – AC-04, AC-12,
// AC-13, AC-20, AC-21): the setting "Allow AI agents to control Pincel",
// off by default and stored as one IndexedDB pref; the endpoint itself,
// its token and its port live in the Tauri process (src-tauri/src/mcp).
// Desktop app only: a browser page cannot host a server.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { MenuAction } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { isTauri } from '../platform';

const PREF = 'aiAgents';

/** The endpoint's state (`Status` in src-tauri/src/mcp/endpoint.rs). */
export interface AgentStatus {
  enabled: boolean;
  port: number | null;
  token: string | null;
  url: string | null;
  command: string | null;
  portChanged: boolean;
  externalSessions: number;
}

export const agentAccess = $state({
  /** The AI agents dialog is open. */
  dialogOpen: false,
  /** The endpoint's state as last reported. */
  status: null as AgentStatus | null,
  /** Open external sessions (the status-bar indicator). */
  sessions: 0,
  /** Why the endpoint could not start, shown in the dialog. */
  error: null as string | null,
  /** A switch is in flight. */
  busy: false,
});

function errText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function apply(status: AgentStatus): void {
  agentAccess.status = status;
  agentAccess.sessions = status.externalSessions;
}

function store(on: boolean): void {
  if (!isIdbAvailable()) return;
  setPref(PREF, on).catch((err: unknown) => console.error('setPref aiAgents failed', err));
}

async function run(command: string, args: Record<string, unknown> = {}): Promise<void> {
  agentAccess.busy = true;
  try {
    apply(await invoke<AgentStatus>(command, args));
    agentAccess.error = null;
  } catch (err) {
    agentAccess.error = errText(err);
  } finally {
    agentAccess.busy = false;
  }
}

/** App start (desktop): follow the session count, and start the endpoint
 *  with its stored token and port when the setting is on (AC-03). Nothing
 *  listens otherwise (AC-01). Returns the function that stops listening. */
export function startAgentAccess(): () => void {
  if (!isTauri()) return () => {};
  let stopped = false;
  let unlisten: (() => void) | null = null;
  listen<number>('mcp-sessions', (e) => {
    if (typeof e.payload === 'number') agentAccess.sessions = e.payload;
  })
    .then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    })
    .catch((err: unknown) => console.error('mcp-sessions listen failed', err));
  if (isIdbAvailable()) {
    getPref(PREF)
      .then((on) => {
        if (on === true && !stopped) void run('mcp_enable', { fresh: false });
      })
      .catch((err: unknown) => console.error('getPref aiAgents failed', err));
  }
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** Turn the setting on (a new token, AC-02) or off (AC-04). */
export async function setAgentAccess(on: boolean): Promise<void> {
  await run(on ? 'mcp_enable' : 'mcp_disable', on ? { fresh: true } : {});
  store(agentAccess.status?.enabled ?? false);
}

/** "Regenerate token" (AC-20). */
export function regenerateToken(): Promise<void> {
  return run('mcp_regenerate_token');
}

/** The indicator's disconnect button (AC-12). */
export function disconnectAgents(): Promise<void> {
  return run('mcp_disconnect');
}

/** Opens the dialog with the endpoint's current state. */
export function openAgentsDialog(): void {
  agentAccess.dialogOpen = true;
  void run('mcp_status');
}

/** View ▸ AI agents…, in the desktop app only (AC-13). */
export function agentsMenuItems(): MenuAction[] {
  return isTauri() ? [{ label: t('menu.aiAgents'), run: openAgentsDialog }] : [];
}
