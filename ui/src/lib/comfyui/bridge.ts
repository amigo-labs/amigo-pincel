// Renderer side of the ComfyUI connection (src-tauri/src/comfyui/,
// docs/specs/comfyui-connect.md). The backend finds, starts and probes the
// server; the webview only shows what it reports. Desktop only.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** `ServerChoice` in src-tauri/src/comfyui/settings.rs. */
export type ServerChoice = 'automatic' | 'existing' | 'custom' | 'managed';

/** `Settings` in src-tauri/src/comfyui/settings.rs (camelCase). */
export interface ComfySettings {
  server: ServerChoice;
  existingDir: string | null;
  customUrl: string;
  /** Set once comfyui-install has installed one. */
  managedDir: string | null;
}

/** `FolderProblem` in src-tauri/src/comfyui/install.rs. */
export type FolderProblem = 'notFound' | 'noComfyUi' | 'noPython' | 'desktop';

/** `ServerInfo` in src-tauri/src/comfyui/api.rs. */
export interface ServerInfo {
  url: string;
  version: string;
  gpu: { name: string; vramTotal: number } | null;
  checkpoints: string[];
  loras: string[];
  missingNodes: string[];
  /** Pincel started this server and ends it on quit. */
  started: boolean;
}

/** `Status` in src-tauri/src/comfyui/manager.rs. */
export type ComfyStatus =
  | { kind: 'idle' }
  | { kind: 'notFound' }
  | { kind: 'notConfigured' }
  | { kind: 'starting' }
  | { kind: 'connected'; server: ServerInfo }
  | { kind: 'stopped'; log: string[] }
  | { kind: 'installationNotFound'; path: string }
  | { kind: 'installationInvalid'; path: string; problem: FolderProblem }
  | { kind: 'invalidUrl' }
  | { kind: 'unreachable'; url: string }
  | { kind: 'notComfy'; url: string }
  | { kind: 'refused'; status: number }
  | { kind: 'unsupported' }
  | { kind: 'failed'; message: string };

/** `FolderCheck` in src-tauri/src/comfyui/manager.rs. */
export type FolderCheck =
  | { kind: 'accepted'; settings: ComfySettings }
  | { kind: 'refused'; problem: FolderProblem };

export function comfySettings(): Promise<ComfySettings> {
  return invoke<ComfySettings>('comfyui_settings');
}

export function comfySetServer(server: ServerChoice): Promise<ComfySettings> {
  return invoke<ComfySettings>('comfyui_set_server', { server });
}

export function comfyChooseFolder(path: string): Promise<FolderCheck> {
  return invoke<FolderCheck>('comfyui_choose_folder', { path });
}

/** Stores `url` as the custom server and tests it. */
export function comfyTestUrl(url: string): Promise<ComfyStatus> {
  return invoke<ComfyStatus>('comfyui_test_url', { url });
}

/** A server is needed: the backend resolves or starts the chosen one. */
export function comfyConnect(): Promise<ComfyStatus> {
  return invoke<ComfyStatus>('comfyui_connect');
}

/** Status changes the backend reports on its own (a start, a process end). */
export function onComfyStatus(handler: (status: ComfyStatus) => void): Promise<UnlistenFn> {
  return listen<ComfyStatus>('comfyui:status', (e) => handler(e.payload));
}
