// The recently painted-with colors of each mode (color-picker AC-12,
// AC-14, AC-24), shown in the picker and in Image mode's Colors panel.
// Kept per mode in the IndexedDB prefs `recentColors:<mode>` (never
// localStorage, AGENTS.md §9); not document state, so no undo entries.

import { pushRecent, readColor, readRecent, toHex, type Rgba8 } from '../color';
import { getPref, setPref } from '../idb/prefs';

/** Editor mode a recent-colors list belongs to. */
export type ColorMode = 'pixel' | 'image';

/** Most recent first, at most `RECENT_MAX` per mode. */
export const recentColors = $state<Record<ColorMode, Rgba8[]>>({ pixel: [], image: [] });

/** Whether a picker interaction (a drag) of each mode is still under way:
 *  the tools paint with its color live, but the last-color prefs wait for
 *  its end — one commit per interaction (color-picker spec). */
export const colorDraft = $state<Record<ColorMode, boolean>>({ pixel: false, image: false });

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

/** A stored foreground / background color (`#rrggbb[aa]`), or `null` when
 *  it is missing, unreadable or the store is unavailable (AC-15, AC-30). */
export async function loadColorPref(key: string): Promise<Rgba8 | null> {
  try {
    return readColor(await getPref(key));
  } catch {
    return null;
  }
}

/** Stores a foreground / background color with its alpha; best effort. */
export function saveColorPref(key: string, c: Rgba8): void {
  void setPref(key, toHex(c, true)).catch(() => {});
}
