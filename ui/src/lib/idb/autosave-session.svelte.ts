// One autosave / recovery loop for both editors (spec §10, M10.3). Every
// `AUTOSAVE_INTERVAL_MS` an edited, unsaved document is encoded by its
// editor and written to the IDB snapshot store under the session's
// `docId`; a save or a fresh open drops it. The first editor of a mode in a
// page load lists the snapshots an earlier page load left (crash, closed
// tab) for the Recovery dialog; later New / Open in the same page do not
// ask again.
//
// Behaviour both modes share:
// - A clean document (undo depth at the last save) is never written.
// - A document swapped during the async write keeps its own baseline.
// - A recovered document counts as unsaved, and its snapshot stays until
//   the user saves — closing the tab right after recovering loses nothing.
// - Discarding the last listed snapshot closes the dialog.
// - Leaving a document (New / Open after "Discard changes", or the editor
//   unmounting) deletes its snapshots: the user chose to drop it.

import type { EditorMode } from '../shell/types';
import {
  latestSnapshot,
  listLatestSnapshots,
  removeSnapshots,
  writeSnapshot,
  type AutosaveSnapshotMeta,
} from './autosave';
import { isIdbAvailable } from './db';

/** Modes whose Recovery probe already ran in this page load. */
const probedModes: Partial<Record<EditorMode, true>> = {};

/** How often an edited document is snapshotted. */
export const AUTOSAVE_INTERVAL_MS = 30_000;

/** What an editor tells the session about its document. */
export interface AutosaveHost {
  mode: EditorMode;
  /** Undo depth of the open document, or `null` while there is none. */
  depth(): number | null;
  /** Undo depth at the last save; equal to `depth()` means clean. */
  savedDepth(): number;
  /** A file operation is running; skip this tick / recovery. */
  busy(): boolean;
  /** Display name stored with the snapshot. */
  name(): string;
  /** File format stored with the snapshot, for editors that track one
   *  apart from the name (`AutosaveSnapshotMeta.format`). */
  format?(): string | null;
  /** The document's project bytes (`.aseprite` / OpenRaster). */
  encode(): Uint8Array | null;
  /** Replace the open document with recovered bytes and mark it unsaved. */
  open(bytes: Uint8Array, meta: AutosaveSnapshotMeta): Promise<void>;
  /** Surface a failure (`action` is what failed). */
  report(action: 'autosave' | 'recover' | 'discard', error: string): void;
}

export class AutosaveSession {
  /** Snapshots offered for recovery; empty once dealt with. */
  snapshots = $state<AutosaveSnapshotMeta[]>([]);
  /** Per-snapshot error from a failed recover / discard. */
  errors = $state<Record<string, string>>({});
  /** Identity of the open document; snapshots are keyed by it. */
  docId: string = crypto.randomUUID();

  readonly available = isIdbAvailable();
  private lastWriteDepth = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  private recovering = false;

  constructor(private readonly host: AutosaveHost) {}

  /** Arm the timer; on the mode's first mount in this page load, also
   *  probe for snapshots an earlier page load left. */
  start(): void {
    if (!this.available || this.timer !== null) return;
    if (!probedModes[this.host.mode]) {
      probedModes[this.host.mode] = true;
      listLatestSnapshots(this.host.mode)
        .then((snaps) => {
          if (this.timer !== null) this.snapshots = snaps;
        })
        .catch((err: unknown) => console.error('autosave probe failed', err));
    }
    this.timer = setInterval(() => void this.tick(), AUTOSAVE_INTERVAL_MS);
  }

  /** Editor unmount: disarm the timer and drop the open document's
   *  snapshots — the shell only remounts after "Discard changes" or on a
   *  clean document. A page unload never unmounts, so a crash or a closed
   *  tab still leaves the snapshot for recovery. */
  close(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    void this.clear();
  }

  /** Write a snapshot if the document changed since the last one. */
  async tick(): Promise<void> {
    const depth = this.host.depth();
    if (!this.available || this.timer === null || depth === null || this.host.busy()) return;
    if (depth === this.lastWriteDepth || depth === this.host.savedDepth()) return;
    await this.write(depth);
  }

  /** The document was saved to a file that lost part of it (a flattened
   *  image): snapshot it now and keep the snapshot, so recovery still
   *  restores every layer (save-original-format AC-13). */
  async keep(): Promise<void> {
    const depth = this.host.depth();
    if (!this.available || depth === null) return;
    await this.write(depth);
  }

  /** Encode the document and store it as the session's snapshot. */
  private async write(depth: number): Promise<void> {
    const id = this.docId;
    try {
      const bytes = this.host.encode();
      if (bytes === null) return;
      const format = this.host.format?.() ?? undefined;
      await writeSnapshot(id, this.host.name(), bytes, this.host.mode, format);
      // A New / Open may have swapped the document during the await; its
      // own baseline must not be clobbered with this stale depth.
      if (this.docId === id) this.lastWriteDepth = depth;
    } catch (err) {
      console.error('autosave failed', err);
      this.host.report('autosave', err instanceof Error ? err.message : String(err));
    }
  }

  /** A document was opened or created: give it an identity (a recent's
   *  id keeps re-opens grouped) and drop stale snapshots for it and for
   *  the document it replaced (discarded, or clean). */
  async opened(id: string = crypto.randomUUID()): Promise<void> {
    const previous = this.docId;
    this.docId = id;
    this.lastWriteDepth = this.host.depth() ?? 0;
    if (previous !== id) await this.clear(previous);
    await this.clear();
  }

  /** The document was saved: nothing left to recover. */
  async saved(): Promise<void> {
    this.lastWriteDepth = this.host.depth() ?? 0;
    await this.clear();
  }

  /** Load `meta`'s snapshot into the editor and continue under its id. */
  async recover(meta: AutosaveSnapshotMeta): Promise<void> {
    if (this.recovering || this.host.busy()) return;
    this.recovering = true;
    try {
      const full = await latestSnapshot(meta.docId);
      if (!full) throw new Error('snapshot bytes not found');
      await this.host.open(full.bytes, meta);
      this.docId = meta.docId;
      this.lastWriteDepth = this.host.depth() ?? 0;
      this.dismiss();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
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
      const msg = err instanceof Error ? err.message : String(err);
      console.error('discard snapshot failed', err);
      this.errors = { ...this.errors, [id]: msg };
      this.host.report('discard', msg);
      return;
    }
    this.snapshots = this.snapshots.filter((s) => s.docId !== id);
    const errors = { ...this.errors };
    delete errors[id];
    this.errors = errors;
    if (this.snapshots.length === 0) this.dismiss();
  }

  /** Close the Recovery dialog; the snapshots stay for a later session. */
  dismiss(): void {
    this.snapshots = [];
    this.errors = {};
  }

  private async clear(id: string = this.docId): Promise<void> {
    if (!this.available) return;
    try {
      await removeSnapshots(id);
    } catch (err) {
      // Best-effort: the save / open itself succeeded.
      console.error('autosave clear failed', err);
    }
  }
}
