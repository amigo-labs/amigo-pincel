// Menu parts both editors show (docs/specs/menu-structure.md): built in
// one place so the entries the modes share keep one order (AC-09).

import { agentsMenuItems } from '../../agent/access.svelte';
import { imageGenerationItems } from '../../comfyui/state.svelte';
import { t } from '../../i18n/index.svelte';
import { hint } from '../../shell/shortcuts';
import type { MenuDef, MenuItem } from './menu';

/** What the Select menu reads and calls. */
export interface SelectMenuContext {
  noDoc: boolean;
  hasSelection: boolean;
  selectAll(): void;
  deselect(): void;
  invertSelection(): void;
  expand(): void;
  contract(): void;
  /** Image mode only: Select ▸ Feather…. */
  feather?: () => void;
}

/** The Select menu: All, Deselect, Invert, then the radius commands (AC-10). */
export function selectMenu(c: SelectMenuContext): MenuDef {
  const none = !c.hasSelection;
  const feather = c.feather;
  return {
    id: 'select',
    label: t('menu.select'),
    items: [
      { label: t('menu.selectAll'), shortcut: hint('selectAll'), disabled: c.noDoc, run: c.selectAll },
      { label: t('menu.deselect'), shortcut: hint('deselect'), disabled: none, run: c.deselect },
      {
        label: t('menu.invertSelection'),
        shortcut: hint('invertSelection'),
        disabled: c.noDoc,
        run: c.invertSelection,
      },
      { kind: 'separator' },
      { label: t('menu.expand'), disabled: none, run: c.expand },
      { label: t('menu.contract'), disabled: none, run: c.contract },
      ...(feather ? [{ label: t('menu.feather'), disabled: none, run: feather }] : []),
    ],
  };
}

/** What Layer ▸ Transform reads and calls. */
export interface LayerTransformContext {
  noDoc: boolean;
  flipH(): void;
  flipV(): void;
  rotateCw(): void;
  rotateCcw(): void;
  rotate180(): void;
  /** The mode's rotate dialog: Rotate… (Pixel) or Rotate / Zoom… (Image). */
  dialog: { label: string; run: () => void };
}

/** The end of the Layer menu: a separator and the Transform submenu (AC-07). */
export function layerTransformItems(c: LayerTransformContext): MenuItem[] {
  const disabled = c.noDoc;
  return [
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: t('menu.transform'),
      items: [
        { label: t('menu.flipH'), shortcut: hint('flipLayerH'), disabled, run: c.flipH },
        { label: t('menu.flipV'), shortcut: hint('flipLayerV'), disabled, run: c.flipV },
        { kind: 'separator' },
        { label: t('menu.rotateCw'), disabled, run: c.rotateCw },
        { label: t('menu.rotateCcw'), disabled, run: c.rotateCcw },
        { label: t('menu.rotate180'), disabled, run: c.rotate180 },
        { kind: 'separator' },
        { label: c.dialog.label, disabled, run: c.dialog.run },
      ],
    },
  ];
}

/** The Tools menu — the desktop app's AI setup — or nothing in a browser (AC-01, AC-02). */
export function toolsMenus(): MenuDef[] {
  const items: MenuItem[] = [...agentsMenuItems(), ...imageGenerationItems()];
  return items.length === 0 ? [] : [{ id: 'tools', label: t('menu.tools'), items }];
}
