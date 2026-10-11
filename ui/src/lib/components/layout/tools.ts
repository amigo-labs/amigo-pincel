// Tool-rail model (spec §9.2). Both modes list their tools as groups: the
// rail shows one button per group, and a group with variants (shapes,
// marquee shapes, lasso kinds) offers them in the options bar through
// `ToolVariants`. Ids are each mode's own tool ids.

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
  shortcut?: string;
  tools: ToolDef<T>[];
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
