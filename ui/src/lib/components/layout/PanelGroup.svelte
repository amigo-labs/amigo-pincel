<script lang="ts" module>
  let groups = 0;
</script>

<script lang="ts">
  // One tab group of the sidebar (docs/specs/sidebar-tab-groups.md): a
  // header with a tab per panel and the collapse control, then the active
  // panel. Every panel of the group stays mounted below — `Panel.svelte`
  // draws nothing for one that is not the active tab or whose group is
  // collapsed — so a panel keeps its drafts and running work across a tab
  // switch (AC-16 – AC-18) while only the active one is on the page (AC-03).
  import type { Snippet } from 'svelte';
  import Icon from '../icons/Icon.svelte';

  interface Tab {
    id: string;
    title: string;
  }
  interface Props {
    tabs: readonly Tab[];
    /** The active tab's panel id. */
    active: string;
    collapsed: boolean;
    /** Chooses a tab: it becomes active and the group expands (AC-04, AC-05). */
    onChoose: (id: string) => void;
    onToggle: () => void;
    /** The group's panels, all of them. */
    children: Snippet;
  }
  const { tabs, active, collapsed, onChoose, onToggle, children }: Props = $props();

  groups += 1;
  const uid = `pc-group-${groups}`;
  const tabId = (id: string): string => `${uid}-${id}`;
  /** The group's name for assistive technology: its panels' titles. */
  const name = $derived(tabs.map((tab) => tab.title).join(', '));

  let row: HTMLDivElement | undefined = $state();

  // Left / Right move to the neighbouring tab and choose it, without
  // wrapping at the ends; Home / End go to the first / last (AC-12).
  function onKeydown(e: KeyboardEvent, index: number): void {
    const last = tabs.length - 1;
    const next =
      e.key === 'ArrowRight'
        ? Math.min(last, index + 1)
        : e.key === 'ArrowLeft'
          ? Math.max(0, index - 1)
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? last
              : -1;
    if (next < 0) return;
    e.preventDefault();
    e.stopPropagation();
    const tab = tabs[next];
    if (!tab) return;
    if (next !== index || collapsed) onChoose(tab.id);
    const el = row?.querySelector<HTMLElement>(`#${CSS.escape(tabId(tab.id))}`);
    el?.focus();
    el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }
</script>

<!-- The header has the docked panel header's height and label style
     (`.am-panel--docked > header`), written with utilities: the shared
     classes have no dense tab row. -->
<div
  class="flex h-8 shrink-0 items-stretch pr-3 pl-3 text-[11px] leading-4 font-semibold tracking-[0.06em] text-pc-faint uppercase"
  class:border-b={!collapsed}
  class:border-pc-border={!collapsed}
>
  <div
    bind:this={row}
    class="flex min-w-0 flex-1 items-stretch gap-3 overflow-x-auto [scrollbar-width:none]"
    role="tablist"
    aria-label={name}
  >
    {#each tabs as tab, i (tab.id)}
      {@const selected = tab.id === active}
      <button
        type="button"
        id={tabId(tab.id)}
        class="shrink-0 border-b-2 tracking-[inherit] whitespace-nowrap uppercase hover:text-pc-text"
        class:border-pc-accent={selected}
        class:text-pc-text={selected}
        class:border-transparent={!selected}
        role="tab"
        aria-selected={selected}
        aria-controls={collapsed ? undefined : `${uid}-panel`}
        tabindex={selected ? 0 : -1}
        onclick={() => {
          if (!selected || collapsed) onChoose(tab.id);
        }}
        onkeydown={(e) => onKeydown(e, i)}
      >
        {tab.title}
      </button>
    {/each}
  </div>
  <button
    type="button"
    class="ml-2 flex w-6 shrink-0 items-center justify-center hover:text-pc-text"
    aria-label={name}
    aria-expanded={!collapsed}
    onclick={onToggle}
  >
    <Icon name={collapsed ? 'chevron-right' : 'chevron-down'} size={14} />
  </button>
</div>
<!-- One element in both states, so the panels inside never remount; it is a
     tab panel only while the group is expanded (AC-12). -->
<div
  id="{uid}-panel"
  class={collapsed ? 'contents' : 'flex min-h-0 flex-1 flex-col'}
  role={collapsed ? undefined : 'tabpanel'}
  aria-labelledby={collapsed ? undefined : tabId(active)}
>
  {@render children()}
</div>
