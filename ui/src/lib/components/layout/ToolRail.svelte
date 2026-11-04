<script lang="ts" generics="T extends string">
  // The tool rail (spec §9.2): a vertical strip left of the canvas, or a
  // horizontal strip along the bottom on phones. One button per group; it
  // shows the group's active (or last used) variant and selects it. A group
  // button also opens a flyout with its variants on a right-click, a long
  // press or Shift+F10 (docs/specs/tool-variant-flyout.md).
  import { onDestroy, untrack } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import ToolFlyout from './ToolFlyout.svelte';
  import {
    LONG_PRESS_MS,
    LONG_PRESS_SLOP,
    flyoutFocus,
    flyoutStep,
    hasFlyout,
    pressTarget,
    startsSection,
    type FlyoutMove,
    type ScreenBox,
    type ToolDef,
    type ToolGroup,
  } from './tools';
  import { actionFor } from '../../shell/shortcuts';
  import { isMac } from '../../platform';
  import { t } from '../../i18n/index.svelte';

  interface Props {
    groups: ToolGroup<T>[];
    active: T;
    onSelect: (tool: T) => void;
    orientation?: 'vertical' | 'horizontal';
    /** Document identity: an open flyout closes when it changes. */
    resetKey?: unknown;
  }
  const { groups, active, onSelect, orientation = 'vertical', resetKey = null }: Props = $props();

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

  let railEl = $state<HTMLElement | null>(null);
  let flyoutEl = $state<HTMLElement | null>(null);

  /** The open flyout: its group, where it is anchored, what it opened over. */
  interface OpenFlyout {
    groupId: string;
    anchor: ScreenBox;
    rail: ScreenBox;
    focusId: T | null;
    activeAtOpen: T;
    keyAtOpen: unknown;
  }
  let open = $state<OpenFlyout | null>(null);
  const openGroup = $derived(open ? (groups.find((g) => g.id === open?.groupId) ?? null) : null);

  /** A press on a group button that may turn into a long press. */
  interface Press {
    pointerId: number;
    x: number;
    y: number;
    timer: ReturnType<typeof setTimeout> | null;
    fired: boolean;
  }
  let press: Press | null = null;
  /** Entry under a long-press pointer. */
  let hot = $state<T | null>(null);
  /** A long press just ended: the click that may follow it is not a selection. */
  let swallowClickUntil = 0;

  const IS_MAC = isMac();

  /** A press that opens a context menu: the right button, or a Control-click on macOS. */
  function isContextPress(e: MouseEvent): boolean {
    return e.button === 2 || (IS_MAC && e.button === 0 && e.ctrlKey);
  }

  function buttonOf(groupId: string): HTMLElement | null {
    return railEl?.querySelector<HTMLElement>(`[data-group="${groupId}"]`) ?? null;
  }

  function box(el: Element): ScreenBox {
    const r = el.getBoundingClientRect();
    return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
  }

  function openFlyout(g: ToolGroup<T>): void {
    const button = buttonOf(g.id);
    if (!button || !railEl || !hasFlyout(g)) return;
    open = {
      groupId: g.id,
      anchor: box(button),
      rail: box(railEl),
      focusId: flyoutFocus(g, active),
      activeAtOpen: active,
      keyAtOpen: resetKey,
    };
  }

  /** Closes the flyout; `focus` returns keyboard focus to its group button. */
  function closeFlyout(focus = false): void {
    const was = open;
    open = null;
    hot = null;
    if (focus && was) buttonOf(was.groupId)?.focus();
  }

  function choose(tool: T): void {
    const entry = openGroup?.tools.find((x) => x.id === tool);
    if (!entry || entry.disabled) return;
    closeFlyout(true);
    onSelect(tool);
  }

  // The flyout closes when the tool changes some other way, the document
  // is replaced or its group stops offering variants.
  $effect(() => {
    const a = active;
    const k = resetKey;
    const g = openGroup;
    untrack(() => {
      if (open && (a !== open.activeAtOpen || k !== open.keyAtOpen || !g || !hasFlyout(g))) closeFlyout();
    });
  });

  function cancelPress(): void {
    if (press?.timer) clearTimeout(press.timer);
    press = null;
  }

  onDestroy(cancelPress);

  function onButtonPointerDown(e: PointerEvent, g: ToolGroup<T>): void {
    // Every button tracks the hold: a long press on a single button opens
    // nothing and selects nothing either.
    if (!e.isPrimary || e.button !== 0 || isContextPress(e)) return;
    cancelPress();
    const pointerId = e.pointerId;
    press = {
      pointerId,
      x: e.clientX,
      y: e.clientY,
      fired: false,
      timer: setTimeout(() => {
        if (press?.pointerId !== pointerId) return;
        press.timer = null;
        press.fired = true;
        if (hasFlyout(g)) openFlyout(g);
      }, LONG_PRESS_MS),
    };
  }

  function onButtonClick(g: ToolGroup<T>, shown: ToolDef<T>): void {
    if (performance.now() < swallowClickUntil) return;
    onSelect(pressTarget(g, shown));
  }

  function onButtonContextMenu(e: MouseEvent, g: ToolGroup<T>): void {
    e.preventDefault();
    // A touch or pen long press fires its own `contextmenu`; the press
    // logic owns that gesture.
    if (press !== null || !hasFlyout(g)) return;
    if (open?.groupId === g.id) {
      // A second right-click on the same button closes; a keyboard-made
      // event (no right button) leaves the flyout it just opened.
      if (isContextPress(e)) closeFlyout();
      return;
    }
    openFlyout(g);
  }

  function onButtonKeyDown(e: KeyboardEvent, g: ToolGroup<T>): void {
    if (e.key !== 'ContextMenu' && !(e.shiftKey && e.key === 'F10')) return;
    e.preventDefault();
    e.stopPropagation();
    openFlyout(g);
  }

  /** The enabled flyout entry at a screen point, if any. */
  function entryAt(x: number, y: number): T | null {
    const hit = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-flyout-tool]');
    if (!hit || !flyoutEl?.contains(hit)) return null;
    const tool = openGroup?.tools.find((x) => x.id === hit.dataset.flyoutTool);
    return tool && !tool.disabled ? tool.id : null;
  }

  function onWindowPointerDown(e: PointerEvent): void {
    // A second pointer makes the press a gesture, not a long press.
    if (press && e.pointerId !== press.pointerId && !press.fired) cancelPress();
    if (!open) return;
    const target = e.target instanceof Node ? e.target : null;
    if (target && flyoutEl?.contains(target)) return;
    // A right-click on the open group's own button toggles it closed in
    // `onButtonContextMenu`.
    if (isContextPress(e) && target && buttonOf(open.groupId)?.contains(target)) return;
    closeFlyout();
  }

  function onWindowPointerMove(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointerId) return;
    if (press.fired) {
      hot = entryAt(e.clientX, e.clientY);
      return;
    }
    if (Math.hypot(e.clientX - press.x, e.clientY - press.y) > LONG_PRESS_SLOP) cancelPress();
  }

  function onWindowPointerUp(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointerId) return;
    const fired = press.fired;
    cancelPress();
    if (!fired) return;
    swallowClickUntil = performance.now() + 400;
    hot = null;
    const tool = entryAt(e.clientX, e.clientY);
    if (tool !== null) choose(tool);
  }

  function onWindowPointerCancel(e: PointerEvent): void {
    if (press && e.pointerId === press.pointerId) cancelPress();
    hot = null;
  }

  // No browser context menu anywhere on the rail: separators, padding and
  // disabled buttons included.
  function onWindowContextMenu(e: MouseEvent): void {
    if (e.target instanceof Node && railEl?.contains(e.target)) e.preventDefault();
  }

  /** Tab stops of the page outside the flyout, in document order. */
  function tabStops(): HTMLElement[] {
    return Array.from(
      document.querySelectorAll<HTMLElement>('a[href], button, input, select, textarea, [tabindex]'),
    ).filter(
      (el) =>
        el.tabIndex >= 0 &&
        !(el instanceof HTMLButtonElement && el.disabled) &&
        el.getClientRects().length > 0 &&
        !flyoutEl?.contains(el),
    );
  }

  /** Tab / Shift+Tab out of the flyout: on to the stop after / before its button. */
  function tabOut(back: boolean): void {
    const button = open ? buttonOf(open.groupId) : null;
    closeFlyout();
    if (!button) return;
    const stops = tabStops();
    const at = stops.indexOf(button);
    const next = at < 0 ? button : stops[(at + (back ? -1 : 1) + stops.length) % stops.length];
    next?.focus();
  }

  const MOVES: Readonly<Record<string, FlyoutMove>> = {
    ArrowDown: 'next',
    ArrowUp: 'prev',
    Home: 'first',
    End: 'last',
  };

  // While the flyout is open it owns the keyboard: its keys act on it, and
  // no other key reaches the editor's shortcuts.
  function onWindowKeyDownCapture(e: KeyboardEvent): void {
    const g = openGroup;
    if (!open || !g) return;
    e.stopPropagation();
    const at = document.activeElement;
    const focused =
      at instanceof HTMLElement && flyoutEl?.contains(at)
        ? (g.tools.find((x) => x.id === at.dataset.flyoutTool)?.id ?? null)
        : null;
    const move = MOVES[e.key];
    if (move) {
      e.preventDefault();
      const next = flyoutStep(g.tools, focused, move);
      if (next !== null) flyoutEl?.querySelector<HTMLElement>(`[data-flyout-tool="${next}"]`)?.focus();
      return;
    }
    switch (e.key) {
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (focused !== null) choose(focused);
        return;
      case 'Escape':
        e.preventDefault();
        closeFlyout(true);
        return;
      case 'Tab':
        e.preventDefault();
        tabOut(e.shiftKey);
        return;
      default:
        // Keep the browser's own action for an editor chord (Ctrl+S's
        // "save page") from standing in for the blocked shortcut.
        if (actionFor(e) !== null || e.ctrlKey || e.metaKey) e.preventDefault();
    }
  }
</script>

<svelte:window
  onkeydowncapture={onWindowKeyDownCapture}
  onpointerdowncapture={onWindowPointerDown}
  onpointermove={onWindowPointerMove}
  onpointerup={onWindowPointerUp}
  onpointercancel={onWindowPointerCancel}
  oncontextmenu={onWindowContextMenu}
  onresize={() => {
    if (open) closeFlyout();
  }}
/>

<div
  bind:this={railEl}
  class={orientation === 'vertical'
    ? 'rail flex w-12 shrink-0 flex-col items-center gap-0.5 overflow-y-auto border-r border-pc-border bg-pc-panel py-1.5 select-none [scrollbar-width:none]'
    : 'rail flex h-14 shrink-0 items-center gap-1 overflow-x-auto border-t border-pc-border bg-pc-panel px-2 select-none [scrollbar-width:none]'}
  role="toolbar"
  aria-label={t('frame.tools')}
  aria-orientation={orientation}
  onscroll={() => {
    if (open) closeFlyout();
  }}
>
  {#each groups as g, i (g.id)}
    {@const tool = current(g)}
    {#if startsSection(groups[i - 1]?.id, g.id)}
      <div
        class={orientation === 'vertical'
          ? 'my-1 h-px w-6 shrink-0 bg-pc-border'
          : 'mx-0.5 h-7 w-px shrink-0 bg-pc-border'}
        role="separator"
        aria-orientation={orientation === 'vertical' ? 'horizontal' : 'vertical'}
      ></div>
    {/if}
    {#if tool}
      {@const on = g.tools.some((x) => x.id === active)}
      {@const flyout = hasFlyout(g)}
      <button
        type="button"
        class="relative flex shrink-0 items-center justify-center rounded-md text-pc-muted hover:bg-pc-hover hover:text-pc-text disabled:opacity-35 disabled:hover:bg-transparent"
        class:h-9={orientation === 'vertical'}
        class:w-9={orientation === 'vertical'}
        class:h-11={orientation === 'horizontal'}
        class:w-11={orientation === 'horizontal'}
        class:tool-on={on}
        style:touch-action={flyout ? (orientation === 'vertical' ? 'pan-y' : 'pan-x') : undefined}
        data-group={g.id}
        title={tool.title ?? `${g.tools.length > 1 ? g.label : tool.label}${g.shortcut ? ` (${g.shortcut})` : ''}`}
        aria-label={g.tools.length > 1 ? `${g.label}: ${tool.label}` : tool.label}
        aria-pressed={on}
        aria-haspopup={flyout ? 'menu' : undefined}
        aria-expanded={flyout ? open?.groupId === g.id : undefined}
        disabled={tool.disabled && g.tools.every((x) => x.disabled)}
        onclick={() => onButtonClick(g, tool)}
        onpointerdown={(e) => onButtonPointerDown(e, g)}
        oncontextmenu={(e) => onButtonContextMenu(e, g)}
        onkeydown={(e) => onButtonKeyDown(e, g)}
      >
        <Icon name={tool.icon} size={orientation === 'vertical' ? 20 : 22} />
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

{#if open && openGroup}
  {#key open}
    <ToolFlyout
      bind:el={flyoutEl}
      group={openGroup}
      {active}
      {hot}
      focusId={open.focusId}
      anchor={open.anchor}
      rail={open.rail}
      {orientation}
      onChoose={choose}
    />
  {/key}
{/if}

<style>
  .rail {
    -webkit-touch-callout: none;
  }
  .tool-on {
    background-color: var(--color-pc-accent);
    color: var(--color-pc-accent-fg);
  }
  .tool-on:hover {
    background-color: var(--color-pc-accent);
    color: var(--color-pc-accent-fg);
  }
</style>
