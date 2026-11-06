// One autosave / recovery loop for both editors (spec §10, M10.3), one
// `AutosaveSession` per open document (docs/specs/document-tabs.md AC-14).
// Every `AUTOSAVE_INTERVAL_MS` an edited, unsaved document is encoded by
// its editor and written to the IDB snapshot store under the session's
// `docId`; a save drops it. When the first editor of a page load starts,
// the shell lists the snapshots of both modes an earlier page load left
// (crash, closed tab) in the Recovery dialog (`RecoveryOffer`); later
// editor starts in the same page do not ask again.
//
// Behaviour both modes share:
// - A clean document (undo depth at the last save) is never written —
//   unless the save lost part of it (`keep()`, a flattened image).
// - A recovered document counts as unsaved, and its snapshot stays until
//   the user saves — closing the browser tab right after recovering loses
//   nothing.
// - Recovering a snapshot opens it as a new document; the dialog stays
//   while snapshots remain, and discarding or recovering the last listed
//   snapshot closes it.
// - Leaving a document (closing its tab after "Don't Save", or the editor
//   unmounting) deletes its snapshot: the user chose to drop it.

import type { EditorMode } from '../shell/types';
import {
  latestSnapshot,
  listLatestSnapshots,
  removeSnapshots,
  writeSnapshot,
  type AutosaveSnapshotMeta,
} from './autosave';
import { isIdbAvailable } from './db';

/** The Recovery probe already ran in this page load. */
let probed = false;

/** How often an edited document is snapshotted. */
export const AUTOSAVE_INTERVAL_MS = 30_000;

/** What an editor tells the session about one of its documents. */
export interface AutosaveHost {
  mode: EditorMode;
  /** Undo depth of the document, or `null` while there is none. */
  depth(): number | null;
  /** Undo depth at the last save; equal to `depth()` means clean. */
  savedDepth(): number;
  /** A file operation on the document is running; skip this tick. */
  busy(): boolean;
  /** Display name stored with the snapshot. */
  name(): string;
  /** File format stored with the snapshot, for editors that track one
   *  apart from the name (`AutosaveSnapshotMeta.format`). */
  format?(): string | null;
  /** The document's project bytes (`.aseprite` / OpenRaster), as the
   *  document is when called — possibly encoded off the main thread. */
  encode(): Uint8Array | null | Promise<Uint8Array | null>;
  /** Surface a failed snapshot write. */
  report(error: string): void;
}

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** The autosave loop of one open document. */
export class AutosaveSession {
  /** Identity of the document; its snapshot is keyed by it. */
  docId: string;

  readonly available = isIdbAvailable();
  private lastWriteDepth = 0;
  /** The file on disk lost part of the document (`keep()`): the snapshot
   *  stays current even at the saved depth, until a full save. */
  private retainAtSaved = false;
  private timer: ReturnType<typeof setInterval> | null = null;
  /** The snapshot write in flight, so `finish()` removes what it wrote. */
  private writing: Promise<void> | null = null;

  /** `docId` continues an earlier identity (a recent's id, or a recovered
   *  snapshot's); by default the document gets a fresh one. */
  constructor(
    private readonly host: AutosaveHost,
    docId: string = crypto.randomUUID(),
  ) {
    this.docId = docId;
    this.lastWriteDepth = host.depth() ?? 0;
  }

  /** Arm the timer. */
  start(): void {
    if (!this.available || this.timer !== null) return;
    this.timer = setInterval(() => void this.tick(), AUTOSAVE_INTERVAL_MS);
  }

  /** The document is left (its tab closed after "Don't Save" or while
   *  clean, or the editor unmounted): disarm the timer and drop its
   *  snapshot. */
  close(): void {
    void this.finish();
  }

  /** Disarm the timer and drop the document's snapshot, resolving once it
   *  is gone — after a write still in flight, so that write leaves nothing
   *  behind (document-tabs edge cases). A storage error is logged, never
   *  thrown: closing must not hang on it (unsaved-changes-prompt AC-05). */
  async finish(): Promise<void> {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    if (this.writing) await this.writing;
    await this.clear();
  }

  /** Write a snapshot if the document changed since the last one. */
  async tick(): Promise<void> {
    const depth = this.host.depth();
    if (!this.available || this.timer === null || depth === null || this.host.busy()) return;
    // One autosave at a time: a tick due while one is still encoding is
    // skipped, and the next tick saves the latest state
    // (performance-background-jobs AC-10).
    if (this.writing) return;
    if (depth === this.lastWriteDepth) return;
    if (depth === this.host.savedDepth() && !this.retainAtSaved) return;
    await this.write(depth);
  }

  /** The document was saved to a file that lost part of it (a flattened
   *  image): snapshot it now and keep the snapshot current, so recovery
   *  still restores every layer (save-original-format AC-13). */
  async keep(): Promise<void> {
    const depth = this.host.depth();
    if (!this.available || depth === null) return;
    this.retainAtSaved = true;
    await this.write(depth);
  }

  /** Encode the document and store it as the session's snapshot. */
  private write(depth: number): Promise<void> {
    const run = this.writeNow(depth).finally(() => {
      if (this.writing === run) this.writing = null;
    });
    this.writing = run;
    return run;
  }

  private async writeNow(depth: number): Promise<void> {
    try {
      const bytes = await this.host.encode();
      if (bytes === null) return;
      const format = this.host.format?.() ?? undefined;
      await writeSnapshot(this.docId, this.host.name(), bytes, this.host.mode, format);
      this.lastWriteDepth = depth;
    } catch (err) {
      console.error('autosave failed', err);
      this.host.report(message(err));
    }
  }

  /** The document was opened from a file (or created): drop a stale
   *  snapshot for its identity — the file is newer. */
  async opened(): Promise<void> {
    this.lastWriteDepth = this.host.depth() ?? 0;
    this.retainAtSaved = false;
    await this.clear();
  }

  /** The document was saved: nothing left to recover. */
  async saved(): Promise<void> {
    this.lastWriteDepth = this.host.depth() ?? 0;
    this.retainAtSaved = false;
    await this.clear();
  }

  private async clear(): Promise<void> {
    if (!this.available) return;
    try {
      await removeSnapshots(this.docId);
    } catch (err) {
      // Best-effort: the save / open itself succeeded.
      console.error('autosave clear failed', err);
    }
  }
}

/** What the shell does with a recovered snapshot. */
export interface RecoveryHost {
  /** A file operation is running; ignore Recover for now. */
  busy(): boolean;
  /** Open the snapshot's bytes as a new, unsaved tab of its mode that
   *  continues under `meta.docId` (document-tabs AC-15). */
  open(bytes: Uint8Array, meta: AutosaveSnapshotMeta): Promise<void>;
  /** Surface a failure (`action` is what failed). */
  report(action: 'recover' | 'discard', error: string): void;
}

/** The Recovery dialog's list: snapshots an earlier page load left. */
export class RecoveryOffer {
  /** Snapshots offered for recovery; empty once dealt with. */
  snapshots = $state<AutosaveSnapshotMeta[]>([]);
  /** Per-snapshot error from a failed recover / discard. */
  errors = $state<Record<string, string>>({});

  private live = false;
  private recovering = false;

  constructor(private readonly host: RecoveryHost) {}

  /** On the first editor start in this page load, list the snapshots of
   *  both modes an earlier page load left. */
  probe(): void {
    this.live = true;
    if (!isIdbAvailable() || probed) return;
    probed = true;
    listLatestSnapshots()
      .then((snaps) => {
        if (this.live) this.snapshots = snaps;
      })
      .catch((err: unknown) => console.error('autosave probe failed', err));
  }

  /** The offer is gone: a late probe result is dropped. */
  stop(): void {
    this.live = false;
  }

  /** Open `meta`'s snapshot as a new document; the dialog keeps the other
   *  snapshots (document-tabs AC-15). */
  async recover(meta: AutosaveSnapshotMeta): Promise<void> {
    if (this.recovering || this.host.busy()) return;
    this.recovering = true;
    try {
      const full = await latestSnapshot(meta.docId);
      if (!full) throw new Error('snapshot bytes not found');
      await this.host.open(full.bytes, meta);
      this.remove(meta.docId);
    } catch (err) {
      const msg = message(err);
      this.errors = { ...this.errors, [meta.docId]: msg };
      this.host.report('recover', msg);
    } finally {
      this.recovering = false;
    }
  }

  /** Delete a snapshot for good. */
  async discard(id: string): Promise<void> {
    try {
      await removeSnapshots(id);
    } catch (err) {
      const msg = message(err);
      console.error('discard snapshot failed', err);
      this.errors = { ...this.errors, [id]: msg };
      this.host.report('discard', msg);
      return;
    }
    this.remove(id);
  }

  /** Close the Recovery dialog; the snapshots stay for a later session. */
  dismiss(): void {
    this.snapshots = [];
    this.errors = {};
  }

  private remove(id: string): void {
    this.snapshots = this.snapshots.filter((s) => s.docId !== id);
    const errors = { ...this.errors };
    delete errors[id];
    this.errors = errors;
    if (this.snapshots.length === 0) this.dismiss();
  }
}
