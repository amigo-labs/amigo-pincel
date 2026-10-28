<script lang="ts">
  // Landing view of the two-mode shell: pick an editor (new pixel sprite /
  // new image), open a file, or re-open a recent one. Dropping a file
  // anywhere on the screen opens it. See docs/specs/pincel.md §9.2.
  //
  // The two New cards are an accordion: a card opens in place with the
  // canvas size and Create, the other one closes. No dialog on top.
  import type { Attachment } from 'svelte/attachments';
  import type { RecentFile } from '../idb/recent-files';
  import type { EditorMode, NewDocParams } from './types';
  import Icon from '../components/icons/Icon.svelte';
  import { openAbout } from '../help/state.svelte';
  import { LOCALES, isLocale, locale, t } from '../i18n/index.svelte';
  import { setLocale } from '../i18n/locale';
  import { hasNativeMenu } from '../platform';
  import { NEW_DOC_LIMITS, clipboardDocSize, newDocParams } from './new-doc';
  import { autoNew, setAutoNew } from './auto-new.svelte';

  let {
    recents,
    busy = false,
    onCreate,
    onOpen,
    onOpenRecent,
    onDropFile,
  }: {
    recents: RecentFile[];
    busy?: boolean;
    onCreate: (params: NewDocParams) => void;
    onOpen: () => void;
    onOpenRecent: (recent: RecentFile) => void;
    onDropFile: (file: File) => void;
  } = $props();

  let dragOver = $state(false);

  const CARDS = [
    { mode: 'pixel', title: 'start.newPixel', hint: 'start.newPixelHint' },
    { mode: 'image', title: 'start.newImage', hint: 'start.newImageHint' },
  ] as const;

  // The open New card and its size fields; `null` while both are closed.
  let expanded = $state<EditorMode | null>(null);
  let width = $state(0);
  let height = $state(0);

  function toggle(mode: EditorMode) {
    if (expanded === mode) {
      expanded = null;
      return;
    }
    expanded = mode;
    const { width: w, height: h } = NEW_DOC_LIMITS[mode];
    width = w;
    height = h;
    // A clipboard image sets the size, unless the user typed one meanwhile.
    void clipboardDocSize(mode).then((p) => {
      if (p && expanded === mode && width === w && height === h) {
        width = p.width;
        height = p.height;
      }
    });
  }

  // Escape closes the open card and puts focus back on its header.
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || expanded === null) return;
    e.preventDefault();
    const mode = expanded;
    expanded = null;
    if (mode) document.getElementById(`new-${mode}-head`)?.focus();
  }

  // Focus the width field when a card opens — with a mouse or pen only, so
  // a phone doesn't push its keyboard over the card.
  const focusOnFinePointer: Attachment<HTMLInputElement> = (input) => {
    if (window.matchMedia('(pointer: fine)').matches) input.select();
  };

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

<svelte:window onkeydown={onWindowKeydown} />

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
      <div class="flex items-center gap-2">
        <!-- No menu bar is mounted here, so About needs its own way in
             wherever the OS draws no native Help menu (spec §11.3). -->
        {#if !hasNativeMenu()}
          <button class="panel-btn text-xs" onclick={openAbout}>{t('menu.about')}</button>
        {/if}
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
    </div>
    <p class="mt-1 text-sm text-pc-muted">{t('start.tagline')}</p>

    <div class="mt-6 flex flex-col gap-3">
      {#each CARDS as card (card.mode)}
        {@const open = expanded === card.mode}
        <div class="start-card" class:start-card-open={open}>
          <button
            id={`new-${card.mode}-head`}
            type="button"
            class="start-card-head"
            onclick={() => toggle(card.mode)}
            disabled={busy}
            aria-label={t(card.title)}
            aria-expanded={open}
            aria-controls={`new-${card.mode}-form`}
          >
            <span class="flex flex-col gap-1">
              <span class="text-base font-semibold">{t(card.title)}</span>
              <span class="text-xs text-pc-muted">{t(card.hint)}</span>
            </span>
            <Icon
              name="chevron-down"
              size={18}
              class={`shrink-0 text-pc-muted transition-transform ${open ? 'rotate-180' : ''}`}
            />
          </button>
          {#if open}
            <form
              id={`new-${card.mode}-form`}
              class="flex flex-wrap items-center gap-2 px-4 pb-4"
              aria-label={t(card.title)}
              onsubmit={(e) => {
                e.preventDefault();
                const params = newDocParams(card.mode, width, height);
                if (autoNew.params?.mode === card.mode) setAutoNew(params);
                onCreate(params);
              }}
            >
              <label class="flex items-center gap-1 text-xs text-pc-muted">
                {t('newDoc.w')}
                <input
                  type="number"
                  min="1"
                  max={NEW_DOC_LIMITS[card.mode].max}
                  step="1"
                  inputmode="numeric"
                  bind:value={width}
                  {@attach focusOnFinePointer}
                  class="w-20 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-sm text-pc-text"
                  aria-label={t('common.width')}
                />
              </label>
              <span class="text-pc-faint">×</span>
              <label class="flex items-center gap-1 text-xs text-pc-muted">
                {t('newDoc.h')}
                <input
                  type="number"
                  min="1"
                  max={NEW_DOC_LIMITS[card.mode].max}
                  step="1"
                  inputmode="numeric"
                  bind:value={height}
                  class="w-20 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-sm text-pc-text"
                  aria-label={t('common.height')}
                />
              </label>
              <span class="text-xs text-pc-faint">px</span>
              <button type="submit" class="panel-btn panel-btn-primary ml-auto" disabled={busy}>
                {t('common.create')}
              </button>
              <label class="flex basis-full items-center gap-2 text-xs text-pc-muted">
                <input
                  type="checkbox"
                  checked={autoNew.params?.mode === card.mode}
                  onchange={(e) =>
                    setAutoNew(
                      e.currentTarget.checked ? newDocParams(card.mode, width, height) : null,
                    )}
                />
                {t('newDoc.autoOpen')}
              </label>
            </form>
          {/if}
        </div>
      {/each}
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
    border-radius: 0.375rem;
    border: 1px solid var(--color-pc-border);
  }
  .start-card:hover:not(.start-card-open):has(.start-card-head:enabled) {
    background-color: var(--color-pc-hover);
    border-color: var(--color-pc-faint);
  }
  .start-card-open {
    background-color: var(--color-pc-raised);
    border-color: var(--color-pc-faint);
  }
  .start-card-head {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 1rem;
    text-align: left;
  }
  .start-card-head:disabled {
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
