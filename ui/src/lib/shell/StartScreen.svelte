<script lang="ts">
  // Landing view of the two-mode shell: pick an editor (new pixel sprite /
  // new image), open a file, or re-open a recent one. Dropping a file
  // anywhere on the screen opens it. See docs/specs/pincel.md §9.2.
  import type { RecentFile } from '../idb/recent-files';
  import type { EditorMode } from './types';
  import { LOCALES, isLocale, locale, t } from '../i18n/index.svelte';
  import { setLocale } from '../i18n/locale';

  let {
    recents,
    busy = false,
    onNew,
    onOpen,
    onOpenRecent,
    onDropFile,
  }: {
    recents: RecentFile[];
    busy?: boolean;
    onNew: (mode: EditorMode) => void;
    onOpen: () => void;
    onOpenRecent: (recent: RecentFile) => void;
    onDropFile: (file: File) => void;
  } = $props();

  let dragOver = $state(false);

  function onDragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
    dragOver = true;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    const file = e.dataTransfer?.files[0];
    if (file) onDropFile(file);
  }

</script>

<main
  class="flex h-full flex-col items-center justify-center bg-pc-bg p-6 text-pc-text"
  class:bg-pc-panel={dragOver}
  ondragover={onDragOver}
  ondragleave={() => (dragOver = false)}
  ondrop={onDrop}
>
  <section
    class="w-full max-w-xl rounded border border-pc-border bg-pc-panel p-6 shadow-xl"
    aria-labelledby="start-title"
  >
    <div class="flex items-start justify-between gap-3">
      <h1 id="start-title" class="text-2xl font-semibold tracking-wide">Pincel</h1>
      <select
        class="opt-field text-xs"
        aria-label={t('start.language')}
        value={locale()}
        onchange={(e) => {
          const v = e.currentTarget.value;
          if (isLocale(v)) setLocale(v);
        }}
      >
        {#each LOCALES as l (l.id)}
          <option value={l.id}>{l.label}</option>
        {/each}
      </select>
    </div>
    <p class="mt-1 text-sm text-pc-muted">{t('start.tagline')}</p>

    <div class="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-2">
      <button
        class="start-card"
        onclick={() => onNew('pixel')}
        disabled={busy}
        aria-label={t('start.newPixel')}
      >
        <span class="text-base font-semibold">{t('start.newPixel')}</span>
        <span class="text-xs text-pc-muted">{t('start.newPixelHint')}</span>
      </button>
      <button
        class="start-card"
        onclick={() => onNew('image')}
        disabled={busy}
        aria-label={t('start.newImage')}
      >
        <span class="text-base font-semibold">{t('start.newImage')}</span>
        <span class="text-xs text-pc-muted">{t('start.newImageHint')}</span>
      </button>
    </div>

    <div class="mt-4 flex items-center gap-2">
      <button class="toolbar-btn" onclick={onOpen} disabled={busy}>{t('start.open')}</button>
      <span class="text-xs text-pc-faint">{t('start.openHint')}</span>
    </div>

    {#if recents.length > 0}
      <h2 class="mt-6 text-xs font-semibold uppercase tracking-wide text-pc-faint">{t('start.recent')}</h2>
      <ul class="mt-2 flex flex-col divide-y divide-pc-border" aria-label={t('start.recentFiles')}>
        {#each recents as r (r.id)}
          <li>
            <button
              class="flex w-full items-center justify-between gap-3 px-2 py-1.5 text-left text-sm hover:bg-pc-hover disabled:opacity-50"
              onclick={() => onOpenRecent(r)}
              disabled={busy}
              title={r.path ?? r.name}
            >
              <span class="truncate">{r.name}</span>
              <span
                class="shrink-0 rounded border border-pc-border px-1.5 text-[10px] uppercase text-pc-muted"
              >
                {t(`mode.${r.mode}`)}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</main>

<style>
  .start-card {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    border-radius: 0.375rem;
    border: 1px solid rgb(64 64 64);
    padding: 1rem;
    text-align: left;
  }
  .start-card:hover:not(:disabled) {
    background-color: rgb(38 38 38);
    border-color: rgb(115 115 115);
  }
  .start-card:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .toolbar-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.125rem 0.5rem;
    font-size: 0.75rem;
  }
  .toolbar-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
