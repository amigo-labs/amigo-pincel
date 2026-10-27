// Renderer side of the desktop updater (src-tauri/src/updater.rs).
//
// The native macOS menu drives the updater from Rust and reports through
// a native dialog; everywhere else the Help menu drives it through these
// three commands and reports in the About dialog. Both write the same
// `updater.json`.

import { invoke } from '@tauri-apps/api/core';

/** Shape of `UpdaterInfo` in src-tauri/src/updater.rs (camelCase). */
export interface UpdaterInfo {
  /** False in debug builds and for copies a package manager owns: the
   *  Help menu then offers no update entries at all. */
  supported: boolean;
  autoUpdate: boolean;
  /** Version already downloaded in this session, used from the next start. */
  ready: string | null;
}

export function updaterState(): Promise<UpdaterInfo> {
  return invoke<UpdaterInfo>('updater_state');
}

/** Check now. `null` means up to date; a version means it was downloaded. */
export function updaterCheck(): Promise<string | null> {
  return invoke<string | null>('updater_check');
}

/** Set the auto-install flag; answers with what was actually stored. */
export function updaterSetAuto(enabled: boolean): Promise<boolean> {
  return invoke<boolean>('updater_set_auto', { enabled });
}
