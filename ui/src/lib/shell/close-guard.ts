// Desktop close guard (docs/specs/unsaved-changes-prompt.md AC-09, AC-10):
// closing the window (title-bar ×, Alt+F4, the taskbar) and macOS Quit
// (a custom `menu:quit` item, since the predefined one exits without a
// close request) ask the unsaved-changes prompt first. A close during a
// running File ▸ Save waits for it to end.
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  discardDocument,
  guardUnsaved,
  hasUnsaved,
  promptBusy,
  runningSave,
} from './unsaved.svelte';

/** How long closing waits for the autosave store to drop a discarded
 *  document's snapshot before it closes anyway. */
const DISCARD_BUDGET_MS = 2_000;

/** A close request is waiting for a running save. */
let waiting = false;

/** Handle a close request: ask first while the document has unsaved
 *  changes, then close the app. Ignored while the prompt is open or an
 *  earlier request waits for a save (AC-08). */
export async function requestClose(): Promise<void> {
  if (waiting || promptBusy()) return;
  const running = runningSave();
  if (running !== null) {
    waiting = true;
    try {
      await running;
    } catch (err) {
      console.error('save before close failed', err);
    } finally {
      waiting = false;
    }
  }
  guardUnsaved(() => void closeNow());
}

/** Close the window (the app ends with its last window). After Don't
 *  Save the document's autosave snapshot goes first (AC-05) — bounded, so
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
    await Promise.race([discardDocument(), budget]);
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
