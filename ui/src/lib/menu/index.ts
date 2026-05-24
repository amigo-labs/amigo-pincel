// Tauri menu bridge for App.svelte.
//
// The native menu lives in Rust (src-tauri/src/main.rs). Each menu
// item carries a stable id like `menu:open` or `recent:<docId>`; the
// Rust `on_menu_event` handler emits a `"menu"` window event with the
// id string as payload. `wireNativeMenu` subscribes to that event and
// dispatches to the matching handler.
//
// `syncRecentMenu` flips the data flow: the renderer-owned recents
// list is pushed back to Rust so it can rebuild the `Open Recent`
// submenu. Only entries with a `path` make sense as native-menu
// targets (FSA handles can't be re-opened by the OS).

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type MenuHandler = () => void | Promise<void>;
export type RecentHandler = (id: string) => void | Promise<void>;

export interface MenuHandlers {
  'menu:new': MenuHandler;
  'menu:open': MenuHandler;
  'menu:save': MenuHandler;
  'menu:saveAs': MenuHandler;
  'menu:undo': MenuHandler;
  'menu:redo': MenuHandler;
  'menu:zoomIn': MenuHandler;
  'menu:zoomOut': MenuHandler;
  'menu:resetZoom': MenuHandler;
  recent: RecentHandler;
}

export async function wireNativeMenu(
  handlers: MenuHandlers,
): Promise<UnlistenFn> {
  return listen<string>('menu', (event) => {
    const payload = event.payload;
    if (payload.startsWith('recent:')) {
      const id = payload.slice('recent:'.length);
      if (id === '_empty') return;
      void handlers.recent(id);
      return;
    }
    const handler = handlers[payload as keyof MenuHandlers];
    if (typeof handler === 'function' && handler !== handlers.recent) {
      void (handler as MenuHandler)();
    }
  });
}

export interface RecentMenuEntry {
  id: string;
  name: string;
}

export async function syncRecentMenu(items: RecentMenuEntry[]): Promise<void> {
  await invoke('set_recent_menu', { items });
}
