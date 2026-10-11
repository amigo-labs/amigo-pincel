<script lang="ts" generics="T extends string">
  // The tool rail (spec §9.2): a vertical strip left of the canvas, or a
  // horizontal strip along the bottom on phones. One button per group; it
  // shows the group's active (or last used) variant and selects it.
  import Icon from '../icons/Icon.svelte';
  import type { ToolGroup } from './tools';

  interface Props {
    groups: ToolGroup<T>[];
    active: T;
    onSelect: (tool: T) => void;
    orientation?: 'vertical' | 'horizontal';
  }
  const { groups, active, onSelect, orientation = 'vertical' }: Props = $props();

  // Last variant picked per group, so the rail button returns to it.
  let lastUsed = $state<Record<string, T>>({});

  $effect(() => {
    for (const g of groups) {
      if (g.tools.some((t) => t.id === active)) lastUsed[g.id] = active;
    }
  });

  function current(g: ToolGroup<T>) {
    const id = g.tools.some((t) => t.id === active) ? active : lastUsed[g.id];
    return g.tools.find((t) => t.id === id) ?? g.tools[0];
  }
</script>

<div
  class={orientation === 'vertical'
    ? 'flex w-12 shrink-0 flex-col items-center gap-0.5 overflow-y-auto border-r border-pc-border bg-pc-panel py-1.5 [scrollbar-width:none]'
    : 'flex h-14 shrink-0 items-center gap-1 overflow-x-auto border-t border-pc-border bg-pc-panel px-2 [scrollbar-width:none]'}
  role="toolbar"
  aria-label="Tools"
  aria-orientation={orientation}
>
  {#each groups as g (g.id)}
    {@const t = current(g)}
    {#if t}
      {@const on = g.tools.some((x) => x.id === active)}
      <button
        type="button"
        class="relative flex shrink-0 items-center justify-center rounded-md text-pc-muted hover:bg-pc-hover hover:text-pc-text disabled:opacity-35 disabled:hover:bg-transparent"
        class:h-9={orientation === 'vertical'}
        class:w-9={orientation === 'vertical'}
        class:h-11={orientation === 'horizontal'}
        class:w-11={orientation === 'horizontal'}
        class:tool-on={on}
        title={t.title ?? `${g.tools.length > 1 ? g.label : t.label}${g.shortcut ? ` (${g.shortcut})` : ''}`}
        aria-label={g.tools.length > 1 ? `${g.label}: ${t.label}` : t.label}
        aria-pressed={on}
        disabled={t.disabled && g.tools.every((x) => x.disabled)}
        onclick={() => onSelect(t.disabled ? (g.tools.find((x) => !x.disabled)?.id ?? t.id) : t.id)}
      >
        <Icon name={t.icon} size={orientation === 'vertical' ? 20 : 22} />
        {#if g.tools.length > 1}
          <svg
            class="pointer-events-none absolute right-0.5 bottom-0.5 opacity-70"
            width="5"
            height="5"
            viewBox="0 0 5 5"
            aria-hidden="true"><path d="M5 0V5H0z" fill="currentColor" /></svg
          >
        {/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .tool-on {
    background-color: var(--color-pc-accent);
    color: var(--color-pc-accent-fg);
  }
  .tool-on:hover {
    background-color: var(--color-pc-accent);
    color: var(--color-pc-accent-fg);
  }
</style>
