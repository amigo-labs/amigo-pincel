// Pixel-mode menu bar (spec §9.2). Same menus, order and labels as Image
// mode where the two overlap, plus Frame; PixelEditor passes its state and
// actions in, so the editor keeps no menu markup.

import { effectMenus } from '../../lib/effects/menu';
import type { EffectDef } from '../../lib/effects/catalog';
import { tidy, type MenuDef, type MenuItem } from '../../lib/components/layout/menu';
import { hint } from '../../lib/shell/shortcuts';
import { languageMenu } from '../../lib/i18n/locale';
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
  duplicateLayer(): void;
  removeLayer(): void;
  mergeDown(): void;
  flatten(): void;
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
    { label: 'Flip Horizontal', disabled, run: () => run('flip_horizontal') },
    { label: 'Flip Vertical', disabled, run: () => run('flip_vertical') },
    { kind: 'separator' },
    { label: 'Rotate 90° CW', disabled, run: () => run('rotate_90_cw') },
    { label: 'Rotate 90° CCW', disabled, run: () => run('rotate_90_ccw') },
    { label: 'Rotate 180°', disabled, run: () => run('rotate_180') },
  ];
}

/** The Pixel-mode menu bar. */
export function pixelMenus(c: PixelMenuContext): MenuDef[] {
  const noDoc = !c.hasDoc || c.busy;
  return [
    {
      id: 'file',
      label: 'File',
      items: tidy([
        { label: 'New…', disabled: c.busy, run: c.newDoc },
        { label: 'Open…', shortcut: hint('open'), disabled: c.busy, run: c.open },
        ...(c.recents
          ? [
              {
                kind: 'submenu' as const,
                label: 'Open Recent',
                disabled: c.recents.length === 0 || c.busy,
                items: c.recents.map((r) => ({ label: r.name, run: () => c.openRecent(r) })),
              },
            ]
          : []),
        { kind: 'separator' },
        {
          label: c.canSaveInPlace ? 'Save' : 'Save (download)',
          shortcut: hint('save'),
          disabled: noDoc,
          run: () => c.save(false),
        },
        ...(c.canSaveInPlace
          ? [{ label: 'Save As…', shortcut: hint('saveAs'), disabled: noDoc, run: () => c.save(true) }]
          : []),
        { kind: 'separator' },
        { label: 'Import PNG as Layer…', disabled: noDoc, run: c.importPng },
        {
          kind: 'submenu',
          label: 'Export',
          disabled: noDoc,
          items: [
            { label: 'PNG (current frame)', shortcut: hint('export'), run: c.exportPng },
            { label: 'Sprite Sheet…', shortcut: hint('exportSheet'), run: c.exportSheet },
          ],
        },
      ]),
    },
    {
      id: 'edit',
      label: 'Edit',
      items: [
        { label: 'Undo', shortcut: hint('undo'), disabled: !c.canUndo, run: c.undo },
        { label: 'Redo', shortcut: hint('redo'), disabled: !c.canRedo, run: c.redo },
        { kind: 'separator' },
        {
          label: 'Delete Selection',
          shortcut: hint('deleteSelection'),
          disabled: !c.hasSelection,
          run: c.deleteSelection,
        },
        { kind: 'separator' },
        { label: 'Swap Colors', shortcut: hint('swapColors'), run: c.swapColors },
        { label: 'Default Colors', shortcut: hint('resetColors'), run: c.resetColors },
      ],
    },
    {
      id: 'select',
      label: 'Select',
      items: [
        { label: 'Select All', shortcut: hint('selectAll'), disabled: noDoc, run: c.selectAll },
        { label: 'Deselect', shortcut: hint('deselect'), disabled: !c.hasSelection, run: c.deselect },
        {
          label: 'Invert Selection',
          shortcut: hint('invertSelection'),
          disabled: noDoc,
          run: c.invertSelection,
        },
        { kind: 'separator' },
        { label: 'Expand 1 px', disabled: !c.hasSelection, run: () => c.growSelection(1) },
        { label: 'Contract 1 px', disabled: !c.hasSelection, run: () => c.growSelection(-1) },
      ],
    },
    {
      id: 'image',
      label: 'Image',
      items: [
        ...transforms(c.transformCanvas, noDoc),
        { kind: 'separator' },
        { label: 'Resize Canvas…', disabled: noDoc, run: c.resizeCanvas },
        { label: 'Scale Image…', disabled: noDoc, run: c.scaleImage },
        { label: 'Crop to Selection', disabled: noDoc || !c.hasSelection, run: c.crop },
      ],
    },
    {
      id: 'layer',
      label: 'Layer',
      items: [
        { label: 'New Layer', disabled: noDoc, run: c.addLayer },
        { label: 'Duplicate Layer', disabled: noDoc, run: c.duplicateLayer },
        { label: 'Delete Layer', disabled: noDoc || c.layerCount <= 1, run: c.removeLayer },
        { kind: 'separator' },
        { label: 'Merge Down', disabled: noDoc || !c.canMergeDown, run: c.mergeDown },
        { label: 'Flatten Image', disabled: noDoc || c.layerCount < 2, run: c.flatten },
        { kind: 'separator' },
        ...transforms(c.transformLayer, noDoc),
      ],
    },
    {
      id: 'frame',
      label: 'Frame',
      items: [
        {
          label: c.playing ? 'Pause' : 'Play',
          shortcut: hint('togglePlay'),
          disabled: noDoc || c.frameCount < 2,
          run: c.togglePlay,
        },
        {
          label: 'Previous Frame',
          shortcut: hint('prevFrame'),
          disabled: noDoc || c.currentFrame === 0,
          run: () => c.stepFrame(-1),
        },
        {
          label: 'Next Frame',
          shortcut: hint('nextFrame'),
          disabled: noDoc || c.currentFrame >= c.frameCount - 1,
          run: () => c.stepFrame(1),
        },
        { kind: 'separator' },
        { label: 'New Frame', disabled: noDoc, run: c.addFrame },
        { label: 'Delete Frame', disabled: noDoc || c.frameCount < 2, run: c.removeFrame },
      ],
    },
    ...effectMenus(c.openEffect, noDoc),
    {
      id: 'view',
      label: 'View',
      items: [
        { label: 'Zoom In', shortcut: hint('zoomIn'), disabled: !c.hasDoc, run: c.zoomIn },
        { label: 'Zoom Out', shortcut: hint('zoomOut'), disabled: !c.hasDoc, run: c.zoomOut },
        { label: 'Fit to Window', shortcut: hint('zoomFit'), disabled: !c.hasDoc, run: c.zoomFit },
        { label: 'Actual Size', shortcut: hint('zoomActual'), disabled: !c.hasDoc, run: c.zoomActual },
        { kind: 'separator' },
        { label: 'Frame-Time Probe', shortcut: 'F2', checked: c.probeOn, run: c.toggleProbe },
        { kind: 'separator' },
        languageMenu(),
      ],
    },
  ];
}
