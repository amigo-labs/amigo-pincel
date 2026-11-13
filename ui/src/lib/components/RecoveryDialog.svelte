<script lang="ts">
  import Button from './Button.svelte';
  import type { AutosaveSnapshotMeta } from '../idb/autosave';
  import { dateTime, decimal, t } from '../i18n/index.svelte';

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

  // In the UI language, not the browser's (docs/specs/more-languages.md AC-09).
  function formatTimestamp(ts: number): string {
    return dateTime(ts);
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${decimal(n / 1024, 1)} KB`;
    return `${decimal(n / (1024 * 1024), 2)} MB`;
  }
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="recovery-title"
>
  <div
    class="flex w-full max-w-lg flex-col gap-4 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-2xl"
  >
    <header class="flex items-baseline justify-between">
      <h2 id="recovery-title" class="text-base font-semibold">{t('recovery.title')}</h2>
      <span class="text-xs text-pc-muted">
        {t('recovery.count', { count: snapshots.length })}
      </span>
    </header>
    <p class="text-xs text-pc-muted">{t('recovery.body')}</p>
    <ul class="flex flex-col gap-2">
      {#each snapshots as snap (snap.docId)}
        <li class="flex flex-col gap-1 rounded border border-pc-border bg-pc-bg px-3 py-2">
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 flex-col">
              <span class="truncate font-medium" title={snap.name}>
                {snap.name}
              </span>
              <span class="text-xs text-pc-faint">
                {t(`mode.${snap.mode ?? 'pixel'}`)} · {formatTimestamp(snap.ts)} · {formatBytes(
                  snap.byteLength,
                )}
              </span>
            </div>
            <div class="flex shrink-0 gap-1">
              <Button variant="primary" onclick={() => onRecover(snap)}>
                {t('recovery.recover')}
              </Button>
              <Button variant="danger" onclick={() => onDiscard(snap.docId)}>
                {t('recovery.discard')}
              </Button>
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
      <Button onclick={onDismiss}>{t('recovery.notNow')}</Button>
    </footer>
  </div>
</div>
