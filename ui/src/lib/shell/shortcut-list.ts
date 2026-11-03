// Rows of Help ▸ Keyboard Shortcuts (docs/specs/shortcut-overview.md),
// generated from the one keymap (`shortcuts.ts`, spec §9.4) so the list
// can't drift from what the keys do. Each editor says which actions it
// binds and which tool groups it has; this module turns that into rows.

import type { ToolGroup } from '../components/layout/tools';
import { t, type MessageKey } from '../i18n/index.svelte';
import {
  TOOL_GROUP_KEYS,
  chordLabels,
  modifierLabel,
  toolHint,
  type ShortcutAction,
} from './shortcuts';

/** Overview sections, in display order. */
export type ShortcutSection = 'file' | 'edit' | 'select' | 'view' | 'tools' | 'frames';

export const SECTION_ORDER: readonly ShortcutSection[] = [
  'file',
  'edit',
  'select',
  'view',
  'tools',
  'frames',
];

/** What a mode contributes: the actions its key handler runs, its tool rail. */
export interface ShortcutSource {
  actions: readonly ShortcutAction[];
  groups: readonly ToolGroup[];
}

/** One line of the overview. */
export interface ShortcutRow {
  /** `undo`, `tool:lasso`, `gesture:spacePan`, … */
  id: string;
  section: ShortcutSection;
  label: string;
  /** Display chords, the primary one first. */
  chords: string[];
  /** Extra text, e.g. that a repeated key cycles the variants. */
  note?: string;
}

// Actions without an obvious menu sit with the related one: colors and
// brush size with the tools, the overview itself with View.
const ACTION_SECTION: Record<ShortcutAction, ShortcutSection> = {
  open: 'file',
  save: 'file',
  saveAs: 'file',
  export: 'file',
  exportSheet: 'file',
  undo: 'edit',
  redo: 'edit',
  cut: 'edit',
  copy: 'edit',
  paste: 'edit',
  pasteNewImage: 'edit',
  deleteSelection: 'edit',
  selectAll: 'select',
  deselect: 'select',
  invertSelection: 'select',
  zoomIn: 'view',
  zoomOut: 'view',
  zoomFit: 'view',
  zoomActual: 'view',
  shortcuts: 'view',
  swapColors: 'tools',
  resetColors: 'tools',
  brushSmaller: 'tools',
  brushLarger: 'tools',
  prevFrame: 'frames',
  nextFrame: 'frames',
  togglePlay: 'frames',
};

/** Row label per action; a `Record`, so a new action without one fails `pnpm check`. */
export const ACTION_LABEL: Record<ShortcutAction, MessageKey> = {
  open: 'menu.open',
  save: 'menu.save',
  saveAs: 'menu.saveAs',
  export: 'keys.export',
  exportSheet: 'keys.exportSheet',
  undo: 'menu.undo',
  redo: 'menu.redo',
  cut: 'menu.cut',
  copy: 'menu.copy',
  paste: 'menu.paste',
  pasteNewImage: 'menu.pasteNewImage',
  deleteSelection: 'menu.deleteSelection',
  selectAll: 'menu.selectAll',
  deselect: 'menu.deselect',
  invertSelection: 'menu.invertSelection',
  zoomIn: 'menu.zoomIn',
  zoomOut: 'menu.zoomOut',
  zoomFit: 'menu.zoomFit',
  zoomActual: 'menu.zoomActual',
  shortcuts: 'menu.shortcuts',
  swapColors: 'menu.swapColors',
  resetColors: 'menu.defaultColors',
  brushSmaller: 'keys.brushSmaller',
  brushLarger: 'keys.brushLarger',
  prevFrame: 'menu.prevFrame',
  nextFrame: 'menu.nextFrame',
  togglePlay: 'keys.togglePlay',
};

export const SECTION_LABEL: Record<ShortcutSection, MessageKey> = {
  file: 'keys.section.file',
  edit: 'keys.section.edit',
  select: 'keys.section.select',
  view: 'keys.section.view',
  tools: 'keys.section.tools',
  frames: 'keys.section.frames',
};

/** The pointer gestures both modes support (spec §9.4). */
function gestureRows(): ShortcutRow[] {
  const drag = t('keys.drag');
  const shift = modifierLabel('shift');
  const alt = modifierLabel('alt');
  return [
    {
      id: 'gesture:spacePan',
      section: 'view',
      label: t('keys.gesture.pan'),
      chords: [`${t('keys.space')} + ${drag}`],
    },
    {
      id: 'gesture:touch',
      section: 'view',
      label: t('keys.gesture.touch'),
      chords: [t('keys.gesture.touchKeys')],
    },
    {
      id: 'gesture:selectAdd',
      section: 'select',
      label: t('keys.gesture.selectAdd'),
      chords: [`${shift} + ${drag}`],
    },
    {
      id: 'gesture:selectSubtract',
      section: 'select',
      label: t('keys.gesture.selectSubtract'),
      chords: [`${alt} + ${drag}`],
    },
    {
      id: 'gesture:selectIntersect',
      section: 'select',
      label: t('keys.gesture.selectIntersect'),
      chords: [`${shift} + ${alt} + ${drag}`],
    },
  ];
}

/** Every row of `source`, sorted into `SECTION_ORDER`. */
export function shortcutList(source: ShortcutSource): ShortcutRow[] {
  const rows: ShortcutRow[] = source.actions.map((action) => ({
    id: action,
    section: ACTION_SECTION[action],
    label: t(ACTION_LABEL[action]),
    chords: chordLabels(action),
  }));
  for (const g of source.groups) {
    const key = toolHint(g.id);
    if (key === undefined || TOOL_GROUP_KEYS[g.id] === undefined) continue;
    const row: ShortcutRow = { id: `tool:${g.id}`, section: 'tools', label: g.label, chords: [key] };
    if (g.tools.length > 1) {
      row.note = t('keys.cycle', { tools: g.tools.map((tool) => tool.label).join(', ') });
    }
    rows.push(row);
  }
  rows.push(...gestureRows());
  // Stable sort: rows keep their order within a section.
  return rows.sort((a, b) => SECTION_ORDER.indexOf(a.section) - SECTION_ORDER.indexOf(b.section));
}

/** Rows whose label contains `query`, ignoring case; all rows for a blank query. */
export function filterRows(rows: readonly ShortcutRow[], query: string): ShortcutRow[] {
  const q = query.trim().toLocaleLowerCase();
  if (q === '') return [...rows];
  return rows.filter((r) => r.label.toLocaleLowerCase().includes(q));
}

/** `rows` split by section, in `SECTION_ORDER`, leaving out empty sections. */
export function groupRows(
  rows: readonly ShortcutRow[],
): { section: ShortcutSection; rows: ShortcutRow[] }[] {
  return SECTION_ORDER.map((section) => ({
    section,
    rows: rows.filter((r) => r.section === section),
  })).filter((s) => s.rows.length > 0);
}
