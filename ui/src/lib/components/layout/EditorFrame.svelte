<script lang="ts">
  // The editor frame both modes share (spec §9.2): top bar with menus and
  // quick actions, tool rail, options bar, canvas, panels, status bar.
  // It restructures by window size (`viewport.svelte.ts`):
  //   wide   ≥1024  rail left · sidebar docked right (toggleable)
  //   medium 640+   rail left · sidebar as a drawer over the canvas
  //   narrow <640   ☰ menu · tools as a bottom strip · panels in a
  //                 bottom sheet with one tab per panel
  // Panel collapse state, sidebar visibility and width persist per mode.
  // While an effect is open (`effectPanel`), its panel sits on top of the
  // sidebar / drawer / sheet, which show for it, and every other panel is
  // inert (effect-panel spec).
  import { onMount, tick, type Snippet } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import IconButton from './IconButton.svelte';
  import MenuBar from './MenuBar.svelte';
  import PanelHost from './PanelHost.svelte';
  import type { MenuDef } from './menu';
  import {
    SIDEBAR_DEFAULT_WIDTH,
    SIDEBAR_MIN_WIDTH,
    loadLayout,
    saveLayout,
    sidebarMaxWidth,
    sidebarWidth,
    type FrameLayout,
    type PanelDef,
  } from './panels.svelte';
  import { splitter } from './splitter';
  import { viewport, watchViewport } from './viewport.svelte';
  import { t } from '../../i18n/index.svelte';
  import { modifiers, notePointer } from '../../shell/modifiers.svelte';
  import ModifierBar from './ModifierBar.svelte';
  import TabBar, { type TabBarProps } from './TabBar.svelte';

  interface Props {
    /** Mode badge and the key layout prefs are stored under. */
    mode: 'pixel' | 'image';
    menus: MenuDef[];
    panels: PanelDef[];
    /** Document name and unsaved marker for the top bar. */
    title: string;
    dirty?: boolean;
    /** The tool rail, rendered for the given orientation. */
    tools: Snippet<['vertical' | 'horizontal']>;
    /** Tool options for the active tool. */
    options?: Snippet;
    /** The canvas viewport. */
    viewport: Snippet;
    /** Top-bar quick actions (undo / redo). */
    quick?: Snippet;
    /** Status bar content. */
    statusbar?: Snippet;
    /** The open effect's panel (title for the sheet tab), or `null`. */
    effectPanel?: { title: string; body: Snippet } | null;
    /** The open documents, drawn as a tab bar above the canvas. */
    tabs?: TabBarProps;
  }
  const {
    mode,
    menus,
    panels,
    title,
    dirty = false,
    tools,
    options,
    viewport: stage,
    quick,
    statusbar,
    effectPanel = null,
    tabs,
  }: Props = $props();

  let layout = $state<FrameLayout>({ collapsed: {}, sidebar: true });
  // Drawer (medium) and bottom sheet (narrow) are per-session, not stored.
  let drawerOpen = $state(false);
  let sheetTab = $state<string | null>(null);

  const sidePanels = $derived(panels.filter((p) => (p.placement ?? 'side') === 'side'));
  const bottomPanels = $derived(panels.filter((p) => p.placement === 'bottom'));
  const size = $derived(viewport.size);
  const sheetPanel = $derived(panels.find((p) => p.id === sheetTab) ?? null);
  const effectOpen = $derived(effectPanel !== null);
  // The effect shows the sidebar, drawer or sheet without touching their
  // own state, so each returns to it when the effect ends (AC-02, AC-12 –
  // AC-14).
  const sidebarShown = $derived(layout.sidebar || effectOpen);
  const drawerShown = $derived(drawerOpen || effectOpen);
  const sheetShown = $derived(sheetPanel !== null || effectOpen);

  // When the effect panel closes, focus goes to the canvas area: the menu
  // entry that opened it is gone (AC-06).
  let stageEl = $state<HTMLElement | null>(null);
  let hadEffect = false;
  $effect(() => {
    const open = effectOpen;
    if (hadEffect && !open) void tick().then(() => stageEl?.focus());
    hadEffect = open;
  });

  onMount(() => {
    const stop = watchViewport();
    void loadLayout(mode).then((l) => (layout = l));
    return stop;
  });

  function isCollapsed(p: PanelDef): boolean {
    return layout.collapsed[p.id] ?? p.defaultCollapsed ?? false;
  }

  function toggleCollapsed(p: PanelDef): void {
    layout.collapsed[p.id] = !isCollapsed(p);
    saveLayout(mode, layout);
  }

  function toggleSidebar(): void {
    if (size === 'wide') {
      layout.sidebar = !layout.sidebar;
      saveLayout(mode, layout);
    } else if (size === 'medium') {
      drawerOpen = !drawerOpen;
    } else {
      sheetTab = sheetTab === null ? (panels[0]?.id ?? null) : null;
    }
  }

  // Docked sidebar width (effect-panel AC-07 – AC-11): the stored width,
  // clamped to what the window allows; the stored value only changes when
  // the user moves the splitter.
  const asideWidth = $derived(sidebarWidth(layout.sidebarWidth, viewport.width));
  const asideMax = $derived(sidebarMaxWidth(viewport.width));
  const SPLITTER_STEP = 16;
  let dragBase = 0;

  function setSidebarWidth(width: number): void {
    layout.sidebarWidth = sidebarWidth(width, viewport.width);
  }

  // The splitter is the sidebar's left edge: moving it left widens the
  // sidebar. A drag that ends any way keeps the width it last set.
  const splitterEvents = {
    begin: () => (dragBase = asideWidth),
    move: (dx: number) => setSidebarWidth(dragBase - dx),
    end: () => saveLayout(mode, layout),
    step: (direction: -1 | 1) => {
      setSidebarWidth(asideWidth - direction * SPLITTER_STEP);
      saveLayout(mode, layout);
    },
    reset: () => {
      layout.sidebarWidth = SIDEBAR_DEFAULT_WIDTH;
      saveLayout(mode, layout);
    },
  };

  const panelsShown = $derived(
    size === 'wide' ? layout.sidebar : size === 'medium' ? drawerOpen : sheetTab !== null,
  );
</script>

<!-- A panel inside the sidebar, bottom dock or sheet; PanelHost hands
     it its slot (title, host, collapse state) through context. -->
{#snippet hosted(p: PanelDef, host: 'side' | 'bottom' | 'sheet')}
  <PanelHost
    title={p.title}
    {host}
    collapsed={isCollapsed(p)}
    onToggle={() => toggleCollapsed(p)}
    body={p.body}
  />
{/snippet}

{#snippet sidebarBody()}
  {#if effectPanel}
    <div class="shrink-0 border-b border-pc-border">{@render effectPanel.body()}</div>
  {/if}
  <!-- Inert while an effect is open: the preview depends on what they set. -->
  <div class="flex min-h-0 flex-1 flex-col" inert={effectOpen}>
    {#each sidePanels as p (p.id)}
      <div
        class="flex min-h-0 flex-col border-b border-pc-border"
        class:flex-1={p.grow && !(isCollapsed(p))}
        class:shrink-0={!p.grow || (isCollapsed(p))}
      >
        {@render hosted(p, 'side')}
      </div>
    {/each}
  </div>
{/snippet}

<div class="flex h-full flex-col overflow-hidden bg-pc-bg text-pc-text">
  <!-- Top bar -->
  <header
    class="flex h-11 shrink-0 items-center gap-1 border-b border-pc-border bg-pc-panel pr-2 pl-1 sm:pl-3"
  >
    {#if size === 'narrow'}
      <MenuBar {menus} compact />
    {:else if size === 'wide'}
      <span class="mr-1 text-sm font-semibold tracking-wide">Pincel</span>
    {/if}
    <span
      class="mr-1 rounded border border-pc-border px-1.5 text-[11px] leading-5 text-pc-accent capitalize"
    >
      {t(`mode.${mode}`)}
    </span>
    {#if size !== 'narrow'}
      <MenuBar {menus} />
    {/if}
    <span class="min-w-0 flex-1"></span>
    <span
      class="min-w-0 shrink truncate text-xs text-pc-muted"
      class:max-w-40={size !== 'wide'}
      title={dirty ? t('frame.unsaved', { title }) : title}
    >
      {title}{#if dirty}<span class="text-amber-400"> ●</span>{/if}
    </span>
    <span class="ml-1 flex items-center">
      {@render quick?.()}
      <IconButton
        icon={size === 'narrow' ? 'layers' : 'sidebar'}
        label={size === 'narrow' ? t('frame.panels') : t('frame.togglePanels')}
        pressed={panelsShown}
        disabled={effectOpen}
        onclick={toggleSidebar}
      />
    </span>
  </header>

  <div class="relative flex min-h-0 flex-1">
    {#if size !== 'narrow'}
      {@render tools('vertical')}
    {/if}

    <div class="flex min-w-0 flex-1 flex-col">
      {#if options}
        <div
          class="flex h-11 shrink-0 items-center gap-3 overflow-x-auto border-b border-pc-border bg-pc-panel px-3 text-xs text-pc-muted [scrollbar-width:none]"
          role="toolbar"
          aria-label={t('frame.toolOptions')}
        >
          {@render options()}
        </div>
      {/if}
      {#if tabs}
        <TabBar {...tabs} />
      {/if}
      <main
        class="relative min-h-0 flex-1 overflow-hidden outline-none"
        tabindex="-1"
        bind:this={stageEl}
        onpointerdowncapture={notePointer}
      >
        {@render stage()}
        {#if modifiers.shown}
          <ModifierBar />
        {/if}
        {#if size === 'narrow' && sheetShown}
          <div
            class="absolute inset-x-0 bottom-0 z-30 flex max-h-[60%] flex-col rounded-t-lg border-t border-pc-border bg-pc-panel shadow-[0_-8px_30px_rgb(0_0_0/0.5)]"
            role="dialog"
            aria-label={t('frame.panels')}
          >
            <div class="flex shrink-0 items-center gap-1 overflow-x-auto px-2 pt-1.5 [scrollbar-width:none]" role="tablist">
              {#if effectPanel}
                <!-- The open effect's own tab, selected while it is open (AC-13). -->
                <button
                  type="button"
                  class="flex shrink-0 items-center gap-1.5 rounded-md bg-pc-active px-2.5 py-1.5 text-xs"
                  role="tab"
                  aria-selected="true"
                >
                  <Icon name="wand" size={15} />
                  {effectPanel.title}
                </button>
              {/if}
              {#each panels as p (p.id)}
                <button
                  type="button"
                  class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs disabled:opacity-35"
                  class:bg-pc-active={!effectPanel && p.id === sheetTab}
                  class:text-pc-muted={effectPanel || p.id !== sheetTab}
                  role="tab"
                  aria-selected={!effectPanel && p.id === sheetTab}
                  disabled={effectOpen}
                  onclick={() => (sheetTab = p.id)}
                >
                  <Icon name={p.icon} size={15} />
                  {p.title}
                </button>
              {/each}
              <span class="flex-1"></span>
              {#if !effectPanel}
                <IconButton icon="close" label={t('frame.closePanels')} size="sm" onclick={() => (sheetTab = null)} />
              {/if}
            </div>
            <div class="flex min-h-0 flex-1 flex-col overflow-y-auto" role="tabpanel">
              {#if effectPanel}
                {@render effectPanel.body()}
              {:else if sheetPanel}
                {#key sheetPanel.id}
                  {@render hosted(sheetPanel, 'sheet')}
                {/key}
              {/if}
            </div>
          </div>
        {/if}
      </main>
      {#if size !== 'narrow'}
        {#each bottomPanels as p (p.id)}
          <div class="shrink-0 border-t border-pc-border bg-pc-panel" inert={effectOpen}>
            {@render hosted(p, 'bottom')}
          </div>
        {/each}
      {/if}
      {#if statusbar && size !== 'narrow'}
        <footer
          class="flex h-7 shrink-0 items-center gap-3 overflow-hidden border-t border-pc-border bg-pc-panel px-3 text-xs whitespace-nowrap text-pc-faint"
        >
          {@render statusbar()}
        </footer>
      {/if}
    </div>

    {#if size === 'wide' && sidebarShown}
      <!-- Splitter: the sidebar's draggable left edge (effect-panel AC-07). -->
      <div
        class="w-1 shrink-0 cursor-col-resize touch-none bg-pc-border hover:bg-pc-accent focus-visible:bg-pc-accent focus-visible:outline-none"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('frame.sidebarWidth')}
        aria-valuenow={asideWidth}
        aria-valuemin={SIDEBAR_MIN_WIDTH}
        aria-valuemax={asideMax}
        {@attach splitter(splitterEvents)}
      ></div>
      <aside
        class="flex shrink-0 flex-col overflow-y-auto bg-pc-panel"
        style:width="{asideWidth}px"
        aria-label={t('frame.panels')}
      >
        {@render sidebarBody()}
      </aside>
    {:else if size === 'medium' && drawerShown}
      <aside
        class="absolute inset-y-0 right-0 z-30 flex w-72 flex-col overflow-y-auto border-l border-pc-border bg-pc-panel shadow-[-8px_0_30px_rgb(0_0_0/0.5)]"
        aria-label={t('frame.panels')}
      >
        {@render sidebarBody()}
      </aside>
    {/if}
  </div>

  {#if size === 'narrow'}
    {@render tools('horizontal')}
  {/if}
</div>
