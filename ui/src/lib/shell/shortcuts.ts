// The one keyboard map both editors use (spec §9.4: "Keyboard shortcuts are
// defined in a single `shortcuts.ts` map"). Same key, same meaning in Pixel
// and Image mode; each editor binds the actions it supports. Menus and
// tooltips print their hints from here, so labels never drift from keys.

import { hasNativeMenu, isInstalledWindow, isMac, isTauri } from '../platform';
import { keymapState, type KeymapId } from './keymap-state.svelte';

/**
 * A key chord. `key` is matched case-insensitively against
 * `KeyboardEvent.key`, or, where `code` is set, `code` against
 * `KeyboardEvent.code` (`key` then only labels the chord).
 */
export interface Chord {
  key: string;
  /**
   * Physical key (`Digit1`, `BracketRight`, `KeyE`) for chords whose
   * `e.key` depends on the layout or the modifiers: Shift+digit types `!`
   * (US) or `=` (German Shift+0), Alt+[ types `“` on macOS, Ctrl+Alt+E is
   * AltGr+E (`€`) on a German Windows keyboard.
   */
  code?: string;
  /** Ctrl on Windows / Linux, ⌘ on macOS. */
  mod?: boolean;
  /** The Control key itself, ⌃ on macOS too (⌘Tab is the app switcher). */
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  /** The native menu bar (macOS desktop app) owns this chord as an
   *  accelerator and fires the menu item instead; the editors leave it. */
  menu?: boolean;
}

/** Editor actions with a shortcut. */
export type ShortcutAction =
  | 'newDocument'
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
  | 'fillSelection'
  | 'zoomIn'
  | 'zoomOut'
  | 'zoomFit'
  | 'zoomActual'
  | 'toggleGrid'
  | 'togglePixelGrid'
  | 'toggleRulers'
  | 'swapColors'
  | 'resetColors'
  | 'brushSmaller'
  | 'brushLarger'
  | 'prevFrame'
  | 'nextFrame'
  | 'togglePlay'
  | 'shortcuts'
  | 'newLayer'
  | 'duplicateLayer'
  | 'deleteLayer'
  | 'mergeDown'
  | 'layerAbove'
  | 'layerBelow'
  | 'newFrame'
  | 'flipLayerH'
  | 'flipLayerV'
  | 'rotateCw'
  | 'rotateCcw'
  | 'toggleUi'
  | OpacityAction
  | 'paletteNext'
  | 'palettePrev'
  | 'commandPalette'
  | 'nextTab'
  | 'prevTab'
  | 'closeTab';

const IS_MAC = isMac();

/**
 * The tab keys (docs/specs/document-tabs.md "Tab keys"). The web build gets
 * no Ctrl / ⌘ chords, which the browser keeps for its own tabs; on macOS
 * next / previous use the Control key (⌘Tab is the app switcher) and ⇧⌘[ / ]
 * as in Safari, and the native menu owns ⌘W (File ▸ Close).
 */
function tabChords(): Record<'nextTab' | 'prevTab' | 'closeTab', Chord[]> {
  const next: Chord = { key: 'PageDown', alt: true };
  const prev: Chord = { key: 'PageUp', alt: true };
  const altW: Chord = { key: 'w', code: 'KeyW', alt: true };
  if (!isTauri()) return { nextTab: [next], prevTab: [prev], closeTab: [altW] };
  if (IS_MAC) {
    return {
      nextTab: [
        { key: 'Tab', ctrl: true },
        { key: ']', code: 'BracketRight', mod: true, shift: true },
        next,
      ],
      prevTab: [
        { key: 'Tab', ctrl: true, shift: true },
        { key: '[', code: 'BracketLeft', mod: true, shift: true },
        prev,
      ],
      closeTab: [{ key: 'w', mod: true, menu: true }, altW],
    };
  }
  return {
    nextTab: [{ key: 'Tab', ctrl: true }, next],
    prevTab: [{ key: 'Tab', ctrl: true, shift: true }, prev],
    closeTab: [{ key: 'w', mod: true }, altW],
  };
}

/** Brush opacity 10 % … 100 % (Image mode): Shift+1 … Shift+9, Shift+0. */
export type OpacityAction =
  | 'opacity10'
  | 'opacity20'
  | 'opacity30'
  | 'opacity40'
  | 'opacity50'
  | 'opacity60'
  | 'opacity70'
  | 'opacity80'
  | 'opacity90'
  | 'opacity100';

/** The opacity actions, 10 % first. */
export const OPACITY_ACTIONS: readonly OpacityAction[] = [
  'opacity10',
  'opacity20',
  'opacity30',
  'opacity40',
  'opacity50',
  'opacity60',
  'opacity70',
  'opacity80',
  'opacity90',
  'opacity100',
];

/** The brush opacity (%) an opacity action sets. */
export function opacityPercent(action: OpacityAction): number {
  return Number(action.slice('opacity'.length));
}

/** Shift+digit on the physical digit row, whatever character it types. */
function shiftDigit(percent: number): Chord {
  const digit = String((percent / 10) % 10);
  return { key: digit, code: `Digit${digit}`, shift: true };
}

/**
 * Keys only the desktop app can take: Ctrl+R reloads a browser tab, so
 * the rotations have no default there (pincel.md Decision Log 2026-10-29).
 */
const DESKTOP = isTauri();

/**
 * Ctrl/⌘+N opens the New dialog only where the page gets the chord: the
 * desktop app and an installed app window. A browser tab keeps it for a new
 * window, so there it has no binding and no hint (power-user-shortcuts-coverage
 * AC-14, AC-20).
 */
export const NEW_DOCUMENT_KEY: boolean = DESKTOP || isInstalledWindow();

/** Primary chord per action, plus alternates matched but not displayed. */
export const SHORTCUTS: Record<ShortcutAction, Chord[]> = {
  // The macOS desktop app's native File ▸ New owns ⌘N (`menu:new`).
  newDocument: NEW_DOCUMENT_KEY ? [{ key: 'n', mod: true, menu: true }] : [],
  open: [{ key: 'o', mod: true }],
  save: [{ key: 's', mod: true }],
  saveAs: [{ key: 's', mod: true, shift: true }],
  export: [{ key: 'e', mod: true }],
  // Ctrl+Alt+E: Ctrl+Shift+E is Merge Down (power-user-shortcuts-coverage).
  exportSheet: [{ key: 'e', code: 'KeyE', mod: true, alt: true }],
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
  // Backspace is Fill Selection (paint.net) on Windows / Linux; Mac
  // laptops have no other delete key, so there it stays Delete Selection
  // and Fill Selection takes ⌥⌫ (docs/specs/image-edit-extras.md AC-14).
  deleteSelection: IS_MAC ? [{ key: 'Delete' }, { key: 'Backspace' }] : [{ key: 'Delete' }],
  fillSelection: [IS_MAC ? { key: 'Backspace', alt: true } : { key: 'Backspace' }],
  zoomIn: [{ key: '+' }, { key: '=' }],
  zoomOut: [{ key: '-' }, { key: '_' }],
  zoomFit: [{ key: '0' }],
  zoomActual: [{ key: '1' }],
  // Pixel mode's grids (docs/specs/pixel-grid.md), as in Aseprite. Shift
  // tells the two apart for `'` (see `shiftMatters`); a US layout types
  // Shift+' as `"`, hence the alternate.
  toggleGrid: [{ key: "'", mod: true }],
  togglePixelGrid: [
    { key: "'", mod: true, shift: true },
    { key: '"', mod: true },
  ],
  // Image mode's rulers (docs/specs/image-view-aids.md AC-17): desktop only,
  // as the rotations; Ctrl+Alt+R is AltGr+R on Windows, hence the physical key.
  toggleRulers: DESKTOP ? [{ key: 'r', code: 'KeyR', mod: true, alt: true }] : [],
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
  // docs/specs/power-user-shortcuts-coverage.md: Aseprite / Photoshop keys.
  // Shift+N (not Ctrl+Shift+N, which a browser tab can't capture).
  newLayer: [{ key: 'n', shift: true }],
  duplicateLayer: [{ key: 'j', mod: true }],
  deleteLayer: [{ key: 'Delete', mod: true, shift: true }],
  mergeDown: [{ key: 'e', mod: true, shift: true }],
  layerAbove: [{ key: 'ArrowUp', alt: true }],
  layerBelow: [{ key: 'ArrowDown', alt: true }],
  // macOS types a dead key for ⌥N, hence the physical key.
  newFrame: [{ key: 'n', code: 'KeyN', alt: true }],
  flipLayerH: [{ key: 'h', shift: true }],
  flipLayerV: [{ key: 'v', shift: true }],
  rotateCw: DESKTOP ? [{ key: 'r', mod: true }] : [],
  rotateCcw: DESKTOP ? [{ key: 'r', mod: true, shift: true }] : [],
  // Only while the canvas stage has focus (`isStageTarget`); elsewhere Tab
  // keeps moving focus.
  toggleUi: [{ key: 'Tab' }],
  opacity10: [shiftDigit(10)],
  opacity20: [shiftDigit(20)],
  opacity30: [shiftDigit(30)],
  opacity40: [shiftDigit(40)],
  opacity50: [shiftDigit(50)],
  opacity60: [shiftDigit(60)],
  opacity70: [shiftDigit(70)],
  opacity80: [shiftDigit(80)],
  opacity90: [shiftDigit(90)],
  opacity100: [shiftDigit(100)],
  paletteNext: [{ key: ']', code: 'BracketRight', alt: true }],
  palettePrev: [{ key: '[', code: 'BracketLeft', alt: true }],
  // docs/specs/power-user-shortcuts-palette.md.
  commandPalette: [{ key: 'k', mod: true }],
  ...tabChords(),
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
  clone: 'c',
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

/** The native menu item (src-tauri/src/main.rs) of each `NATIVE_MENU_ACTIONS` entry. */
const NATIVE_MENU_IDS: Partial<Record<ShortcutAction, string>> = {
  open: 'menu:open',
  save: 'menu:save',
  saveAs: 'menu:saveAs',
  export: 'menu:exportPng',
  exportSheet: 'menu:exportSheet',
  undo: 'menu:undo',
  redo: 'menu:redo',
  shortcuts: 'menu:shortcuts',
};

function keyLabel(key: string): string {
  if (key === 'Delete') return 'Del';
  if (key === 'Enter') return '↵';
  if (key === 'PageDown') return IS_MAC ? '⇟' : 'Page Down';
  if (key === 'PageUp') return IS_MAC ? '⇞' : 'Page Up';
  if (key === 'ArrowUp') return '↑';
  if (key === 'ArrowDown') return '↓';
  if (key === 'ArrowLeft') return '←';
  if (key === 'ArrowRight') return '→';
  return key.length === 1 ? key.toUpperCase() : key;
}

/** Human label of a chord: `Ctrl+Shift+S`, or `⇧⌘S` on macOS (modifiers
 *  in the macOS order ⌃ ⌥ ⇧ ⌘). */
export function chordLabel(c: Chord): string {
  if (IS_MAC) {
    const mods = `${c.ctrl ? '⌃' : ''}${c.alt ? '⌥' : ''}${c.shift ? '⇧' : ''}${c.mod ? '⌘' : ''}`;
    return `${mods}${keyLabel(c.key)}`;
  }
  const ctrl = c.mod === true || c.ctrl === true;
  const parts = [ctrl ? 'Ctrl' : '', c.alt ? 'Alt' : '', c.shift ? 'Shift' : '', keyLabel(c.key)];
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

/**
 * Shift counts for letters; for `'`, whose Shift chord is a separate
 * action (Ctrl+' / Ctrl+Shift+'); for named keys (Tab, Delete, arrows),
 * where Shift+Tab moves focus back; and for physical-key chords. Other
 * shifted punctuation (`+`, `_`) carries it anyway.
 */
function shiftMatters(c: Chord): boolean {
  return c.code !== undefined || /^[a-z']$/i.test(c.key) || c.key.length > 1;
}

function chordMatches(e: KeyboardEvent, c: Chord): boolean {
  if (c.menu === true && hasNativeMenu()) return false;
  const mod = e.ctrlKey || e.metaKey;
  if ((c.mod === true || c.ctrl === true) !== mod || Boolean(c.alt) !== e.altKey) return false;
  if (c.ctrl === true && (!e.ctrlKey || e.metaKey)) return false;
  if (shiftMatters(c) && Boolean(c.shift) !== e.shiftKey) return false;
  if (c.code !== undefined) return e.code === c.code;
  return e.key.toLowerCase() === c.key.toLowerCase();
}

/** The label key of a physical key whose typed character is unreliable, if any. */
function keyOfCode(code: string): string | null {
  const digit = /^Digit(\d)$/.exec(code);
  if (digit) return digit[1] ?? null;
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return (letter[1] ?? '').toLowerCase();
  if (code === 'BracketLeft') return '[';
  if (code === 'BracketRight') return ']';
  return null;
}

/**
 * The chord a key press makes, in stored form (`mod`, Shift where it
 * counts). Shift+digit and Alt+letter / bracket record the physical key,
 * as the defaults do, since their `e.key` depends on the layout.
 */
export function chordFromEvent(e: KeyboardEvent): Chord {
  const physical = keyOfCode(e.code);
  const byCode =
    physical !== null &&
    ((e.shiftKey && /^Digit/.test(e.code)) || (e.altKey && !/^Digit/.test(e.code)));
  const key = byCode ? physical : e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const c: Chord = byCode ? { key, code: e.code } : { key };
  if (e.ctrlKey || e.metaKey) c.mod = true;
  if (e.shiftKey && shiftMatters(c)) c.shift = true;
  if (e.altKey) c.alt = true;
  return c;
}

/** True when two chords are the same key press. */
export function sameChord(a: Chord, b: Chord): boolean {
  if (a.code !== undefined && b.code !== undefined) {
    if (a.code !== b.code) return false;
  } else if (a.key.toLowerCase() !== b.key.toLowerCase()) return false;
  if (Boolean(a.mod) !== Boolean(b.mod) || Boolean(a.alt) !== Boolean(b.alt)) return false;
  return !(shiftMatters(a) || shiftMatters(b)) || Boolean(a.shift) === Boolean(b.shift);
}

/**
 * The action `e` triggers, if any. Physical-key chords go first, so
 * German Shift+0 (`=`) sets the opacity instead of zooming in.
 */
export function actionFor(e: KeyboardEvent): ShortcutAction | null {
  const actions = Object.keys(SHORTCUTS) as ShortcutAction[];
  for (const byCode of [true, false]) {
    for (const action of actions) {
      const chords = effectiveChords(action).filter((c) => (c.code !== undefined) === byCode);
      if (chords.some((c) => chordMatches(e, c))) return action;
    }
  }
  return null;
}

/** True when `target` is the canvas stage, the only place Tab toggles the UI. */
export function isStageTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.dataset.stage !== undefined;
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

/**
 * True for a focused control that a Space tap activates (button, checkbox,
 * radio). Both editors still pan on a Space-drag over such a control, but
 * leave the keydown alone so a tap reaches it (effect-catalog-v2 AC-15).
 */
export function isSpaceActivated(target: EventTarget | null): boolean {
  if (target instanceof HTMLButtonElement) return true;
  return (
    target instanceof HTMLInputElement && ['checkbox', 'radio', 'button'].includes(target.type)
  );
}

// Keys a native menu accelerator can name (Tauri's accelerator syntax).
const ACCELERATOR_PUNCTUATION = new Set(['`', '\\', '[', ']', ',', '=', '-', '.', "'", ';', '/']);
const ACCELERATOR_NAMES = new Set([
  'Backspace',
  'Enter',
  'Tab',
  'Delete',
  'Escape',
  'Home',
  'End',
  'Insert',
  'PageUp',
  'PageDown',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
]);

/** `chord` as a native menu accelerator (`CmdOrCtrl+Shift+U`), or `null` if it can't be one. */
export function menuAccelerator(c: Chord): string | null {
  let key: string;
  if (/^[a-z0-9]$/i.test(c.key)) key = c.key.toUpperCase();
  else if (ACCELERATOR_PUNCTUATION.has(c.key) || ACCELERATOR_NAMES.has(c.key)) key = c.key;
  else if (c.key === ' ') key = 'Space';
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(c.key)) key = c.key;
  else return null;
  const mods = [c.mod ? 'CmdOrCtrl' : '', c.alt ? 'Alt' : '', c.shift ? 'Shift' : ''];
  return [...mods.filter(Boolean), key].join('+');
}

/**
 * True when the native menu (where `hasNativeMenu()` holds) fires `action`
 * itself, so the editors must leave its key alone. A rebound chord the
 * menu can't express as an accelerator stays with the editors instead.
 */
export function nativeMenuOwns(action: ShortcutAction): boolean {
  if (!NATIVE_MENU_ACTIONS.has(action)) return false;
  const first = effectiveChords(action)[0];
  return first === undefined || menuAccelerator(first) !== null;
}

/**
 * Accelerators of the native menu items whose action the user rebound
 * (`menu:undo` → `CmdOrCtrl+U`; empty for none), for `set_menu_accelerators`.
 * Items left out keep their default accelerator.
 */
export function nativeAccelerators(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [action, id] of Object.entries(NATIVE_MENU_IDS) as [ShortcutAction, string][]) {
    if (keymapState.overrides[action] === undefined) continue;
    const first = effectiveChords(action)[0];
    out[id] = first ? (menuAccelerator(first) ?? '') : '';
  }
  return out;
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
