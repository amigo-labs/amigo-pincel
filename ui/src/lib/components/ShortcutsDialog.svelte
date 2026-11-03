<script lang="ts">
  // Help ▸ Keyboard Shortcuts (docs/specs/shortcut-overview.md): every
  // shortcut of the current mode, generated from the one keymap
  // (lib/shell/shortcuts.ts), filterable and printable. Mounted by
  // App.svelte; the editor that opened it supplies its actions and tools.
  // Rows of actions and tool groups can be rebound, reset, exported and
  // imported (docs/specs/power-user-shortcuts-keymap.md).
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { KeymapId } from '../shell/keymap-state.svelte';
  import {
    bind,
    conflictFor,
    isCurrentChord,
    isOverridden,
    resetAllBindings,
    resetBinding,
  } from '../shell/keymap';
  import {
    SECTION_LABEL,
    filterRows,
    groupRows,
    keymapLabel,
    shortcutList,
    type ShortcutRow,
    type ShortcutSource,
  } from '../shell/shortcut-list';
  import { chordFromEvent, chordLabel, type Chord } from '../shell/shortcuts';

  let { source, onClose }: { source: ShortcutSource; onClose: () => void } = $props();

  let query = $state('');
  let filterField: HTMLInputElement | undefined = $state();
  let root: HTMLDivElement | undefined = $state();
  /** The entry waiting for its new chord. */
  let capturing = $state<KeymapId | null>(null);
  /** A captured chord another entry already uses, awaiting Replace / Cancel. */
  let conflict = $state<{ id: KeymapId; chord: Chord; owner: KeymapId } | null>(null);

  const rows = $derived(shortcutList(source));
  const sections = $derived(groupRows(filterRows(rows, query)));

  onMount(() => filterField?.focus());

  /** Keys that only modify another: pressing one alone completes no capture. */
  const MODIFIER_KEYS = new Set(['Shift', 'Control', 'Alt', 'AltGraph', 'Meta', 'OS', 'CapsLock']);

  function bindable(r: ShortcutRow): KeymapId | null {
    return r.id.startsWith('gesture:') ? null : (r.id as KeymapId);
  }

  function startCapture(id: KeymapId): void {
    conflict = null;
    capturing = id;
  }

  function captured(id: KeymapId, chord: Chord): void {
    capturing = null;
    if (isCurrentChord(id, chord)) return;
    const owner = conflictFor(chord, id);
    if (owner !== null) {
      conflict = { id, chord, owner };
      return;
    }
    bind(id, chord);
  }

  // The warning's buttons vanish with it; keep the focus (and so Escape
  // and the key guard) inside the dialog.
  function closeConflict(): void {
    conflict = null;
    root?.focus();
  }

  function replace(): void {
    if (conflict !== null) bind(conflict.id, conflict.chord);
    closeConflict();
  }

  // Keys typed here (the filter above all) must not reach the editor's
  // window-level shortcuts: `b` in the filter is a letter, not the brush;
  // a chord pressed for Change is the new binding, not a command.
  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (capturing !== null) {
      e.preventDefault();
      if (e.key === 'Escape') capturing = null;
      else if (!MODIFIER_KEYS.has(e.key)) captured(capturing, chordFromEvent(e));
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      if (conflict !== null) closeConflict();
      else onClose();
    }
  }
</script>

<div
  bind:this={root}
  class="shortcuts-print fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="shortcuts-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="shortcuts-panel flex max-h-full w-full max-w-3xl flex-col rounded border border-pc-border bg-pc-panel shadow-xl"
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
    {#if conflict !== null}
      <div
        class="shortcuts-noprint flex flex-wrap items-center gap-2 border-b border-pc-border bg-pc-raised px-4 py-2 text-sm"
        role="alert"
      >
        <span class="flex-1">
          {t('keys.conflict', {
            chord: chordLabel(conflict.chord),
            owner: keymapLabel(conflict.owner, source.groups),
          })}
        </span>
        <button class="panel-btn panel-btn-primary" onclick={replace}>{t('keys.replace')}</button>
        <button class="panel-btn" onclick={closeConflict}>{t('common.cancel')}</button>
      </div>
    {/if}
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
              <th scope="col" class="shortcuts-noprint">{t('keys.colEdit')}</th>
            </tr>
          </thead>
          <tbody>
            {#each s.rows as r (r.id)}
              {@const id = bindable(r)}
              <tr class="border-t border-pc-border/60">
                <th scope="row" class="w-1/3 py-1 pr-3 text-left font-normal text-pc-text">
                  {r.label}
                </th>
                <td class="py-1 pr-3 whitespace-nowrap">
                  {#if id !== null && capturing === id}
                    <span class="text-xs text-pc-accent">{t('keys.capture')}</span>
                  {:else}
                    {#each r.chords as chord, i (i)}
                      <kbd
                        class="mr-1 inline-block rounded border border-pc-border bg-pc-raised px-1.5 font-mono text-xs text-pc-text"
                        >{chord}</kbd
                      >
                    {/each}
                  {/if}
                </td>
                <td class="py-1 pr-3 text-xs text-pc-muted">{r.note ?? ''}</td>
                <td class="shortcuts-noprint py-1 text-right whitespace-nowrap">
                  {#if id !== null}
                    <button class="panel-btn" onclick={() => startCapture(id)}>
                      {t('keys.change')}
                    </button>
                    <button
                      class="panel-btn"
                      disabled={!isOverridden(id)}
                      onclick={() => resetBinding(id)}
                    >
                      {t('keys.reset')}
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="py-4 text-center text-sm text-pc-muted">{t('keys.empty')}</p>
      {/each}
    </div>
    <div class="shortcuts-noprint flex flex-wrap justify-end gap-2 border-t border-pc-border px-4 py-2">
      <button class="panel-btn" onclick={resetAllBindings}>{t('keys.resetAll')}</button>
    </div>
  </div>
</div>
