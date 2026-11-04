// Sidebar / bottom-dock panel model and persisted layout (spec §9.2:
// "Right-side panels are … collapsible. Layout state persists per-document
// type."). Editors describe their panels as `PanelDef`s; `EditorFrame`
// hosts them (docked sidebar, drawer, or phone bottom sheet) and hands
// each one a `PanelSlot` through context, which `Panel.svelte` reads.

import { getContext, setContext, type Snippet } from 'svelte';
import { getPref, setPref } from '../../idb/prefs';
import type { IconName } from '../icons/icons';

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
  /** Starts collapsed until the user expands it (rarely used panels). */
  defaultCollapsed?: boolean;
}

/** Where and how a panel is currently shown. */
export interface PanelSlot {
  readonly title: string;
  /** `sheet` panels are tabs of the phone bottom sheet: no collapse. */
  readonly host: 'side' | 'bottom' | 'sheet';
  readonly collapsed: boolean;
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

/** Persisted chrome layout of one editor mode. */
export interface FrameLayout {
  collapsed: Record<string, boolean>;
  /** Docked sidebar shown (wide windows). */
  sidebar: boolean;
  /** Docked sidebar width in px as the user last set it; `undefined` =
   *  default. Kept as stored even when the window clamps it (effect-panel
   *  AC-11) — `sidebarWidth()` gives the width actually shown. */
  sidebarWidth?: number;
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

/** Load the stored layout for `mode`, or defaults. */
export async function loadLayout(mode: string): Promise<FrameLayout> {
  const fallback: FrameLayout = { collapsed: {}, sidebar: true };
  try {
    const v = (await getPref(prefKey(mode))) as Partial<FrameLayout> | undefined;
    if (!v || typeof v !== 'object') return fallback;
    const width = v.sidebarWidth;
    return {
      collapsed: typeof v.collapsed === 'object' && v.collapsed ? { ...v.collapsed } : {},
      sidebar: v.sidebar !== false,
      ...(typeof width === 'number' && Number.isFinite(width) ? { sidebarWidth: width } : {}),
    };
  } catch {
    // IndexedDB unavailable (private mode): run on defaults.
    return fallback;
  }
}

/** Store the layout for `mode`; failures are ignored (prefs are best effort). */
export function saveLayout(mode: string, layout: FrameLayout): void {
  void setPref(prefKey(mode), $state.snapshot(layout)).catch(() => {});
}
