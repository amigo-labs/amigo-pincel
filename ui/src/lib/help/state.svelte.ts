// Help menu state, shared by the shell and both editors (spec §9.2).
//
// The Help menu exists wherever the OS draws no native menu bar — the web
// build, and the desktop app on Windows and Linux (spec §11.3). Its entries
// are the ones the native macOS menu serves there: About Pincel, and the
// updater's manual check (the update choice lives in the About dialog).
//
// The state lives here rather than in a mode's store because `App.svelte`
// renders the dialog: that covers the start screen, where no editor and so
// no menu bar is mounted at all.

import { comfyDialogOpen } from '../comfyui/state.svelte';
import { isTauri } from '../platform';
import type { PaletteSource } from '../shell/command-palette';
import type { ShortcutSource } from '../shell/shortcut-list';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  UPDATER_CHANGED,
  updaterCheck,
  updaterCheckOnOpen,
  updaterMarkAnnounced,
  updaterRelease,
  updaterSetMode,
  updaterState,
  type CheckOutcome,
  type UpdateMode,
  type UpdaterInfo,
} from './updater';

/** Outcome of an update check, shown in the About dialog. */
export type UpdateResult =
  | { kind: 'upToDate' }
  | { kind: 'available'; version: string }
  | { kind: 'ready'; version: string }
  | { kind: 'failed'; error: string };

export const help = $state<{
  about: boolean;
  /** Edit ▸ Settings… (docs/specs/settings-dialog.md). */
  settings: boolean;
  /** What Help ▸ Keyboard Shortcuts lists; `null` while it is closed. */
  shortcuts: ShortcutSource | null;
  /** What the command palette (Ctrl+K) searches; `null` while it is closed. */
  palette: PaletteSource | null;
  /** `null` until loaded, and forever outside the desktop app. */
  updater: UpdaterInfo | null;
  checking: boolean;
  result: UpdateResult | null;
  /** The first-start question waits to be asked (docs/specs/desktop-app.md
   *  AC-22); the shell shows it once the file-association hint is closed. */
  question: boolean;
  /** The last update choice could not be stored (AC-28). */
  notSaved: boolean;
  /** Restart Now failed. */
  restartError: string | null;
}>({
  about: false,
  settings: false,
  shortcuts: null,
  palette: null,
  updater: null,
  checking: false,
  result: null,
  question: false,
  notSaved: false,
  restartError: null,
});

/** Open the About dialog (Help ▸ About Pincel, the start screen's button,
 *  the macOS menu). In a copy that can update itself this checks at once
 *  unless the choice is "never check" (AC-31); a version downloaded in
 *  this run is reported again without a check. */
export function openAbout(): void {
  help.about = true;
  const updater = help.updater;
  if (updater === null || !updater.supported || help.checking || help.question) return;
  if (updater.ready !== null) {
    help.result = { kind: 'ready', version: updater.ready };
    return;
  }
  if (updater.mode === 'off') return;
  runCheck(updaterCheckOnOpen);
}

export function closeAbout(): void {
  help.about = false;
  // A running check still reports when the dialog opens again.
  if (!help.checking) help.result = null;
  help.restartError = null;
}

/** Open the Settings dialog (Edit ▸ Settings…, Ctrl+, / ⌘,, or the start
 *  screen's button; docs/specs/settings-dialog.md). */
export function openSettings(): void {
  help.settings = true;
}

export function closeSettings(): void {
  help.settings = false;
}

/**
 * Open Help ▸ Keyboard Shortcuts with the mounted editor's actions and tool
 * groups (docs/specs/shortcut-overview.md). Only editors call this: the
 * start screen has no mode and so no keymap to list.
 */
export function openShortcuts(source: ShortcutSource): void {
  help.shortcuts = source;
}

export function closeShortcuts(): void {
  help.shortcuts = null;
}

/**
 * Open the command palette over the mounted editor's menus and tools
 * (docs/specs/power-user-shortcuts-palette.md). Only editors call this:
 * the start screen has no menu bar to search.
 */
export function openPalette(source: PaletteSource): void {
  help.palette = source;
}

export function closePalette(): void {
  help.palette = null;
}

/** True while a shell dialog (About, Settings, Keyboard Shortcuts, command
 *  palette, Image generation) owns the keyboard. */
export function helpDialogOpen(): boolean {
  return (
    help.about ||
    help.settings ||
    help.shortcuts !== null ||
    help.palette !== null ||
    help.question ||
    comfyDialogOpen()
  );
}

/** Load what the Help menu, the About dialog and the notice draw from.
 *  No-op in a browser. */
export async function loadUpdaterState(): Promise<void> {
  if (!isTauri()) return;
  try {
    help.updater = await updaterState();
  } catch (err) {
    console.error('updater_state failed', err);
  }
}

/**
 * Start the updater's side of the shell: follow the desktop shell's
 * `updater-changed` events (armed before the first read, so none is
 * lost), then either ask the first-start question or release the
 * background check (AC-22, AC-24, AC-25). Returns the unlisten function.
 */
export async function initUpdater(): Promise<UnlistenFn | null> {
  if (!isTauri()) return null;
  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen(UPDATER_CHANGED, () => void loadUpdaterState());
  } catch (err) {
    console.error('updater listen failed', err);
  }
  await loadUpdaterState();
  const updater = help.updater;
  if (updater === null || !updater.supported) return unlisten;
  if (updater.chosen) release();
  else help.question = true;
  return unlisten;
}

function release(): void {
  updaterRelease().catch((err: unknown) => {
    console.error('updater_release failed', err);
  });
}

/** Confirm on the first-start question: store `mode` (AC-23). */
export function confirmQuestion(mode: UpdateMode): void {
  help.question = false;
  void chooseMode(mode).finally(release);
}

/** Escape or the close button on the first-start question: nothing is
 *  stored, "install automatically" applies for this run (AC-23). */
export function dismissQuestion(): void {
  help.question = false;
  release();
}

/** Make `mode` the update choice; it applies at once (AC-28). */
export async function chooseMode(mode: UpdateMode): Promise<void> {
  if (help.updater !== null) help.updater.mode = mode;
  try {
    const answer = await updaterSetMode(mode);
    help.notSaved = !answer.saved;
    if (help.updater !== null) {
      help.updater.mode = answer.mode;
      help.updater.chosen = true;
    }
  } catch (err) {
    help.notSaved = true;
    console.error('updater_set_mode failed', err);
  }
}

function report(outcome: CheckOutcome | null): void {
  if (outcome === null) return;
  if (outcome.state === 'upToDate') {
    help.result = { kind: 'upToDate' };
    return;
  }
  help.result = { kind: outcome.state, version: outcome.version };
  if (outcome.state === 'ready' && help.updater !== null) {
    help.updater.ready = outcome.version;
    help.updater.announce = null;
  }
}

function runCheck(check: () => Promise<CheckOutcome | null>): void {
  help.checking = true;
  help.result = null;
  help.restartError = null;
  check()
    .then(report)
    .catch((err: unknown) => {
      help.result = { kind: 'failed', error: String(err) };
    })
    .finally(() => {
      help.checking = false;
      void loadUpdaterState();
    });
}

/** Help ▸ Check for Updates…, the About dialog's button and Install (the
 *  notice, the About dialog): check and download whatever the update
 *  choice; the About dialog opens and reports (AC-16, AC-26). */
export function runUpdateCheck(): void {
  help.about = true;
  if (help.checking) return;
  runCheck(updaterCheck);
}

/** Later on the notice, or closing it (AC-26): `version` is not
 *  announced again. */
export function laterUpdate(version: string): void {
  if (help.updater !== null) help.updater.announce = null;
  updaterMarkAnnounced(version).catch((err: unknown) => {
    console.error('updater_mark_announced failed', err);
  });
}
