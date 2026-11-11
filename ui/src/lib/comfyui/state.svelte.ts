// View ▸ Image generation… (docs/specs/comfyui-connect.md): the dialog's
// state, shared by both editors' View menus. `App.svelte` renders the
// dialog. Desktop only: in a browser the menu entry does not exist (AC-01).

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { MenuItem } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { isTauri } from '../platform';
import {
  comfyChooseFolder,
  comfyConnect,
  comfySetServer,
  comfySettings,
  comfyTestUrl,
  onComfyStatus,
  type ComfySettings,
  type ComfyStatus,
  type FolderProblem,
  type ServerChoice,
} from './bridge';

export const comfy = $state<{
  open: boolean;
  settings: ComfySettings | null;
  status: ComfyStatus;
  /** A connect or test call is running. */
  busy: boolean;
  /** Why the last picked folder was refused (AC-05); `null` otherwise. */
  folderProblem: FolderProblem | null;
  /** The custom URL as typed. */
  url: string;
  /** A backend call failed outright. */
  error: string | null;
}>({
  open: false,
  settings: null,
  status: { kind: 'idle' },
  busy: false,
  folderProblem: null,
  url: '',
  error: null,
});

/** Counts connect / test calls, so only the latest one's answer is shown. */
let request = 0;
let unlisten: Promise<UnlistenFn> | null = null;

function fail(err: unknown): void {
  comfy.error = String(err);
}

/** Asks the backend for a server and shows the answer. */
async function run(call: () => Promise<ComfyStatus>): Promise<void> {
  const id = ++request;
  comfy.busy = true;
  comfy.error = null;
  try {
    const status = await call();
    if (id === request) comfy.status = status;
  } catch (err) {
    if (id === request) fail(err);
  } finally {
    if (id === request) comfy.busy = false;
  }
}

/** View ▸ Image generation…: opening it needs a server (AC-03). */
export function openImageGeneration(): void {
  comfy.open = true;
  comfy.folderProblem = null;
  comfy.error = null;
  unlisten ??= onComfyStatus((status) => {
    comfy.status = status;
  });
  void (async () => {
    try {
      comfy.settings = await comfySettings();
      comfy.url = comfy.settings.customUrl;
    } catch (err) {
      fail(err);
      return;
    }
    await run(comfyConnect);
  })();
}

export function closeImageGeneration(): void {
  comfy.open = false;
}

/** The user picks a server choice. */
export async function chooseServer(server: ServerChoice): Promise<void> {
  comfy.folderProblem = null;
  try {
    comfy.settings = await comfySetServer(server);
  } catch (err) {
    fail(err);
    return;
  }
  // A custom URL waits for Test connection; the others are tried now.
  if (server === 'custom') {
    comfy.status = { kind: 'idle' };
    return;
  }
  await run(comfyConnect);
}

/** "Choose folder…": the existing installation (AC-05, AC-13). */
export async function pickFolder(): Promise<void> {
  let picked: string | string[] | null;
  try {
    picked = await openDialog({ directory: true, multiple: false });
  } catch (err) {
    fail(err);
    return;
  }
  if (typeof picked !== 'string') return;
  try {
    const check = await comfyChooseFolder(picked);
    if (check.kind === 'refused') {
      comfy.folderProblem = check.problem;
      return;
    }
    comfy.folderProblem = null;
    comfy.settings = check.settings;
  } catch (err) {
    fail(err);
    return;
  }
  await run(comfyConnect);
}

/** "Test connection" for the custom URL (AC-08). */
export async function testConnection(): Promise<void> {
  const url = comfy.url;
  await run(() => comfyTestUrl(url));
  if (comfy.settings !== null) comfy.settings = { ...comfy.settings, server: 'custom', customUrl: url };
}

/** True when `text` is an http(s) URL whose host is this machine; `null`
 *  when it is no such URL at all (the backend says why on Test connection). */
export function isLoopbackUrl(text: string): boolean | null {
  const match = /^https?:\/\/(\[[^\]/]+\]|[^/:?#\s@]+)(?::\d+)?(?:[/?#]|$)/i.exec(text.trim());
  if (match === null) return null;
  const host = (match[1] ?? '').toLowerCase();
  return host === 'localhost' || host === '[::1]' || /^127(\.\d{1,3}){3}$/.test(host);
}

/** True while the dialog owns the keyboard. */
export function comfyDialogOpen(): boolean {
  return comfy.open;
}

/** View ▸ Image generation… for both editors; nothing in a browser (AC-01). */
export function imageGenerationItems(): MenuItem[] {
  if (!isTauri()) return [];
  return [{ kind: 'separator' }, { label: t('menu.imageGeneration'), run: openImageGeneration }];
}
