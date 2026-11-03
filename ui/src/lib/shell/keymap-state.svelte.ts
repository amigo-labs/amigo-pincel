// The user's keymap overrides (docs/specs/power-user-shortcuts-keymap.md),
// kept apart from `shortcuts.ts` so that module stays plain TypeScript and
// this one imports nothing but types: `shortcuts.ts` reads the overrides,
// `keymap.ts` changes and stores them.

import type { Chord, ShortcutAction } from './shortcuts';

/** A rebindable entry: a shortcut action or a tool group (`tool:lasso`). */
export type KeymapId = ShortcutAction | `tool:${string}`;

/** User overrides on top of the defaults; an empty array means unbound. */
export type Keymap = Partial<Record<KeymapId, Chord[]>>;

/** Reactive, so menus, tooltips and the overview re-render on a change. */
export const keymapState = $state<{ overrides: Keymap }>({ overrides: {} });

/** The overrides as a plain object (IndexedDB and JSON can't take a proxy). */
export function overridesSnapshot(): Keymap {
  return $state.snapshot(keymapState.overrides) as Keymap;
}
