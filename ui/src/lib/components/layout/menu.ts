// Data model of the editor menu bar (spec §9.2). Both modes describe their
// menus with these types; `MenuBar.svelte` renders them as dropdowns on
// wide screens and as one stacked sheet behind ☰ on narrow ones, so the
// two presentations can never disagree.

/** A clickable entry. */
export interface MenuAction {
  kind?: 'action';
  label: string;
  /** Shortcut shown right-aligned, e.g. `Ctrl+S` (display only). */
  shortcut?: string;
  disabled?: boolean;
  /** Renders a check mark (toggles such as panel visibility). */
  checked?: boolean;
  run: () => void;
}

/** A nested list (Export ▸, Recent ▸). */
export interface MenuSubmenu {
  kind: 'submenu';
  label: string;
  disabled?: boolean;
  items: MenuItem[];
}

/** A divider line. */
export interface MenuSeparator {
  kind: 'separator';
}

/** One row of a menu. */
export type MenuItem = MenuAction | MenuSubmenu | MenuSeparator;

/** One top-level menu (File, Edit, …). */
export interface MenuDef {
  id: string;
  label: string;
  items: MenuItem[];
}

/** `items` with separators at the edges or doubled up removed, so a menu
 *  can list optional groups without fiddling with dividers. */
export function tidy(items: MenuItem[]): MenuItem[] {
  const out: MenuItem[] = [];
  for (const it of items) {
    if (it.kind === 'separator' && (out.length === 0 || out.at(-1)?.kind === 'separator')) {
      continue;
    }
    out.push(it);
  }
  while (out.at(-1)?.kind === 'separator') out.pop();
  return out;
}
