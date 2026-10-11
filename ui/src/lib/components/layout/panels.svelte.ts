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
    return {
      collapsed: typeof v.collapsed === 'object' && v.collapsed ? { ...v.collapsed } : {},
      sidebar: v.sidebar !== false,
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
