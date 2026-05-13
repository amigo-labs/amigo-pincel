<script lang="ts">
  import type { AutosaveSnapshotMeta } from '../idb/autosave';

  // Modal shown on app boot when the IDB autosave store holds
  // snapshots from a prior session. The parent owns the snapshot
  // list and the load / discard side effects — this component is
  // purely presentational and emits callbacks.
  //
  // Each row is one doc's latest snapshot metadata (the bytes stay
  // on disk until the user clicks Recover). `onRecover` triggers a
  // bytes fetch + `Document.openAseprite` load and re-binds the
  // parent's `docId` to the snapshot's id so subsequent saves and
  // autosaves stay grouped under the same identity. `onDiscard`
  // drops every row for that docId. `onDismiss` closes the dialog
  // without touching the store — the snapshots survive to the next
  // boot. `errors` carries per-row failure messages so a failed
  // Recover / Discard surfaces against the offending row without
  // closing the dialog.
  let {
    snapshots,
    errors = {},
    onRecover,
    onDiscard,
    onDismiss,
  }: {
    snapshots: AutosaveSnapshotMeta[];
    errors?: Record<string, string>;
    onRecover: (snap: AutosaveSnapshotMeta) => void;
    onDiscard: (docId: string) => void;
    onDismiss: () => void;
  } = $props();

  function formatTimestamp(ts: number): string {
    const d = new Date(ts);
    return d.toLocaleString();
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(2)} MB`;
  }
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="recovery-title"
>
  <div
    class="flex w-full max-w-lg flex-col gap-4 rounded border border-neutral-700 bg-neutral-900 p-4 text-sm text-neutral-100 shadow-2xl"
  >
    <header class="flex items-baseline justify-between">
      <h2 id="recovery-title" class="text-base font-semibold">
        Recover unsaved work?
      </h2>
      <span class="text-xs text-neutral-400">
        {snapshots.length} snapshot{snapshots.length === 1 ? '' : 's'}
      </span>
    </header>
    <p class="text-xs text-neutral-400">
      Pincel found autosaved snapshots from a previous session. Recover
      a snapshot to continue editing, or discard it to drop it
      permanently.
    </p>
    <ul class="flex flex-col gap-2">
      {#each snapshots as snap (snap.docId)}
        <li
          class="flex flex-col gap-1 rounded border border-neutral-800 bg-neutral-950 px-3 py-2"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 flex-col">
              <span class="truncate font-medium" title={snap.name}>
                {snap.name}
              </span>
              <span class="text-xs text-neutral-500">
                {formatTimestamp(snap.ts)} · {formatBytes(snap.byteLength)}
              </span>
            </div>
            <div class="flex shrink-0 gap-1">
              <button
                class="recovery-btn recovery-btn-primary"
                onclick={() => onRecover(snap)}
              >
                Recover
              </button>
              <button
                class="recovery-btn"
                onclick={() => onDiscard(snap.docId)}
              >
                Discard
              </button>
            </div>
          </div>
          {#if errors[snap.docId]}
            <p class="text-xs text-rose-400" role="alert">
              {errors[snap.docId]}
            </p>
          {/if}
        </li>
      {/each}
    </ul>
    <footer class="flex justify-end">
      <button class="recovery-btn" onclick={onDismiss}>Not now</button>
    </footer>
  </div>
</div>

<style>
  .recovery-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.25rem 0.625rem;
    font-size: 0.75rem;
  }
  .recovery-btn:hover {
    background-color: rgb(38 38 38);
  }
  .recovery-btn-primary {
    background-color: rgb(37 99 235);
    border-color: rgb(59 130 246);
  }
  .recovery-btn-primary:hover {
    background-color: rgb(29 78 216);
  }
</style>
