// The command palette's entries and search
// (docs/specs/power-user-shortcuts-palette.md). Entries come from the
// editor's own menu data and tool rail, so the palette can't list a
// command the menus don't have; queries match the label in the UI
// language and in English.

import type { MenuDef, MenuItem } from '../components/layout/menu';
import type { ToolGroup } from '../components/layout/tools';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { englishLabel, t } from '../i18n/index.svelte';

/** What the mounted editor offers the palette. */
export interface PaletteSource {
  /** The editor's menu bar, read when the palette opens. */
  menus: () => MenuDef[];
  /** The editor's tool rail. */
  groups: () => readonly ToolGroup[];
  /** Activates a tool, as a click on the tool rail does. */
  selectTool: (id: string) => void;
  /** "Go to frame…" (Pixel mode only): frame count and the jump (1-based). */
  frames?: { count: () => number; goto: (frame: number) => void };
}

/** One palette row. */
export interface PaletteEntry {
  /** Stable across languages: the English path where it has one. */
  id: string;
  /** `Layer ▸ Merge Down`, in the UI language. */
  label: string;
  /** The same path in English, if every part has an English text. */
  english: string | null;
  shortcut: string;
  disabled: boolean;
  /** `goto` asks for a frame number before it runs. */
  kind: 'command' | 'goto';
  run: () => void;
}

const SEP = ' ▸ ';

function flatten(
  items: readonly MenuItem[],
  path: string[],
  english: (string | null)[],
  disabled: boolean,
  out: PaletteEntry[],
): void {
  for (const item of items) {
    if (item.kind === 'separator') continue;
    const p = [...path, item.label];
    const e = [...english, englishLabel(item.label)];
    const off = disabled || Boolean(item.disabled);
    if (item.kind === 'submenu') {
      flatten(item.items, p, e, off, out);
      continue;
    }
    const en = e.every((x): x is string => x !== null) ? e.join(SEP) : null;
    out.push({
      id: en ?? p.join(SEP),
      label: p.join(SEP),
      english: en,
      shortcut: item.shortcut ?? '',
      disabled: off,
      kind: 'command',
      run: item.run,
    });
  }
}

/** Every menu command, every tool and (Pixel mode) "Go to frame…", in menu order. */
export function paletteEntries(source: PaletteSource): PaletteEntry[] {
  const out: PaletteEntry[] = [];
  for (const menu of source.menus()) {
    flatten(menu.items, [menu.label], [englishLabel(menu.label)], false, out);
  }
  for (const group of source.groups()) {
    for (const tool of group.tools) {
      const en = englishLabel(tool.label);
      out.push({
        id: `tool:${tool.id}`,
        label: t('cmd.tool', { name: tool.label }),
        english: en === null ? null : `Tool: ${en}`,
        shortcut: group.shortcut ?? '',
        disabled: Boolean(tool.disabled),
        kind: 'command',
        run: () => source.selectTool(tool.id),
      });
    }
  }
  if (source.frames) {
    out.push({
      id: 'goto-frame',
      label: t('cmd.gotoFrame'),
      english: 'Go to frame…',
      shortcut: '',
      disabled: false,
      kind: 'goto',
      run: () => {},
    });
  }
  return out;
}

/**
 * How well `query` fuzzy-matches `text` (higher is better), or `null`:
 * the query's characters (spaces ignored, any case) must appear in order.
 * Runs of adjacent characters, word starts, a plain substring and a
 * command name that starts with the query score up; long labels slightly down.
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLocaleLowerCase().replace(/\s+/g, '');
  if (q === '') return 0;
  const s = text.toLocaleLowerCase();
  let score = 0;
  let at = 0;
  let prev = -2;
  for (const ch of q) {
    const i = s.indexOf(ch, at);
    if (i < 0) return null;
    score += 1;
    if (i === prev + 1) score += 2;
    if (i === 0 || /[\s▸(]/.test(s[i - 1] ?? '')) score += 3;
    prev = i;
    at = i + 1;
  }
  const whole = query.trim().toLocaleLowerCase();
  if (s.includes(whole)) score += 10;
  // The command's own name (the last path part) starting with the query.
  if ((s.split(SEP).at(-1) ?? '').startsWith(whole)) score += 5;
  return score - s.length / 100;
}

/**
 * The entries to show: for an empty query all of them, the `recent` ids
 * first (most recent first); otherwise the matches, best first.
 */
export function filterEntries(
  entries: readonly PaletteEntry[],
  query: string,
  recent: readonly string[],
): PaletteEntry[] {
  if (query.trim() === '') {
    const rank = (e: PaletteEntry) => {
      const i = recent.indexOf(e.id);
      return i < 0 ? recent.length : i;
    };
    return [...entries].sort((a, b) => rank(a) - rank(b));
  }
  const scored: { e: PaletteEntry; score: number }[] = [];
  for (const e of entries) {
    const local = fuzzyScore(query, e.label);
    const english = e.english === null ? null : fuzzyScore(query, e.english);
    const best = Math.max(local ?? -Infinity, english ?? -Infinity);
    if (best > -Infinity) scored.push({ e, score: best });
  }
  return scored.sort((a, b) => b.score - a.score).map((x) => x.e);
}

/** How many recently run commands the palette remembers. */
export const RECENT_LIMIT = 5;
const RECENT_PREF = 'commandRecents';
let recentCache: string[] = [];
let recentLoaded = false;

/** The remembered commands, most recent first (loaded from IndexedDB once). */
export async function loadRecentCommands(): Promise<string[]> {
  if (recentLoaded || !isIdbAvailable()) return recentCache;
  try {
    const v = await getPref(RECENT_PREF);
    if (Array.isArray(v) && !recentLoaded) {
      recentCache = v.filter((x): x is string => typeof x === 'string').slice(0, RECENT_LIMIT);
    }
  } catch (err) {
    console.error('getPref commandRecents failed', err);
  }
  recentLoaded = true;
  return recentCache;
}

/** Remembers a command run from the palette; returns the new list. */
export function noteRecentCommand(id: string): string[] {
  recentCache = [id, ...recentCache.filter((x) => x !== id)].slice(0, RECENT_LIMIT);
  recentLoaded = true;
  if (isIdbAvailable()) {
    setPref(RECENT_PREF, [...recentCache]).catch((err: unknown) => {
      console.error('setPref commandRecents failed', err);
    });
  }
  return recentCache;
}
