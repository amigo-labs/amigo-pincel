<script lang="ts" generics="T extends string">
  // Variant switcher for the active tool group (spec §9.2), shown at the
  // start of the options bar: Rectangle / Ellipse / … for Shapes, the
  // marquee shapes for Select, and so on. Touch-friendly replacement for a
  // long-press flyout.
  import Icon from '../icons/Icon.svelte';
  import { groupOf, type ToolGroup } from './tools';
  import { t } from '../../i18n/index.svelte';

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
    aria-label={t('frame.variant', { group: group.label })}
  >
    {#each group.tools as tool (tool.id)}
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded text-pc-muted hover:bg-pc-hover hover:text-pc-text disabled:opacity-35"
        class:variant-on={tool.id === active}
        role="radio"
        aria-checked={tool.id === active}
        aria-label={tool.label}
        title={tool.title ?? tool.label}
        disabled={tool.disabled}
        onclick={() => onSelect(tool.id)}
      >
        <Icon name={tool.icon} size={17} />
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
