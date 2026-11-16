<script lang="ts" generics="T extends string">
  // Variant switcher for the active tool group (spec §9.2), shown at the
  // start of the options bar: Rectangle / Ellipse / … for Shapes, the
  // marquee shapes for Select, and so on. The touch-friendly path that
  // needs no hold; the rail's flyout (`ToolFlyout`) is the faster one.
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
    class="am-seg am-seg--sm shrink-0"
    role="radiogroup"
    aria-label={t('frame.variant', { group: group.label })}
  >
    {#each group.tools as tool (tool.id)}
      <button
        type="button"
        role="radio"
        aria-checked={tool.id === active}
        aria-label={tool.label}
        title={tool.title ?? tool.label}
        disabled={tool.disabled}
        onclick={() => onSelect(tool.id)}
      >
        <Icon name={tool.icon} size={14} />
      </button>
    {/each}
  </div>
{/if}
