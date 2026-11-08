<script lang="ts">
  // The editor frame both modes share (spec §9.2): top bar with menus and
  // quick actions, tool rail, options bar, canvas, panels, status bar.
  // It restructures by window size (`viewport.svelte.ts`):
  //   wide   ≥1024  rail left · sidebar docked right (toggleable)
  //   medium 640+   rail left · sidebar as a drawer over the canvas
  //   narrow <640   ☰ menu · tools as a bottom strip · panels in a
  //                 bottom sheet with one tab per panel
  // Panel collapse state, sidebar visibility and width persist per mode.
  // While an effect is open (`effectPanel`), its panel is a small dialog
  // over the canvas area (docked to the bottom edge on phones) and the
  // regions around the canvas area are dimmed and inert (effect-panel spec).
  import { onMount, tick, type Snippet } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import { mover, provideEffectHost } from './effect-host';
  import IconButton from './IconButton.svelte';
  import MenuBar from './MenuBar.svelte';
  import PanelHost from './PanelHost.svelte';
  import type { MenuDef } from './menu';
  import {
    SIDEBAR_DEFAULT_WIDTH,
    SIDEBAR_MIN_WIDTH,
    effectPanelPos,
    loadLayout,
    saveLayout,
    sidebarMaxWidth,
    sidebarWidth,
    type EffectPos,
    type FrameLayout,
    type PanelDef,
  } from './panels.svelte';
  import { splitter } from './splitter';
  import { viewport, watchViewport } from './viewport.svelte';
  import { t } from '../../i18n/index.svelte';
  import { modifiers, notePointer } from '../../shell/modifiers.svelte';
  import ModifierBar from './ModifierBar.svelte';
  import TabBar, { type TabBarProps } from './TabBar.svelte';
  import { chrome } from './chrome.svelte';

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
    /** A short offer shown in the header's free space (the tour hint,
     *  first-run-tutorial AC-10). */
    headerHint?: Snippet;
    /** The open effect's panel, or `null`. */
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
    headerHint,
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

  // The effect panel's place (AC-18 – AC-20): the stored offset from the
  // canvas area's top-right corner, clamped so the panel stays wholly
  // inside the canvas area (AC-19); the stored value only changes when
  // the user moves the panel.
  let areaWidth = $state(0);
  let areaHeight = $state(0);
  let panelWidth = $state(0);
  let panelHeight = $state(0);
  const effectPos = $derived(
    effectPanelPos(
      layout.effectPos,
      { width: areaWidth, height: areaHeight },
      { width: panelWidth, height: panelHeight },
    ),
  );
  let moveBase: EffectPos = { x: 0, y: 0 };

  function setEffectPos(pos: EffectPos): void {
    layout.effectPos = effectPanelPos(
      pos,
      { width: areaWidth, height: areaHeight },
      { width: panelWidth, height: panelHeight },
    );
  }

  // Screen movement right / down shrinks the right offset / grows the top one.
  provideEffectHost({
    get floating() {
      return viewport.size !== 'narrow';
    },
    titleBar: mover({
      enabled: () => viewport.size !== 'narrow',
      begin: () => (moveBase = effectPos),
      move: (dx, dy) => setEffectPos({ x: moveBase.x - dx, y: moveBase.y + dy }),
      end: () => saveLayout(mode, layout),
      step: (dx, dy) => {
        setEffectPos({ x: effectPos.x - dx, y: effectPos.y + dy });
        saveLayout(mode, layout);
      },
    }),
  });

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

  // The canvas stage takes focus on a press, so Tab there toggles the UI
  // (power-user-shortcuts-coverage AC-09) while Tab anywhere else keeps
  // moving focus. A press on a control inside the stage leaves it alone.
  function onStagePointerDown(e: PointerEvent): void {
    notePointer(e);
    const target = e.target instanceof Element ? e.target : null;
    const control = target?.closest(
      'input, textarea, select, button, a, [contenteditable], [tabindex]:not([data-stage]), [role="dialog"]',
    );
    if (!control) stageEl?.focus({ preventScroll: true });
  }

  // `data-dimmed` shades a region outside the canvas area (app.css).
  const dim = $derived(effectOpen ? '' : undefined);

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
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
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
  <!-- Top bar. While an effect is open it is dimmed and inert except the
       Effects and Adjustments menus, which MenuBar lifts above the shade
       (AC-03, AC-05). -->
  <header
    class="relative flex h-11 shrink-0 items-center gap-1 border-b border-pc-border bg-pc-panel pr-2 pl-1 sm:pl-3"
    data-dimmed={dim}
  >
    {#if size === 'narrow'}
      <MenuBar {menus} compact restricted={effectOpen} />
    {:else if size === 'wide'}
      <span class="mr-1 text-sm font-semibold tracking-wide" inert={effectOpen}>Pincel</span>
    {/if}
    <span
      class="mr-1 rounded bg-pc-accent/15 px-1.5 text-[11px] leading-5 font-semibold text-pc-accent-text capitalize"
      inert={effectOpen}
    >
      {t(`mode.${mode}`)}
    </span>
    {#if size !== 'narrow'}
      <MenuBar {menus} restricted={effectOpen} />
    {/if}
    <span class="flex min-w-0 flex-1 justify-center overflow-hidden" inert={effectOpen}>
      {@render headerHint?.()}
    </span>
    <span
      class="min-w-0 shrink truncate text-xs text-pc-muted"
      class:max-w-40={size !== 'wide'}
      title={dirty ? t('frame.unsaved', { title }) : title}
      inert={effectOpen}
    >
      {title}{#if dirty}<span class="text-amber-400"> ●</span>{/if}
    </span>
    <span class="ml-1 flex items-center" inert={effectOpen} data-tour="quick">
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
    {#if size !== 'narrow' && !chrome.hidden}
      <div class="relative flex shrink-0" inert={effectOpen} data-dimmed={dim}>
        {@render tools('vertical')}
      </div>
    {/if}

    <div class="flex min-w-0 flex-1 flex-col">
      {#if options && !chrome.hidden}
        <div class="relative shrink-0" inert={effectOpen} data-dimmed={dim}>
          <div
            class:scroll-fade-x={size === 'narrow'}
            class="flex h-11 items-center gap-4 overflow-x-auto border-b border-pc-border bg-pc-panel px-3 text-xs text-pc-muted [scrollbar-width:none]"
            role="toolbar"
            aria-label={t('frame.toolOptions')}
          >
            {@render options()}
          </div>
        </div>
      {/if}
      {#if tabs}
        <div class="relative shrink-0" inert={effectOpen} data-dimmed={dim}>
          <TabBar {...tabs} />
        </div>
      {/if}
      <main
        bind:this={stageEl}
        bind:clientWidth={areaWidth}
        bind:clientHeight={areaHeight}
        class="relative min-h-0 flex-1 overflow-hidden outline-none"
        tabindex="-1"
        data-stage=""
        onpointerdowncapture={onStagePointerDown}
      >
        {@render stage()}
        {#if modifiers.shown}
          <ModifierBar />
        {/if}
        {#if size === 'narrow' && sheetPanel && !chrome.hidden}
          <div
            class="absolute inset-x-0 bottom-0 z-30 flex max-h-[60%] flex-col rounded-t-lg border-t border-pc-border bg-pc-panel shadow-[0_-8px_30px_rgb(0_0_0/0.5)]"
            role="dialog"
            aria-label={t('frame.panels')}
            inert={effectOpen}
            data-dimmed={dim}
          >
            <div class="flex shrink-0 items-center gap-1 overflow-x-auto px-2 pt-1.5 [scrollbar-width:none]" role="tablist">
              {#each panels as p (p.id)}
                <button
                  type="button"
                  class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs"
                  class:bg-pc-active={p.id === sheetTab}
                  class:text-pc-muted={p.id !== sheetTab}
                  role="tab"
                  aria-selected={p.id === sheetTab}
                  onclick={() => (sheetTab = p.id)}
                >
                  <Icon name={p.icon} size={15} />
                  {p.title}
                </button>
              {/each}
              <span class="flex-1"></span>
              <IconButton icon="close" label={t('frame.closePanels')} size="sm" onclick={() => (sheetTab = null)} />
            </div>
            <div class="flex min-h-0 flex-1 flex-col overflow-y-auto" role="tabpanel">
              {#key sheetPanel.id}
                {@render hosted(sheetPanel, 'sheet')}
              {/key}
            </div>
          </div>
        {/if}
        {#if effectPanel && size !== 'narrow'}
          <!-- The effect panel floats over the canvas area (AC-01, AC-12). -->
          <div
            class="absolute z-[35] flex max-h-full w-80 flex-col"
            style:top="{effectPos.y}px"
            style:right="{effectPos.x}px"
            bind:offsetWidth={panelWidth}
            bind:offsetHeight={panelHeight}
          >
            {@render effectPanel.body()}
          </div>
        {/if}
      </main>
      {#if size !== 'narrow' && !chrome.hidden}
        {#each bottomPanels as p (p.id)}
          <div
            class="relative shrink-0 border-t border-pc-border bg-pc-panel"
            inert={effectOpen}
            data-dimmed={dim}
          >
            {@render hosted(p, 'bottom')}
          </div>
        {/each}
      {/if}
      {#if statusbar && size !== 'narrow'}
        <footer
          inert={effectOpen}
          data-dimmed={dim}
          class="relative flex h-7 shrink-0 items-center gap-3 overflow-hidden border-t border-pc-border bg-pc-panel px-3 text-xs whitespace-nowrap text-pc-faint"
        >
          {@render statusbar()}
        </footer>
      {/if}
    </div>

    {#if chrome.hidden}
      <!-- Toggle UI: no panels. -->
    {:else if size === 'wide' && layout.sidebar}
      <!-- Splitter: the sidebar's draggable left edge (effect-panel AC-07). -->
      <div
        inert={effectOpen}
        data-dimmed={dim}
        class="relative w-1 shrink-0 cursor-col-resize touch-none bg-pc-border hover:bg-pc-accent focus-visible:bg-pc-accent focus-visible:outline-none"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('frame.sidebarWidth')}
        aria-valuenow={asideWidth}
        aria-valuemin={SIDEBAR_MIN_WIDTH}
        aria-valuemax={asideMax}
        {@attach splitter(splitterEvents)}
      ></div>
      <aside
        class="relative flex shrink-0 flex-col bg-pc-panel"
        style:width="{asideWidth}px"
        aria-label={t('frame.panels')}
        inert={effectOpen}
        data-dimmed={dim}
      >
        {@render sidebarBody()}
      </aside>
    {:else if size === 'medium' && drawerOpen}
      <aside
        class="absolute inset-y-0 right-0 z-30 flex w-72 flex-col border-l border-pc-border bg-pc-panel shadow-[-8px_0_30px_rgb(0_0_0/0.5)]"
        aria-label={t('frame.panels')}
        inert={effectOpen}
        data-dimmed={dim}
      >
        {@render sidebarBody()}
      </aside>
    {/if}
  </div>

  {#if size === 'narrow' && !chrome.hidden}
    <div class="relative shrink-0" inert={effectOpen} data-dimmed={dim}>
      {@render tools('horizontal')}
    </div>
  {/if}

  {#if effectPanel && size === 'narrow'}
    <!-- Phones: docked to the bottom edge across the window, over the
         sheet, at most half the window tall (AC-13). -->
    <div class="fixed inset-x-0 bottom-0 z-[35] flex max-h-[50vh] flex-col">
      {@render effectPanel.body()}
    </div>
  {/if}
</div>
