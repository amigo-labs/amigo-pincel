<script lang="ts">
  // The editor frame both modes share (spec §9.2, docs/specs/frame-layout.md),
  // top to bottom: app bar (menus, undo / redo, command search, panels
  // toggle), tab bar, then tool rail · options bar over the canvas · panels,
  // then the status bar. App bar, tab bar and status bar span the window.
  // In the macOS desktop app, which draws no menus, the tabs sit in the
  // app bar's row on medium and wide windows (frame-layout AC-14).
  // It restructures by window size (`viewport.svelte.ts`):
  //   wide   ≥1024  rail left · sidebar docked right (toggleable)
  //   medium 640+   rail left · sidebar as a drawer over the canvas
  //   narrow <640   ☰ menu · tools as a bottom strip · panels in a
  //                 bottom sheet with one tab per panel
  // Panel collapse state, sidebar visibility and width persist per mode,
  // and so does which panels the user removed in View ▸ Panels
  // (docs/specs/panel-visibility.md): a removed panel stays mounted and
  // draws nothing, so it shows what it showed when it comes back.
  // While an effect is open (`effectPanel`), its panel is a small dialog
  // over the canvas area (docked to the bottom edge on phones) and the
  // regions around the canvas area are dimmed and inert (effect-panel spec).
  // While an Assistant answer runs, the tool rail, options bar, tab bar,
  // quick actions and every menu but View are inert (ai-chat-panel AC-10);
  // the canvas still zooms and pans, the panels stay usable.
  // In the macOS desktop app the frame draws no menus: the native menu bar
  // mirrors them while this editor's tab is active (macos-native-menu).
  import { onMount, tick, untrack, type Snippet } from 'svelte';
  import AgentIndicator from '../../agent/AgentIndicator.svelte';
  import { assistantBusy } from '../../assistant/chat.svelte';
  import { generationBusy } from '../../comfyui/generation.svelte';
  import { buildMirror } from '../../menu/native-mirror';
  import { showEditorMenus } from '../../menu/native-sync';
  import { textFocus, trackTextFocus } from '../../menu/text-focus.svelte';
  import { hasNativeMenu } from '../../platform';
  import Icon from '../icons/Icon.svelte';
  import { mover, provideEffectHost } from './effect-host';
  import IconButton from './IconButton.svelte';
  import MenuBar from './MenuBar.svelte';
  import OptionsBar from './OptionsBar.svelte';
  import PanelGroup from './PanelGroup.svelte';
  import PanelHost from './PanelHost.svelte';
  import { PANEL_GROUPS, groupState, nextEnabled, resetPanels, type GroupState } from './groups';
  import type { MenuDef, MenuItem } from './menu';
  import {
    SIDEBAR_DEFAULT_WIDTH,
    SIDEBAR_MIN_WIDTH,
    effectPanelPos,
    loadLayout,
    panelRequest,
    saveLayout,
    setPanelMenu,
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
  import ZoomControl, { type ZoomProps } from './ZoomControl.svelte';
  import { statusDrop, type StatusInfo } from './status';
  import { chrome } from './chrome.svelte';

  interface Props {
    /** Mode badge and the key layout prefs are stored under. */
    mode: 'pixel' | 'image';
    menus: MenuDef[];
    panels: PanelDef[];
    /** The tool rail, rendered for the given orientation. */
    tools: Snippet<['vertical' | 'horizontal']>;
    /** Tool options for the active tool. */
    options?: Snippet;
    /** The active tool's name, first in the options bar
     *  (options-bar-layout AC-01). */
    toolName?: string;
    /** The canvas viewport. */
    viewport: Snippet;
    /** App-bar quick actions (undo / redo). */
    quick?: Snippet;
    /** Opens the command palette (the app bar's search control,
     *  frame-layout AC-04). */
    onSearch?: () => void;
    /** Tooltip hint of the command palette's shortcut. */
    searchHint?: string;
    /** Status bar content; the frame lays it out the same in both modes
     *  (frame-layout AC-06). */
    status?: StatusInfo;
    /** A mode's own status readout between the text and the pointer
     *  position (Pixel mode's frame-time probe). */
    statusExtra?: Snippet;
    /** The zoom control: last in the status bar, floating over the canvas
     *  on a narrow window (frame-layout AC-11). */
    zoom?: ZoomProps | null;
    /** A short offer shown in the header's free space (the tour hint,
     *  first-run-tutorial AC-10). */
    headerHint?: Snippet;
    /** The open effect's panel, or `null`. */
    effectPanel?: { title: string; body: Snippet } | null;
    /** The open documents, drawn as a tab bar above the canvas. */
    tabs?: TabBarProps;
    /** This editor's tab is the shown one; only then does it feed the
     *  native menu bar (macos-native-menu AC-05). */
    active?: boolean;
  }
  const {
    mode,
    menus,
    panels,
    tools,
    options,
    toolName = '',
    viewport: stage,
    quick,
    onSearch,
    searchHint = '',
    status,
    statusExtra,
    zoom = null,
    headerHint,
    effectPanel = null,
    tabs,
    active = true,
  }: Props = $props();

  let layout = $state<FrameLayout>({ collapsed: {}, sidebar: true });
  // Drawer (medium) and bottom sheet (narrow) are per-session, not stored.
  let drawerOpen = $state(false);
  let sheetTab = $state<string | null>(null);

  const sidePanels = $derived(panels.filter((p) => (p.placement ?? 'side') === 'side'));
  const bottomPanels = $derived(panels.filter((p) => p.placement === 'bottom'));
  const size = $derived(viewport.size);
  const removedIds = $derived(layout.removed ?? []);
  const isRemoved = (p: PanelDef): boolean => removedIds.includes(p.id);
  /** The enabled panels: the sidebar's, and the bottom sheet's tabs. */
  const enabledSide = $derived(sidePanels.filter((p) => !isRemoved(p)));
  const sheetPanels = $derived(panels.filter((p) => !isRemoved(p)));
  const sheetPanel = $derived(sheetPanels.find((p) => p.id === sheetTab) ?? null);
  /** The panels toggle has something to show (panel-visibility AC-12). */
  const hasPanels = $derived(size === 'narrow' ? sheetPanels.length > 0 : enabledSide.length > 0);
  const effectOpen = $derived(effectPanel !== null);
  const answering = $derived(assistantBusy());
  /** The panel that stays usable while an answer runs (AC-10). */
  const ASSISTANT_PANEL = 'assistant';
  const GENERATE_PANEL = 'generate';
  /** Menus that stay usable while an answer runs (AC-10). */
  const liveMenus = $derived(answering ? ['view'] : null);

  /** The OS draws the menu bar: the frame draws none (macos-native-menu AC-01). */
  const nativeMenu = hasNativeMenu();
  /** macOS desktop app on a medium or wide window: the tabs share the
   *  app bar's row (frame-layout AC-14). */
  const tabsInAppBar = $derived(nativeMenu && size !== 'narrow');

  // Mirror the menus into the native bar while this tab shows, with the
  // inert menus dimmed (AC-03 – AC-06) and the text-editing entries
  // following the focus (AC-12, AC-13).
  $effect(() => {
    if (!nativeMenu || !active) return;
    trackTextFocus();
    // Help stays usable, as the native Help did before (AC-06).
    const isLive = (m: MenuDef) =>
      m.id === 'help' ||
      ((!effectOpen || m.liveWithEffect === true) && (liveMenus === null || liveMenus.includes(m.id)));
    showEditorMenus(buildMirror(menus, { live: isLive, textFocus: textFocus.on }));
  });

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
    return () => {
      stop();
      setPanelMenu(mode, null);
    };
  });

  // The sidebar's blocks (sidebar-tab-groups AC-01, AC-02): consecutive
  // panels of one group share a block with a tab per panel, any other
  // panel is a block of its own.
  type Block =
    | { kind: 'single'; panel: PanelDef }
    | { kind: 'group'; id: string; panels: PanelDef[] };
  const blocks = $derived.by(() => {
    const out: Block[] = [];
    for (const p of sidePanels) {
      const last = out.at(-1);
      if (p.group !== undefined && last?.kind === 'group' && last.id === p.group) last.panels.push(p);
      else if (p.group !== undefined) out.push({ kind: 'group', id: p.group, panels: [p] });
      else out.push({ kind: 'single', panel: p });
    }
    return out;
  });

  /** Active tab and collapse state of `group`, for the panels it has here. */
  function stateOf(group: string): GroupState {
    const ids = sidePanels.filter((p) => p.group === group).map((p) => p.id);
    return groupState(layout, group, ids);
  }

  function setGroup(group: string, next: GroupState): void {
    layout.groups = { ...layout.groups, [group]: next };
    saveLayout(mode, layout);
  }

  /** Whether the panel's content is hidden: a single panel is collapsed, a
   *  tab of a group is not the active one or its group is collapsed, or
   *  the panel is removed. */
  function isCollapsed(p: PanelDef): boolean {
    if (isRemoved(p)) return true;
    if (p.group !== undefined) {
      const state = stateOf(p.group);
      return state.collapsed || state.active !== p.id;
    }
    return layout.collapsed[p.id] ?? p.defaultCollapsed ?? false;
  }

  // A group's lone enabled panel draws its own header (panel-visibility
  // AC-05); its toggle collapses the group.
  function toggleCollapsed(p: PanelDef): void {
    if (p.group !== undefined) {
      const state = stateOf(p.group);
      setGroup(p.group, { ...state, collapsed: !state.collapsed });
      return;
    }
    layout.collapsed[p.id] = !isCollapsed(p);
    saveLayout(mode, layout);
  }

  // A request to show a panel (`expandPanel`): expanded, and visible in
  // the sidebar, the drawer or the bottom sheet.
  let seenRequest = panelRequest.n;
  $effect(() => {
    const { n, mode: requested, id } = panelRequest;
    if (n === seenRequest) return;
    seenRequest = n;
    if (requested === mode) untrack(() => revealPanel(id));
  });

  function revealPanel(id: string): void {
    const p = panels.find((def) => def.id === id);
    if (p === undefined) return;
    // A removed panel is enabled first (panel-visibility AC-06).
    if (isRemoved(p)) setRemoved(removedIds.filter((other) => other !== id));
    if (p.group !== undefined) layout.groups = { ...layout.groups, [p.group]: { active: id, collapsed: false } };
    else if (isCollapsed(p)) layout.collapsed[id] = false;
    if (size === 'wide') layout.sidebar = true;
    else if (size === 'medium') drawerOpen = true;
    else sheetTab = id;
    saveLayout(mode, layout);
  }

  function setRemoved(ids: string[]): void {
    if (ids.length > 0) layout.removed = ids;
    else delete layout.removed;
  }

  // View ▸ Panels (panel-visibility). Removing the active tab of a group
  // or of the bottom sheet hands the place to its neighbour (AC-04).
  function removePanel(p: PanelDef): void {
    const removed = [...removedIds, p.id];
    if (p.group !== undefined) {
      const state = stateOf(p.group);
      const ids = sidePanels.filter((other) => other.group === p.group).map((other) => other.id);
      const next = nextEnabled(ids, p.id, removed);
      if (state.active === p.id && next !== '') {
        layout.groups = { ...layout.groups, [p.group]: { active: next, collapsed: state.collapsed } };
      }
    }
    if (sheetTab === p.id) {
      const next = nextEnabled(
        panels.map((other) => other.id),
        p.id,
        removed,
      );
      sheetTab = next === '' ? null : next;
    }
    setRemoved(removed);
    saveLayout(mode, layout);
  }

  // Enabling brings the panel into view (AC-06); the Timeline is docked
  // again, or shown in the bottom sheet (AC-07).
  function enablePanel(p: PanelDef): void {
    if ((p.placement ?? 'side') === 'side') {
      revealPanel(p.id);
      return;
    }
    setRemoved(removedIds.filter((id) => id !== p.id));
    if (size === 'narrow') sheetTab = p.id;
    saveLayout(mode, layout);
  }

  function resetAllPanels(): void {
    layout = resetPanels($state.snapshot(layout));
    saveLayout(mode, layout);
  }

  // One entry per panel in sidebar order, the Timeline after them, then
  // Reset Panels (AC-01, AC-02). Work in progress keeps its panel: the
  // panel is the only place to follow or stop it (AC-15).
  const panelItems = $derived.by((): MenuItem[] => {
    const generating = generationBusy();
    const locked = (p: PanelDef): boolean =>
      (p.id === ASSISTANT_PANEL && answering) || (p.id === GENERATE_PANEL && generating);
    return [
      ...[...sidePanels, ...bottomPanels].map(
        (p): MenuItem => ({
          label: p.title,
          checked: !isRemoved(p),
          disabled: locked(p),
          run: () => (isRemoved(p) ? enablePanel(p) : removePanel(p)),
        }),
      ),
      { kind: 'separator' },
      { label: t('menu.resetPanels'), disabled: answering || generating, run: resetAllPanels },
    ];
  });
  $effect(() => setPanelMenu(mode, panelItems));

  function toggleSidebar(): void {
    if (size === 'wide') {
      layout.sidebar = !layout.sidebar;
      saveLayout(mode, layout);
    } else if (size === 'medium') {
      drawerOpen = !drawerOpen;
    } else {
      sheetTab = sheetTab === null ? (sheetPanels[0]?.id ?? null) : null;
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

  // Status bar (frame-layout AC-07): the text region takes what the other
  // items leave; when even an empty text region is not enough, the pointer
  // position goes first and the probe readout next. Left-out items stay
  // measurable (invisible, out of the flow) so they return when there is room.
  let statusRow = $state(0);
  let statusTail = $state(0);
  let statusPos = $state(0);
  let statusProbe = $state(0);
  let dropped = $state<0 | 1 | 2>(0);
  $effect(() => {
    const posShown = status?.ready === true && status.position !== null;
    const position = posShown ? statusPos : 0;
    const probe = status?.ready === true && statusExtra ? statusProbe : 0;
    // `statusTail` holds the position while that is in the flow.
    const inFlow = untrack(() => dropped) === 0 && posShown;
    const tail = statusTail - (inFlow ? statusPos + 12 : 0);
    dropped = statusDrop(statusRow, tail, position, probe);
  });

  /** The zoom menu closes when its context changes (frame-layout AC-10). */
  const zoomCloseKey = $derived(`${tabs?.active ?? ''}|${effectOpen}|${size}`);

  // Narrow windows: the floating zoom control stays above the panel sheet
  // and the docked effect panel (frame-layout AC-11, AC-12).
  let sheetHeight = $state(0);
  let effectDockHeight = $state(0);
  const zoomLift = $derived.by(() => {
    const sheet = size === 'narrow' && sheetPanel && !chrome.hidden ? sheetHeight : 0;
    // The dock is fixed to the window's bottom edge; the tool strip (56 px)
    // lies between that edge and the canvas area unless the UI is hidden.
    const strip = chrome.hidden ? 0 : 56;
    const dock = effectPanel && size === 'narrow' ? Math.max(0, effectDockHeight - strip) : 0;
    return 12 + Math.max(sheet, dock);
  });

  // `data-dimmed` shades a region outside the canvas area (app.css).
  const dim = $derived(effectOpen ? '' : undefined);

  const panelsShown = $derived(
    hasPanels && (size === 'wide' ? layout.sidebar : size === 'medium' ? drawerOpen : sheetTab !== null),
  );
</script>

<!-- A panel inside the sidebar, bottom dock or sheet; PanelHost hands
     it its slot (title, host, collapse state) through context. -->
{#snippet hosted(p: PanelDef, host: 'side' | 'bottom' | 'sheet', grouped = false)}
  <!-- While an answer runs only the Assistant panel takes input; the
       others would change the document (AC-10). -->
  <div class="contents" inert={answering && p.id !== ASSISTANT_PANEL}>
    <PanelHost
      title={p.title}
      {host}
      {grouped}
      removed={isRemoved(p)}
      collapsed={isCollapsed(p)}
      onToggle={() => toggleCollapsed(p)}
      body={p.body}
    />
  </div>
{/snippet}

{#snippet sidebarBody()}
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
    {#each blocks as block (block.kind === 'group' ? `group:${block.id}` : block.panel.id)}
      {#if block.kind === 'group'}
        {@const state = stateOf(block.id)}
        {@const enabled = block.panels.filter((p) => !isRemoved(p))}
        {@const grows = PANEL_GROUPS[block.id]?.grow === true && !state.collapsed}
        {@const tabbed = enabled.length > 1}
        <!-- Growing groups share the height the other blocks leave in equal
             parts, down to a floor that keeps a header, list and buttons
             inside; a group that does not grow is as high as its panel's
             content (sidebar-tab-groups AC-13). The tab row stays usable
             while an answer runs: only the panels below it are inert.
             With one enabled panel the group is drawn like a single panel,
             with none it is not drawn (panel-visibility AC-05). -->
        <div
          class="flex-col border-pc-border"
          class:flex={enabled.length > 0}
          class:hidden={enabled.length === 0}
          class:border-b={tabbed}
          class:flex-1={grows}
          class:basis-0={grows}
          class:min-h-40={grows}
          class:min-h-0={!grows}
          class:shrink-0={!grows}
          data-panel-group={block.id}
        >
          <PanelGroup
            tabs={enabled}
            active={state.active}
            collapsed={state.collapsed}
            onChoose={(id) => setGroup(block.id, { active: id, collapsed: false })}
            onToggle={() => setGroup(block.id, { ...state, collapsed: !state.collapsed })}
          >
            {#each block.panels as p (p.id)}
              {@render hosted(p, 'side', tabbed)}
            {/each}
          </PanelGroup>
        </div>
      {:else}
        {@const p = block.panel}
        <!-- A growing panel shrinks with the space left, but not below a
             floor that keeps its header, list and buttons inside it: with
             less room the sidebar scrolls instead of the panel's buttons
             spilling over the panel below. -->
        <div
          class="flex-col"
          class:flex={!isRemoved(p)}
          class:hidden={isRemoved(p)}
          class:flex-1={p.grow && !isCollapsed(p)}
          class:min-h-40={p.grow && !isCollapsed(p)}
          class:min-h-0={!p.grow || isCollapsed(p)}
          class:shrink-0={!p.grow || isCollapsed(p)}
        >
          {@render hosted(p, 'side')}
        </div>
      {/if}
    {/each}
  </div>
{/snippet}

<div class="flex h-full flex-col overflow-hidden bg-pc-bg text-pc-text">
  <!-- Top bar. While an effect is open it is dimmed and inert except the
       Effects and Adjustments menus, which MenuBar lifts above the shade
       (AC-03, AC-05). -->
  <header
    class="relative flex shrink-0 items-center gap-1 border-b border-pc-border bg-pc-panel pr-2 pl-1 sm:pl-3"
    class:h-11={size === 'narrow'}
    class:h-10={size !== 'narrow'}
    data-frame="appbar"
    data-dimmed={dim}
  >
    <!-- In the macOS desktop app the native menu bar has the menus
         (macos-native-menu AC-01). -->
    {#if size === 'narrow' && !nativeMenu}
      <MenuBar {menus} compact restricted={effectOpen} only={liveMenus} />
    {:else if size === 'wide'}
      <span class="mr-1 shrink-0 text-sm font-semibold tracking-wide" inert={effectOpen}>Pincel</span>
    {/if}
    {#if size !== 'narrow' && !nativeMenu}
      <MenuBar {menus} restricted={effectOpen} only={liveMenus} />
    {/if}
    {#if tabs && tabsInAppBar}
      <div class="flex h-full min-w-0 flex-1 items-end" inert={effectOpen || answering}>
        <TabBar {...tabs} inline />
      </div>
      <span class="flex shrink-0 justify-center overflow-hidden" inert={effectOpen}>
        {@render headerHint?.()}
      </span>
    {/if}
    <span class="flex shrink-0 items-center" inert={effectOpen || answering} data-tour="quick">
      {@render quick?.()}
    </span>
    {#if !tabsInAppBar}
      <span class="flex min-w-0 flex-1 justify-center overflow-hidden" inert={effectOpen}>
        {@render headerHint?.()}
      </span>
    {/if}
    <span class="ml-1 flex shrink-0 items-center gap-1" inert={effectOpen}>
      {#if onSearch}
        <span class="flex items-center" inert={answering} data-search="">
          {#if size === 'wide'}
            <button
              type="button"
              class="btn-press flex h-7 w-56 items-center gap-2 rounded-pc-control border border-pc-border bg-pc-bg px-2 text-xs text-pc-muted hover:border-pc-faint hover:text-pc-text"
              aria-label={t('frame.search')}
              onclick={onSearch}
            >
              <Icon name="search" size={15} class="shrink-0" />
              <span class="min-w-0 flex-1 truncate text-left">{t('frame.search')}</span>
              {#if searchHint}<kbd class="am-kbd">{searchHint}</kbd>{/if}
            </button>
          {:else}
            <IconButton icon="search" label={t('frame.search')} hint={searchHint} onclick={onSearch} />
          {/if}
        </span>
      {/if}
      <span class="flex items-center" data-panels-toggle="">
        <IconButton
          icon={size === 'narrow' ? 'layers' : 'sidebar'}
          label={size === 'narrow' ? t('frame.panels') : t('frame.togglePanels')}
          pressed={panelsShown}
          disabled={effectOpen || !hasPanels}
          onclick={toggleSidebar}
        />
      </span>
    </span>
  </header>

  {#if tabs && !tabsInAppBar}
    <div class="relative shrink-0" inert={effectOpen || answering} data-dimmed={dim}>
      <TabBar {...tabs} />
    </div>
  {/if}

  <div class="relative flex min-h-0 flex-1">
    {#if size !== 'narrow' && !chrome.hidden}
      <div class="relative flex shrink-0" inert={effectOpen || answering} data-dimmed={dim}>
        {@render tools('vertical')}
      </div>
    {/if}

    <div class="flex min-w-0 flex-1 flex-col">
      {#if options && !chrome.hidden}
        <div class="relative shrink-0" inert={effectOpen || answering} data-dimmed={dim}>
          <OptionsBar name={toolName} narrow={size === 'narrow'} disabled={effectOpen || answering}>
            {@render options()}
          </OptionsBar>
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
        {#if zoom && size === 'narrow'}
          <ZoomControl
            {...zoom}
            floating
            lift={zoomLift}
            disabled={status?.ready === false}
            closeKey={zoomCloseKey}
          />
        {/if}
        {#if size === 'narrow' && sheetPanel && !chrome.hidden}
          <div
            class="absolute inset-x-0 bottom-0 z-30 flex max-h-[60%] flex-col rounded-t-lg border-t border-pc-border bg-pc-panel shadow-[0_-8px_30px_rgb(0_0_0/0.5)]"
            role="dialog"
            aria-label={t('frame.panels')}
            inert={effectOpen}
            data-dimmed={dim}
            bind:offsetHeight={sheetHeight}
          >
            <!-- The shared tab row; the close button sits beside it, not in it. -->
            <div class="flex shrink-0 items-end pr-2 pl-3">
              <div class="am-tabs min-w-0 flex-1 [scrollbar-width:none]" role="tablist">
                {#each sheetPanels as p (p.id)}
                  <button
                    type="button"
                    class="inline-flex shrink-0 items-center gap-1.5"
                    role="tab"
                    aria-selected={p.id === sheetTab}
                    onclick={() => (sheetTab = p.id)}
                  >
                    <Icon name={p.icon} size={15} />
                    {p.title}
                  </button>
                {/each}
              </div>
              <span class="flex h-9 shrink-0 items-center border-b border-pc-border pl-2">
                <IconButton icon="close" label={t('frame.closePanels')} size="sm" onclick={() => (sheetTab = null)} />
              </span>
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
          <!-- A removed Timeline leaves its height to the canvas
               (panel-visibility AC-08). -->
          <div
            class="relative shrink-0 border-t border-pc-border bg-pc-panel"
            class:hidden={isRemoved(p)}
            inert={effectOpen}
            data-dimmed={dim}
          >
            {@render hosted(p, 'bottom')}
          </div>
        {/each}
      {/if}
    </div>

    {#if chrome.hidden}
      <!-- Toggle UI: no panels. -->
    {:else if size === 'wide' && layout.sidebar}
      <!-- Splitter: the sidebar's draggable left edge (effect-panel AC-07). -->
      <!-- With every sidebar panel removed the sidebar is not drawn; it
           stays mounted so the panels keep their state (AC-12, AC-14). -->
      <div
        inert={effectOpen}
        data-dimmed={dim}
        class:hidden={!hasPanels}
        class="relative w-1 shrink-0 cursor-col-resize touch-none bg-pc-border hover:bg-pc-accent focus-visible:z-10 focus-visible:bg-pc-focus focus-visible:outline-offset-0"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('frame.sidebarWidth')}
        aria-valuenow={asideWidth}
        aria-valuemin={SIDEBAR_MIN_WIDTH}
        aria-valuemax={asideMax}
        {@attach splitter(splitterEvents)}
      ></div>
      <aside
        class="relative shrink-0 flex-col bg-pc-panel"
        class:flex={hasPanels}
        class:hidden={!hasPanels}
        style:width="{asideWidth}px"
        aria-label={t('frame.panels')}
        inert={effectOpen}
        data-dimmed={dim}
      >
        {@render sidebarBody()}
      </aside>
    {:else if size === 'medium' && drawerOpen}
      <aside
        class:flex={hasPanels}
        class:hidden={!hasPanels}
        class="absolute inset-y-0 right-0 z-30 w-72 flex-col border-l border-pc-border bg-pc-panel shadow-[-8px_0_30px_rgb(0_0_0/0.5)]"
        aria-label={t('frame.panels')}
        inert={effectOpen}
        data-dimmed={dim}
      >
        {@render sidebarBody()}
      </aside>
    {/if}
  </div>

  {#if status && size !== 'narrow'}
    <footer
      data-frame="status"
      class="flex h-7 shrink-0 items-stretch border-t border-pc-border bg-pc-panel text-xs whitespace-nowrap text-pc-faint"
    >
      <!-- While an effect is open everything but the zoom control is
           dimmed and inert (frame-layout AC-12). -->
      <div
        class="relative flex min-w-0 flex-1 items-center gap-3 overflow-hidden pr-1.5 pl-3"
        inert={effectOpen}
        data-dimmed={dim}
        bind:clientWidth={statusRow}
      >
        <span class="min-w-0 flex-1 truncate text-pc-muted" title={status.text} data-status-text="">
          {#if status.warning && status.text}
            <span class="am-chip am-chip--warning inline-flex max-w-full overflow-hidden">{status.text}</span>
          {:else}
            {status.text}
          {/if}
        </span>
        {#if status.ready}
          {#if statusExtra}
            <span
              class={dropped === 2 ? 'invisible absolute' : 'flex shrink-0 items-center'}
              bind:offsetWidth={statusProbe}
            >
              {@render statusExtra()}
            </span>
          {/if}
          <span class="flex shrink-0 items-center gap-3" bind:offsetWidth={statusTail}>
            <AgentIndicator />
            {#if status.position !== null}
              <span
                class={dropped >= 1 ? 'invisible absolute' : 'shrink-0 tabular-nums'}
                data-testid="status-pointer"
                bind:offsetWidth={statusPos}
              >
                {status.position}
              </span>
            {/if}
            <span class="shrink-0 tabular-nums" data-status-size="">{status.size}</span>
            {#if status.frame}
              <span class="shrink-0 tabular-nums" data-status-frame="">{status.frame}</span>
            {/if}
          </span>
        {:else}
          <AgentIndicator />
        {/if}
      </div>
      {#if zoom}
        <ZoomControl {...zoom} disabled={!status.ready} closeKey={zoomCloseKey} />
      {/if}
    </footer>
  {/if}
  {#if size === 'narrow' && !chrome.hidden}
    <div class="relative shrink-0" inert={effectOpen || answering} data-dimmed={dim}>
      {@render tools('horizontal')}
    </div>
  {/if}

  {#if effectPanel && size === 'narrow'}
    <!-- Phones: docked to the bottom edge across the window, over the
         sheet, at most half the window tall (AC-13). -->
    <div
      class="fixed inset-x-0 bottom-0 z-[35] flex max-h-[50vh] flex-col"
      bind:offsetHeight={effectDockHeight}
    >
      {@render effectPanel.body()}
    </div>
  {/if}
</div>
