// The reply shape of a live agent call (`pincel_agent::live::LiveReply`),
// which the desktop app hands back to the MCP endpoint
// (docs/specs/mcp-live-desktop.md), and the files and dialogs around it.
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { withHostDialog } from './busy';

/** One file a writing tool produced, its bytes in base64. */
export interface WireFile {
  path: string;
  data: string;
}

/** A tool's reply as the endpoint takes it. */
export type Wire =
  | { ok: true; json: unknown; png?: string; files?: WireFile[] }
  | { ok: false; error: string; unknown?: boolean };

/** The reply of a call the user cancelled (AC-11, AC-15, AC-16). */
export const USER_CANCELLED = 'user cancelled';

export function ok(json: unknown): Wire {
  return { ok: true, json };
}

export function fail(error: string): Wire {
  return { ok: false, error };
}

/** Parses the editor's JSON reply. */
export function parseWire(text: string): Wire {
  try {
    const v: unknown = JSON.parse(text);
    if (typeof v === 'object' && v !== null && 'ok' in v) return v as Wire;
  } catch {
    // fall through
  }
  return fail('the editor returned no readable result');
}

/** The file name of `path` (either separator). */
export function baseName(path: string): string {
  return path.replace(/^.*[/\\]/, '');
}

/** Tells the endpoint that a dialog the call `id` opened waits for the
 *  user, so the wait does not count towards its timeout (AC-19). */
async function waiting(id: number, on: boolean): Promise<void> {
  try {
    await invoke('mcp_call_waiting', { id, waiting: on });
  } catch (err) {
    console.error('mcp_call_waiting failed', err);
  }
}

/** Runs `wait` (a dialog of call `id` that waits for the user) with the
 *  call's timeout paused. */
export async function userWait<T>(id: number, wait: () => Promise<T>): Promise<T> {
  await waiting(id, true);
  try {
    return await wait();
  } finally {
    await waiting(id, false);
  }
}

/** The native save dialog with `name` filled in; `null` on cancel. */
export function pickSavePath(id: number, name: string): Promise<string | null> {
  return userWait(id, () =>
    withHostDialog(async () => {
      const picked = await saveDialog({ defaultPath: name });
      return typeof picked === 'string' ? picked : null;
    }),
  );
}

/** The native open dialog starting at `path`; `null` on cancel. */
export function pickOpenPath(id: number, path: string): Promise<string | null> {
  return userWait(id, () =>
    withHostDialog(async () => {
      const picked = await openDialog({ multiple: false, defaultPath: path });
      return typeof picked === 'string' ? picked : null;
    }),
  );
}

export async function readPath(path: string): Promise<Uint8Array> {
  const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', { path });
  return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
}

export async function writePath(path: string, bytes: Uint8Array): Promise<void> {
  await invoke('write_file_bytes', { path, bytes: Array.from(bytes) });
}

/** Decodes base64 to bytes. */
export function fromBase64(data: string): Uint8Array {
  const bin = atob(data);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
  return out;
}
