// Changing, resetting and storing the custom keymap
// (docs/specs/power-user-shortcuts-keymap.md). The overrides live in
// `keymap-state.svelte.ts`; `shortcuts.ts` merges them over the defaults.
// Stored chords use the platform-neutral `mod`, so one keymap works on
// every OS.

import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';
import { keymapState, overridesSnapshot, type Keymap, type KeymapId } from './keymap-state.svelte';
import { KEYMAP_IDS, effectiveChords, sameChord, type Chord } from './shortcuts';

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
  const c: Chord = { key: v.key };
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
