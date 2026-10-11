<script lang="ts" generics="T extends string">
  // Variant switcher for the active tool group (spec §9.2), shown at the
  // start of the options bar: Rectangle / Ellipse / … for Shapes, the
  // marquee shapes for Select, and so on. Touch-friendly replacement for a
  // long-press flyout.
  import Icon from '../icons/Icon.svelte';
  import { groupOf, type ToolGroup } from './tools';

  interface Props {
    groups: ToolGroup<T>[];
    active: T;
    onSelect: (tool: T) => void;
  }
  const { groups, active, onSelect }: Props = $props();

  const group = $derived(groupOf(groups, active));
</script>

{#if group && group.tools.length > 1}
  <div
    class="flex shrink-0 items-center gap-0.5 rounded-md border border-pc-border p-0.5"
    role="radiogroup"
    aria-label={`${group.label} variant`}
  >
    {#each group.tools as t (t.id)}
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded text-pc-muted hover:bg-pc-hover hover:text-pc-text disabled:opacity-35"
        class:variant-on={t.id === active}
        role="radio"
        aria-checked={t.id === active}
        aria-label={t.label}
        title={t.title ?? t.label}
        disabled={t.disabled}
        onclick={() => onSelect(t.id)}
      >
        <Icon name={t.icon} size={17} />
      </button>
    {/each}
  </div>
{/if}

<style>
  .variant-on {
    background-color: var(--color-pc-active);
    color: var(--color-pc-text);
  }
</style>
