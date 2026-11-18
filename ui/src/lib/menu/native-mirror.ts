// The editor menus as the native macOS menu bar shows them
// (docs/specs/macos-native-menu.md, Decision Log 2026-11-18).
//
// `buildMirror` turns the active editor's `MenuDef[]` into plain data for
// the shell's `set_editor_menu` (src-tauri/src/native_menu.rs `Node`) plus
// the table of `run` closures its entries fire, and the set of actions
// whose chords the native menu binds — the editors leave those keys to it.

import type { MenuDef, MenuItem } from '../components/layout/menu';
import {
  FORMERLY_NATIVE_ACTIONS,
  SHORTCUTS,
  effectiveChords,
  hint,
  nativeChord,
  type ShortcutAction,
} from '../shell/shortcuts';

/** The predefined items a mirrored menu uses. */
export type SystemRole = 'cut' | 'copy' | 'paste' | 'selectAll';

/** One entry of the native menu bar, as `set_editor_menu` takes it. */
export type NativeNode =
  | {
      kind: 'item';
      id: string;
      label: string;
      enabled: boolean;
      checked?: boolean;
      accelerator?: string;
    }
  | { kind: 'submenu'; label: string; enabled: boolean; items: NativeNode[] }
  | { kind: 'separator' }
  | { kind: 'system'; role: SystemRole; label: string };

/** The mirrored menus and what the renderer keeps for them. */
export interface Mirror {
  /** The top-level menus, each a `submenu` node. */
  menus: NativeNode[];
  /** Entry id → the action it runs; only usable entries are listed. */
  runs: Map<string, () => void>;
  /** The actions whose chord the native menu binds. */
  bound: Set<ShortcutAction>;
}

export interface MirrorOptions {
  /** False for a menu that is inert right now (an effect is open, an
   *  Assistant answer runs): its entries are dimmed. */
  live: (menu: MenuDef) => boolean;
  /** A text field has focus (AC-12, AC-13). */
  textFocus: boolean;
}

/** Actions whose entries become the system's items while a text field has
 *  focus; Paste always is one — the editors take a paste from the `paste`
 *  event the system item sends. */
const TEXT_ROLES: Partial<Record<ShortcutAction, SystemRole>> = {
  cut: 'cut',
  copy: 'copy',
  paste: 'paste',
  selectAll: 'selectAll',
};

/** The App menu's own chords besides Settings… (src-tauri native_menu.rs). */
const APP_CHORDS = ['CmdOrCtrl+H', 'CmdOrCtrl+Alt+H', 'CmdOrCtrl+Q'];

/** The action behind each shortcut hint the menus print. */
function actionsByHint(): Map<string, ShortcutAction> {
  const out = new Map<string, ShortcutAction>();
  for (const action of Object.keys(SHORTCUTS) as ShortcutAction[]) {
    const h = hint(action);
    if (h !== '' && !out.has(h)) out.set(h, action);
  }
  return out;
}

/** The native menu data for `menus` (macos-native-menu AC-03 – AC-13, AC-18). */
export function buildMirror(menus: readonly MenuDef[], opts: MirrorOptions): Mirror {
  const byHint = actionsByHint();
  const runs = new Map<string, () => void>();
  const bound = new Set<ShortcutAction>();
  const settings = effectiveChords('settings')[0];
  const used = new Set<string>(APP_CHORDS);
  const settingsChord = settings ? nativeChord(settings) : null;
  if (settingsChord !== null) used.add(settingsChord);

  function chordOf(action: ShortcutAction | undefined): string | undefined {
    if (action === undefined) return undefined;
    if (opts.textFocus && !FORMERLY_NATIVE_ACTIONS.has(action)) return undefined;
    const first = effectiveChords(action)[0];
    const chord = first ? nativeChord(first) : null;
    // One chord, one entry: the App menu's and an earlier entry's win.
    if (chord === null || used.has(chord)) return undefined;
    used.add(chord);
    bound.add(action);
    return chord;
  }

  function convert(items: readonly MenuItem[], path: string, live: boolean): NativeNode[] {
    return items.map((it, i): NativeNode => {
      const at = `${path}.${i}`;
      if (it.kind === 'separator') return { kind: 'separator' };
      if (it.kind === 'submenu') {
        const enabled = live && it.disabled !== true;
        return {
          kind: 'submenu',
          label: it.label,
          enabled,
          items: convert(it.items, at, enabled),
        };
      }
      const action = it.shortcut ? byHint.get(it.shortcut) : undefined;
      const role = action === undefined ? undefined : TEXT_ROLES[action];
      if (role !== undefined && (role === 'paste' || opts.textFocus)) {
        return { kind: 'system', role, label: it.label };
      }
      const id = `ed:${at}|${it.label}`;
      const enabled = live && it.disabled !== true;
      if (enabled) runs.set(id, it.run);
      const node: NativeNode = { kind: 'item', id, label: it.label, enabled };
      if (it.checked !== undefined) node.checked = it.checked;
      const accelerator = chordOf(action);
      if (accelerator !== undefined) node.accelerator = accelerator;
      return node;
    });
  }

  const out = menus.map(
    (m): NativeNode => ({
      kind: 'submenu',
      label: m.label,
      enabled: true,
      items: convert(m.items, m.id, opts.live(m)),
    }),
  );
  return { menus: out, runs, bound };
}
