// The `doc.*` tools the desktop app serves through its own flows
// (docs/specs/mcp-live-desktop.md AC-10, AC-11, AC-15): New and Open add a
// tab like the user's New / Open, Close asks about unsaved changes, Save
// and Save As show the editor's dialogs. Each answers with the agent
// tools' result shape, the document id being the tab's.
import { openIn, serial } from '../shell/editors.svelte';
import { activateTab, session } from '../shell/session.svelte';
import { promptBusy } from '../shell/unsaved.svelte';
import type { OpenedFile } from '../fs';
import { mimeFromName } from '../fs';
import type { EditorMode } from '../shell/types';
import type { LiveCall } from './live-host';
import { docIdOf, runOn, targetTab } from './tab-calls';
import { USER_CANCELLED, baseName, fail, ok, pickOpenPath, readPath, userWait, type Wire } from './wire';

/** What the tools need from the shell beyond its modules. */
export interface DocShell {
  /** Open an already-read file as the user's Open does (a new tab, or the
   *  tab holding the same file); resolves `false` when it could not be
   *  opened (the shell shows why). */
  openFile(opened: OpenedFile, mode?: EditorMode): Promise<boolean>;
}

const DOC_TOOLS = new Set(['doc.list', 'doc.new', 'doc.open', 'doc.close', 'doc.save']);

/** Pixel / Image canvas limits of the New dialog. */
const MAX_PIXEL_SIDE = 4096;
const MAX_IMAGE_SIDE = 16_384;
const MAX_IMAGE_AREA = 8192 * 8192;

export function isDocTool(tool: string): boolean {
  return DOC_TOOLS.has(tool);
}

/** `doc.info` of the active tab. */
function activeInfo(): Wire {
  const tab = session.active;
  return tab === null ? fail('no document is open') : runOn(tab, 'doc.info', {});
}

function list(): Wire {
  const documents = session.tabs.map((tab) => {
    const info = runOn(tab, 'doc.info', {});
    return info.ok ? info.json : { doc: docIdOf(tab), name: tab.doc.name(), mode: tab.mode };
  });
  const active = session.active === null ? null : docIdOf(session.active);
  return ok({ active, documents });
}

/** The size error of the New dialog's limits, or `null`. */
function sizeError(mode: EditorMode, width: number, height: number): string | null {
  const max = mode === 'pixel' ? MAX_PIXEL_SIDE : MAX_IMAGE_SIDE;
  for (const [key, v] of [
    ['width', width],
    ['height', height],
  ] as const) {
    if (!Number.isInteger(v) || v < 1 || v > max) return `${key} must be 1–${max}`;
  }
  if (mode === 'image' && width * height > MAX_IMAGE_AREA) {
    return `${width}×${height} px exceeds the Image document area limit of 8192 × 8192 px (64 Mpx)`;
  }
  return null;
}

async function newDoc(args: Record<string, unknown>): Promise<Wire> {
  const mode = args.mode;
  if (mode !== 'pixel' && mode !== 'image') return fail('argument `mode` must be "pixel" or "image"');
  const width = Number(args.width);
  const height = Number(args.height);
  const bad = sizeError(mode, width, height);
  if (bad !== null) return fail(bad);
  if (mode === 'image' && args.palette !== undefined) {
    return fail('`palette` applies to Pixel documents; Image documents have no palette');
  }
  const failed = await new Promise<string | null>((resolve) => {
    void serial(async () => resolve(await openIn(mode, (s) => s.create({ mode, width, height }))));
  });
  if (failed !== null) return fail(`cannot create: ${failed}`);
  const tab = session.active;
  if (tab === null) return fail('cannot create the document');
  if (typeof args.palette === 'string' && args.palette !== 'db32') {
    const loaded = runOn(tab, 'palette.load_preset', { preset: args.palette });
    if (!loaded.ok) return loaded;
  }
  return activeInfo();
}

async function openDoc(call: LiveCall, args: Record<string, unknown>, shell: DocShell): Promise<Wire> {
  if (typeof args.path !== 'string') return fail('missing required argument `path`');
  const mode = args.mode === 'pixel' || args.mode === 'image' ? args.mode : undefined;
  const picked = await pickOpenPath(call.id, args.path);
  if (picked === null) return fail(USER_CANCELLED);
  const bytes = await readPath(picked);
  const name = baseName(picked);
  const opened: OpenedFile = { name, bytes, mime: mimeFromName(name), handle: null, path: picked };
  if (!(await shell.openFile(opened, mode))) return fail(`cannot open ${name}`);
  return activeInfo();
}

/** Resolves once the unsaved-changes prompt (and a save it started) is
 *  done. */
async function promptDone(): Promise<void> {
  while (promptBusy()) await new Promise((r) => setTimeout(r, 50));
}

async function closeDoc(call: LiveCall, args: Record<string, unknown>): Promise<Wire> {
  const tab = targetTab(args);
  if (typeof tab === 'string') return fail(tab);
  if (tab.doc.running() !== null) return fail('the document is being saved; try again when it is done');
  const id = docIdOf(tab);
  tab.doc.close();
  if (promptBusy()) await userWait(call.id, promptDone);
  // Closing frees the document after the prompt; give the editor a tick.
  await new Promise((r) => setTimeout(r, 0));
  if (session.tabs.includes(tab)) return fail(`${USER_CANCELLED}: the document stays open`);
  const active = session.active === null ? null : docIdOf(session.active);
  return ok({ closed: id, active, open: session.tabs.length });
}

async function saveDoc(call: LiveCall, args: Record<string, unknown>): Promise<Wire> {
  const tab = targetTab(args);
  if (typeof tab === 'string') return fail(tab);
  if (session.active !== tab) activateTab(tab);
  // A new path is Save As: its dialogs ask where (AC-10).
  const saved = await userWait(call.id, () =>
    typeof args.path === 'string' ? tab.doc.saveAs() : tab.doc.save(),
  );
  if (!saved) return fail(`${USER_CANCELLED}: the document was not saved`);
  return runOn(tab, 'doc.info', {});
}

/** Runs one of the shell's `doc.*` tools. */
export function docTool(call: LiveCall, args: Record<string, unknown>, shell: DocShell): Promise<Wire> {
  switch (call.tool) {
    case 'doc.list':
      return Promise.resolve(list());
    case 'doc.new':
      return newDoc(args);
    case 'doc.open':
      return openDoc(call, args, shell);
    case 'doc.close':
      return closeDoc(call, args);
    default:
      return saveDoc(call, args);
  }
}
