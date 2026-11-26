<script lang="ts">
  // Editor menu bar (spec §9.2). Wide and medium windows get a row of
  // dropdown menus; `compact` collapses them behind one ☰ button into a
  // stacked sheet (phones). Dropdowns are `position: fixed`, anchored to
  // their trigger, so a horizontally scrolling bar never clips them.
  // `restricted` (an effect is open, effect-panel AC-03) leaves only the
  // menus marked `liveWithEffect` usable: the others are inert, and the
  // live triggers sit above the top bar's shade (`z-[46]`, app.css 45),
  // on an opaque background so the shade does not show through them.
  import { activeOnFocus } from './menu-focus';
  import Icon from '../icons/Icon.svelte';
  import type { MenuAction, MenuDef, MenuItem } from './menu';
  import { t } from '../../i18n/index.svelte';

  interface Props {
    menus: MenuDef[];
    /** Collapse into a single ☰ sheet. */
    compact?: boolean;
    /** Only the `liveWithEffect` menus stay usable. */
    restricted?: boolean;
    /** When set, only the menus with these ids stay usable; the others are
     *  inert (an Assistant answer runs, ai-chat-panel AC-10). */
    only?: readonly string[] | null;
  }
  const { menus, compact = false, restricted = false, only = null }: Props = $props();

  const isLive = (m: MenuDef): boolean =>
    (!restricted || m.liveWithEffect === true) && (only === null || only.includes(m.id));
  const liveMenus = $derived(menus.filter(isLive));

  // Open top-level menu id (or '☰' for the compact sheet).
  let openId = $state<string | null>(null);
  let anchor = $state<{ left: number; top: number }>({ left: 0, top: 0 });
  // Open submenu: its label and flyout position (wide), or the labels
  // expanded inline (compact).
  let sub = $state<{ label: string; left: number; top: number } | null>(null);
  let expanded = $state<Record<string, boolean>>({});
  let dropdown = $state<HTMLElement | null>(null);

  const openMenu = $derived(menus.find((m) => m.id === openId) ?? null);

  function place(el: HTMLElement): void {
    const r = el.getBoundingClientRect();
    anchor = { left: Math.max(4, Math.min(r.left, window.innerWidth - 240)), top: r.bottom + 2 };
  }

  function toggle(id: string, e: MouseEvent): void {
    sub = null;
    if (openId === id) {
      openId = null;
      return;
    }
    place(e.currentTarget as HTMLElement);
    openId = id;
  }

  function hoverSwitch(id: string, e: MouseEvent): void {
    // Menubar convention: once one menu is open, hovering its siblings
    // switches without another click.
    if (openId === null || openId === '☰' || openId === id) return;
    sub = null;
    place(e.currentTarget as HTMLElement);
    openId = id;
  }

  function close(): void {
    cancelSubClose();
    openId = null;
    sub = null;
  }

  function run(item: MenuAction): void {
    if (item.disabled) return;
    close();
    item.run();
  }

  // Leaving a submenu row for a plain row closes the flyout after a short
  // grace period, so a pointer travelling diagonally to the flyout across
  // other rows does not lose it.
  let subCloseTimer: ReturnType<typeof setTimeout> | null = null;

  function cancelSubClose(): void {
    if (subCloseTimer !== null) clearTimeout(subCloseTimer);
    subCloseTimer = null;
  }

  function scheduleSubClose(): void {
    cancelSubClose();
    subCloseTimer = setTimeout(() => (sub = null), 300);
  }

  function openSub(label: string, e: Event): void {
    cancelSubClose();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const right = r.right + 220 < window.innerWidth;
    sub = { label, left: right ? r.right - 2 : r.left - 218, top: r.top - 4 };
  }

  function focusables(): HTMLElement[] {
    return dropdown
      ? Array.from(dropdown.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([disabled])'))
      : [];
  }

  function onKeydown(e: KeyboardEvent): void {
    if (openId === null) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const items = focusables();
      if (items.length === 0) return;
      const at = items.indexOf(document.activeElement as HTMLElement);
      const step = e.key === 'ArrowDown' ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
      return;
    }
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && !compact) {
      e.preventDefault();
      const row = liveMenus;
      const at = row.findIndex((m) => m.id === openId);
      const next = row[(at + (e.key === 'ArrowRight' ? 1 : -1) + row.length) % row.length];
      const trigger = document.querySelector<HTMLElement>(`[data-menu-trigger="${next?.id}"]`);
      if (next && trigger) {
        sub = null;
        // A menu bar that scrolls (a medium window, long menu names) brings
        // the trigger into view first (frame-layout AC-18).
        trigger.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        place(trigger);
        openId = next.id;
      }
    }
  }

  function onWindowChange(): void {
    if (openId !== null) close();
  }
</script>

<svelte:window onkeydown={onKeydown} onresize={onWindowChange} onblur={onWindowChange} />

{#snippet checkMark()}
  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7" /></svg>
{/snippet}

{#snippet rows(items: MenuItem[], depth: number)}
  {#each items as item, i (i)}
    {#if item.kind === 'separator'}
      <hr />
    {:else if item.kind === 'submenu'}
      {#if compact}
        <button
          type="button"
          class="w-full"
          use:activeOnFocus
          style:padding-left="{0.75 + depth * 0.75}rem"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={expanded[item.label] ?? false}
          disabled={item.disabled}
          onclick={() => (expanded[item.label] = !(expanded[item.label] ?? false))}
        >
          <span class="am-menu-label"
            ><span class="am-menu-check align-middle" aria-hidden="true"></span>{item.label}</span
          >
          <Icon name={expanded[item.label] ? 'chevron-down' : 'chevron-right'} size={14} />
        </button>
        {#if expanded[item.label]}
          {@render rows(item.items, depth + 1)}
        {/if}
      {:else}
        <button
          type="button"
          class="w-full"
          use:activeOnFocus={sub?.label === item.label}
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={sub?.label === item.label}
          disabled={item.disabled}
          onclick={(e) => openSub(item.label, e)}
          onpointerenter={(e) => {
            if (e.pointerType === 'mouse') openSub(item.label, e);
          }}
        >
          <span class="am-menu-label"
            ><span class="am-menu-check align-middle" aria-hidden="true"></span>{item.label}</span
          >
          <Icon name="chevron-right" size={14} />
        </button>
      {/if}
    {:else}
      <button
        type="button"
        class={['w-full', item.danger && 'am-danger']}
        use:activeOnFocus
        style:padding-left={compact ? `${0.75 + depth * 0.75}rem` : undefined}
        role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
        aria-checked={item.checked}
        disabled={item.disabled}
        onclick={() => run(item)}
        onpointerenter={() => {
          if (!compact && depth === 0 && sub !== null) scheduleSubClose();
        }}
      >
        <span class="am-menu-label"
          ><span class="am-menu-check align-middle text-pc-accent-text" aria-hidden="true"
            >{#if item.checked}{@render checkMark()}{/if}</span
          >{item.label}</span
        >
        {#if item.shortcut}
          <!-- Visual hint only: the accessible name stays the label. -->
          <kbd class="am-kbd" aria-hidden="true">{item.shortcut}</kbd>
        {/if}
      </button>
    {/if}
  {/each}
{/snippet}

{#if compact}
  <button
    type="button"
    class="flex h-9 w-9 items-center justify-center rounded-pc-control hover:bg-pc-hover"
    class:bg-pc-active={openId === '☰'}
    class:relative={restricted}
    class:z-[46]={restricted}
    class:bg-pc-panel={restricted && openId !== '☰'}
    aria-label={t('frame.menu')}
    aria-haspopup="menu"
    aria-expanded={openId === '☰'}
    onclick={(e) => toggle('☰', e)}
  >
    <Icon name="menu" size={20} />
  </button>
{:else}
  <!-- Above the backdrop while a menu is open, so hovering a sibling
       trigger switches menus (see `hoverSwitch`). -->
  <div
    class="flex min-w-0 items-center overflow-x-auto [scrollbar-width:none]"
    class:relative={openId !== null && !restricted}
    class:z-50={openId !== null && !restricted}
    role="menubar"
  >
    {#each menus as m (m.id)}
      <button
        type="button"
        class="shrink-0 rounded-pc-control px-2 py-1 text-sm whitespace-nowrap hover:bg-pc-hover lg:px-2.5"
        class:bg-pc-active={openId === m.id}
        class:relative={restricted && isLive(m)}
        class:z-[46]={restricted && isLive(m)}
        class:bg-pc-panel={restricted && isLive(m) && openId !== m.id}
        inert={!isLive(m)}
        data-menu-trigger={m.id}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={openId === m.id}
        onclick={(e) => toggle(m.id, e)}
        onpointerenter={(e) => hoverSwitch(m.id, e)}
      >
        {m.label}
      </button>
    {/each}
  </div>
{/if}

{#if openId !== null}
  <!-- Backdrop closes the menu on an outside click. -->
  <button
    type="button"
    class="fixed inset-0 z-40 cursor-default"
    tabindex="-1"
    aria-label={t('frame.closeMenu')}
    onclick={close}
  ></button>
  {#if openId === '☰'}
    <div
      bind:this={dropdown}
      class="am-menu fixed z-50 max-h-[80vh] w-72 max-w-[calc(100vw-1rem)] overflow-y-auto *:shrink-0"
      style:left="{anchor.left}px"
      style:top="{anchor.top}px"
      role="menu"
      aria-label={t('frame.menu')}
    >
      {#each menus as m, i (m.id)}
        {#if i > 0}<hr />{/if}
        <div class="relative" inert={!isLive(m)} data-dimmed={isLive(m) ? undefined : ''}>
          <div class="px-3 pt-1.5 pb-0.5 text-[11px] font-semibold tracking-wide text-pc-faint uppercase">
            {m.label}
          </div>
          {@render rows(m.items, 0)}
        </div>
      {/each}
    </div>
  {:else if openMenu}
    <div
      bind:this={dropdown}
      class="am-menu fixed z-50 max-h-[80vh] min-w-56 overflow-y-auto *:shrink-0"
      style:left="{anchor.left}px"
      style:top="{anchor.top}px"
      role="menu"
      aria-label={openMenu.label}
    >
      {@render rows(openMenu.items, 0)}
    </div>
    {#if sub}
      {@const subItem = openMenu.items.find((it) => it.kind === 'submenu' && it.label === sub?.label)}
      {#if subItem?.kind === 'submenu'}
        <div
          class="am-menu fixed z-50 max-h-[70vh] min-w-52 overflow-y-auto *:shrink-0"
          style:left="{sub.left}px"
          style:top="{sub.top}px"
          role="menu"
          aria-label={subItem.label}
          tabindex="-1"
          onpointerenter={cancelSubClose}
        >
          {@render rows(subItem.items, 1)}
        </div>
      {/if}
    {/if}
  {/if}
{/if}

