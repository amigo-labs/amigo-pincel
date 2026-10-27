// Help menu state, shared by the shell and both editors (spec §9.2).
//
// The Help menu exists wherever the OS draws no native menu bar — the web
// build, and the desktop app on Windows and Linux (spec §11.3). Its entries
// are the ones the native macOS menu serves there: About Pincel, and the
// updater's manual check and auto-install toggle.
//
// The state lives here rather than in a mode's store because `App.svelte`
// renders the dialog: that covers the start screen, where no editor and so
// no menu bar is mounted at all.

import { isTauri } from '../platform';
import { updaterCheck, updaterSetAuto, updaterState, type UpdaterInfo } from './updater';

/** Outcome of a manual update check, shown in the About dialog. */
export type UpdateResult =
  | { kind: 'upToDate' }
  | { kind: 'ready'; version: string }
  | { kind: 'failed'; error: string };

export const help = $state<{
  about: boolean;
  /** `null` until loaded, and forever outside the desktop app. */
  updater: UpdaterInfo | null;
  checking: boolean;
  result: UpdateResult | null;
}>({ about: false, updater: null, checking: false, result: null });

/** Open the About dialog (Help ▸ About Pincel, or the start screen's button). */
export function openAbout(): void {
  help.about = true;
}

export function closeAbout(): void {
  help.about = false;
}

/** Load what the Help menu draws its update entries from. No-op in a browser. */
export async function loadUpdaterState(): Promise<void> {
  if (!isTauri()) return;
  try {
    help.updater = await updaterState();
  } catch (err) {
    console.error('updater_state failed', err);
  }
}

/** Help ▸ Check for Updates…: the About dialog reports the outcome. */
export function runUpdateCheck(): void {
  if (help.checking) return;
  help.checking = true;
  help.result = null;
  help.about = true;
  updaterCheck()
    .then((version) => {
      help.result = version === null ? { kind: 'upToDate' } : { kind: 'ready', version };
      if (version !== null && help.updater !== null) help.updater.ready = version;
    })
    .catch((err: unknown) => {
      help.result = { kind: 'failed', error: String(err) };
    })
    .finally(() => {
      help.checking = false;
    });
}

/** Help ▸ Install Updates Automatically. The checkbox follows the stored
 *  value, not the click, so a failed write leaves it where it was. */
export function setAutoUpdate(enabled: boolean): void {
  updaterSetAuto(enabled)
    .then((stored) => {
      if (help.updater !== null) help.updater.autoUpdate = stored;
    })
    .catch((err: unknown) => {
      console.error('updater_set_auto failed', err);
    });
}
