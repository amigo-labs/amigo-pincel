// The one keyboard map both editors use (spec §9.4: "Keyboard shortcuts are
// defined in a single `shortcuts.ts` map"). Same key, same meaning in Pixel
// and Image mode; each editor binds the actions it supports. Menus and
// tooltips print their hints from here, so labels never drift from keys.

import { isMac } from '../platform';
import { keymapState, type KeymapId } from './keymap-state.svelte';

/** A key chord. `key` is matched case-insensitively against `KeyboardEvent.key`. */
export interface Chord {
  key: string;
  /** Ctrl on Windows / Linux, ⌘ on macOS. */
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

/** Editor actions with a shortcut. */
export type ShortcutAction =
  | 'open'
  | 'save'
  | 'saveAs'
  | 'export'
  | 'exportSheet'
  | 'undo'
  | 'redo'
  | 'cut'
  | 'copy'
  | 'paste'
  | 'pasteNewImage'
  | 'selectAll'
  | 'deselect'
  | 'invertSelection'
  | 'deleteSelection'
  | 'zoomIn'
  | 'zoomOut'
  | 'zoomFit'
  | 'zoomActual'
  | 'swapColors'
  | 'resetColors'
  | 'brushSmaller'
  | 'brushLarger'
  | 'prevFrame'
  | 'nextFrame'
  | 'togglePlay'
  | 'shortcuts';

/** Primary chord per action, plus alternates matched but not displayed. */
export const SHORTCUTS: Record<ShortcutAction, Chord[]> = {
  open: [{ key: 'o', mod: true }],
  save: [{ key: 's', mod: true }],
  saveAs: [{ key: 's', mod: true, shift: true }],
  export: [{ key: 'e', mod: true }],
  exportSheet: [{ key: 'e', mod: true, shift: true }],
  undo: [{ key: 'z', mod: true }],
  redo: [
    { key: 'z', mod: true, shift: true },
    { key: 'y', mod: true },
  ],
  // Handled in keydown, not by hijacking the `copy` / `cut` events: the
  // keydown is user activation for `navigator.clipboard.write` (spec §5.4).
  cut: [{ key: 'x', mod: true }],
  copy: [{ key: 'c', mod: true }],
  // Menu hint only: editors leave the key to the browser, whose `paste`
  // event carries the clipboard without a permission prompt.
  paste: [{ key: 'v', mod: true }],
  // No browser fires `paste` for this chord: the async Clipboard API reads it.
  pasteNewImage: [{ key: 'v', mod: true, alt: true }],
  selectAll: [{ key: 'a', mod: true }],
  deselect: [{ key: 'd', mod: true }],
  invertSelection: [{ key: 'i', mod: true, shift: true }],
  deleteSelection: [{ key: 'Delete' }, { key: 'Backspace' }],
  zoomIn: [{ key: '+' }, { key: '=' }],
  zoomOut: [{ key: '-' }, { key: '_' }],
  zoomFit: [{ key: '0' }],
  zoomActual: [{ key: '1' }],
  swapColors: [{ key: 'x' }],
  resetColors: [{ key: 'd' }],
  brushSmaller: [{ key: '[' }],
  brushLarger: [{ key: ']' }],
  prevFrame: [{ key: ',' }],
  nextFrame: [{ key: '.' }],
  togglePlay: [{ key: 'Enter' }],
  // Help ▸ Keyboard Shortcuts. `/` is matched without Shift, so layouts
  // that type it with Shift (German: Ctrl+Shift+7) reach it too.
  shortcuts: [{ key: '/', mod: true }],
};

/**
 * Tool-group keys (Aseprite-style where the two modes agree). Repeated
 * presses cycle a group's variants. Group ids are shared with the tool
 * rail (`ToolGroup.id`), so a mode only lists the groups it has.
 */
export const TOOL_GROUP_KEYS: Record<string, string> = {
  move: 'v',
  hand: 'h',
  zoom: 'z',
  select: 'm',
  lasso: 'q',
  wand: 'w',
  pencil: 'b',
  eraser: 'e',
  fill: 'g',
  gradient: 'n',
  eyedropper: 'i',
  recolor: 'r',
  smudge: 's',
  line: 'l',
  shapes: 'u',
  text: 't',
};

/**
 * Chords the native menu binds as accelerators (src-tauri/src/main.rs).
 * That menu exists on macOS only, so the editors leave these to it where
 * `hasNativeMenu()` holds and an action would otherwise fire twice; the web
 * build and the Windows / Linux desktop app handle them themselves.
 */
export const NATIVE_MENU_ACTIONS: ReadonlySet<ShortcutAction> = new Set<ShortcutAction>([
  'open',
  'save',
  'saveAs',
  'export',
  'exportSheet',
  'undo',
  'redo',
  'shortcuts',
]);

const IS_MAC = isMac();

function keyLabel(key: string): string {
  if (key === 'Delete') return 'Del';
  if (key === 'Enter') return '↵';
  return key.length === 1 ? key.toUpperCase() : key;
}

/** Human label of a chord: `Ctrl+Shift+S`, or `⇧⌘S` on macOS. */
export function chordLabel(c: Chord): string {
  if (IS_MAC) {
    return `${c.alt ? '⌥' : ''}${c.shift ? '⇧' : ''}${c.mod ? '⌘' : ''}${keyLabel(c.key)}`;
  }
  const parts = [c.mod ? 'Ctrl' : '', c.alt ? 'Alt' : '', c.shift ? 'Shift' : '', keyLabel(c.key)];
  return parts.filter(Boolean).join('+');
}

/** Default chords of a keymap entry (an action, or a tool group's key). */
export function defaultChords(id: KeymapId): Chord[] {
  if (id.startsWith('tool:')) {
    const key = TOOL_GROUP_KEYS[id.slice('tool:'.length)];
    return key ? [{ key }] : [];
  }
  return SHORTCUTS[id as ShortcutAction] ?? [];
}

/**
 * The chords an entry answers to: the user's override where there is one
 * (docs/specs/power-user-shortcuts-keymap.md), else the defaults. Reads
 * the reactive overrides, so callers in a `$derived` or template update.
 */
export function effectiveChords(id: KeymapId): Chord[] {
  return keymapState.overrides[id] ?? defaultChords(id);
}

/** Every rebindable entry: all actions and all tool groups, both modes. */
export const KEYMAP_IDS: readonly KeymapId[] = [
  ...(Object.keys(SHORTCUTS) as ShortcutAction[]),
  ...Object.keys(TOOL_GROUP_KEYS).map((g): KeymapId => `tool:${g}`),
];

/** Display label of an action's primary chord. */
export function hint(action: ShortcutAction): string {
  const c = effectiveChords(action)[0];
  return c ? chordLabel(c) : '';
}

/** A tooltip: `label (chord)`, or just `label` while the action is unbound. */
export function withHint(label: string, action: ShortcutAction): string {
  const h = hint(action);
  return h ? `${label} (${h})` : label;
}

/** Display labels of all of an action's chords, the primary (= `hint`) first. */
export function chordLabels(action: ShortcutAction): string[] {
  return effectiveChords(action).map(chordLabel);
}

/** A modifier key's name in the platform's notation: `Shift`, or `⇧` on macOS. */
export function modifierLabel(modifier: 'shift' | 'alt'): string {
  if (IS_MAC) return modifier === 'shift' ? '⇧' : '⌥';
  return modifier === 'shift' ? 'Shift' : 'Alt';
}

/** Display label of a tool group's key, or `undefined` (no key, or unbound). */
export function toolHint(groupId: string): string | undefined {
  const c = effectiveChords(`tool:${groupId}`)[0];
  return c ? chordLabel(c) : undefined;
}

/** Shift only counts for letters: shifted punctuation (`+`, `_`) carries it anyway. */
function shiftMatters(key: string): boolean {
  return /^[a-z]$/i.test(key);
}

function chordMatches(e: KeyboardEvent, c: Chord): boolean {
  const mod = e.ctrlKey || e.metaKey;
  if (Boolean(c.mod) !== mod || Boolean(c.alt) !== e.altKey) return false;
  if (shiftMatters(c.key) && Boolean(c.shift) !== e.shiftKey) return false;
  return e.key.toLowerCase() === c.key.toLowerCase();
}

/** The chord a key press makes, in stored form (`mod`, Shift only for letters). */
export function chordFromEvent(e: KeyboardEvent): Chord {
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const c: Chord = { key };
  if (e.ctrlKey || e.metaKey) c.mod = true;
  if (e.shiftKey && shiftMatters(key)) c.shift = true;
  if (e.altKey) c.alt = true;
  return c;
}

/** True when two chords are the same key press. */
export function sameChord(a: Chord, b: Chord): boolean {
  if (a.key.toLowerCase() !== b.key.toLowerCase()) return false;
  if (Boolean(a.mod) !== Boolean(b.mod) || Boolean(a.alt) !== Boolean(b.alt)) return false;
  return !shiftMatters(a.key) || Boolean(a.shift) === Boolean(b.shift);
}

/** The action `e` triggers, if any. */
export function actionFor(e: KeyboardEvent): ShortcutAction | null {
  for (const action of Object.keys(SHORTCUTS) as ShortcutAction[]) {
    if (effectiveChords(action).some((c) => chordMatches(e, c))) return action;
  }
  return null;
}

/** The tool group a key selects, if any. A bare key ignores Shift, as it always has. */
export function toolGroupForKey(e: KeyboardEvent): string | null {
  const bare = !(e.ctrlKey || e.metaKey || e.altKey);
  const key = e.key.toLowerCase();
  for (const group of Object.keys(TOOL_GROUP_KEYS)) {
    const hit = effectiveChords(`tool:${group}`).some((c) =>
      c.mod || c.alt || c.shift ? chordMatches(e, c) : bare && c.key.toLowerCase() === key,
    );
    if (hit) return group;
  }
  return null;
}

/** True when keyboard input belongs to a text field, not the editor. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLSelectElement) return true;
  if (target instanceof HTMLInputElement) {
    return !['checkbox', 'radio', 'range', 'button', 'color'].includes(target.type);
  }
  return false;
}
