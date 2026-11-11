<script lang="ts">
  // The font family control of both Text tools (docs/specs/system-fonts.md):
  // a button showing the current family, and a list of every family with
  // search, each entry in its own font (AC-01 – AC-04), the note why system
  // fonts are missing (AC-13) and "Add font file…" (AC-20).
  import { tick } from 'svelte';
  import { t } from '../../i18n/index.svelte';
  import { filterFamilies } from '../../fonts/faces';
  import {
    addFontFiles,
    cssFamily,
    familyList,
    fontSource,
    requestSystemFonts,
    type AddResult,
  } from '../../fonts/source.svelte';

  let {
    family,
    missing,
    onPick,
    onAdded,
  }: {
    family: string;
    /** The family is unavailable and Liberation Sans is used (AC-11). */
    missing: boolean;
    onPick: (family: string) => void;
    onAdded: (result: AddResult) => void;
  } = $props();

  let open = $state(false);
  let query = $state('');
  let active = $state(0);
  let pos = $state({ left: 0, top: 0, width: 260, maxHeight: 320 });
  let button: HTMLButtonElement | undefined = $state();
  let panel: HTMLDivElement | undefined = $state();
  let search: HTMLInputElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();
  const uid = `font-list-${Math.random().toString(36).slice(2)}`;

  const families = $derived(familyList());
  const shown = $derived(filterFamilies(families, query));
  const access = $derived(fontSource.access);
  /** "Add font file…" while system fonts are not accessible (AC-01). */
  const canAdd = $derived(access !== 'desktop' && access !== 'granted');
  const note = $derived(
    access === 'denied'
      ? t('font.noteDenied')
      : access === 'dismissed'
        ? t('font.noteDismissed')
        : access === 'unsupported'
          ? t('font.noteUnsupported')
          : null,
  );

  function place(): void {
    const r = button?.getBoundingClientRect();
    if (!r) return;
    const width = Math.min(300, window.innerWidth - 16);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8));
    const below = window.innerHeight - r.bottom - 12;
    const above = r.top - 12;
    const maxHeight = Math.max(160, Math.min(380, Math.max(below, above)));
    const top =
      below >= Math.min(maxHeight, 240) || below >= above ? r.bottom + 4 : r.top - 4 - maxHeight;
    pos = { left, top: Math.max(8, top), width, maxHeight };
  }

  function toggle(): void {
    if (open) {
      close(false);
      return;
    }
    // From the click itself: the permission prompt needs it (AC-12).
    void requestSystemFonts();
    query = '';
    active = Math.max(0, shown.indexOf(family));
    place();
    open = true;
    void tick().then(() => {
      search?.focus();
      scrollActive();
    });
  }

  function close(refocus: boolean): void {
    open = false;
    if (refocus) button?.focus();
  }

  function pick(name: string): void {
    onPick(name);
    close(true);
  }

  function scrollActive(): void {
    panel?.querySelector(`#${uid}-${active}`)?.scrollIntoView({ block: 'nearest' });
  }

  function onSearchKey(e: KeyboardEvent): void {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = shown.length;
      if (n === 0) return;
      active = (active + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      void tick().then(scrollActive);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      const f = shown[active];
      if (f !== undefined) pick(f);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    }
  }

  function onWindowPointer(e: PointerEvent): void {
    if (!open) return;
    const target = e.target as Node | null;
    if (target && (panel?.contains(target) || button?.contains(target))) return;
    close(false);
  }

  async function onFiles(): Promise<void> {
    const files = [...(fileInput?.files ?? [])];
    if (fileInput) fileInput.value = '';
    if (files.length === 0) return;
    const result = await addFontFiles(files);
    onAdded(result);
    if (result.families.length > 0) close(true);
  }
</script>

<svelte:window onpointerdown={onWindowPointer} onresize={() => open && place()} />

<button
  bind:this={button}
  type="button"
  class="opt-field flex max-w-48 min-w-28 items-center gap-1 truncate text-left"
  aria-haspopup="dialog"
  aria-expanded={open}
  aria-label={t('font.control', { family })}
  title={missing ? t('font.missingTitle', { family }) : family}
  data-font-control
  onclick={toggle}
>
  <span class="truncate" style:font-family={cssFamily(family)}>{family}</span>
  {#if missing}
    <span class="shrink-0 text-xs text-pc-danger-text" data-font-missing
      >({t('font.missingMark')})</span
    >
  {/if}
</button>

{#if open}
  <div
    bind:this={panel}
    role="dialog"
    aria-label={t('font.list')}
    class="fixed z-50 flex flex-col gap-1 rounded border border-pc-border bg-pc-panel p-2 text-sm text-pc-text shadow-2xl"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:width="{pos.width}px"
    style:max-height="{pos.maxHeight}px"
    data-font-list
  >
    <input
      bind:this={search}
      bind:value={query}
      type="search"
      class="opt-field w-full"
      placeholder={t('font.search')}
      aria-label={t('font.search')}
      aria-controls="{uid}-list"
      aria-activedescendant={shown.length > 0 ? `${uid}-${active}` : undefined}
      onkeydown={onSearchKey}
      oninput={() => (active = 0)}
    />
    <ul
      id="{uid}-list"
      role="listbox"
      aria-label={t('font.families')}
      class="min-h-0 flex-1 overflow-y-auto"
    >
      {#each shown as name, i (name)}
        <li
          id="{uid}-{i}"
          role="option"
          tabindex="-1"
          aria-selected={name === family}
          class="cursor-pointer truncate rounded px-2 py-1 text-base [content-visibility:auto] hover:bg-pc-hover"
          class:bg-pc-selected={i === active}
          style:font-family={cssFamily(name)}
          onclick={() => pick(name)}
          onkeydown={(e) => e.key === 'Enter' && pick(name)}
          onpointermove={() => (active = i)}
        >
          {name}
        </li>
      {/each}
    </ul>
    {#if shown.length === 0}
      <p class="px-2 py-1 text-pc-muted" role="status">{t('font.noMatch')}</p>
    {/if}
    {#if fontSource.listing}
      <p class="px-2 text-xs text-pc-muted" role="status">{t('font.listing')}</p>
    {/if}
    {#if note}
      <p class="px-2 text-xs text-pc-muted" data-font-note>{note}</p>
    {/if}
    {#if canAdd}
      <button type="button" class="panel-btn" onclick={() => fileInput?.click()}
        >{t('font.addFile')}</button
      >
      <input
        bind:this={fileInput}
        type="file"
        multiple
        accept=".ttf,.otf,.ttc,.otc,font/ttf,font/otf,font/collection"
        class="hidden"
        aria-label={t('font.addFile')}
        onchange={onFiles}
      />
    {/if}
  </div>
{/if}
