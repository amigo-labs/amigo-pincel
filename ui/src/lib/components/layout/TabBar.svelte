<script lang="ts" module>
  /** One open document in the tab bar (docs/specs/document-tabs.md AC-01). */
  export interface TabItem {
    id: number;
    name: string;
    dirty: boolean;
  }

  /** What an editor hands `EditorFrame` to draw its tab bar. */
  export interface TabBarProps {
    tabs: readonly TabItem[];
    active: number | null;
    /** Tooltip hint of the close shortcut, e.g. `Ctrl+W`. */
    closeHint?: string;
    onSelect: (id: number) => void;
    onClose: (id: number) => void;
  }
</script>

<script lang="ts">
  // The tab bar both editors show between the options bar and the canvas
  // (docs/specs/document-tabs.md AC-01, AC-04, AC-07): one tab per open
  // document in opening order, the active one selected, each with an
  // unsaved marker and a close button. It scrolls horizontally when the
  // tabs do not fit and keeps the active tab in view.
  import Icon from '../icons/Icon.svelte';
  import { t } from '../../i18n/index.svelte';

  const { tabs, active, closeHint, onSelect, onClose }: TabBarProps = $props();

  let bar: HTMLDivElement | undefined = $state();

  $effect(() => {
    void tabs.length;
    if (active === null || !bar) return;
    const el = bar.querySelector<HTMLElement>(`[data-tab-id="${active}"]`);
    el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  function onAux(e: MouseEvent, id: number): void {
    if (e.button !== 1) return;
    e.preventDefault();
    onClose(id);
  }
</script>

<div
  bind:this={bar}
  class="flex h-8 shrink-0 items-end gap-px overflow-x-auto border-b border-pc-border bg-pc-bg px-1 [scrollbar-width:thin]"
  role="tablist"
  aria-label={t('tabs.label')}
>
  {#each tabs as tab (tab.id)}
    {@const selected = tab.id === active}
    <div
      class="flex h-7 max-w-56 shrink-0 items-center rounded-t border border-b-0 text-xs"
      class:border-pc-border={selected}
      class:bg-pc-panel={selected}
      class:text-pc-text={selected}
      class:border-transparent={!selected}
      class:text-pc-muted={!selected}
      data-tab-id={tab.id}
      role="presentation"
      onmousedown={(e) => {
        // A middle press would start the browser's autoscroll.
        if (e.button === 1) e.preventDefault();
      }}
      onauxclick={(e) => onAux(e, tab.id)}
    >
      <button
        type="button"
        class="flex h-full min-w-0 items-center gap-1 pr-1 pl-2.5 hover:text-pc-text"
        role="tab"
        aria-selected={selected}
        tabindex={selected ? 0 : -1}
        onclick={() => onSelect(tab.id)}
      >
        <span class="truncate">{tab.name}</span>
        {#if tab.dirty}
          <span class="text-amber-400" aria-hidden="true">●</span>
          <span class="sr-only">{t('tabs.unsaved')}</span>
        {/if}
      </button>
      <button
        type="button"
        class="mr-1 flex h-5 w-5 shrink-0 items-center justify-center rounded text-pc-muted hover:bg-pc-hover hover:text-pc-text"
        aria-label={t('tabs.closeNamed', { name: tab.name })}
        title={closeHint ? `${t('tabs.close')} (${closeHint})` : t('tabs.close')}
        onclick={() => onClose(tab.id)}
      >
        <Icon name="close" size={13} />
      </button>
    </div>
  {/each}
</div>
