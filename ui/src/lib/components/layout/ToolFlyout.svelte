<script lang="ts" generics="T extends string">
  // Variant flyout of one tool-rail group (spec §9.2, tool-variant-flyout):
  // a small menu next to the group button, opened by a right-click, a long
  // press or Shift+F10. `ToolRail` owns opening, closing and the keyboard;
  // this component draws the entries and places itself.
  import { tick, untrack } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import { placeFlyout, type FlyoutPlacement, type ScreenBox, type ToolGroup } from './tools';

  interface Props {
    group: ToolGroup<T>;
    active: T;
    /** Entry under a long-press finger (highlighted like a hover). */
    hot: T | null;
    /** Entry that takes focus once the flyout is placed. */
    focusId: T | null;
    anchor: ScreenBox;
    rail: ScreenBox;
    orientation: 'vertical' | 'horizontal';
    onChoose: (tool: T) => void;
    /** The menu element, for the rail's outside-press and release checks. */
    el?: HTMLElement | null;
  }
  let { group, active, hot, focusId, anchor, rail, orientation, onChoose, el = $bindable(null) }: Props =
    $props();

  let place = $state<FlyoutPlacement | null>(null);

  // Measure the natural size once mounted, then place and focus.
  $effect(() => {
    const menu = el;
    if (!menu) return;
    untrack(() => {
      const box = menu.getBoundingClientRect();
      place = placeFlyout(
        anchor,
        rail,
        { width: box.width, height: menu.scrollHeight },
        { width: window.innerWidth, height: window.innerHeight },
        orientation,
      );
      // Focus once placed: a hidden element takes no focus.
      const id = focusId;
      if (id !== null) {
        void tick().then(() =>
          menu.querySelector<HTMLElement>(`[data-flyout-tool="${id}"]`)?.focus({ preventScroll: true }),
        );
      }
    });
  });
</script>

<div
  bind:this={el}
  class="flyout fixed z-50 flex min-w-44 flex-col overflow-y-auto select-none"
  style:left="{place?.left ?? 0}px"
  style:top="{place?.top ?? 0}px"
  style:max-height={place ? `${place.maxHeight}px` : undefined}
  style:visibility={place ? 'visible' : 'hidden'}
  role="menu"
  aria-label={group.label}
  tabindex="-1"
  oncontextmenu={(e) => e.preventDefault()}
>
  <div
    class="shrink-0 px-3 pt-1 pb-1 text-[11px] font-semibold tracking-wide text-pc-faint"
    data-flyout-header
    aria-hidden="true"
  >
    {group.label}{group.shortcut ? ` (${group.shortcut})` : ''}
  </div>
  {#each group.tools as tool (tool.id)}
    {@const on = tool.id === active}
    <button
      type="button"
      class="entry"
      class:entry-hot={hot === tool.id && !tool.disabled}
      role="menuitemradio"
      aria-checked={on}
      tabindex="-1"
      disabled={tool.disabled}
      title={tool.title}
      data-flyout-tool={tool.id}
      onclick={() => {
        if (!tool.disabled) onChoose(tool.id);
      }}
    >
      <span class="w-4 shrink-0 text-pc-accent" aria-hidden="true">{on ? '✓' : ''}</span>
      <Icon name={tool.icon} size={17} />
      <span class="flex-1">{tool.label}</span>
    </button>
  {/each}
</div>

<style>
  .flyout {
    border-radius: 0.375rem;
    border: 1px solid var(--color-pc-border);
    background-color: var(--color-pc-panel);
    padding: 0.25rem 0;
    font-size: 0.875rem;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.45);
    -webkit-touch-callout: none;
  }
  .flyout:focus {
    outline: none;
  }
  .entry {
    display: flex;
    flex-shrink: 0;
    width: 100%;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem 0.35rem 0.5rem;
    text-align: left;
    white-space: nowrap;
    color: var(--color-pc-text);
  }
  .entry:hover:not(:disabled),
  .entry:focus,
  .entry-hot {
    background-color: var(--color-pc-hover);
    outline: none;
  }
  .entry:disabled {
    opacity: 0.4;
  }
</style>
