// Keeps the native macOS menu bar in step with the active editor
// (docs/specs/macos-native-menu.md AC-05, AC-14, AC-15, AC-17).
//
// One call in flight at a time; while it runs only the newest wanted menu
// waits, and a menu equal to the one installed is not sent again. The
// editors' key handlers leave a chord to the native menu only once the
// menu that binds it is in place (`setNativeBound`).

import { invoke } from '@tauri-apps/api/core';
import { setNativeBound, type ShortcutAction } from '../shell/shortcuts';
import type { Mirror, NativeNode } from './native-mirror';

interface Wanted {
  json: string;
  menus: NativeNode[] | null;
  bound: ReadonlySet<ShortcutAction> | null;
}

/** What the native menu shows: the start screen's menu at launch. */
let installed = JSON.stringify(null);
let wanted: Wanted | null = null;
let sending = false;
/** The entries of the newest mirror, by id. */
let runs: ReadonlyMap<string, () => void> = new Map();

/** Shows `mirror` in the native menu bar, or the start screen's menu for `null`. */
export function showEditorMenus(mirror: Mirror | null): void {
  runs = mirror?.runs ?? new Map();
  const menus = mirror?.menus ?? null;
  wanted = { json: JSON.stringify(menus), menus, bound: mirror?.bound ?? null };
  void flush();
}

async function flush(): Promise<void> {
  if (sending || wanted === null) return;
  const next = wanted;
  wanted = null;
  if (next.json === installed) {
    setNativeBound(next.bound);
    return;
  }
  sending = true;
  try {
    await invoke('set_editor_menu', { menus: next.menus });
    installed = next.json;
    setNativeBound(next.bound);
  } catch (err: unknown) {
    // The previous menu stays, and so do the keys it left to the editors;
    // the next change sends again (AC-14).
    console.error('set_editor_menu failed', err);
  } finally {
    sending = false;
    void flush();
  }
}

/** Runs the mirrored entry `id`; an entry the current menus no longer
 *  list, or list dimmed, does nothing (AC-17). */
export function runEditorEntry(id: string): void {
  runs.get(id)?.();
}
