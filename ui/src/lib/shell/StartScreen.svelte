<script lang="ts">
  // Landing view of the two-mode shell: pick an editor (new pixel sprite /
  // new image), open a file, or re-open a recent one. Dropping a file
  // anywhere on the screen opens it; the shell watches the drop
  // (`lib/fs/drop`) and says when to show the drop state. See
  // docs/specs/pincel.md §9.2.
  //
  // The layout follows the person's history: on a first run one calm
  // column with the two New cards as the hero and Open… / samples under
  // them; once there are recent files, two columns from 960 px — start new
  // on the left, pick up where you left off (Open…, Recent, samples) on the
  // right. The open New card's size form is joined to its card. One New
  // card open at a time, no dialog on top.
  import type { Snippet } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import type { RecentFile } from '../idb/recent-files';
  import type { EditorMode, NewDocParams } from './types';
  import Icon from '../components/icons/Icon.svelte';
  import { openAbout, openSettings } from '../help/state.svelte';
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
    onOpenClipboard = () => {},
    clipboardCard,
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
    /** "Open from clipboard" › "As pixel sprite" / "As image"
     *  (docs/specs/copy-cut-paste.md AC-34 – AC-36). */
    onOpenClipboard?: (mode: EditorMode) => void;
    /** The desktop app's clipboard card, shown between the New cards and
     *  the "Open from clipboard" row (copy-cut-paste AC-27). */
    clipboardCard?: Snippet;
    /** A file drag is over the screen: show the drop state. */
    dropHover?: boolean;
  } = $props();

  const CARDS = [
    { mode: 'pixel', title: 'start.newPixel', hint: 'start.newPixelHint', icon: 'mode-pixel' },
    { mode: 'image', title: 'start.newImage', hint: 'start.newImageHint', icon: 'mode-image' },
  ] as const;

  const blocked = $derived(busy || unsupported);
  // Recent files exist: the returning arrangement.
  const returning = $derived(recents.length > 0);

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

<!-- The two New cards. In a row from 560 px (stacked on phones); the open
     one's size form sits under the row, joined to its card. -->
{#snippet newBlock()}
  <div class="start-pair" data-tour="modes">
    {#each CARDS as card, i (card.mode)}
      {@const open = expanded === card.mode}
      <button
        id={`new-${card.mode}-head`}
        type="button"
        class={`start-card start-tile cell-${i + 1}`}
        class:is-open={open}
        onclick={() => toggle(card.mode)}
        disabled={blocked}
        aria-label={t(card.title)}
        aria-expanded={open}
        aria-controls={`new-${card.mode}-form`}
      >
        <span class="flex h-6 items-center gap-3">
          <Icon
            name={card.icon}
            size={24}
            class={`shrink-0 ${open ? 'text-pc-accent-text' : 'text-pc-muted'}`}
          />
          <span class="min-w-0 flex-1 text-base leading-6 font-semibold">{t(card.title)}</span>
          <Icon
            name="chevron-down"
            size={16}
            class={`shrink-0 text-pc-faint transition-transform ${open ? 'rotate-180' : ''}`}
          />
        </span>
        <span class="mt-2 block text-[13px] leading-5 text-pc-muted">{t(card.hint)}</span>
      </button>
      {#if open}
        <!-- novalidate: an out-of-range size is clamped by `newDocParams`,
             not refused by the browser's min / max check (app-shell AC-06). -->
        <form
          id={`new-${card.mode}-form`}
          class={`start-panel span join-${i + 1}`}
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
                  class="opt-field start-ctl w-20 px-2 text-sm"
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
                  class="opt-field start-ctl w-20 px-2 text-sm"
                  aria-label={t('common.height')}
                />
              </label>
              <span class="text-xs leading-4 text-pc-faint">px</span>
            </span>
            <button
              type="submit"
              class="panel-btn panel-btn-primary start-ctl w-full px-4 text-sm leading-5 min-[35rem]:ml-auto min-[35rem]:w-auto"
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
  {@render clipboardCard?.()}
  <!-- Open from clipboard, right under the New cards (copy-cut-paste
       AC-34): enabled whenever the cards are, whatever the clipboard
       holds — only a read can tell, and in the browser a read asks. -->
  <div
    class="start-surface mt-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2 rounded-lg border border-pc-border bg-pc-panel py-2 pr-2 pl-4"
    role="group"
    aria-labelledby="start-clip-title"
  >
    <span id="start-clip-title" class="text-[13px] leading-5 font-semibold text-pc-muted">
      {t('start.fromClipboard')}
    </span>
    <span class="flex flex-wrap gap-2">
      {#each CARDS as card (card.mode)}
        <button
          type="button"
          class="panel-btn start-ctl gap-2 px-3 text-sm leading-5"
          disabled={blocked}
          onclick={() => onOpenClipboard(card.mode)}
          ><Icon name={card.icon} size={16} class="shrink-0 text-pc-muted" />{t(
            card.mode === 'pixel' ? 'start.clipPixel' : 'start.clipImage',
          )}</button
        >
      {/each}
    </span>
  </div>
{/snippet}

{#snippet openCard(cell: string)}
  <button
    type="button"
    class={`start-card start-card-sub ${cell}`}
    onclick={onOpen}
    disabled={blocked}
    data-tour="open"
  >
    {t('start.open')}
    <Icon name="chevron-right" size={16} class="shrink-0 text-pc-faint" />
  </button>
{/snippet}

<!-- `inRow`: the samples card is the second cell of the Open… | samples
     pair (first run); otherwise it stands alone at the end of the column. -->
{#snippet samples(inRow: boolean)}
  <button
    type="button"
    class={`start-card start-card-sub ${inRow ? 'cell-2' : ''}`}
    class:is-open={samplesOpen}
    aria-expanded={samplesOpen}
    aria-controls="start-samples"
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
      id="start-samples"
      class={`start-panel span grid gap-2 min-[35rem]:grid-cols-2 ${inRow ? 'join-2' : 'min-[60rem]:grid-cols-1'}`}
      role="group"
      aria-label={t('start.openSample')}
    >
      <button
        type="button"
        class="panel-btn start-ctl justify-start gap-2 px-3 text-sm leading-5"
        disabled={blocked}
        onclick={() => onOpenSample('sprite')}
        ><Icon name="mode-pixel" size={16} class="shrink-0 text-pc-muted" />{t(
          'start.sampleSprite',
        )}</button
      >
      <button
        type="button"
        class="panel-btn start-ctl justify-start gap-2 px-3 text-sm leading-5"
        disabled={blocked}
        onclick={() => onOpenSample('image')}
        ><Icon name="mode-image" size={16} class="shrink-0 text-pc-muted" />{t(
          'start.sampleImage',
        )}</button
      >
    </div>
  {/if}
{/snippet}

{#snippet openHint()}
  <p class="mt-3 max-w-[30rem] text-[13px] leading-5 text-pc-faint">{t('start.openHint')}</p>
{/snippet}

<main
  class="flex min-h-full flex-col items-center bg-pc-bg px-4 pt-8 pb-12 text-pc-text sm:px-8 sm:pt-12 lg:pt-16"
  class:bg-pc-panel={dropHover}
  class:start-drop={dropHover}
  class:cursor-progress={busy}
>
  <section
    class="w-full max-w-[40rem]"
    class:start-wide={returning}
    aria-labelledby="start-title"
    aria-busy={busy}
  >
    <!-- Title, then the utilities: on the h1 line where they fit, on the
         next line on phones. -->
    <div class="flex flex-wrap items-start justify-between gap-x-6 gap-y-4">
      <div class="min-w-0 flex-[1_1_18rem]">
        <h1 id="start-title" class="text-[1.75rem] leading-9 font-semibold tracking-tight">
          Pincel
        </h1>
        <p class="mt-2 max-w-[30rem] text-sm leading-5 text-pc-muted">{t('start.tagline')}</p>
      </div>
      <!-- No menu bar is mounted here, so About needs its own way in
           wherever the OS draws no native Help menu (spec §11.3). -->
      <div class="flex flex-wrap items-center gap-2 sm:h-9 sm:shrink-0">
        {#if !hasNativeMenu()}
          <span class="flex items-center gap-2" data-tour="start-help">
            <button
              type="button"
              class="panel-btn leading-4 pointer-coarse:min-h-11"
              onclick={() => startTour('start')}>{t('menu.tour')}</button
            >
            <button
              type="button"
              class="panel-btn leading-4 pointer-coarse:min-h-11"
              onclick={openSettings}>{t('settings.title')}</button
            >
            <button
              type="button"
              class="panel-btn leading-4 pointer-coarse:min-h-11"
              onclick={openAbout}>{t('menu.about')}</button
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
      <div class="mt-6 flex max-w-[40rem] flex-col gap-3">
        {#if bannerShown()}
          <div
            class="start-surface flex flex-wrap items-center gap-x-4 gap-y-3 rounded-lg border border-pc-border bg-pc-panel px-4 py-3"
            role="region"
            aria-label={t('tour.title')}
          >
            <span class="min-w-[12rem] flex-1 text-sm leading-5">{t('tour.bannerText')}</span>
            <span class="flex gap-2">
              <button
                type="button"
                class="panel-btn panel-btn-primary leading-4 pointer-coarse:min-h-11"
                onclick={() => answerBanner(true)}>{t('tour.bannerTake')}</button
              >
              <button
                type="button"
                class="panel-btn leading-4 pointer-coarse:min-h-11"
                onclick={() => answerBanner(false)}>{t('tour.bannerDecline')}</button
              >
            </span>
          </div>
        {/if}
        {#if unsupported}
          <p
            role="alert"
            class="start-surface rounded-lg border border-pc-border bg-pc-panel px-4 py-3 text-sm leading-5"
          >
            {t('start.unsupportedBrowser')}
          </p>
        {/if}
      </div>
    {/if}

    {#if returning}
      <!-- Returning: start new | pick up where you left off. Two columns at
           one top edge from 960 px; one column in the same order below. -->
      <div class="start-split mt-6">
        <div class="min-w-0">
          {@render newBlock()}
        </div>
        <div class="min-w-0">
          {@render openCard('')}
          {@render openHint()}
          <div class="start-surface mt-6 rounded-lg border border-pc-border bg-pc-panel">
            <h2
              class="border-b border-pc-border px-4 py-2 text-[13px] leading-4 font-semibold text-pc-muted"
            >
              {t('start.recent')}
            </h2>
            <ul class="flex flex-col divide-y divide-pc-border" aria-label={t('start.recentFiles')}>
              {#each recents as r (r.id)}
                <li>
                  <button
                    type="button"
                    class="start-row"
                    onclick={() => onOpenRecent(r)}
                    disabled={blocked}
                    title={r.path ?? r.name}
                  >
                    <span class="min-w-0 truncate">{r.name}</span>
                    <span class="flex w-16 items-center gap-2 text-xs leading-4 text-pc-muted">
                      <Icon name={`mode-${r.mode}`} size={16} class="shrink-0 text-pc-faint" />
                      {t(`mode.${r.mode}`)}
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          </div>
          <div class="start-stack mt-6">
            {@render samples(false)}
          </div>
        </div>
      </div>
    {:else}
      <!-- First run: the New cards are the page; Open… and the samples sit
           under them as a quieter pair. -->
      <div class="mt-6">
        {@render newBlock()}
      </div>
      <div class="start-pair mt-6">
        {@render openCard('cell-1')}
        {@render samples(true)}
      </div>
      {@render openHint()}
    {/if}
  </section>
</main>

<style>
  .start-drop {
    outline: 2px dashed var(--color-pc-accent);
    outline-offset: -0.75rem;
  }
  /* Surfaces lift off the page while a file is dragged over it. */
  .start-drop .start-card:enabled:not(.is-open),
  .start-drop .start-surface {
    background-color: var(--color-pc-raised);
  }

  /* Returning: from 960 px the frame widens to 40rem + 32 + 26rem. */
  @media (min-width: 60rem) {
    .start-wide {
      max-width: 68rem;
    }
  }
  .start-split {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    row-gap: 1.5rem;
  }
  @media (min-width: 60rem) {
    .start-split {
      grid-template-columns: minmax(0, 1fr) 26rem;
      column-gap: 2rem;
      align-items: start;
    }
  }

  /* A pair of cards: stacked on phones, a row from 560 px; what a card
     opens spans the row below it. Cards in a row share one height. */
  .start-pair,
  .start-stack {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 0.75rem;
  }
  @media (min-width: 35rem) {
    .start-pair {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .start-pair > .cell-1 {
      grid-row: 1;
      grid-column: 1;
    }
    .start-pair > .cell-2 {
      grid-row: 1;
      grid-column: 2;
    }
    .start-pair > .span {
      grid-row: 2;
      grid-column: 1 / -1;
    }
  }

  .start-card {
    position: relative;
    width: 100%;
    min-width: 0;
    border-radius: 0.5rem;
    border: 1px solid var(--color-pc-border);
    background-color: var(--color-pc-panel);
    color: var(--color-pc-text);
    text-align: left;
    overflow-wrap: break-word;
  }
  /* Inset ring: never crosses the join, never clipped. */
  .start-card:focus-visible {
    outline-offset: -4px;
  }
  .start-card:hover:enabled:not(.is-open) {
    background-color: var(--color-pc-hover);
    border-color: var(--color-pc-faint);
  }
  .start-card:active:enabled {
    background-color: var(--color-pc-active);
  }
  .start-card:disabled,
  .start-row:disabled {
    background-color: transparent;
    color: var(--color-pc-faint);
    opacity: 0.7;
    cursor: not-allowed;
  }

  /* The open card and its panel are one shape: the card drops its bottom
     corners and edge and bridges the 12px gap over the panel's top edge. */
  .start-card.is-open {
    z-index: 1;
    border-color: var(--color-pc-faint);
    border-bottom-color: var(--color-pc-raised);
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
    background-color: var(--color-pc-raised);
  }
  .start-card.is-open::after {
    content: '';
    position: absolute;
    top: 100%;
    left: -1px;
    right: -1px;
    height: calc(0.75rem + 2px);
    border: 1px solid var(--color-pc-faint);
    border-top: 0;
    border-bottom: 0;
    background-color: inherit;
  }
  .start-panel {
    border-radius: 0 0 0.5rem 0.5rem;
    border: 1px solid var(--color-pc-faint);
    background-color: var(--color-pc-raised);
    /* The card above already gives 16px + the 12px bridge: 4px on top. */
    padding: 0.25rem 1rem 1rem;
  }
  @media (min-width: 35rem) {
    .start-pair > .join-1 {
      border-top-right-radius: 0.5rem;
    }
    .start-pair > .join-2 {
      border-top-left-radius: 0.5rem;
    }
  }

  /* New card: icon, title and chevron on one 24px line, the hint below
     across the full card width. */
  .start-tile {
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    padding: 1rem;
  }

  /* Secondary card: label and its affordance on one line. */
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

  /* 36px control tier, 44px under a coarse pointer. */
  .start-ctl {
    min-height: 2.25rem;
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
  .start-row:focus-visible {
    outline-offset: -3px;
  }
  li:last-child > .start-row {
    border-radius: 0 0 0.4375rem 0.4375rem;
  }
  .start-row:hover:enabled {
    background-color: var(--color-pc-hover);
  }
  .start-row:active:enabled {
    background-color: var(--color-pc-active);
  }
  @media (pointer: coarse) {
    .start-ctl,
    .start-row {
      min-height: 2.75rem;
    }
  }
</style>
