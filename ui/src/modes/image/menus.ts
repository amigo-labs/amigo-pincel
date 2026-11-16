// Image-mode menu bar (spec §9.2). Same menus, order and labels as Pixel
// mode where the two overlap; entries Image mode lacks are simply absent.
// ImageEditor passes its state and actions in, so the editor keeps no
// menu markup.

import { tidy, type MenuDef } from '../../lib/components/layout/menu';
import {
  layerTransformItems,
  selectMenu,
  toolsMenus,
} from '../../lib/components/layout/shared-menus';
import { recentMenuItems } from '../../lib/components/layout/recent-menu';
import type { EffectDef } from '../../lib/effects/catalog';
import { effectMenus } from '../../lib/effects/menu';
import { helpMenus, settingsMenuItems } from '../../lib/help/menu';
import { t, type MessageKey } from '../../lib/i18n/index.svelte';
import type { RecentFile } from '../../lib/idb/recent-files';
import { droppedImagesMenu } from '../../lib/shell/drop-choice.svelte';
import { hint } from '../../lib/shell/shortcuts';
import type { ViewUnit } from './view-units';

/** View ▸ Units entries (docs/specs/image-view-aids.md AC-11). */
const UNIT_ITEMS: readonly (readonly [ViewUnit, MessageKey])[] = [
  ['px', 'menu.unitPixels'],
  ['in', 'menu.unitInches'],
  ['cm', 'menu.unitCentimeters'],
];

/** What the Image menus read and call. */
export interface ImageMenuContext {
  hasDoc: boolean;
  /** The active tab is being saved: File ▸ Close does nothing. */
  saving: boolean;
  /** A modal dialog is open: File ▸ Print… is disabled. */
  modalOpen: boolean;
  /** `null` hides Open Recent (no FSA handles / IndexedDB). */
  recents: RecentFile[] | null;
  canUndo: boolean;
  canRedo: boolean;
  hasSelection: boolean;
  canDeleteLayer: boolean;
  canMergeDown: boolean;
  /** The active layer is a group (Layer ▸ Merge Group). */
  groupActive: boolean;
  showRulers: boolean;
  showPixelGrid: boolean;
  units: ViewUnit;

  newDoc(): void;
  open(): void;
  openRecent(r: RecentFile): void;
  close(): void;
  save(asNew: boolean): void;
  exportAs(format: 'png' | 'jpeg' | 'webp'): void;
  exportSvg(): void;
  print(): void;
  undo(): void;
  redo(): void;
  copy(kind: 'copy' | 'merged' | 'cut'): void;
  paste(): void;
  pasteNewImage(): void;
  deleteSelection(): void;
  fillSelection(): void;
  swapColors(): void;
  resetColors(): void;
  selectAll(): void;
  deselect(): void;
  invertSelection(): void;
  /** Opens the radius prompt for Select ▸ Expand… / Contract… / Feather…. */
  growSelection(op: 'expand' | 'contract' | 'feather'): void;
  flipCanvas(horizontal: boolean): void;
  rotateCanvas(op: 'cw90' | 'ccw90' | 'rotate_180'): void;
  resizeCanvas(): void;
  scaleImage(): void;
  crop(): void;
  autoCrop(): void;
  addLayer(): void;
  addGroup(): void;
  duplicateLayer(): void;
  renameLayer(): void;
  deleteLayer(): void;
  mergeDown(): void;
  mergeGroup(): void;
  mergeVisible(): void;
  flatten(): void;
  flipLayer(horizontal: boolean): void;
  rotateLayer90(ccw: boolean): void;
  rotateLayer180(): void;
  /** Layer ▸ Rotate / Zoom…. */
  rotateZoom(): void;
  openEffect(def: EffectDef): void;
  zoomIn(): void;
  zoomOut(): void;
  zoomFit(): void;
  zoomActual(): void;
  toggleRulers(): void;
  togglePixelGrid(): void;
  setUnits(units: ViewUnit): void;
  /** View ▸ Toggle UI (power-user-shortcuts-coverage AC-17). */
  toggleUi(): void;
  /** Image ▸ Recognize Text… (ocr-dialog). */
  recognizeText(): void;
  /** Help ▸ Keyboard Shortcuts. */
  openShortcuts(): void;
}

/** The Image-mode menu bar. */
export function imageMenus(c: ImageMenuContext): MenuDef[] {
  const noDoc = !c.hasDoc;
  return [
    {
      id: 'file',
      label: t('menu.file'),
      items: tidy([
        { label: t('menu.new'), shortcut: hint('newDocument'), run: c.newDoc },
        { label: t('menu.open'), shortcut: hint('open'), run: c.open },
        ...recentMenuItems(c.recents, c.openRecent),
        droppedImagesMenu(),
        { kind: 'separator' },
        { label: t('menu.close'), shortcut: hint('closeTab'), disabled: noDoc || c.saving, run: c.close },
        { kind: 'separator' },
        { label: t('menu.save'), shortcut: hint('save'), disabled: noDoc, run: () => c.save(false) },
        { label: t('menu.saveAs'), shortcut: hint('saveAs'), disabled: noDoc, run: () => c.save(true) },
        { kind: 'separator' },
        {
          kind: 'submenu',
          label: t('menu.export'),
          disabled: noDoc,
          items: [
            { label: t('menu.exportPng'), shortcut: hint('export'), run: () => c.exportAs('png') },
            { label: t('menu.exportJpeg'), run: () => c.exportAs('jpeg') },
            // Lossless only: no pure-Rust lossy encoder (ADR-007).
            { label: t('menu.exportWebp'), run: () => c.exportAs('webp') },
            { label: t('menu.exportSvg'), run: c.exportSvg },
          ],
        },
        { label: t('menu.print'), shortcut: hint('print'), disabled: noDoc || c.modalOpen, run: c.print },
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
        {
          label: t('menu.fillSelection'),
          shortcut: hint('fillSelection'),
          disabled: noDoc,
          run: c.fillSelection,
        },
        { kind: 'separator' },
        { label: t('menu.swapColors'), shortcut: hint('swapColors'), run: c.swapColors },
        { label: t('menu.defaultColors'), shortcut: hint('resetColors'), run: c.resetColors },
        ...settingsMenuItems(),
      ],
    },
    selectMenu({
      noDoc,
      hasSelection: c.hasSelection,
      selectAll: c.selectAll,
      deselect: c.deselect,
      invertSelection: c.invertSelection,
      expand: () => c.growSelection('expand'),
      contract: () => c.growSelection('contract'),
      feather: () => c.growSelection('feather'),
    }),
    {
      id: 'image',
      label: t('menu.image'),
      items: [
        { label: t('menu.flipH'), disabled: noDoc, run: () => c.flipCanvas(true) },
        { label: t('menu.flipV'), disabled: noDoc, run: () => c.flipCanvas(false) },
        { kind: 'separator' },
        {
          label: t('menu.rotateCw'),
          shortcut: hint('rotateCw'),
          disabled: noDoc,
          run: () => c.rotateCanvas('cw90'),
        },
        {
          label: t('menu.rotateCcw'),
          shortcut: hint('rotateCcw'),
          disabled: noDoc,
          run: () => c.rotateCanvas('ccw90'),
        },
        { label: t('menu.rotate180'), disabled: noDoc, run: () => c.rotateCanvas('rotate_180') },
        { kind: 'separator' },
        { label: t('menu.resizeCanvas'), disabled: noDoc, run: c.resizeCanvas },
        { label: t('menu.scaleImage'), disabled: noDoc, run: c.scaleImage },
        { label: t('menu.crop'), disabled: !c.hasSelection, run: c.crop },
        { label: t('menu.autoCrop'), disabled: noDoc, run: c.autoCrop },
        { kind: 'separator' },
        { label: t('menu.recognizeText'), disabled: noDoc, run: c.recognizeText },
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
          danger: true,
          shortcut: hint('deleteLayer'),
          disabled: !c.canDeleteLayer,
          run: c.deleteLayer,
        },
        { kind: 'separator' },
        { label: t('menu.mergeDown'), shortcut: hint('mergeDown'), disabled: !c.canMergeDown, run: c.mergeDown },
        { label: t('menu.mergeGroup'), disabled: !c.groupActive, run: c.mergeGroup },
        { label: t('menu.mergeVisible'), disabled: noDoc, run: c.mergeVisible },
        { label: t('menu.flatten'), disabled: noDoc, run: c.flatten },
        ...layerTransformItems({
          noDoc,
          flipH: () => c.flipLayer(true),
          flipV: () => c.flipLayer(false),
          rotateCw: () => c.rotateLayer90(false),
          rotateCcw: () => c.rotateLayer90(true),
          rotate180: c.rotateLayer180,
          dialog: { label: t('menu.rotateZoom'), run: c.rotateZoom },
        }),
      ],
    },
    ...effectMenus(c.openEffect, noDoc, 'image'),
    {
      id: 'view',
      label: t('menu.view'),
      items: [
        { label: t('menu.zoomIn'), shortcut: hint('zoomIn'), disabled: noDoc, run: c.zoomIn },
        { label: t('menu.zoomOut'), shortcut: hint('zoomOut'), disabled: noDoc, run: c.zoomOut },
        { label: t('menu.zoomFit'), shortcut: hint('zoomFit'), disabled: noDoc, run: c.zoomFit },
        { label: t('menu.zoomActual'), shortcut: hint('zoomActual'), disabled: noDoc, run: c.zoomActual },
        { kind: 'separator' },
        { label: t('menu.rulers'), shortcut: hint('toggleRulers'), checked: c.showRulers, run: c.toggleRulers },
        {
          label: t('menu.pixelGrid'),
          shortcut: hint('togglePixelGrid'),
          checked: c.showPixelGrid,
          run: c.togglePixelGrid,
        },
        {
          kind: 'submenu',
          label: t('menu.units'),
          items: UNIT_ITEMS.map(([units, label]) => ({
            label: t(label),
            checked: c.units === units,
            run: () => c.setUnits(units),
          })),
        },
        { kind: 'separator' },
        { label: t('menu.toggleUi'), shortcut: hint('toggleUi'), run: c.toggleUi },
      ],
    },
    ...toolsMenus(),
    ...helpMenus(c.openShortcuts, 'image'),
  ];
}
