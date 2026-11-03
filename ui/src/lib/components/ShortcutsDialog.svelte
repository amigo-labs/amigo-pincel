<script lang="ts">
  // Help ▸ Keyboard Shortcuts (docs/specs/shortcut-overview.md): every
  // shortcut of the current mode, generated from the one keymap
  // (lib/shell/shortcuts.ts), filterable and printable. Mounted by
  // App.svelte; the editor that opened it supplies its actions and tools.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import {
    SECTION_LABEL,
    filterRows,
    groupRows,
    shortcutList,
    type ShortcutSource,
  } from '../shell/shortcut-list';

  let { source, onClose }: { source: ShortcutSource; onClose: () => void } = $props();

  let query = $state('');
  let filterField: HTMLInputElement | undefined = $state();

  const rows = $derived(shortcutList(source));
  const sections = $derived(groupRows(filterRows(rows, query)));

  onMount(() => filterField?.focus());

  // Keys typed here (the filter above all) must not reach the editor's
  // window-level shortcuts: `b` in the filter is a letter, not the brush.
  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }
</script>

<div
  class="shortcuts-print fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="shortcuts-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="shortcuts-panel flex max-h-full w-full max-w-2xl flex-col rounded border border-pc-border bg-pc-panel shadow-xl"
  >
    <div class="flex items-center gap-3 border-b border-pc-border px-4 py-3">
      <h2 id="shortcuts-title" class="flex-1 text-lg font-semibold">{t('menu.shortcuts')}</h2>
      <button class="panel-btn shortcuts-noprint" onclick={onClose}>{t('common.close')}</button>
    </div>
    <div class="shortcuts-noprint border-b border-pc-border px-4 py-2">
      <input
        bind:this={filterField}
        bind:value={query}
        type="search"
        class="opt-field w-full py-1 text-sm"
        placeholder={t('keys.filter')}
        aria-label={t('keys.filter')}
      />
    </div>
    <div class="shortcuts-list min-h-0 flex-1 overflow-y-auto px-4 py-2" data-testid="shortcut-list">
      {#each sections as s (s.section)}
        <table class="mb-3 w-full border-collapse text-sm">
          <caption class="py-1 text-left text-xs font-semibold tracking-wide text-pc-muted uppercase">
            {t(SECTION_LABEL[s.section])}
          </caption>
          <thead class="sr-only">
            <tr>
              <th scope="col">{t('keys.colAction')}</th>
              <th scope="col">{t('keys.colKeys')}</th>
              <th scope="col">{t('keys.colNote')}</th>
            </tr>
          </thead>
          <tbody>
            {#each s.rows as r (r.id)}
              <tr class="border-t border-pc-border/60">
                <th scope="row" class="w-2/5 py-1 pr-3 text-left font-normal text-pc-text">
                  {r.label}
                </th>
                <td class="py-1 pr-3 whitespace-nowrap">
                  {#each r.chords as chord, i (i)}
                    <kbd
                      class="mr-1 inline-block rounded border border-pc-border bg-pc-raised px-1.5 font-mono text-xs text-pc-text"
                      >{chord}</kbd
                    >
                  {/each}
                </td>
                <td class="py-1 text-xs text-pc-muted">{r.note ?? ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="py-4 text-center text-sm text-pc-muted">{t('keys.empty')}</p>
      {/each}
    </div>
  </div>
</div>
