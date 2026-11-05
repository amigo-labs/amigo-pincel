// Tool-rail model (spec §9.2). Both modes list their tools as groups: the
// rail shows one button per group, and a group with variants (shapes,
// marquee shapes, lasso kinds) offers them in the options bar through
// `ToolVariants` and in the rail button's flyout (`ToolFlyout`). Ids are
// each mode's own tool ids.

import type { IconName } from '../icons/icons';

/** One selectable tool. */
export interface ToolDef<T extends string = string> {
  id: T;
  label: string;
  icon: IconName;
  disabled?: boolean;
  /** Tooltip override (e.g. why a tool is disabled). */
  title?: string;
}

/** A rail button: one tool, or several variants of one tool. */
export interface ToolGroup<T extends string = string> {
  id: string;
  label: string;
  /** Single-key shortcut, shown in the tooltip. */
  shortcut?: string | undefined;
  tools: ToolDef<T>[];
}

/**
 * Rail sections (spec §9.2): the rail draws a separator wherever the
 * section changes, so related tools read as one block. Keyed by group id,
 * which both modes share; a group missing here starts no new section.
 */
const RAIL_SECTIONS: Record<string, string> = {
  move: 'navigate',
  hand: 'navigate',
  zoom: 'navigate',
  select: 'select',
  lasso: 'select',
  wand: 'select',
  pencil: 'paint',
  eraser: 'paint',
  recolor: 'paint',
  smudge: 'paint',
  fill: 'color',
  gradient: 'color',
  eyedropper: 'color',
  line: 'draw',
  shapes: 'draw',
  text: 'draw',
  stamp: 'sprite',
  slice: 'sprite',
};

/** Whether the rail separates group `id` from the group before it, `prev`. */
export function startsSection(prev: string | undefined, id: string): boolean {
  if (prev === undefined) return false;
  const a = RAIL_SECTIONS[prev];
  const b = RAIL_SECTIONS[id];
  return a !== undefined && b !== undefined && a !== b;
}

/** The group `tool` belongs to. */
export function groupOf<T extends string>(groups: ToolGroup<T>[], tool: T): ToolGroup<T> | undefined {
  return groups.find((g) => g.tools.some((t) => t.id === tool));
}

/**
 * The tool a press of group `groupId`'s key selects from `current`: the
 * group's first enabled tool, or — when `current` is already in it — the
 * next enabled variant (repeated presses cycle, the Aseprite pattern).
 */
export function cycleGroup<T extends string>(
  groups: ToolGroup<T>[],
  groupId: string,
  current: T,
): T | null {
  const g = groups.find((x) => x.id === groupId);
  if (!g) return null;
  const enabled = g.tools.filter((t) => !t.disabled);
  if (enabled.length === 0) return null;
  const at = enabled.findIndex((t) => t.id === current);
  return (enabled[(at + 1) % enabled.length] ?? enabled[0])?.id ?? null;
}

/** Hold time (ms) after which a press on a group button opens its flyout. */
export const LONG_PRESS_MS = 500;

/** Movement (CSS px) that turns a press into a drag and cancels the long press. */
export const LONG_PRESS_SLOP = 10;

/**
 * Whether group `g`'s rail button opens a variant flyout (right-click,
 * long press, Shift+F10): it has several variants and at least one of
 * them is enabled.
 */
export function hasFlyout(g: ToolGroup): boolean {
  return g.tools.length > 1 && g.tools.some((t) => !t.disabled);
}

/**
 * The tool a short press on group `g`'s rail button selects: the variant
 * the button shows, or the group's first enabled one when that is disabled.
 */
export function pressTarget<T extends string>(g: ToolGroup<T>, shown: ToolDef<T>): T {
  if (!shown.disabled) return shown.id;
  return g.tools.find((t) => !t.disabled)?.id ?? shown.id;
}

/** A keyboard move inside the flyout. */
export type FlyoutMove = 'next' | 'prev' | 'first' | 'last';

/**
 * The enabled variant a flyout key moves to from `from`: Arrow Down /
 * Up step to the next / previous enabled one and wrap around, Home / End
 * go to the first / last enabled one. `null` when none is enabled.
 */
export function flyoutStep<T extends string>(tools: ToolDef<T>[], from: T | null, move: FlyoutMove): T | null {
  const enabled = tools.filter((t) => !t.disabled);
  if (enabled.length === 0) return null;
  if (move === 'first') return enabled[0]?.id ?? null;
  if (move === 'last') return enabled[enabled.length - 1]?.id ?? null;
  const at = enabled.findIndex((t) => t.id === from);
  if (at < 0) return (move === 'next' ? enabled[0] : enabled[enabled.length - 1])?.id ?? null;
  const step = move === 'next' ? 1 : -1;
  return enabled[(at + step + enabled.length) % enabled.length]?.id ?? null;
}

/**
 * The flyout entry that takes focus when it opens: the active tool when
 * it belongs to the group and is enabled, else the first enabled variant.
 */
export function flyoutFocus<T extends string>(g: ToolGroup<T>, active: T): T | null {
  const own = g.tools.find((t) => t.id === active && !t.disabled);
  return own?.id ?? flyoutStep(g.tools, null, 'first');
}

/** A screen rectangle in CSS px (the parts of a `DOMRect` placement needs). */
export interface ScreenBox {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Where the flyout goes: its top-left corner and the most height it may take. */
export interface FlyoutPlacement {
  left: number;
  top: number;
  maxHeight: number;
}

/** Space (CSS px) between the flyout and the rail, and between it and a window edge. */
const FLYOUT_GAP = 4;

/**
 * Places a flyout of natural `size` for `button` on `rail`: right of a
 * vertical rail with the top edges aligned; above a horizontal strip with
 * the left edges aligned. A position crossing a window edge is shifted
 * back inside; a flyout taller than the window is limited to it (its
 * entries scroll).
 */
export function placeFlyout(
  button: ScreenBox,
  rail: ScreenBox,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
  orientation: 'vertical' | 'horizontal',
): FlyoutPlacement {
  const maxHeight = Math.max(0, viewport.height - 2 * FLYOUT_GAP);
  const height = Math.min(size.height, maxHeight);
  const width = Math.min(size.width, Math.max(0, viewport.width - 2 * FLYOUT_GAP));
  const left = orientation === 'vertical' ? rail.right + FLYOUT_GAP : button.left;
  const top = orientation === 'vertical' ? button.top : rail.top - FLYOUT_GAP - height;
  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, hi));
  return {
    left: clamp(left, FLYOUT_GAP, viewport.width - FLYOUT_GAP - width),
    top: clamp(top, FLYOUT_GAP, viewport.height - FLYOUT_GAP - height),
    maxHeight,
  };
}
