// Sidebar / bottom-dock panel model and persisted layout (spec §9.2:
// "Right-side panels are … collapsible. Layout state persists per-document
// type."). Editors describe their panels as `PanelDef`s; `EditorFrame`
// hosts them (docked sidebar, drawer, or phone bottom sheet) and hands
// each one a `PanelSlot` through context, which `Panel.svelte` reads.

import { getContext, setContext, untrack, type Snippet } from 'svelte';
import { getPref, setPref } from '../../idb/prefs';
import { t } from '../../i18n/index.svelte';
import type { IconName } from '../icons/icons';
import { normalizeLayout, type EffectPos, type FrameLayout } from './groups';
import type { MenuItem, MenuSubmenu } from './menu';

export type { EffectPos, FrameLayout, GroupState } from './groups';

/** One panel an editor offers. */
export interface PanelDef {
  id: string;
  title: string;
  icon: IconName;
  body: Snippet;
  /** `side` (default) docks in the sidebar; `bottom` under the canvas. */
  placement?: 'side' | 'bottom';
  /** Grows to fill spare sidebar height (long lists such as Layers). */
  grow?: boolean;
  /** Starts collapsed until the user expands it (rarely used panels). A
   *  panel inside a group follows its group instead. */
  defaultCollapsed?: boolean;
  /** The tab group the panel belongs to (`PANEL_GROUPS` in `groups.ts`);
   *  consecutive panels of one group share a block with a tab per panel
   *  (sidebar-tab-groups AC-03). Without one the panel is a block of its own. */
  group?: string;
}

/** Where and how a panel is currently shown. */
export interface PanelSlot {
  readonly title: string;
  /** `sheet` panels are tabs of the phone bottom sheet: no collapse. */
  readonly host: 'side' | 'bottom' | 'sheet';
  readonly collapsed: boolean;
  /** The panel is a tab of a group: the group draws the header, and while
   *  `collapsed` (not the active tab, or the group is collapsed) the panel
   *  is not on the page at all (sidebar-tab-groups AC-03, AC-06). */
  readonly grouped?: boolean;
  /** The user removed the panel (View ▸ Panels): it stays mounted, so it
   *  keeps what it showed, and draws nothing (panel-visibility AC-14). */
  readonly removed?: boolean;
  toggle(): void;
}

const SLOT = Symbol('pc-panel-slot');

/** Provide the slot for the panel rendered inside the caller. */
export function providePanelSlot(slot: PanelSlot): void {
  setContext(SLOT, slot);
}

/** The slot of the enclosing frame panel, if any. */
export function panelSlot(): PanelSlot | undefined {
  return getContext<PanelSlot | undefined>(SLOT);
}

/** The last request to show a panel of one mode's editor frame
 *  (comfyui-generation-info AC-05); `n` counts the requests. */
export const panelRequest = $state({ mode: '', id: '', n: 0 });

/** Expands panel `id` of `mode`'s editor frame and brings it into view
 *  (sidebar, drawer or bottom sheet). */
export function expandPanel(mode: string, id: string): void {
  panelRequest.mode = mode;
  panelRequest.id = id;
  panelRequest.n += 1;
}

// View ▸ Panels (panel-visibility AC-01): the editors build their menus,
// the frame knows the panels and the layout, so each mode's frame publishes
// the submenu's entries here and the editors place the submenu.
let panelMenus = $state.raw<Record<string, MenuItem[]>>({});

/** Publishes the View ▸ Panels entries of `mode`'s editor frame; `null`
 *  when the frame goes away. */
export function setPanelMenu(mode: string, items: MenuItem[] | null): void {
  const rest = { ...untrack(() => panelMenus) };
  if (items === null) delete rest[mode];
  else rest[mode] = items;
  panelMenus = rest;
}

/** The View ▸ Panels submenu of `mode`: a check mark per panel, then
 *  Reset Panels. */
export function panelsSubmenu(mode: string): MenuSubmenu {
  return { kind: 'submenu', label: t('frame.panels'), items: panelMenus[mode] ?? [] };
}

/** Default effect panel offset: 16 px inside the canvas area's top-right corner. */
export const EFFECT_PANEL_MARGIN = 16;

/** Width and height in px. */
export interface Size {
  width: number;
  height: number;
}

/** The position to show for a stored `pos`: the default when missing,
 *  else the nearest one that keeps a `panel`-sized box wholly inside
 *  `area` (effect-panel AC-19). */
export function effectPanelPos(pos: EffectPos | undefined, area: Size, panel: Size): EffectPos {
  const p = pos ?? { x: EFFECT_PANEL_MARGIN, y: EFFECT_PANEL_MARGIN };
  const clamp = (v: number, max: number) => Math.round(Math.min(Math.max(0, max), Math.max(0, v)));
  return { x: clamp(p.x, area.width - panel.width), y: clamp(p.y, area.height - panel.height) };
}

/** Default docked sidebar width in px (effect-panel AC-08). */
export const SIDEBAR_DEFAULT_WIDTH = 288;
/** Narrowest docked sidebar in px. */
export const SIDEBAR_MIN_WIDTH = 240;
/** Widest docked sidebar in px, unless half the window is less. */
export const SIDEBAR_MAX_WIDTH = 560;

/** Widest sidebar `windowWidth` allows: 560 px or half the window. */
export function sidebarMaxWidth(windowWidth: number): number {
  return Math.max(SIDEBAR_MIN_WIDTH, Math.min(SIDEBAR_MAX_WIDTH, Math.floor(windowWidth / 2)));
}

/** The width to show for a stored `width` in a `windowWidth` window:
 *  the default when missing or not finite, else clamped to the limits. */
export function sidebarWidth(width: number | undefined, windowWidth: number): number {
  const w = width !== undefined && Number.isFinite(width) ? width : SIDEBAR_DEFAULT_WIDTH;
  return Math.round(Math.min(sidebarMaxWidth(windowWidth), Math.max(SIDEBAR_MIN_WIDTH, w)));
}

function prefKey(mode: string): string {
  return `layout:${mode}`;
}

/** Load the stored layout for `mode`, or the first-use layout. */
export async function loadLayout(mode: string): Promise<FrameLayout> {
  try {
    return normalizeLayout(await getPref(prefKey(mode)));
  } catch {
    // IndexedDB unavailable (private mode): run on defaults.
    return normalizeLayout(undefined);
  }
}

/** Store the layout for `mode`; failures are ignored (prefs are best effort). */
export function saveLayout(mode: string, layout: FrameLayout): void {
  void setPref(prefKey(mode), $state.snapshot(layout)).catch(() => {});
}
