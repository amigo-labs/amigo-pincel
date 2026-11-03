// Changing, resetting and storing the custom keymap
// (docs/specs/power-user-shortcuts-keymap.md). The overrides live in
// `keymap-state.svelte.ts`; `shortcuts.ts` merges them over the defaults.
// Stored chords use the platform-neutral `mod`, so one keymap works on
// every OS.

import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { keymapState, overridesSnapshot, type Keymap, type KeymapId } from './keymap-state.svelte';
import { KEYMAP_IDS, defaultChords, effectiveChords, sameChord, type Chord } from './shortcuts';

/** IndexedDB pref holding the overrides. */
export const KEYMAP_PREF = 'keymap';

/** Writes the overrides, fire-and-forget: without IndexedDB they last the session. */
function persist(): void {
  if (!isIdbAvailable()) return;
  setPref(KEYMAP_PREF, overridesSnapshot()).catch((err: unknown) => {
    console.error('setPref keymap failed', err);
  });
}

/** Loads the stored overrides (once, at start-up). Ignores what isn't a keymap. */
export async function loadKeymap(): Promise<void> {
  if (!isIdbAvailable()) return;
  try {
    const stored = await getPref(KEYMAP_PREF);
    const parsed = stored === undefined ? null : sanitize(stored);
    if (parsed !== null) keymapState.overrides = parsed;
  } catch (err) {
    console.error('getPref keymap failed', err);
  }
}

/** The entry other than `except` that answers to `chord`, if any. */
export function conflictFor(chord: Chord, except: KeymapId): KeymapId | null {
  for (const id of KEYMAP_IDS) {
    if (id === except) continue;
    if (effectiveChords(id).some((c) => sameChord(c, chord))) return id;
  }
  return null;
}

/** True when `chord` is already `id`'s primary chord. */
export function isCurrentChord(id: KeymapId, chord: Chord): boolean {
  const first = effectiveChords(id)[0];
  return first !== undefined && sameChord(first, chord);
}

/**
 * Binds `chord` as `id`'s only chord (alternates are dropped). Any other
 * entry that answered to it loses it — the Replace of a conflict.
 */
export function bind(id: KeymapId, chord: Chord): void {
  const next: Keymap = { ...overridesSnapshot() };
  for (const other of KEYMAP_IDS) {
    if (other === id) continue;
    const chords = effectiveChords(other);
    if (chords.some((c) => sameChord(c, chord))) {
      next[other] = chords.filter((c) => !sameChord(c, chord));
    }
  }
  next[id] = [chord];
  keymapState.overrides = next;
  persist();
}

/** Restores `id`'s default chords. */
export function resetBinding(id: KeymapId): void {
  const next: Keymap = { ...overridesSnapshot() };
  delete next[id];
  keymapState.overrides = next;
  persist();
}

/** Restores every default. */
export function resetAllBindings(): void {
  keymapState.overrides = {};
  persist();
}

/** True when `id` differs from its defaults. */
export function isOverridden(id: KeymapId): boolean {
  return keymapState.overrides[id] !== undefined;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** `v` as a stored chord, or `null` when it isn't one. */
export function asChord(v: unknown): Chord | null {
  if (!isRecord(v) || typeof v.key !== 'string' || v.key === '') return null;
  const c: Chord = { key: v.key.length === 1 ? v.key.toLowerCase() : v.key };
  for (const flag of ['mod', 'shift', 'alt'] as const) {
    const f = v[flag];
    if (f === undefined) continue;
    if (typeof f !== 'boolean') return null;
    if (f) c[flag] = true;
  }
  return c;
}

/** A stored keymap with unknown ids and malformed chords dropped; `null` if not an object. */
function sanitize(v: unknown): Keymap | null {
  if (!isRecord(v)) return null;
  const out: Keymap = {};
  for (const id of KEYMAP_IDS) {
    const chords = v[id];
    if (!Array.isArray(chords)) continue;
    const parsed = chords.map(asChord);
    if (parsed.every((c): c is Chord => c !== null)) out[id] = parsed;
  }
  return out;
}

/** Marks a keymap file; `version` changes only with an incompatible format. */
const KEYMAP_FORMAT = 'pincel-keymap';
const KEYMAP_VERSION = 1;

/** The overrides as a keymap file (JSON): only what differs from the defaults. */
export function exportKeymapJson(): string {
  return `${JSON.stringify(
    { format: KEYMAP_FORMAT, version: KEYMAP_VERSION, bindings: overridesSnapshot() },
    null,
    2,
  )}\n`;
}

/** What an import did, or why it changed nothing. */
export type ImportResult =
  | { ok: true; ignored: number; replaced: number }
  | { ok: false; reason: 'invalid' }
  | { ok: false; reason: 'duplicate'; chord: Chord };

/**
 * Makes the overrides of a keymap file the effective keymap. Unknown
 * entries are skipped and counted; a chord the file binds to two entries
 * rejects it; a default the file doesn't name loses any chord the file
 * binds, as with Replace. A rejected file leaves the keymap unchanged.
 */
export function importKeymapText(text: string): ImportResult {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return { ok: false, reason: 'invalid' };
  }
  if (
    !isRecord(data) ||
    data.format !== KEYMAP_FORMAT ||
    data.version !== KEYMAP_VERSION ||
    !isRecord(data.bindings)
  ) {
    return { ok: false, reason: 'invalid' };
  }
  const known = new Set<string>(KEYMAP_IDS);
  const next: Keymap = {};
  let ignored = 0;
  for (const [id, value] of Object.entries(data.bindings)) {
    if (!Array.isArray(value)) return { ok: false, reason: 'invalid' };
    const chords = value.map(asChord);
    if (!chords.every((c): c is Chord => c !== null)) return { ok: false, reason: 'invalid' };
    if (!known.has(id)) {
      ignored += 1;
      continue;
    }
    next[id as KeymapId] = chords.filter((c, i) => chords.findIndex((d) => sameChord(c, d)) === i);
  }
  const bound: { id: KeymapId; chord: Chord }[] = [];
  for (const [id, chords] of Object.entries(next) as [KeymapId, Chord[]][]) {
    for (const chord of chords) {
      if (bound.some((b) => b.id !== id && sameChord(b.chord, chord))) {
        return { ok: false, reason: 'duplicate', chord };
      }
      bound.push({ id, chord });
    }
  }
  let replaced = 0;
  for (const id of KEYMAP_IDS) {
    if (next[id] !== undefined) continue;
    const defaults = defaultChords(id);
    const kept = defaults.filter((c) => !bound.some((b) => sameChord(b.chord, c)));
    if (kept.length !== defaults.length) {
      next[id] = kept;
      replaced += 1;
    }
  }
  keymapState.overrides = next;
  persist();
  return { ok: true, ignored, replaced };
}
