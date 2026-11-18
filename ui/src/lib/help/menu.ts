// The Help menu both modes append to their menu bar (spec §9.2).

import { tidy, type MenuDef, type MenuItem } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { hasNativeMenu } from '../platform';
import { hint } from '../shell/shortcuts';
import { help, openAbout, openSettings, runUpdateCheck } from './state.svelte';
import { startTour, type TourContext } from '../tour/tour-state.svelte';

/** The end of both editors' Edit menu: Settings… (docs/specs/settings-dialog.md);
 *  nothing on macOS, where the native App menu holds it (macos-native-menu AC-08). */
export function settingsMenuItems(): MenuItem[] {
  if (hasNativeMenu()) return [];
  return [
    { kind: 'separator' },
    { label: t('menu.settings'), shortcut: hint('settings'), run: openSettings },
  ];
}

/**
 * The Help menu (Keyboard Shortcuts, Tour, the mode's `diagnostics` entries,
 * About, the updater's entries). On macOS the native menu bar mirrors it, and
 * About and the updater live in the native App menu instead
 * (docs/specs/macos-native-menu.md AC-08).
 */
export function helpMenus(
  openShortcuts: () => void,
  tourContext?: TourContext,
  diagnostics: MenuItem[] = [],
): MenuDef[] {
  const updater = help.updater;
  // No update entries where nothing can be installed: a browser, a debug
  // build, or a copy a package manager owns (`updater::supported()`).
  const updates: MenuItem[] =
    updater !== null && updater.supported
      ? [
          { kind: 'separator' },
          { label: t('menu.checkUpdates'), disabled: help.checking, run: runUpdateCheck },
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
        ...diagnostics,
        ...(hasNativeMenu()
          ? []
          : [{ kind: 'separator' } satisfies MenuItem, { label: t('menu.about'), run: openAbout }, ...updates]),
      ]),
    },
  ];
}
