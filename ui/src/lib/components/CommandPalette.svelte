<script lang="ts">
  // The command palette, Ctrl+K (docs/specs/power-user-shortcuts-palette.md):
  // a search over every menu command and tool of the mounted editor, with
  // the shortcut of each. Mounted by App.svelte; the editor that opened it
  // supplies its menus and tools. A combobox controlling a listbox (WAI-ARIA
  // APG); ↑ / ↓ stop at the ends, Enter runs, Escape closes.
  import { onMount, tick } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import {
    filterEntries,
    loadRecentCommands,
    noteRecentCommand,
    paletteEntries,
    type PaletteEntry,
    type PaletteSource,
  } from '../shell/command-palette';

  interface Props {
    source: PaletteSource;
    onClose: () => void;
  }
  const { source, onClose }: Props = $props();

  let query = $state('');
  let active = $state(0);
  let recent = $state<string[]>([]);
  // "Go to frame…" swaps the search for a frame-number field.
  let asking = $state(false);
  let frameText = $state('');
  let frameInvalid = $state(false);
  let search = $state<HTMLInputElement | null>(null);
  let frameField = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLElement | null>(null);

  const entries = $derived(paletteEntries(source));
  const shown = $derived(filterEntries(entries, query, recent));
  const frameCount = $derived(source.frames?.count() ?? 0);

  onMount(() => {
    search?.focus();
    void loadRecentCommands().then((r) => (recent = [...r]));
  });

  function optionId(i: number): string {
    return `cmd-option-${i}`;
  }

  function move(step: number): void {
    if (shown.length === 0) return;
    active = Math.max(0, Math.min(shown.length - 1, active + step));
    void tick().then(() =>
      list?.querySelector(`#${optionId(active)}`)?.scrollIntoView({ block: 'nearest' }),
    );
  }

  function pick(entry: PaletteEntry | undefined): void {
    if (!entry || entry.disabled) return;
    recent = [...noteRecentCommand(entry.id)];
    if (entry.kind === 'goto') {
      asking = true;
      void tick().then(() => frameField?.focus());
      return;
    }
    onClose();
    entry.run();
  }

  function gotoFrame(): void {
    const frames = source.frames;
    const n = /^\s*\d+\s*$/.test(frameText) ? Number(frameText) : NaN;
    if (!frames || !Number.isInteger(n) || n < 1 || n > frameCount) {
      frameInvalid = true;
      return;
    }
    onClose();
    frames.goto(n);
  }

  function onKeydown(e: KeyboardEvent): void {
    // The palette owns these keys: once it closes, the editor's window
    // handler must not see the same Enter (Play) or Escape (Deselect).
    if (['Escape', 'Enter', 'ArrowUp', 'ArrowDown'].includes(e.key)) e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (asking) {
      if (e.key === 'Enter') {
        e.preventDefault();
        gotoFrame();
      }
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      move(e.key === 'ArrowDown' ? 1 : -1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      pick(shown[active]);
    }
  }
</script>

<div
  class="fixed inset-0 z-20 flex items-start justify-center bg-black/50 p-4 pt-[12vh]"
  role="dialog"
  aria-modal="true"
  aria-label={t('cmd.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="flex max-h-[70vh] w-full max-w-xl flex-col rounded border border-pc-border bg-pc-panel shadow-xl"
  >
    {#if asking}
      <label class="flex flex-col gap-1 px-4 py-3 text-xs text-pc-muted">
        {t('cmd.frameNumber', { n: frameCount })}
        <input
          bind:this={frameField}
          bind:value={frameText}
          class="opt-field w-full text-sm"
          type="text"
          inputmode="numeric"
          aria-invalid={frameInvalid}
          oninput={() => (frameInvalid = false)}
        />
      </label>
    {:else}
      <input
        bind:this={search}
        bind:value={query}
        class="opt-field m-3 text-sm"
        type="text"
        role="combobox"
        aria-label={t('cmd.title')}
        aria-expanded="true"
        aria-controls="cmd-list"
        aria-autocomplete="list"
        aria-activedescendant={shown.length > 0 ? optionId(active) : undefined}
        placeholder={t('cmd.placeholder')}
        oninput={() => (active = 0)}
      />
      <ul
        bind:this={list}
        id="cmd-list"
        class="min-h-0 flex-1 overflow-y-auto border-t border-pc-border py-1"
        role="listbox"
        aria-label={t('cmd.list')}
      >
        {#each shown as entry, i (entry.id + entry.label)}
          <li
            id={optionId(i)}
            class="flex cursor-default items-center gap-3 px-4 py-1.5 text-sm"
            class:bg-pc-active={i === active}
            class:opacity-50={entry.disabled}
            role="option"
            aria-selected={i === active}
            aria-disabled={entry.disabled}
            onpointermove={() => (active = i)}
            onclick={() => pick(entry)}
            onkeydown={onKeydown}
          >
            <span class="min-w-0 flex-1 truncate">{entry.label}</span>
            {#if entry.shortcut}
              <kbd class="shrink-0 text-xs text-pc-faint">{entry.shortcut}</kbd>
            {/if}
          </li>
        {/each}
      </ul>
      {#if shown.length === 0}
        <p class="px-4 py-3 text-xs text-pc-muted">{t('cmd.empty')}</p>
      {/if}
    {/if}
  </div>
</div>
