// The one keyboard map both editors use (spec §9.4: "Keyboard shortcuts are
// defined in a single `shortcuts.ts` map"). Same key, same meaning in Pixel
// and Image mode; each editor binds the actions it supports. Menus and
// tooltips print their hints from here, so labels never drift from keys.

import { isMac } from '../platform';

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
  | 'togglePlay';

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

/** Display label of an action's primary chord. */
export function hint(action: ShortcutAction): string {
  const c = SHORTCUTS[action][0];
  return c ? chordLabel(c) : '';
}

/** Display label of a tool group's key, or `undefined`. */
export function toolHint(groupId: string): string | undefined {
  const k = TOOL_GROUP_KEYS[groupId];
  return k ? k.toUpperCase() : undefined;
}

function chordMatches(e: KeyboardEvent, c: Chord): boolean {
  const mod = e.ctrlKey || e.metaKey;
  if (Boolean(c.mod) !== mod || Boolean(c.alt) !== e.altKey) return false;
  // Shifted punctuation (`+`, `_`) arrives with shiftKey set; only enforce
  // Shift for letter chords.
  if (/^[a-z]$/i.test(c.key) && Boolean(c.shift) !== e.shiftKey) return false;
  return e.key.toLowerCase() === c.key.toLowerCase();
}

/** The action `e` triggers, if any. */
export function actionFor(e: KeyboardEvent): ShortcutAction | null {
  for (const [action, chords] of Object.entries(SHORTCUTS) as [ShortcutAction, Chord[]][]) {
    if (chords.some((c) => chordMatches(e, c))) return action;
  }
  return null;
}

/** The tool group a bare key selects, if any. */
export function toolGroupForKey(e: KeyboardEvent): string | null {
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  const key = e.key.toLowerCase();
  for (const [group, k] of Object.entries(TOOL_GROUP_KEYS)) {
    if (k === key) return group;
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
