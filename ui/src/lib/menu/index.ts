// Tauri menu bridge for the app shell.
//
// The native menu lives in Rust (src-tauri/src/main.rs). Each menu
// item carries a stable id like `menu:open` or `recent:<docId>`; the
// Rust `on_menu_event` handler emits a `"menu"` window event with the
// id string as payload. `wireNativeMenu` subscribes to that event and
// dispatches to the matching handler.
//
// Two handler levels: the shell serves New / Open / Open Recent (it
// owns the mode routing), the mounted editor serves the rest and
// registers its handlers through `EditorProps.registerMenuHandlers`.
// Items the current editor does not serve are ignored.
//
// `syncRecentMenu` flips the data flow: the renderer-owned recents
// list is pushed back to Rust so it can rebuild the `Open Recent`
// submenu. Only entries with a `path` make sense as native-menu
// targets (FSA handles can't be re-opened by the OS).

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type MenuHandler = () => void | Promise<void>;
export type RecentHandler = (id: string) => void | Promise<void>;

/** Menu items served by whichever editor is mounted. */
export type EditorMenuId =
  | 'menu:save'
  | 'menu:saveAs'
  | 'menu:undo'
  | 'menu:redo'
  | 'menu:zoomIn'
  | 'menu:zoomOut'
  | 'menu:resetZoom';

export type EditorMenuHandlers = Partial<Record<EditorMenuId, MenuHandler>>;

/** Menu items served by the shell, plus a getter for the editor's. */
export interface ShellMenuHandlers {
  'menu:new': MenuHandler;
  'menu:open': MenuHandler;
  recent: RecentHandler;
  /** Current editor handlers (`null` while the start screen shows). */
  editor: () => EditorMenuHandlers | null;
}

export async function wireNativeMenu(handlers: ShellMenuHandlers): Promise<UnlistenFn> {
  return listen<string>('menu', (event) => {
    const payload = event.payload;
    if (payload.startsWith('recent:')) {
      const id = payload.slice('recent:'.length);
      if (id === '_empty') return;
      void handlers.recent(id);
      return;
    }
    if (payload === 'menu:new' || payload === 'menu:open') {
      void handlers[payload]();
      return;
    }
    const editorHandler = handlers.editor()?.[payload as EditorMenuId];
    if (typeof editorHandler === 'function') void editorHandler();
  });
}

export interface RecentMenuEntry {
  id: string;
  name: string;
}

export async function syncRecentMenu(items: RecentMenuEntry[]): Promise<void> {
  await invoke('set_recent_menu', { items });
}
