// A document snapshot handed to the job worker in parts
// (docs/specs/performance-background-jobs.md AC-04). The page freezes the
// document first — a copy that shares its pixels, so it costs no pixel copy
// and later edits stay out of it — and then copies one layer per task into
// its own buffer, so no frame copies the whole document. The buffers are
// transferred to the worker, not cloned.

import { core } from '../core/wasm';

/** A snapshot in parts: `core.snapshotHeader`, each layer's pixels, the
 *  selection's coverage. */
export interface SnapshotParts {
  readonly header: Uint8Array<ArrayBuffer>;
  readonly layers: readonly Uint8Array<ArrayBuffer>[];
  readonly selection: Uint8Array<ArrayBuffer> | null;
}

/** The buffers `parts` hands over by transfer. */
export function partsTransfer(parts: SnapshotParts): ArrayBuffer[] {
  const buffers = [parts.header.buffer, ...parts.layers.map((l) => l.buffer)];
  if (parts.selection) buffers.push(parts.selection.buffer);
  return buffers;
}

/** Yields to the event loop, so a frame can be drawn between two copies. */
function nextTask(): Promise<void> {
  return new Promise((resolve) => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      channel.port1.close();
      resolve();
    };
    channel.port2.postMessage(null);
  });
}

/** Copies frozen document `frozen` (`core.freezeDocument`) out in parts,
 *  one layer per task. */
export async function readParts(frozen: number): Promise<SnapshotParts> {
  const header = new Uint8Array(core.snapshotHeader(frozen));
  const count = core.documentInfo(frozen).layer_count;
  const layers: Uint8Array<ArrayBuffer>[] = [];
  for (let i = 0; i < count; i++) {
    await nextTask();
    layers.push(core.snapshotLayer(frozen, i));
  }
  return { header, layers, selection: core.snapshotSelection(frozen) };
}

/** Opens `parts` as a new document in this thread's core. */
export function openParts(parts: SnapshotParts): number {
  return core.openSnapshotParts(parts.header, [...parts.layers], parts.selection);
}
