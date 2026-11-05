// Pixel-mode menu bar (spec §9.2). Same menus, order and labels as Image
// mode where the two overlap, plus Frame; PixelEditor passes its state and
// actions in, so the editor keeps no menu markup.

import { effectMenus } from '../../lib/effects/menu';
import { helpMenus } from '../../lib/help/menu';
import { recentMenuItems } from '../../lib/components/layout/recent-menu';
import { droppedImagesMenu } from '../../lib/shell/drop-choice.svelte';
import type { EffectDef } from '../../lib/effects/catalog';
import { tidy, type MenuDef, type MenuItem } from '../../lib/components/layout/menu';
import { hint } from '../../lib/shell/shortcuts';
import { languageMenu } from '../../lib/i18n/locale';
import { reduceMotionMenuItem } from '../../lib/motion.svelte';
import { t } from '../../lib/i18n/index.svelte';
import type { RecentFile } from '../../lib/idb/recent-files';
import type { AxisMode } from './stores/editor.svelte';

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
  /** View ▸ Show Pixel Grid / Show Grid (pixel-grid). */
  showPixelGrid: boolean;
  showGrid: boolean;
  /** View ▸ Tiled Mode (symmetry-tiled-mode AC-12). */
  tiled: AxisMode;
  setTiled(mode: AxisMode): void;

  newDoc(): void;
  open(): void;
  openRecent(r: RecentFile): void;
  save(asNew: boolean): void;
  /** File ▸ Close: close the active tab (document-tabs AC-07). */
  close(): void;
  /** The active tab is being saved: File ▸ Close does nothing (AC-17). */
  saving: boolean;
  importPng(): void;
  exportPng(): void;
  exportSheet(): void;
  exportAnimation(): void;
  exportSvg(): void;
  undo(): void;
  redo(): void;
  deleteSelection(): void;
  /** Edit ▸ Cut / Copy / Copy Merged (spec §5.4). */
  copy(kind: 'copy' | 'merged' | 'cut'): void;
  paste(): void;
  /** Edit ▸ Paste as New Image (copy-cut-paste AC-23). */
  pasteNewImage(): void;
  swapColors(): void;
  resetColors(): void;
  selectAll(): void;
  deselect(): void;
  invertSelection(): void;
  /** Opens the radius prompt for Select ▸ Expand… / Contract…. */
  growSelection(mode: 'expand' | 'contract'): void;
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
  togglePixelGrid(): void;
  toggleGrid(): void;
  openGridSettings(): void;
  /** Help ▸ Keyboard Shortcuts. */
  openShortcuts(): void;
  /** View ▸ Toggle UI (power-user-shortcuts-coverage AC-17). */
  toggleUi(): void;
}

/** Flips and rotations; `layer` picks whose keys the hints show (flips: layer, 90°: image). */
function transforms(
  run: (op: TransformOp) => void,
  disabled: boolean,
  layer: boolean,
): MenuItem[] {
  const flipKey = (a: 'flipLayerH' | 'flipLayerV') => (layer ? hint(a) : '');
  const rotateKey = (a: 'rotateCw' | 'rotateCcw') => (layer ? '' : hint(a));
  return [
    { label: t('menu.flipH'), shortcut: flipKey('flipLayerH'), disabled, run: () => run('flip_horizontal') },
    { label: t('menu.flipV'), shortcut: flipKey('flipLayerV'), disabled, run: () => run('flip_vertical') },
    { kind: 'separator' },
    { label: t('menu.rotateCw'), shortcut: rotateKey('rotateCw'), disabled, run: () => run('rotate_90_cw') },
    { label: t('menu.rotateCcw'), shortcut: rotateKey('rotateCcw'), disabled, run: () => run('rotate_90_ccw') },
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
        { label: t('menu.new'), shortcut: hint('newDocument'), disabled: c.busy, run: c.newDoc },
        { label: t('menu.open'), shortcut: hint('open'), disabled: c.busy, run: c.open },
        ...recentMenuItems(c.recents, c.openRecent, c.busy),
        droppedImagesMenu(),
        { kind: 'separator' },
        { label: t('menu.close'), shortcut: hint('closeTab'), disabled: noDoc || c.saving, run: c.close },
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
            { label: t('menu.exportAnimation'), run: c.exportAnimation },
            { label: t('menu.exportSvg'), run: c.exportSvg },
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
        { label: t('menu.cut'), shortcut: hint('cut'), disabled: noDoc, run: () => c.copy('cut') },
        { label: t('menu.copy'), shortcut: hint('copy'), disabled: noDoc, run: () => c.copy('copy') },
        { label: t('menu.copyMerged'), disabled: noDoc, run: () => c.copy('merged') },
        { label: t('menu.paste'), shortcut: hint('paste'), disabled: noDoc, run: c.paste },
        { label: t('menu.pasteNewImage'), shortcut: hint('pasteNewImage'), run: c.pasteNewImage },
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
        { label: t('menu.expand'), disabled: !c.hasSelection, run: () => c.growSelection('expand') },
        {
          label: t('menu.contract'),
          disabled: !c.hasSelection,
          run: () => c.growSelection('contract'),
        },
      ],
    },
    {
      id: 'image',
      label: t('menu.image'),
      items: [
        ...transforms(c.transformCanvas, noDoc, false),
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
        { label: t('menu.newLayer'), shortcut: hint('newLayer'), disabled: noDoc, run: c.addLayer },
        { label: t('menu.newGroup'), disabled: noDoc, run: c.addGroup },
        {
          label: t('menu.duplicateLayer'),
          shortcut: hint('duplicateLayer'),
          disabled: noDoc,
          run: c.duplicateLayer,
        },
        { label: t('menu.renameLayer'), disabled: noDoc, run: c.renameLayer },
        {
          label: t('menu.deleteLayer'),
          shortcut: hint('deleteLayer'),
          disabled: noDoc || c.layerCount <= 1,
          run: c.removeLayer,
        },
        { kind: 'separator' },
        {
          label: t('menu.mergeDown'),
          shortcut: hint('mergeDown'),
          disabled: noDoc || !c.canMergeDown,
          run: c.mergeDown,
        },
        { label: t('menu.flatten'), disabled: noDoc || c.layerCount < 2, run: c.flatten },
        { kind: 'separator' },
        ...transforms(c.transformLayer, noDoc, true),
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
        { label: t('menu.newFrame'), shortcut: hint('newFrame'), disabled: noDoc, run: c.addFrame },
        { label: t('menu.deleteFrame'), disabled: noDoc || c.frameCount < 2, run: c.removeFrame },
      ],
    },
    ...effectMenus(c.openEffect, noDoc, 'pixel'),
    {
      id: 'view',
      label: t('menu.view'),
      items: [
        { label: t('menu.zoomIn'), shortcut: hint('zoomIn'), disabled: !c.hasDoc, run: c.zoomIn },
        { label: t('menu.zoomOut'), shortcut: hint('zoomOut'), disabled: !c.hasDoc, run: c.zoomOut },
        { label: t('menu.zoomFit'), shortcut: hint('zoomFit'), disabled: !c.hasDoc, run: c.zoomFit },
        { label: t('menu.zoomActual'), shortcut: hint('zoomActual'), disabled: !c.hasDoc, run: c.zoomActual },
        { kind: 'separator' },
        {
          label: t('menu.showPixelGrid'),
          shortcut: hint('togglePixelGrid'),
          checked: c.showPixelGrid,
          run: c.togglePixelGrid,
        },
        { label: t('menu.showGrid'), shortcut: hint('toggleGrid'), checked: c.showGrid, run: c.toggleGrid },
        { label: t('menu.gridSettings'), disabled: !c.hasDoc, run: c.openGridSettings },
        {
          kind: 'submenu',
          label: t('menu.tiledMode'),
          items: (
            [
              ['none', t('menu.tiledNone')],
              ['x', t('menu.tiledX')],
              ['y', t('menu.tiledY')],
              ['xy', t('menu.tiledBoth')],
            ] as const
          ).map(([mode, label]) => ({ label, checked: c.tiled === mode, run: () => c.setTiled(mode) })),
        },
        { kind: 'separator' },
        { label: t('menu.toggleUi'), shortcut: hint('toggleUi'), run: c.toggleUi },
        { label: t('menu.probe'), shortcut: 'F2', checked: c.probeOn, run: c.toggleProbe },
        reduceMotionMenuItem(),
        { kind: 'separator' },
        languageMenu(),
      ],
    },
    ...helpMenus(c.openShortcuts),
  ];
}
