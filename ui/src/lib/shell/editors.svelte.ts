// The editors the shell mounts (docs/specs/document-tabs.md AC-16, AC-21 –
// AC-24): at most one per mode, mounted when the first document of that
// mode opens (its chunk and wasm module are fetched then, and stay in the
// page after a release) and released with its last tab. Documents are
// opened one after another in the order they arrived; while a mode's
// editor loads, the open tabs stay usable.
import type { Component } from 'svelte';
import type { EditorMenuHandlers } from '../menu';
import { tabCount } from './session.svelte';
import type { EditorMode, EditorProps, EditorSession } from './types';

/** An editor chunk, as its dynamic `import()` resolves. */
export type EditorModule = { default: Component<EditorProps> };

/** How the shell fetches each mode's editor chunk. */
export type EditorLoaders = Record<EditorMode, () => Promise<EditorModule>>;

class Editors {
  /** The mounted editor component per mode; `null` while released. */
  component = $state.raw<Record<EditorMode, Component<EditorProps> | null>>({
    pixel: null,
    image: null,
  });
  /** The mode whose editor is loading for a document, if any. */
  loading = $state<EditorMode | null>(null);
  /** Open requests that have not finished yet. */
  pending = $state(0);
}

export const editors = new Editors();

interface Waiter {
  promise: Promise<EditorSession>;
  resolve: (s: EditorSession) => void;
  reject: (err: unknown) => void;
}

let loaders: EditorLoaders | null = null;
const sessions: Partial<Record<EditorMode, EditorSession>> = {};
const waiters: Partial<Record<EditorMode, Waiter>> = {};
const menus: Record<EditorMode, EditorMenuHandlers | null> = { pixel: null, image: null };
let chain: Promise<void> = Promise.resolve();

/** Hand the module the editor chunks (App mount). */
export function setEditorLoaders(next: EditorLoaders): void {
  loaders = next;
}

/** Run `job` after every open request that arrived before it (AC-24). */
export function serial(job: () => Promise<void>): Promise<void> {
  editors.pending += 1;
  const run = chain
    .then(job)
    .catch((err: unknown) => console.error('open request failed', err))
    .finally(() => {
      editors.pending -= 1;
    });
  chain = run;
  return run;
}

/** The session of `mode`'s editor, mounting the editor first when it is
 *  released; rejects when its chunk cannot be loaded (AC-23). */
export function ensureEditor(mode: EditorMode): Promise<EditorSession> {
  const ready = sessions[mode];
  if (ready) return Promise.resolve(ready);
  const known = waiters[mode];
  if (known) return known.promise;
  let resolve: (s: EditorSession) => void = () => {};
  let reject: (err: unknown) => void = () => {};
  const promise = new Promise<EditorSession>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  waiters[mode] = { promise, resolve, reject };
  const load = loaders?.[mode];
  if (!load) {
    fail(mode, new Error('no editor loader'));
    return promise;
  }
  load()
    .then((mod) => {
      editors.component = { ...editors.component, [mode]: mod.default };
    })
    .catch((err: unknown) => fail(mode, err));
  return promise;
}

function fail(mode: EditorMode, err: unknown): void {
  const waiter = waiters[mode];
  delete waiters[mode];
  releaseEditor(mode);
  waiter?.reject(err);
}

/** `mode`'s editor mounted and takes documents. */
export function registerSession(mode: EditorMode, session: EditorSession): () => void {
  sessions[mode] = session;
  const waiter = waiters[mode];
  delete waiters[mode];
  waiter?.resolve(session);
  return () => {
    if (sessions[mode] === session) delete sessions[mode];
  };
}

/** Unmount `mode`'s editor (its last tab closed, or its first document
 *  could not be opened). Its chunk and wasm module stay in the page. */
export function releaseEditor(mode: EditorMode): void {
  delete sessions[mode];
  menus[mode] = null;
  if (editors.component[mode] !== null) {
    editors.component = { ...editors.component, [mode]: null };
  }
}

/** Release `mode`'s editor once it holds no tab. */
export function releaseIfEmpty(mode: EditorMode): void {
  if (tabCount(mode) === 0 && waiters[mode] === undefined) releaseEditor(mode);
}

/** The native-menu items `mode`'s editor serves. */
export function setMenuHandlers(mode: EditorMode, handlers: EditorMenuHandlers | null): void {
  menus[mode] = handlers;
}

/** The native-menu items of `mode`'s editor, if it is mounted. */
export function menuHandlers(mode: EditorMode | null): EditorMenuHandlers | null {
  return mode === null ? null : menus[mode];
}

/**
 * Open one document in `mode`'s editor (loading it if needed): `run` asks
 * the editor's session and resolves `null` once the tab is open, or the
 * error. Resolves `null` on success, else the error; an editor left
 * without a tab is released again (AC-23).
 */
export async function openIn(
  mode: EditorMode,
  run: (session: EditorSession) => Promise<string | null>,
): Promise<string | null> {
  let session: EditorSession;
  if (sessions[mode] === undefined) editors.loading = mode;
  try {
    session = await ensureEditor(mode);
  } catch (err) {
    editors.loading = null;
    return err instanceof Error ? err.message : String(err);
  }
  let error: string | null;
  try {
    error = await run(session);
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  } finally {
    editors.loading = null;
  }
  if (error !== null) releaseIfEmpty(mode);
  return error;
}
