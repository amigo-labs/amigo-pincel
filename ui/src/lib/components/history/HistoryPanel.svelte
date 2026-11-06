<script lang="ts">
  // The History panel both editors show (docs/specs/history-panel.md): the
  // start entry, one row per undo step, the redo steps dimmed after the
  // current state; a click, Enter or Space on a row jumps there, the four
  // buttons rewind, undo, redo and fast-forward. The editor owns the
  // history and the jump; this component only lists and asks.
  import Icon from '../icons/Icon.svelte';
  import Panel from '../layout/Panel.svelte';
  import type { IconName } from '../icons/icons';
  import { t } from '../../i18n/index.svelte';
  import type { MessageKey } from '../../i18n/en';
  import {
    historyStartName,
    historyStepName,
    type HistoryOrigin,
    type HistorySnapshot,
  } from '../../history/names';

  interface Props {
    history: HistorySnapshot;
    origin: HistoryOrigin;
    /** `true` while a stroke or another pointer gesture runs: a jump request
     *  is then ignored, not queued (AC-11). */
    busy: () => boolean;
    /** Undo (`delta < 0`) or redo (`delta > 0`) `|delta|` steps at once. */
    onJump: (delta: number) => void;
  }

  const { history, origin, busy, onJump }: Props = $props();

  const listId = $props.id();

  const rows = $derived([
    historyStartName(origin, history.dropped, t),
    ...history.undo.map((k) => historyStepName(k, t)),
    ...history.redo.map((k) => historyStepName(k, t)),
  ]);
  const current = $derived(history.undo.length);
  const last = $derived(rows.length - 1);

  /** The row the keyboard focus is on (moves without jumping, AC-10). */
  let focused = $state(0);
  let jumping = false;
  let list: HTMLUListElement | undefined = $state();

  // The current state's row comes into view and takes the keyboard focus
  // marker whenever the history changes (AC-08).
  $effect(() => {
    const at = current;
    void rows;
    focused = at;
    list?.querySelector(`[data-row="${at}"]`)?.scrollIntoView({ block: 'nearest' });
  });

  /** Makes row `row` the current state (AC-05); ignored while busy. */
  function jumpTo(row: number): void {
    const delta = row - current;
    if (delta === 0 || jumping || busy()) return;
    jumping = true;
    try {
      onJump(delta);
    } finally {
      jumping = false;
    }
  }

  /** The row a navigation key moves the focus to; `null` for other keys. */
  function target(key: string): number | null {
    switch (key) {
      case 'ArrowUp':
        return Math.max(0, focused - 1);
      case 'ArrowDown':
        return Math.min(last, focused + 1);
      case 'Home':
        return 0;
      case 'End':
        return last;
      default:
        return null;
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      // Handled here: the editor's Space-pan and Enter shortcuts skip a
      // key whose default is prevented (AC-18).
      e.preventDefault();
      e.stopPropagation();
      jumpTo(focused);
      return;
    }
    const next = target(e.key);
    if (next === null) return;
    // A row with focus and its list both listen: handle the key once.
    e.preventDefault();
    e.stopPropagation();
    focused = next;
    list?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  const buttons: { icon: IconName; label: MessageKey; delta: () => number }[] = [
    { icon: 'rewind', label: 'historyPanel.rewind', delta: () => -current },
    { icon: 'undo', label: 'historyPanel.undo', delta: () => (current > 0 ? -1 : 0) },
    { icon: 'redo', label: 'historyPanel.redo', delta: () => (current < last ? 1 : 0) },
    { icon: 'fast-forward', label: 'historyPanel.fastForward', delta: () => last - current },
  ];
</script>

<Panel>
  <div class="flex gap-1">
    {#each buttons as b (b.icon)}
      <button
        type="button"
        class="panel-btn flex-1"
        aria-label={t(b.label)}
        title={t(b.label)}
        disabled={b.delta() === 0}
        onclick={() => jumpTo(current + b.delta())}
      >
        <Icon name={b.icon} size={16} />
      </button>
    {/each}
  </div>
  <ul
    bind:this={list}
    role="listbox"
    tabindex="0"
    aria-label={t('historyPanel.list')}
    aria-activedescendant={`${listId}-${focused}`}
    class="history-list flex max-h-64 min-h-0 flex-col overflow-y-auto rounded border border-pc-border py-1"
    onkeydown={onKeyDown}
  >
    {#each rows as name, i (i)}
      <li
        id={`${listId}-${i}`}
        data-row={i}
        role="option"
        tabindex="-1"
        aria-selected={i === current}
        class="history-row shrink-0 cursor-pointer truncate px-2 py-0.5 text-sm text-pc-text"
        class:bg-pc-selected={i === current}
        class:opacity-50={i > current}
        class:history-focus={i === focused}
        title={name}
        onclick={() => jumpTo(i)}
        onkeydown={onKeyDown}
      >
        {name}
      </li>
    {/each}
  </ul>
</Panel>

<style>
  .history-list:focus-visible .history-focus {
    outline: 1px solid var(--color-pc-accent);
    outline-offset: -1px;
  }
  .history-row:hover {
    background-color: var(--color-pc-hover);
  }
</style>
