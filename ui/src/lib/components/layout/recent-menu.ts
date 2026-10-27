// File ▸ Open Recent, shared by both modes (spec §9.2).
//
// Lives beside the menu data model rather than in `lib/menu/` — that one
// is the Tauri bridge, which feeds the *native* recents submenu through
// `set_recent_menu` and knows nothing about `MenuDef`.

import type { RecentFile } from '../../idb/recent-files';
import { t } from '../../i18n/index.svelte';
import type { MenuItem } from './menu';

/**
 * The Open Recent submenu, or nothing when the registry is unavailable:
 * re-opening needs something to re-open with — an FSA handle on the web,
 * a path under Tauri — and IndexedDB to remember it in. Pass `null` for
 * `recents` in that case and the entry stays out of the menu entirely,
 * rather than sitting there permanently empty.
 */
export function recentMenuItems(
  recents: RecentFile[] | null,
  open: (recent: RecentFile) => void,
  disabled = false,
): MenuItem[] {
  if (recents === null) return [];
  return [
    {
      kind: 'submenu',
      label: t('menu.openRecent'),
      disabled: recents.length === 0 || disabled,
      items: recents.map((r) => ({ label: r.name, run: () => open(r) })),
    },
  ];
}
