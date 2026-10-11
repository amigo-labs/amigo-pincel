<script lang="ts">
  // Chrome of one editor panel (spec §9.2): a header with the collapse
  // toggle and title from the hosting frame, the panel's own actions on
  // the right, then the body. In the phone bottom sheet the tab strip
  // carries the title, so only the actions row remains.
  import type { Snippet } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import { panelSlot } from './panels.svelte';

  interface Props {
    /** Title when the panel is rendered outside an `EditorFrame`. */
    title?: string;
    /** Small counter next to the title (layers, swatches, …). */
    count?: number | string;
    actions?: Snippet;
    children: Snippet;
    /** Body classes (padding / gap overrides). */
    bodyClass?: string;
  }
  const { title, count, actions, children, bodyClass = 'flex flex-col gap-2 px-3 pb-3' }: Props =
    $props();

  const slot = panelSlot();
  const name = $derived(slot?.title ?? title ?? '');
  const host = $derived(slot?.host ?? 'side');
  const collapsed = $derived(host !== 'sheet' && (slot?.collapsed ?? false));
</script>

<section class="flex min-h-0 flex-col text-sm" class:flex-1={!collapsed} aria-label={name}>
  <header class="flex h-9 shrink-0 items-center gap-1 pr-2" class:pl-3={host === 'sheet'}>
    {#if host !== 'sheet'}
      <button
        type="button"
        class="flex h-full min-w-0 flex-1 items-center gap-1 pl-1.5 text-left text-[11px] font-semibold tracking-wide text-pc-muted uppercase hover:text-pc-text"
        aria-expanded={!collapsed}
        onclick={() => slot?.toggle()}
      >
        <Icon name={collapsed ? 'chevron-right' : 'chevron-down'} size={14} />
        <span class="truncate">{name}</span>
        {#if count !== undefined}
          <span class="ml-1 font-normal text-pc-faint">{count}</span>
        {/if}
      </button>
    {:else if count !== undefined}
      <span class="flex-1 text-xs text-pc-faint">{count}</span>
    {:else}
      <span class="flex-1"></span>
    {/if}
    {#if actions && !collapsed}
      <div class="flex shrink-0 items-center gap-1">{@render actions()}</div>
    {/if}
  </header>
  {#if !collapsed}
    <div class="min-h-0 flex-1 {bodyClass}">{@render children()}</div>
  {/if}
</section>
