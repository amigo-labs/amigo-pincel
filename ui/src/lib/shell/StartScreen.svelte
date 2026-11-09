<script lang="ts">
  // Landing view of the two-mode shell: pick an editor (new pixel sprite /
  // new image), open a file, or re-open a recent one. Dropping a file
  // anywhere on the screen opens it; the shell watches the drop
  // (`lib/fs/drop`) and says when to show the drop state. See
  // docs/specs/pincel.md §9.2.
  //
  // The ways in are one grid of cards: the two New cards large in the first
  // row, Open… and Open a sample half-height in the second. What a card
  // opens (a New card's size form, the samples) spans the grid width right
  // under its row. One New card open at a time, no dialog on top.
  import type { Attachment } from 'svelte/attachments';
  import type { RecentFile } from '../idb/recent-files';
  import type { EditorMode, NewDocParams } from './types';
  import Icon from '../components/icons/Icon.svelte';
  import { openAbout } from '../help/state.svelte';
  import { LOCALES, isLocale, locale, t } from '../i18n/index.svelte';
  import { setLocale } from '../i18n/locale';
  import { hasNativeMenu } from '../platform';
  import {
    NEW_DOC_LIMITS,
    clipboardDocSize,
    newDocParams,
    prefillKeepingSelection,
  } from './new-doc';
  import { autoNew, setAutoNew } from './auto-new.svelte';
  import { answerBanner, bannerShown, startTour } from '../tour/tour-state.svelte';

  let {
    recents,
    busy = false,
    unsupported = false,
    onCreate,
    onOpen,
    onOpenRecent,
    onOpenSample = () => {},
    dropHover = false,
  }: {
    recents: RecentFile[];
    busy?: boolean;
    /** The browser lacks wasm SIMD: a notice, and New / Open stay off
     *  (performance-build-simd AC-07). */
    unsupported?: boolean;
    onCreate: (params: NewDocParams) => void;
    onOpen: () => void;
    onOpenRecent: (recent: RecentFile) => void;
    /** "Open a sample" picked the sprite or the image sample
     *  (docs/specs/first-run-tutorial.md AC-13, AC-14). */
    onOpenSample?: (sample: 'sprite' | 'image') => void;
    /** A file drag is over the screen: show the drop state. */
    dropHover?: boolean;
  } = $props();

  const CARDS = [
    { mode: 'pixel', title: 'start.newPixel', hint: 'start.newPixelHint', icon: 'mode-pixel' },
    { mode: 'image', title: 'start.newImage', hint: 'start.newImageHint', icon: 'mode-image' },
  ] as const;

  const blocked = $derived(busy || unsupported);

  // The open New card and its size fields; `null` while both are closed.
  let expanded = $state<EditorMode | null>(null);
  let samplesOpen = $state(false);
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
        prefillKeepingSelection(() => {
          width = p.width;
          height = p.height;
        });
      }
    });
  }

  // Escape closes the open form and puts focus back on its card.
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || expanded === null) return;
    e.preventDefault();
    const mode = expanded;
    expanded = null;
    if (mode) document.getElementById(`new-${mode}-head`)?.focus();
  }

  // Focus the width field when a form opens — with a mouse or pen only, so
  // a phone doesn't push its keyboard over the form.
  const focusOnFinePointer: Attachment<HTMLInputElement> = (input) => {
    if (window.matchMedia('(pointer: fine)').matches) input.select();
  };
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main
  class="flex min-h-full flex-col items-center bg-pc-bg px-4 pt-8 pb-12 text-pc-text sm:px-8 sm:pt-12 lg:pt-24"
  class:bg-pc-panel={dropHover}
  class:start-drop={dropHover}
>
  <section class="w-full max-w-[44rem]" aria-labelledby="start-title" aria-busy={busy}>
    <!-- Title left, utilities right on the h1 line; on phones the utility
         row sits above the title. -->
    <div class="flex flex-col-reverse gap-4 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
      <div class="min-w-0">
        <h1 id="start-title" class="text-[1.75rem] leading-9 font-semibold tracking-tight">
          Pincel
        </h1>
        <p class="mt-2 max-w-[30rem] text-sm leading-5 text-pc-muted">{t('start.tagline')}</p>
      </div>
      <!-- No menu bar is mounted here, so About needs its own way in
           wherever the OS draws no native Help menu (spec §11.3). -->
      <div class="flex flex-wrap items-center justify-end gap-2 self-end sm:h-9 sm:shrink-0 sm:self-start">
        {#if !hasNativeMenu()}
          <span class="flex items-center gap-2" data-tour="start-help">
            <button class="panel-btn leading-4 pointer-coarse:min-h-11" onclick={() => startTour('start')}
              >{t('menu.tour')}</button
            >
            <button class="panel-btn leading-4 pointer-coarse:min-h-11" onclick={openAbout}
              >{t('menu.about')}</button
            >
          </span>
        {/if}
        <select
          class="panel-btn leading-4 pointer-coarse:min-h-11"
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

    {#if bannerShown() || unsupported}
      <div class="mt-6 flex flex-col gap-3">
        {#if bannerShown()}
          <div
            class="flex flex-wrap items-center gap-x-4 gap-y-3 rounded-lg border border-pc-border bg-pc-panel px-4 py-3"
            role="region"
            aria-label={t('tour.title')}
          >
            <span class="min-w-[12rem] flex-1 text-sm leading-5">{t('tour.bannerText')}</span>
            <span class="flex gap-2">
              <button
                class="panel-btn panel-btn-primary leading-4 pointer-coarse:min-h-11"
                onclick={() => answerBanner(true)}>{t('tour.bannerTake')}</button
              >
              <button
                class="panel-btn leading-4 pointer-coarse:min-h-11"
                onclick={() => answerBanner(false)}>{t('tour.bannerDecline')}</button
              >
            </span>
          </div>
        {/if}
        {#if unsupported}
          <p
            role="alert"
            class="rounded-lg border border-pc-border bg-pc-panel px-4 py-3 text-sm leading-5"
          >
            {t('start.unsupportedBrowser')}
          </p>
        {/if}
      </div>
    {/if}

    <!-- Row 1: the two New cards, large; the open one's size form spans the
         grid below them (on phones it follows its own card). -->
    <div class="start-grid mt-6" data-tour="modes">
      {#each CARDS as card (card.mode)}
        {@const open = expanded === card.mode}
        <button
          id={`new-${card.mode}-head`}
          type="button"
          class="start-card start-card-new"
          class:start-card-open={open}
          class:start-cell-1={card.mode === 'pixel'}
          class:start-cell-2={card.mode === 'image'}
          onclick={() => toggle(card.mode)}
          disabled={blocked}
          aria-label={t(card.title)}
          aria-expanded={open}
          aria-controls={`new-${card.mode}-form`}
        >
          <span class="start-card-icon">
            <Icon name={card.icon} size={24} class={open ? 'text-pc-accent-text' : 'text-pc-muted'} />
          </span>
          <span class="start-card-text">
            <span class="block text-base leading-5 font-semibold">{t(card.title)}</span>
            <span class="mt-1 block text-[13px] leading-5 text-pc-muted">{t(card.hint)}</span>
          </span>
          <span class="start-card-chev">
            <Icon
              name="chevron-down"
              size={16}
              class={`text-pc-faint transition-transform ${open ? 'rotate-180' : ''}`}
            />
          </span>
        </button>
        {#if open}
          <!-- novalidate: an out-of-range size is clamped by `newDocParams`,
               not refused by the browser's min / max check (app-shell AC-06). -->
          <form
            id={`new-${card.mode}-form`}
            class="start-panel start-span"
            aria-label={t(card.title)}
            novalidate
            onsubmit={(e) => {
              e.preventDefault();
              const params = newDocParams(card.mode, width, height);
              if (autoNew.params?.mode === card.mode) setAutoNew(params);
              onCreate(params);
            }}
          >
            <div class="flex flex-wrap items-center gap-3">
              <span class="flex items-center gap-2">
                <label class="flex items-center gap-2 text-xs leading-4 text-pc-muted">
                  {t('newDoc.w')}
                  <input
                    type="number"
                    min="1"
                    max={NEW_DOC_LIMITS[card.mode].max}
                    step="1"
                    inputmode="numeric"
                    bind:value={width}
                    {@attach focusOnFinePointer}
                    class="opt-field h-9 w-20 px-2 text-sm pointer-coarse:h-11"
                    aria-label={t('common.width')}
                  />
                </label>
                <span class="text-xs leading-4 text-pc-faint">×</span>
                <label class="flex items-center gap-2 text-xs leading-4 text-pc-muted">
                  {t('newDoc.h')}
                  <input
                    type="number"
                    min="1"
                    max={NEW_DOC_LIMITS[card.mode].max}
                    step="1"
                    inputmode="numeric"
                    bind:value={height}
                    class="opt-field h-9 w-20 px-2 text-sm pointer-coarse:h-11"
                    aria-label={t('common.height')}
                  />
                </label>
                <span class="text-xs leading-4 text-pc-faint">px</span>
              </span>
              <button
                type="submit"
                class="panel-btn panel-btn-primary h-9 w-full px-4 text-sm leading-5 min-[35rem]:ml-auto min-[35rem]:w-auto pointer-coarse:h-11"
                disabled={blocked}
              >
                {t('common.create')}
              </button>
            </div>
            <label
              class="mt-3 flex items-center gap-2 text-xs leading-4 text-pc-muted pointer-coarse:min-h-11"
            >
              <input
                type="checkbox"
                checked={autoNew.params?.mode === card.mode}
                onchange={(e) =>
                  setAutoNew(e.currentTarget.checked ? newDocParams(card.mode, width, height) : null)}
              />
              {t('newDoc.autoOpen')}
            </label>
          </form>
        {/if}
      {/each}
    </div>

    <!-- Row 2: open a file or a sample — half-height, secondary. -->
    <div class="start-grid mt-3">
      <button
        class="start-card start-card-sub start-cell-1"
        onclick={onOpen}
        disabled={blocked}
        data-tour="open"
      >
        {t('start.open')}
        <Icon name="chevron-right" size={16} class="shrink-0 text-pc-faint" />
      </button>
      <button
        class="start-card start-card-sub start-cell-2"
        class:start-card-open={samplesOpen}
        aria-expanded={samplesOpen}
        disabled={blocked}
        data-tour="samples"
        onclick={() => (samplesOpen = !samplesOpen)}
      >
        {t('start.openSample')}
        <Icon
          name="chevron-down"
          size={16}
          class={`shrink-0 text-pc-faint transition-transform ${samplesOpen ? 'rotate-180' : ''}`}
        />
      </button>
      {#if samplesOpen}
        <div
          class="start-panel start-span grid gap-2 min-[35rem]:grid-cols-2"
          role="group"
          aria-label={t('start.openSample')}
        >
          <button
            class="panel-btn h-9 justify-start gap-2 px-3 text-sm leading-5 pointer-coarse:h-11"
            disabled={blocked}
            onclick={() => onOpenSample('sprite')}
            ><Icon name="mode-pixel" size={16} class="shrink-0 text-pc-muted" />{t(
              'start.sampleSprite',
            )}</button
          >
          <button
            class="panel-btn h-9 justify-start gap-2 px-3 text-sm leading-5 pointer-coarse:h-11"
            disabled={blocked}
            onclick={() => onOpenSample('image')}
            ><Icon name="mode-image" size={16} class="shrink-0 text-pc-muted" />{t(
              'start.sampleImage',
            )}</button
          >
        </div>
      {/if}
    </div>
    <p class="mt-3 max-w-[30rem] text-[13px] leading-5 text-pc-faint">{t('start.openHint')}</p>

    {#if recents.length > 0}
      <!-- Recent: a compact table on the grid's edges — caption row, then
           one row per file (name | mode). -->
      <div class="mt-8 overflow-hidden rounded-lg border border-pc-border bg-pc-panel">
        <h2
          class="border-b border-pc-border px-4 py-2 text-[13px] leading-5 font-semibold text-pc-muted"
        >
          {t('start.recent')}
        </h2>
        <ul class="flex flex-col divide-y divide-pc-border" aria-label={t('start.recentFiles')}>
          {#each recents as r (r.id)}
            <li>
              <button
                class="start-row"
                onclick={() => onOpenRecent(r)}
                disabled={blocked}
                title={r.path ?? r.name}
              >
                <span class="min-w-0 truncate">{r.name}</span>
                <span
                  class="w-14 rounded border border-pc-border text-center text-xs leading-4 text-pc-muted"
                >
                  {t(`mode.${r.mode}`)}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </section>
</main>

<style>
  .start-drop {
    outline: 2px dashed var(--color-pc-accent);
    outline-offset: -0.75rem;
  }
  /* Cards lift off the page while a file is dragged over it. */
  .start-drop .start-card:enabled:not(.start-card-open) {
    background-color: var(--color-pc-raised);
  }

  /* One column on phones; two from 560 px, where row 1 holds the cards and
     an opened panel spans both columns below them. Cards keep their own
     height (top-aligned, never stretched). */
  .start-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: start;
    gap: 0.75rem;
  }
  @media (min-width: 35rem) {
    .start-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .start-cell-1 {
      grid-row: 1;
      grid-column: 1;
    }
    .start-cell-2 {
      grid-row: 1;
      grid-column: 2;
    }
    .start-span {
      grid-row: 2;
      grid-column: 1 / -1;
    }
  }

  .start-card {
    min-width: 0;
    border-radius: 0.5rem;
    border: 1px solid var(--color-pc-border);
    background-color: var(--color-pc-panel);
    color: var(--color-pc-text);
    text-align: left;
    overflow-wrap: break-word;
  }
  .start-card:hover:enabled {
    background-color: var(--color-pc-hover);
    border-color: var(--color-pc-faint);
  }
  .start-card:active:enabled {
    background-color: var(--color-pc-active);
  }
  .start-card-open {
    background-color: var(--color-pc-raised);
    border-color: var(--color-pc-faint);
  }
  /* Disabled: the same look as a disabled panel-btn — outline only. */
  .start-card:disabled,
  .start-row:disabled {
    background-color: transparent;
    color: var(--color-pc-faint);
    opacity: 0.7;
    cursor: not-allowed;
  }

  /* Large New card. Phone: icon | text | chevron in one row; from 560 px
     icon and chevron on top, title and hint below. The icon and chevron
     boxes are as tall as the title line, so they centre on it. */
  .start-card-new {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas: 'icon text chev';
    column-gap: 0.75rem;
    align-items: start;
    padding: 1rem;
  }
  .start-card-icon,
  .start-card-chev {
    display: flex;
    height: 1.25rem;
    align-items: center;
  }
  .start-card-icon {
    grid-area: icon;
  }
  .start-card-chev {
    grid-area: chev;
  }
  .start-card-text {
    grid-area: text;
    min-width: 0;
  }
  @media (min-width: 35rem) {
    .start-card-new {
      grid-template-areas:
        'icon . chev'
        'text text text';
      row-gap: 0.75rem;
    }
    .start-card-icon,
    .start-card-chev {
      height: 1.5rem;
    }
  }

  /* Half-height secondary card: label and its affordance on one line. */
  .start-card-sub {
    display: flex;
    min-height: 3.5rem;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0 1rem;
    font-size: 0.875rem;
    line-height: 1.25rem;
    font-weight: 450;
  }

  /* What a card opens: the New size form, the samples. */
  .start-panel {
    border-radius: 0.5rem;
    border: 1px solid var(--color-pc-faint);
    background-color: var(--color-pc-raised);
    padding: 1rem;
  }

  .start-row {
    display: grid;
    width: 100%;
    min-height: 2.25rem;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    text-align: left;
    font-size: 0.875rem;
    line-height: 1.25rem;
  }
  .start-row:hover:enabled {
    background-color: var(--color-pc-hover);
  }
  .start-row:active:enabled {
    background-color: var(--color-pc-active);
  }
  @media (pointer: coarse) {
    .start-row {
      min-height: 2.75rem;
    }
  }
</style>
