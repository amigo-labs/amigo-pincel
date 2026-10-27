// The Help menu both modes append to their menu bar (spec §9.2).

import type { MenuDef } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { hasNativeMenu } from '../platform';
import { openAbout } from './state.svelte';

/**
 * The Help menu, or nothing where the OS already draws one: on macOS the
 * native menu bar owns Help, and a second copy inside the window would be
 * the duplication this menu exists to avoid (spec §11.3).
 */
export function helpMenus(): MenuDef[] {
  if (hasNativeMenu()) return [];
  return [
    {
      id: 'help',
      label: t('menu.help'),
      items: [{ label: t('menu.about'), run: openAbout }],
    },
  ];
}
