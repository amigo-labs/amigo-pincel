// The Help menu both modes append to their menu bar (spec §9.2).

import { tidy, type MenuDef, type MenuItem } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { hasNativeMenu } from '../platform';
import { hint } from '../shell/shortcuts';
import { help, openAbout, runUpdateCheck, setAutoUpdate } from './state.svelte';
import { startTour, type TourContext } from '../tour/tour.svelte';

/**
 * The Help menu (Keyboard Shortcuts, About, the updater's entries), or nothing where the OS already draws one: on macOS the
 * native menu bar owns Help, and a second copy inside the window would be
 * the duplication this menu exists to avoid (spec §11.3).
 */
export function helpMenus(openShortcuts: () => void, tourContext?: TourContext): MenuDef[] {
  if (hasNativeMenu()) return [];
  const updater = help.updater;
  // No update entries where nothing can be installed: a browser, a debug
  // build, or a copy a package manager owns (`updater::supported()`).
  const updates: MenuItem[] =
    updater !== null && updater.supported
      ? [
          { kind: 'separator' },
          { label: t('menu.checkUpdates'), disabled: help.checking, run: runUpdateCheck },
          {
            label: t('menu.autoUpdate'),
            checked: updater.autoUpdate,
            run: () => setAutoUpdate(!updater.autoUpdate),
          },
        ]
      : [];
  return [
    {
      id: 'help',
      label: t('menu.help'),
      items: tidy([
        { label: t('menu.shortcuts'), shortcut: hint('shortcuts'), run: openShortcuts },
        ...(tourContext
          ? [{ label: t('menu.tour'), run: () => startTour(tourContext) } satisfies MenuItem]
          : []),
        { kind: 'separator' },
        { label: t('menu.about'), run: openAbout },
        ...updates,
      ]),
    },
  ];
}
