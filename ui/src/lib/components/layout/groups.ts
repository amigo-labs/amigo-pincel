// The sidebar's fixed tab groups and the stored frame layout, as plain
// functions (docs/specs/sidebar-tab-groups.md). No runes here, so a
// Node-side test can import it.

/** Offset of the effect panel's top-right corner from the canvas area's
 *  top-right corner, in px: `x` to the left, `y` down (effect-panel AC-18). */
export interface EffectPos {
  x: number;
  y: number;
}

/** What a group stores: its active tab and whether it is collapsed. */
export interface GroupState {
  active: string;
  collapsed: boolean;
}

/** Persisted chrome layout of one editor mode. */
export interface FrameLayout {
  /** Collapse state of single panels, by panel id. A panel inside a group
   *  ignores its entry (a layout stored before the groups may hold one). */
  collapsed: Record<string, boolean>;
  /** Docked sidebar shown (wide windows). */
  sidebar: boolean;
  /** Docked sidebar width in px as the user last set it; `undefined` =
   *  default. Kept as stored even when the window clamps it (effect-panel
   *  AC-11) — `sidebarWidth()` gives the width actually shown. */
  sidebarWidth?: number;
  /** Effect panel position as the user last put it; `undefined` = default.
   *  Kept as stored even when the canvas area clamps it (effect-panel
   *  AC-19) — `effectPanelPos()` gives the position actually shown. */
  effectPos?: EffectPos;
  /** Active tab and collapse state of the tab groups, by group id; a group
   *  without an entry is in its first-use state. */
  groups?: Record<string, GroupState>;
}

/** One fixed tab group of the sidebar. */
export interface GroupDef {
  /** The tab that is active on first use. */
  first: string;
  /** Collapsed on first use. */
  collapsed: boolean;
  /** Shares the sidebar height the other blocks leave (AC-13); a group that
   *  does not grow is as high as its panel's content. */
  grow: boolean;
}

/** The groups and their first-use state (sidebar-tab-groups AC-07). A panel
 *  joins one through `PanelDef.group`; the editors list the panels. */
export const PANEL_GROUPS: Readonly<Record<string, GroupDef>> = {
  /** Layers | History */
  document: { first: 'layers', collapsed: false, grow: true },
  /** Preview | Tilesets | Slices */
  assets: { first: 'preview', collapsed: true, grow: false },
  /** Generate | Assistant (desktop app) */
  ai: { first: 'assistant', collapsed: false, grow: true },
};

/**
 * The state of `group` in `layout`, for the panels the group has on this
 * platform (`panelIds`, in tab order): the stored state, or the first-use
 * one. A stored active tab the group does not have falls back to the
 * first-use tab, or to the first panel where that is missing too (AC-10).
 */
export function groupState(
  layout: Pick<FrameLayout, 'groups'>,
  group: string,
  panelIds: readonly string[],
): GroupState {
  const def = PANEL_GROUPS[group];
  const stored = layout.groups?.[group];
  const fallback = def && panelIds.includes(def.first) ? def.first : (panelIds[0] ?? '');
  return {
    active: stored && panelIds.includes(stored.active) ? stored.active : fallback,
    collapsed: stored ? stored.collapsed : (def?.collapsed ?? false),
  };
}

function isEffectPos(v: unknown): v is EffectPos {
  if (typeof v !== 'object' || v === null) return false;
  const { x, y } = v as Record<string, unknown>;
  return typeof x === 'number' && Number.isFinite(x) && typeof y === 'number' && Number.isFinite(y);
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/**
 * Reads a stored layout: every field is taken only when it has the right
 * shape, anything else falls back to its default, and a value that is no
 * layout at all gives the first-use layout (AC-10). A layout stored before
 * the groups has no `groups`, so its groups start in their first-use state
 * while single panels keep their collapse state (AC-09).
 */
export function normalizeLayout(raw: unknown): FrameLayout {
  const layout: FrameLayout = { collapsed: {}, sidebar: true };
  if (!isRecord(raw)) return layout;
  if (isRecord(raw.collapsed)) {
    for (const [id, value] of Object.entries(raw.collapsed)) {
      if (typeof value === 'boolean') layout.collapsed[id] = value;
    }
  }
  layout.sidebar = raw.sidebar !== false;
  const width = raw.sidebarWidth;
  if (typeof width === 'number' && Number.isFinite(width)) layout.sidebarWidth = width;
  if (isEffectPos(raw.effectPos)) layout.effectPos = { x: raw.effectPos.x, y: raw.effectPos.y };
  if (isRecord(raw.groups)) {
    const groups: Record<string, GroupState> = {};
    for (const [id, value] of Object.entries(raw.groups)) {
      if (isRecord(value) && typeof value.active === 'string' && typeof value.collapsed === 'boolean') {
        groups[id] = { active: value.active, collapsed: value.collapsed };
      }
    }
    if (Object.keys(groups).length > 0) layout.groups = groups;
  }
  return layout;
}
