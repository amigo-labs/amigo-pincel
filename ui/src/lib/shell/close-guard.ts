// Desktop close guard (docs/specs/unsaved-changes-prompt.md AC-09, AC-10):
// closing the window (title-bar ×, Alt+F4, the taskbar) and macOS Quit
// (a custom `menu:quit` item, since the predefined one exits without a
// close request) ask the unsaved-changes prompt first — about every
// unsaved tab (docs/specs/document-tabs.md). A close during a running
// File ▸ Save waits for it to end.
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { discardDocuments, guardUnsaved, hasUnsaved } from './unsaved.svelte';

/** How long closing waits for the autosave store to drop a discarded
 *  document's snapshot before it closes anyway. */
const DISCARD_BUDGET_MS = 2_000;

/** Handle a close request: wait for a running save, ask first while any
 *  tab has unsaved changes (document-tabs AC-10, AC-11, AC-17), then close
 *  the app. Ignored while the prompt is open or an earlier request waits
 *  for a save (unsaved-changes-prompt AC-08). */
export async function requestClose(): Promise<void> {
  await guardUnsaved(() => void closeNow());
}

/** Close the window (the app ends with its last window). After Don't
 *  Save the documents' autosave snapshots go first (AC-05) — bounded, so
 *  a storage error never keeps the app open. */
async function closeNow(): Promise<void> {
  if (hasUnsaved()) await dropSnapshot();
  try {
    await getCurrentWindow().destroy();
  } catch (err) {
    console.error('closing the window failed', err);
  }
}

async function dropSnapshot(): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const budget = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, DISCARD_BUDGET_MS);
  });
  try {
    await Promise.race([discardDocuments(), budget]);
  } catch (err) {
    console.error('dropping the autosave snapshot failed', err);
  } finally {
    clearTimeout(timer);
  }
}

/** Route the window's close requests through `requestClose` (Tauri only). */
export function wireCloseGuard(): Promise<UnlistenFn> {
  return getCurrentWindow().onCloseRequested((event) => {
    event.preventDefault();
    void requestClose();
  });
}
