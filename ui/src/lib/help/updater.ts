// Renderer side of the desktop updater (src-tauri/src/updater.rs).
//
// The native macOS menu drives the updater from Rust and reports through
// a native dialog; everywhere else the Help menu drives it through these
// commands and reports in the About dialog. Both write the same
// `updater.json`.

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
  /** Version already downloaded in this session, used from the next start. */
  ready: string | null;
  /** Why this copy cannot update itself, where the About dialog names it. */
  unsupported: 'package' | 'appimage' | null;
}

/** Answer of `updater_set_mode`: the choice that applies, and whether it
 *  was stored (false: it applies for this run only). */
export interface ModeSaved {
  mode: UpdateMode;
  saved: boolean;
}

export function updaterState(): Promise<UpdaterInfo> {
  return invoke<UpdaterInfo>('updater_state');
}

/** Check now. `null` means up to date; a version means it was downloaded. */
export function updaterCheck(): Promise<string | null> {
  return invoke<string | null>('updater_check');
}

/** Make `mode` the update choice for this run, and store it if possible. */
export function updaterSetMode(mode: UpdateMode): Promise<ModeSaved> {
  return invoke<ModeSaved>('updater_set_mode', { mode });
}
