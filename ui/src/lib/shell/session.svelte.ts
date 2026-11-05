// Shell session state (docs/specs/document-tabs.md): one tab list for the
// documents of both modes, in opening order, and the active tab. Each
// mode's editor holds its own tabs; it hands every one to the shell as an
// `EditorDocument` (`addTab`), and the shell decides which tab — of which
// mode — is active. Only the active tab's editor is shown and receives
// input (AC-21); the editors themselves are mounted by `editors.svelte.ts`.
import type { FileFormat } from '../fs';
import type { EditorDocument, EditorMode } from './types';

/** One open document in the shell's tab bar. */
export interface ShellTab {
  /** Stable id for the tab bar; never reused. */
  readonly key: number;
  readonly mode: EditorMode;
  readonly doc: EditorDocument;
}

class Session {
  /** The open tabs of both modes, in opening order (tabs never reorder). */
  tabs = $state.raw<readonly ShellTab[]>([]);
  /** The tab the canvas, panels, menus and status bar show. */
  active = $state.raw<ShellTab | null>(null);
}

export const session = new Session();

/** The tab bar's horizontal scroll, shared by both editors' bars so it is
 *  kept across a mode switch (document-tabs Terms, "Tab bar"). */
export const tabScroll = { left: 0 };

let nextKey = 1;

/** Which editor a file format belongs to. Sprites are Pixel mode;
 *  every raster format is Image mode. (A PNG opened through Pixel mode's
 *  own Open stays there — that path names the mode.) */
export function detectMode(format: FileFormat | null): EditorMode {
  return format === 'aseprite' ? 'pixel' : 'image';
}

/** The mode whose editor is shown, `null` while no tab is open. */
export function shownMode(): EditorMode | null {
  return session.active?.mode ?? null;
}

/** The open tabs' documents, in tab order. */
export function openDocuments(): EditorDocument[] {
  return session.tabs.map((t) => t.doc);
}

/** How many tabs of `mode` are open. */
export function tabCount(mode: EditorMode): number {
  return session.tabs.filter((t) => t.mode === mode).length;
}

/** The tab of `doc`, if it is open. */
export function tabOf(doc: EditorDocument): ShellTab | null {
  return session.tabs.find((t) => t.doc === doc) ?? null;
}

/** An editor opened `doc` as a new tab of `mode`: it goes after the last
 *  tab and becomes the active one (AC-02). The editor has already made it
 *  its own active tab. Returns the function that removes the tab again. */
export function addTab(mode: EditorMode, doc: EditorDocument): () => void {
  const tab: ShellTab = { key: nextKey++, mode, doc };
  session.tabs = [...session.tabs, tab];
  session.active = tab;
  return () => removeTab(tab);
}

/** Make `tab` the active one, showing its mode's editor (AC-04). */
export function activateTab(tab: ShellTab): void {
  tab.doc.activate();
  session.active = tab;
}

/** Make `doc`'s tab the active one. */
export function activateDocument(doc: EditorDocument): void {
  const tab = tabOf(doc);
  if (tab) activateTab(tab);
}

/** The tab with `key`, if it is open. */
export function tabByKey(key: number): ShellTab | null {
  return session.tabs.find((t) => t.key === key) ?? null;
}

/** Step to the next / previous tab across both modes, wrapping (AC-04). */
export function stepTab(step: 1 | -1): void {
  const n = session.tabs.length;
  const active = session.active;
  if (n === 0 || active === null) return;
  const at = session.tabs.indexOf(active);
  const next = session.tabs[(at + step + n) % n];
  if (next && next !== active) activateTab(next);
}

/** A tab closed: it leaves the bar, and when it was active the tab to its
 *  right becomes active, whatever its mode — the one to its left when it
 *  was the last (AC-07). */
function removeTab(tab: ShellTab): void {
  const at = session.tabs.indexOf(tab);
  if (at < 0) return;
  const rest = session.tabs.filter((t) => t !== tab);
  session.tabs = rest;
  if (session.active !== tab) return;
  const next = rest[at] ?? rest[at - 1] ?? null;
  if (next) activateTab(next);
  else session.active = null;
}
