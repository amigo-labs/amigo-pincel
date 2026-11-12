// Renderer side of the desktop updater (src-tauri/src/updater.rs).
//
// Every entry point reports in the in-app About dialog — the Help menu,
// the start screen and, on macOS, the native menu, whose About and Check
// for Updates… arrive as `menu` events (docs/specs/desktop-app.md AC-17).

import { invoke } from '@tauri-apps/api/core';

/** The update choice (`Mode` in src-tauri/src/updater.rs). */
export type UpdateMode = 'auto' | 'notify' | 'off';

/** Shape of `UpdaterInfo` in src-tauri/src/updater.rs (camelCase). */
export interface UpdaterInfo {
  /** False in debug builds and for copies a package manager owns: the
   *  Help menu then offers no update entries at all. */
  supported: boolean;
  /** The update choice that applies. */
  mode: UpdateMode;
  /** False until a choice is stored or made in this run: the first-start
   *  question asks (docs/specs/desktop-app.md AC-22). */
  chosen: boolean;
  /** Version already downloaded in this session. */
  ready: string | null;
  /** A version a background check found under "notify", for the notice. */
  announce: string | null;
  /** A check talks to the server right now. */
  checking: boolean;
  /** Why this copy cannot update itself, where the About dialog names it. */
  unsupported: 'package' | 'appimage' | null;
}

/** What a check found (`Outcome` in src-tauri/src/updater.rs). */
export type CheckOutcome =
  | { state: 'upToDate' }
  | { state: 'available'; version: string }
  | { state: 'ready'; version: string };

/** Answer of `updater_set_mode`: the choice that applies, and whether it
 *  was stored (false: it applies for this run only). */
export interface ModeSaved {
  mode: UpdateMode;
  saved: boolean;
}

/** Event the desktop shell sends when the updater's state changed. */
export const UPDATER_CHANGED = 'updater-changed';

export function updaterState(): Promise<UpdaterInfo> {
  return invoke<UpdaterInfo>('updater_state');
}

/** Check now and download a newer version, whatever the update choice
 *  (Check for Updates…, Install). */
export function updaterCheck(): Promise<CheckOutcome> {
  return invoke<CheckOutcome>('updater_check');
}

/** The About dialog's check on opening; `null` under "never check". */
export function updaterCheckOnOpen(): Promise<CheckOutcome | null> {
  return invoke<CheckOutcome | null>('updater_check_on_open');
}

/** Make `mode` the update choice for this run, and store it if possible. */
export function updaterSetMode(mode: UpdateMode): Promise<ModeSaved> {
  return invoke<ModeSaved>('updater_set_mode', { mode });
}

/** Later: `version` is not announced again. */
export function updaterMarkAnnounced(version: string): Promise<void> {
  return invoke<void>('updater_mark_announced', { version });
}

/** Restart Now: start Pincel again after it closes. Fails when nothing
 *  is downloaded. */
export function updaterArmRestart(): Promise<void> {
  return invoke<void>('updater_arm_restart');
}

/** Let the background check run (the first-start question is closed). */
export function updaterRelease(): Promise<void> {
  return invoke<void>('updater_release');
}
