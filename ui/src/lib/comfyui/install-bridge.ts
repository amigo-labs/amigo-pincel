// Renderer side of the managed ComfyUI install (src-tauri/src/comfyui/managed.rs,
// docs/specs/comfyui-install.md). The backend checks, downloads and installs;
// the webview shows the wizard and the job's progress. Desktop only.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ComfyStatus } from './bridge';

/** `Backend` in src-tauri/src/comfyui/hardware.rs. */
export type Backend = 'cuda' | 'mps';

/** `Unsupported` in hardware.rs (AC-01). */
export type Unsupported =
  | { kind: 'otherGpu'; name: string }
  | { kind: 'noNvidiaDriver' }
  | { kind: 'intelMac' }
  | { kind: 'armCpu' }
  | { kind: 'otherOs' };

/** `Hardware` in hardware.rs (AC-01, AC-02). */
export interface Hardware {
  os: 'windows' | 'linux' | 'macos' | 'other';
  /** VRAM (NVIDIA) or unified memory (Mac) in bytes. */
  device: { name: string; memory: number; backend: Backend } | null;
  unsupported: Unsupported | null;
  slow: boolean;
}

/** `Package` in src-tauri/src/comfyui/pins.rs (AC-04). */
export interface Package {
  id: string;
  name: string;
  kind: 'checkpoint' | 'lora';
  file: string;
  size: number;
  license: string;
  licenseUrl: string;
  minVram: number;
  minMemory: number;
  requires: string | null;
}

/** `Step` / `Progress` in src-tauri/src/comfyui/engine.rs (AC-06). */
export type Step = 'python' | 'comfyui' | 'packages' | 'model';
export interface Progress {
  step: Step;
  package: string | null;
  done: number | null;
  total: number | null;
  line: string | null;
}

/** `InstallError` in src-tauri/src/comfyui/download.rs (AC-07, AC-08). */
export type InstallError =
  | { kind: 'corrupted'; file: string }
  | { kind: 'gone'; file: string }
  | { kind: 'offline' }
  | { kind: 'diskFull' }
  | { kind: 'setupFailed'; log: string[] }
  | { kind: 'http'; file: string; status: number }
  | { kind: 'cancelled' }
  | { kind: 'updateNotStarted'; log: string[] }
  | { kind: 'generationRunning' }
  | { kind: 'busy' }
  | { kind: 'failed'; message: string };

/** `JobKind` / `JobEvent` in managed.rs. */
export type JobKind = 'install' | 'packages' | 'update';
export type JobEvent =
  | { kind: 'progress'; job: JobKind; progress: Progress }
  | { kind: 'done'; job: JobKind; status: ComfyStatus }
  | { kind: 'failed'; job: JobKind; error: InstallError };

/** `ManagedState` in managed.rs. */
export type ManagedState =
  | { kind: 'none' }
  | { kind: 'missing'; dir: string }
  | { kind: 'incomplete'; dir: string; packages: string[] }
  | { kind: 'complete'; dir: string; comfyui: string; models: string[]; updateAvailable: boolean };

/** `LocationCheck` in managed.rs (AC-03). */
export interface LocationCheck {
  target: string;
  free: number | null;
  writable: boolean;
  existing: 'none' | 'incomplete' | 'complete';
}

/** `InstallInfo` in managed.rs. */
export interface InstallInfo {
  hardware: Hardware;
  catalog: Package[];
  baseSize: number | null;
  reserve: number;
  accepted: string[];
  managed: ManagedState;
  job: { job: JobKind; progress: Progress | null } | null;
  generating: boolean;
}

/** `null` from a backend without the managed install. */
export function installInfo(): Promise<InstallInfo | null> {
  return invoke<InstallInfo | null>('comfyui_install_info');
}

export function checkLocation(path: string): Promise<LocationCheck> {
  return invoke<LocationCheck>('comfyui_check_location', { path });
}

export function acceptLicenses(ids: string[]): Promise<string[]> {
  return invoke<string[]>('comfyui_accept_licenses', { ids });
}

/** Starts the install job; rejects with an `InstallError`. */
export function startInstall(location: string, packages: string[]): Promise<void> {
  return invoke<void>('comfyui_install', { location, packages });
}

export function resumeInstall(): Promise<void> {
  return invoke<void>('comfyui_install_resume');
}

export function cancelInstall(): Promise<void> {
  return invoke<void>('comfyui_install_cancel');
}

export function managedState(): Promise<ManagedState> {
  return invoke<ManagedState>('comfyui_managed');
}

export function useInstallation(location: string): Promise<ComfyStatus> {
  return invoke<ComfyStatus>('comfyui_use_installation', { location });
}

export function changePackages(add: string[], remove: string[]): Promise<void> {
  return invoke<void>('comfyui_change_packages', { add, remove });
}

export function updateComfy(): Promise<void> {
  return invoke<void>('comfyui_update');
}

export function uninstallComfy(): Promise<void> {
  return invoke<void>('comfyui_uninstall');
}

/** Progress and end of the running job. */
export function onInstallEvent(handler: (event: JobEvent) => void): Promise<UnlistenFn> {
  return listen<JobEvent>('comfyui:install', (e) => handler(e.payload));
}

/** Narrows a rejection to an `InstallError`; anything else becomes `failed`. */
export function asInstallError(err: unknown): InstallError {
  if (typeof err === 'object' && err !== null && 'kind' in err && typeof err.kind === 'string') {
    return err as InstallError;
  }
  return { kind: 'failed', message: String(err) };
}
