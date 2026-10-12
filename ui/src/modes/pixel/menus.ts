// Pixel-mode menu bar (spec §9.2). Same menus, order and labels as Image
// mode where the two overlap, plus Frame; PixelEditor passes its state and
// actions in, so the editor keeps no menu markup.

import { effectMenus } from '../../lib/effects/menu';
import type { EffectDef } from '../../lib/effects/catalog';
import { tidy, type MenuDef, type MenuItem } from '../../lib/components/layout/menu';
import { hint } from '../../lib/shell/shortcuts';
import { languageMenu } from '../../lib/i18n/locale';
import { t } from '../../lib/i18n/index.svelte';
import type { RecentFile } from '../../lib/idb/recent-files';

/** Layer / canvas transform ids the wasm `Document` takes. */
export type TransformOp =
  | 'flip_horizontal'
  | 'flip_vertical'
  | 'rotate_90_cw'
  | 'rotate_90_ccw'
  | 'rotate_180';

/** What the Pixel menus read and call. */
export interface PixelMenuContext {
  hasDoc: boolean;
  busy: boolean;
  /** Save writes in place (FSA / Tauri); otherwise it downloads. */
  canSaveInPlace: boolean;
  /** `null` hides Open Recent (no FSA handles / IndexedDB). */
  recents: RecentFile[] | null;
  canUndo: boolean;
  canRedo: boolean;
  hasSelection: boolean;
  /** The active layer can merge into the one below. */
  canMergeDown: boolean;
  layerCount: number;
  frameCount: number;
  currentFrame: number;
  playing: boolean;
  probeOn: boolean;

  newDoc(): void;
  open(): void;
  openRecent(r: RecentFile): void;
  save(asNew: boolean): void;
  importPng(): void;
  exportPng(): void;
  exportSheet(): void;
  undo(): void;
  redo(): void;
  deleteSelection(): void;
  swapColors(): void;
  resetColors(): void;
  selectAll(): void;
  deselect(): void;
  invertSelection(): void;
  growSelection(delta: 1 | -1): void;
  transformCanvas(op: TransformOp): void;
  transformLayer(op: TransformOp): void;
  resizeCanvas(): void;
  scaleImage(): void;
  crop(): void;
  addLayer(): void;
  addGroup(): void;
  duplicateLayer(): void;
  removeLayer(): void;
  mergeDown(): void;
  flatten(): void;
  renameLayer(): void;
  togglePlay(): void;
  stepFrame(delta: 1 | -1): void;
  addFrame(): void;
  removeFrame(): void;
  openEffect(def: EffectDef): void;
  zoomIn(): void;
  zoomOut(): void;
  zoomFit(): void;
  zoomActual(): void;
  toggleProbe(): void;
}

function transforms(run: (op: TransformOp) => void, disabled: boolean): MenuItem[] {
  return [
    { label: t('menu.flipH'), disabled, run: () => run('flip_horizontal') },
    { label: t('menu.flipV'), disabled, run: () => run('flip_vertical') },
    { kind: 'separator' },
    { label: t('menu.rotateCw'), disabled, run: () => run('rotate_90_cw') },
    { label: t('menu.rotateCcw'), disabled, run: () => run('rotate_90_ccw') },
    { label: t('menu.rotate180'), disabled, run: () => run('rotate_180') },
  ];
}

/** The Pixel-mode menu bar. */
export function pixelMenus(c: PixelMenuContext): MenuDef[] {
  const noDoc = !c.hasDoc || c.busy;
  return [
    {
      id: 'file',
      label: t('menu.file'),
      items: tidy([
        { label: t('menu.new'), disabled: c.busy, run: c.newDoc },
        { label: t('menu.open'), shortcut: hint('open'), disabled: c.busy, run: c.open },
        ...(c.recents
          ? [
              {
                kind: 'submenu' as const,
                label: t('menu.openRecent'),
                disabled: c.recents.length === 0 || c.busy,
                items: c.recents.map((r) => ({ label: r.name, run: () => c.openRecent(r) })),
              },
            ]
          : []),
        { kind: 'separator' },
        {
          label: c.canSaveInPlace ? t('menu.save') : t('menu.saveDownload'),
          shortcut: hint('save'),
          disabled: noDoc,
          run: () => c.save(false),
        },
        ...(c.canSaveInPlace
          ? [{ label: t('menu.saveAs'), shortcut: hint('saveAs'), disabled: noDoc, run: () => c.save(true) }]
          : []),
        { kind: 'separator' },
        { label: t('menu.importPng'), disabled: noDoc, run: c.importPng },
        {
          kind: 'submenu',
          label: t('menu.export'),
          disabled: noDoc,
          items: [
            { label: t('menu.exportPngFrame'), shortcut: hint('export'), run: c.exportPng },
            { label: t('menu.exportSheet'), shortcut: hint('exportSheet'), run: c.exportSheet },
          ],
        },
      ]),
    },
    {
      id: 'edit',
      label: t('menu.edit'),
      items: [
        { label: t('menu.undo'), shortcut: hint('undo'), disabled: !c.canUndo, run: c.undo },
        { label: t('menu.redo'), shortcut: hint('redo'), disabled: !c.canRedo, run: c.redo },
        { kind: 'separator' },
        {
          label: t('menu.deleteSelection'),
          shortcut: hint('deleteSelection'),
          disabled: !c.hasSelection,
          run: c.deleteSelection,
        },
        { kind: 'separator' },
        { label: t('menu.swapColors'), shortcut: hint('swapColors'), run: c.swapColors },
        { label: t('menu.defaultColors'), shortcut: hint('resetColors'), run: c.resetColors },
      ],
    },
    {
      id: 'select',
      label: t('menu.select'),
      items: [
        { label: t('menu.selectAll'), shortcut: hint('selectAll'), disabled: noDoc, run: c.selectAll },
        { label: t('menu.deselect'), shortcut: hint('deselect'), disabled: !c.hasSelection, run: c.deselect },
        {
          label: t('menu.invertSelection'),
          shortcut: hint('invertSelection'),
          disabled: noDoc,
          run: c.invertSelection,
        },
        { kind: 'separator' },
        { label: t('menu.expand1'), disabled: !c.hasSelection, run: () => c.growSelection(1) },
        { label: t('menu.contract1'), disabled: !c.hasSelection, run: () => c.growSelection(-1) },
      ],
    },
    {
      id: 'image',
      label: t('menu.image'),
      items: [
        ...transforms(c.transformCanvas, noDoc),
        { kind: 'separator' },
        { label: t('menu.resizeCanvas'), disabled: noDoc, run: c.resizeCanvas },
        { label: t('menu.scaleImage'), disabled: noDoc, run: c.scaleImage },
        { label: t('menu.crop'), disabled: noDoc || !c.hasSelection, run: c.crop },
      ],
    },
    {
      id: 'layer',
      label: t('menu.layer'),
      items: [
        { label: t('menu.newLayer'), disabled: noDoc, run: c.addLayer },
        { label: t('menu.newGroup'), disabled: noDoc, run: c.addGroup },
        { label: t('menu.duplicateLayer'), disabled: noDoc, run: c.duplicateLayer },
        { label: t('menu.renameLayer'), disabled: noDoc, run: c.renameLayer },
        { label: t('menu.deleteLayer'), disabled: noDoc || c.layerCount <= 1, run: c.removeLayer },
        { kind: 'separator' },
        { label: t('menu.mergeDown'), disabled: noDoc || !c.canMergeDown, run: c.mergeDown },
        { label: t('menu.flatten'), disabled: noDoc || c.layerCount < 2, run: c.flatten },
        { kind: 'separator' },
        ...transforms(c.transformLayer, noDoc),
      ],
    },
    {
      id: 'frame',
      label: t('menu.frame'),
      items: [
        {
          label: c.playing ? t('menu.pause') : t('menu.play'),
          shortcut: hint('togglePlay'),
          disabled: noDoc || c.frameCount < 2,
          run: c.togglePlay,
        },
        {
          label: t('menu.prevFrame'),
          shortcut: hint('prevFrame'),
          disabled: noDoc || c.currentFrame === 0,
          run: () => c.stepFrame(-1),
        },
        {
          label: t('menu.nextFrame'),
          shortcut: hint('nextFrame'),
          disabled: noDoc || c.currentFrame >= c.frameCount - 1,
          run: () => c.stepFrame(1),
        },
        { kind: 'separator' },
        { label: t('menu.newFrame'), disabled: noDoc, run: c.addFrame },
        { label: t('menu.deleteFrame'), disabled: noDoc || c.frameCount < 2, run: c.removeFrame },
      ],
    },
    ...effectMenus(c.openEffect, noDoc),
    {
      id: 'view',
      label: t('menu.view'),
      items: [
        { label: t('menu.zoomIn'), shortcut: hint('zoomIn'), disabled: !c.hasDoc, run: c.zoomIn },
        { label: t('menu.zoomOut'), shortcut: hint('zoomOut'), disabled: !c.hasDoc, run: c.zoomOut },
        { label: t('menu.zoomFit'), shortcut: hint('zoomFit'), disabled: !c.hasDoc, run: c.zoomFit },
        { label: t('menu.zoomActual'), shortcut: hint('zoomActual'), disabled: !c.hasDoc, run: c.zoomActual },
        { kind: 'separator' },
        { label: t('menu.probe'), shortcut: 'F2', checked: c.probeOn, run: c.toggleProbe },
        { kind: 'separator' },
        languageMenu(),
      ],
    },
  ];
}
