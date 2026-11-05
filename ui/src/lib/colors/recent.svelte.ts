// The recently painted-with colors of each mode (color-picker AC-12,
// AC-14, AC-24), shown in the picker and in Image mode's Colors panel.
// Kept per mode in the IndexedDB prefs `recentColors:<mode>` (never
// localStorage, AGENTS.md §9); not document state, so no undo entries.

import { pushRecent, readRecent, toHex, type Rgba8 } from '../color';
import { getPref, setPref } from '../idb/prefs';

/** Editor mode a recent-colors list belongs to. */
export type ColorMode = 'pixel' | 'image';

/** Most recent first, at most `RECENT_MAX` per mode. */
export const recentColors = $state<Record<ColorMode, Rgba8[]>>({ pixel: [], image: [] });

const key = (mode: ColorMode): string => `recentColors:${mode}`;

/** Records `used` (foreground first) as painted with in `mode` and stores
 *  the list; storing is best effort. */
export function noteUsed(mode: ColorMode, used: readonly Rgba8[]): void {
  if (used.length === 0) return;
  const next = pushRecent(recentColors[mode], used);
  recentColors[mode] = next;
  void setPref(
    key(mode),
    next.map((c) => toHex(c, true)),
  ).catch(() => {});
}

/** Loads the stored list of `mode`; unreadable entries are skipped and
 *  an unavailable store leaves the list empty (color-picker AC-30). */
export async function loadRecent(mode: ColorMode): Promise<void> {
  try {
    recentColors[mode] = readRecent(await getPref(key(mode)));
  } catch {
    recentColors[mode] = [];
  }
}
