<script lang="ts" generics="T extends string">
  // Variant flyout of one tool-rail group (spec §9.2, tool-variant-flyout):
  // a small menu next to the group button, opened by a right-click, a long
  // press or Shift+F10. `ToolRail` owns opening, closing and the keyboard;
  // this component draws the entries and places itself.
  import { tick, untrack } from 'svelte';
  import { activeOnFocus } from './menu-focus';
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
  class="flyout am-menu fixed z-(--pc-z-dialog) min-w-44 overflow-y-auto select-none *:shrink-0"
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
    class="shrink-0 px-3 pt-1 pb-1 text-(length:--pc-text-label) font-semibold tracking-wide text-pc-faint"
    data-flyout-header
    aria-hidden="true"
  >
    {group.label}{group.shortcut ? ` (${group.shortcut})` : ''}
  </div>
  {#each group.tools as tool (tool.id)}
    {@const on = tool.id === active}
    <button
      type="button"
      use:activeOnFocus={hot === tool.id && !tool.disabled}
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
      <span class="am-menu-label inline-flex items-center"
        ><span class="am-menu-check text-pc-accent-text" aria-hidden="true"
          >{#if on}<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7" /></svg>{/if}</span
        ><Icon
          name={tool.icon}
          size={17}
        /><span class="ml-2">{tool.label}</span></span
      >
    </button>
  {/each}
</div>

<style>
  .flyout {
    -webkit-touch-callout: none;
  }
  .flyout:focus {
    outline: none;
  }
</style>
