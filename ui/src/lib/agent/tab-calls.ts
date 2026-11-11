// Agent calls on the documents in the shell's tabs
// (docs/specs/mcp-live-desktop.md): the id a tab has for agents, the tab a
// call targets, and one call on a tab's document.
import { session, type ShellTab } from '../shell/session.svelte';
import type { AgentFile } from '../shell/types';
import { parseWire, type Wire } from './wire';

/** The id a tab has for agents. */
export function docIdOf(tab: ShellTab): string {
  return `tab-${tab.key}`;
}

/** The tab a call targets: its `doc` argument, else the active tab
 *  (AC-15), or the error (AC-09). */
export function targetTab(args: Record<string, unknown>): ShellTab | string {
  const id = args.doc;
  if (typeof id === 'string') {
    return (
      session.tabs.find((t) => docIdOf(t) === id) ??
      `no open document with id \`${id}\`; doc.list shows the open documents`
    );
  }
  return session.active ?? 'no document is open; call doc.new or doc.open first';
}

/** Runs `tool` on `tab`'s document; the tab's `dirty` and `path` replace
 *  the ones the tools cannot know. */
export function runOn(
  tab: ShellTab,
  tool: string,
  args: Record<string, unknown>,
  file: AgentFile | null = null,
): Wire {
  const id = docIdOf(tab);
  const wire = parseWire(tab.doc.agentCall(id, tool, JSON.stringify(args), file));
  if (wire.ok && typeof wire.json === 'object' && wire.json !== null) {
    const json = wire.json as Record<string, unknown>;
    if (json.doc === id && 'dirty' in json) {
      json.dirty = tab.doc.dirty();
      json.path = tab.doc.location().path ?? null;
      json.name = tab.doc.name();
    }
  }
  return wire;
}
