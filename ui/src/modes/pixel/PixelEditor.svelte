<script lang="ts">
  import { onMount } from 'svelte';
  import { loadCore } from '../../lib/core';
  import {
    bindTabHooks,
    detachAllTabs,
    isTabDirty,
    requestCloseTab,
    runningSave,
    saveTab,
  } from './tab-session';
  import TileEditor from './components/TileEditor.svelte';
  import TilesetPanel from './components/TilesetPanel.svelte';
  import SlicesPanel from './components/SlicesPanel.svelte';
  import LayersPanel from './components/LayersPanel.svelte';
  import TimelinePanel from './components/TimelinePanel.svelte';
  import { isShapeTool, toolGroups } from './tools';
  import SpriteSheetDialog from './components/SpriteSheetDialog.svelte';
  import AnimationExportDialog from './components/AnimationExportDialog.svelte';
  import SvgExportDialog from '../../lib/components/SvgExportDialog.svelte';
  import PalettePanel from './components/PalettePanel.svelte';
  import EffectPanel from '../../lib/components/EffectPanel.svelte';
  import { effectLabel } from '../../lib/effects/labels';
  import { activeEffect } from '../../lib/effects/session.svelte';
  import PreviewPanel from './components/PreviewPanel.svelte';
  import ColorPair from '../../lib/components/ColorPair.svelte';
  import EditorFrame from '../../lib/components/layout/EditorFrame.svelte';
  import Panel from '../../lib/components/layout/Panel.svelte';
  import IconButton from '../../lib/components/layout/IconButton.svelte';
  import ToolRail from '../../lib/components/layout/ToolRail.svelte';
  import ToolVariants from '../../lib/components/layout/ToolVariants.svelte';
  import ZoomControl from '../../lib/components/layout/ZoomControl.svelte';
  import { cycleGroup, groupOf } from '../../lib/components/layout/tools';
  import {
    actionFor,
    hint,
    isSpaceActivated,
    isStageTarget,
    isTypingTarget,
    nativeMenuOwns,
    toolGroupForKey,
    type ShortcutAction,
  } from '../../lib/shell/shortcuts';
  import { toggleUi } from '../../lib/components/layout/chrome.svelte';
  import { pixelMenus } from './menus';
  import TextDialog from './components/TextDialog.svelte';
  import BrushOptions from './components/BrushOptions.svelte';
  import {
    axisX2,
    axisY2,
    coversX,
    coversY,
    endAxisDrag,
    moveAxisDrag,
    resetAxis,
    startAxisDrag,
    tileOffsets,
  } from './symmetry';
  import { isBrushTool, loadBrushPref, saveBrushPref, stepBrushSize } from './brush';
  import {
    activeTileSize,
    applyGridSettings,
    gridSettingsInitial,
    loadGridPref,
    openGridSettings,
    toggleGrid,
    togglePixelGrid,
  } from './grid';
  import GridSettingsDialog from './components/GridSettingsDialog.svelte';
  import PngExportDialog from './components/PngExportDialog.svelte';
  import { loadExportScale } from './export-scale';
  import ResizeDialog from './components/ResizeDialog.svelte';
  import type { EditorProps } from '../../lib/shell/types';
  import { getPref, setPref } from '../../lib/idb/prefs';
  import { syncRecentMenu } from '../../lib/menu';
  import { listRecents } from '../../lib/idb/recent-files';
  import { packColor, unpackColor } from '../../lib/color';
  import { t } from '../../lib/i18n/index.svelte';
  import {
    closeShortcuts,
    helpDialogOpen,
    openPalette,
    openShortcuts,
  } from '../../lib/help/state.svelte';
  import RadiusDialog from '../../lib/components/RadiusDialog.svelte';
  import RenameDialog from '../../lib/components/RenameDialog.svelte';
  import PasteLargerDialog from '../../lib/components/PasteLargerDialog.svelte';
  import {
    answerPaste,
    onPaste,
    pasteAsNewImage,
    pasteFile,
    pasteFromClipboard,
    runCopy,
  } from './clipboard-ops';
  import {
    addFrame,
    addGroup,
    addLayer,
    applyOnion,
    deleteSelectionPixels,
    deselect,
    duplicateLayer,
    flattenLayers,
    mergeDownLayer,
    moveLayerTo,
    redo,
    removeFrame,
    removeLayerById,
    resetColors,
    selectAction,
    selectAll,
    setFrame,
    stepActiveLayer,
    stepPaletteColor,
    stopPlayback,
    swapColors,
    targetLayer,
    togglePlay,
    undo,
  } from './doc-ops';
  import {
    bindShell,
    closeSvgDialog,
    createTab,
    describeAnimation,
    exportAnimation,
    exportSvg,
    openSvgDialog,
    traceSvg,
    exportPng,
    exportSheet,
    fsAccessAvailable,
    idbAvailable,
    importPngLayer,
    openDoc,
    openNewDialog,
    openOpened,
    openRecent,
    openRecovered,
    nativeMenuHost,
    openAnimationDialog,
    openSheetDialog,
    openPngDialog,
    recentsAvailable,
    tauriHost,
  } from './file-ops';
  import {
    applyEffectParams,
    cancelEffect,
    imageAction,
    openEffect,
    previewEffectParams,
    submitResize,
  } from './image-ops';
  import {
    BG_TOOLS,
    cancelMarqueeDrag,
    cancelPolygon,
    commitPolygon,
    gestureInProgress,
    onDoubleClick,
    onPointerCancel,
    onPointerDown,
    onPointerLeave,
    onPointerMove,
    onPointerUp,
    onWindowBlur,
  } from './pointer';
  import { createRenderer, syncMeta, syncSelection, tick, toggleProbe } from './render';
  import { applyTextParams, cancelText, previewTextParams } from './text-tool';
  import {
    MAX_ZOOM,
    MIN_ZOOM,
    onWheel,
    resetView,
    zoomActual,
    zoomIn,
    zoomOut,
  } from './view-ops';
  import {
    MAX_DOC_SIZE,
    colors,
    dialogs,
    ed,
    file,
    live,
    play,
    probe,
    resetPixelStore,
    tabs,
    toolState,
    view,
  } from './stores/editor.svelte';
  import { dropWandLink, relinkWand } from './wand-link';

  // Shell contract (src/App.svelte mounts at most one editor per mode):
  // whether the active tab is this editor's, the shared tab bar, and the
  // callbacks the shell serves — New dialog, routing, the unsaved-changes
  // prompt, and the native-menu items this editor handles.
  let {
    active,
    tabBar,
    onStepTab,
    onRequestNew,
    onOpenForeign,
    onOpenFile,
    confirmClose,
    registerDocument,
    activateDocument,
    reportError,
    registerSession,
    onEmpty,
    onNotice,
    registerDropTarget,
    registerMenuHandlers,
  }: EditorProps = $props();

  // Module-level store: start every mount from a fresh state.
  resetPixelStore();
  bindShell({
    requestNew: () => onRequestNew(),
    openForeign: (opened) => onOpenForeign(opened),
    openFile: (opened, mode) => onOpenFile(opened, mode),
    notice: (message) => onNotice(message),
  });
  bindTabHooks({
    registerDocument: (doc) => registerDocument(doc),
    confirmClose: (doc, close) => confirmClose(doc, close),
    activateDocument: (doc) => activateDocument(doc),
    reportError: (doc, error) => reportError(doc, error),
    onEmpty: () => onEmpty(),
  });

  function save(forceAs = false): Promise<boolean> {
    return saveTab(tabs.active, forceAs);
  }

  // Release version stamped in by the release workflow; `dev` locally.
  const appVersion = import.meta.env.VITE_APP_VERSION ?? 'dev';

  // "Modified since last save" — drives the footer dot, the tab title,
  // and the window title.
  const isDirty = $derived(isTabDirty(tabs.active));
  // Last-used foreground color, persisted across sessions (prefs IDB
  // store). Loaded once on mount; written on every change thereafter.
  const COLOR_PREF = 'lastColor';
  const BG_COLOR_PREF = 'lastBgColor';

  const sheetTags = $derived.by(() => {
    void ed.docRev;
    if (!ed.doc) return [];
    const out: { name: string; from: number; to: number }[] = [];
    for (let i = 0; i < ed.doc.tagCount; i++) {
      out.push({ name: ed.doc.tagName(i), from: ed.doc.tagFrom(i), to: ed.doc.tagTo(i) });
    }
    return out;
  });

  // Filter space presses originating in any form input or
  // contenteditable element so the user keeps native space-bar behavior
  // there (typing a literal space in a future search / filename box,
  // toggling a checkbox, etc.). The current toolbar only exposes a
  // color input (no text intake) and a hidden file input, but the
  // guard is forward-looking and conservative — any `<input>`,
  // `<textarea>`, or contenteditable target opts out.
  function isEditableTarget(target: EventTarget | null): boolean {
    return isTypingTarget(target);
  }

  // Merge Down needs an image layer directly below an image layer.
  const canMergeDown = $derived.by(() => {
    void ed.docRev;
    const id = ed.activeLayerId;
    if (!ed.doc || id === null) return false;
    for (let i = 1; i < ed.doc.layerCount; i += 1) {
      if (ed.doc.layerIdAt(i) !== id) continue;
      return (
        ed.doc.layerKind(id) === 'image' && ed.doc.layerKind(ed.doc.layerIdAt(i - 1)) === 'image'
      );
    }
    return false;
  });

  const layerCountNow = $derived.by(() => {
    void ed.docRev;
    return ed.doc?.layerCount ?? 0;
  });

  const railGroups = $derived(toolGroups(toolState.stampTile));

  /** Every shared action `runShortcut` (or, for `paste`, the paste event) serves. */
  const PIXEL_ACTIONS: readonly ShortcutAction[] = [
    'open',
    'save',
    'saveAs',
    'export',
    'exportSheet',
    'undo',
    'redo',
    'cut',
    'copy',
    'paste',
    'pasteNewImage',
    'deleteSelection',
    'selectAll',
    'deselect',
    'invertSelection',
    'zoomIn',
    'zoomOut',
    'zoomFit',
    'zoomActual',
    'toggleGrid',
    'togglePixelGrid',
    'shortcuts',
    'swapColors',
    'resetColors',
    'brushSmaller',
    'brushLarger',
    'prevFrame',
    'nextFrame',
    'togglePlay',
    'newLayer',
    'duplicateLayer',
    'deleteLayer',
    'mergeDown',
    'layerAbove',
    'layerBelow',
    'newFrame',
    'flipLayerH',
    'flipLayerV',
    'rotateCw',
    'rotateCcw',
    'toggleUi',
    'paletteNext',
    'palettePrev',
    'commandPalette',
  ];

  /** Help ▸ Keyboard Shortcuts, listing what this mode binds. */
  function openOverview(): void {
    openShortcuts({ actions: PIXEL_ACTIONS, groups: railGroups });
  }
  /** Ctrl+K: the command palette over these menus and tools (power-user-shortcuts-palette). */
  function openCommandPalette(): void {
    openPalette({
      menus: () => menus,
      groups: () => railGroups,
      selectTool: (id) => {
        if (groupOf(railGroups, id as typeof toolState.tool)) toolState.tool = id as typeof toolState.tool;
      },
      frames: {
        count: () => ed.frameCount,
        goto: (frame) => {
          stopPlayback();
          setFrame(frame - 1);
        },
      },
    });
  }
  const toolLabel = $derived(
    groupOf(railGroups, toolState.tool)?.tools.find((x) => x.id === toolState.tool)?.label ?? '',
  );

  const menus = $derived(
    pixelMenus({
      hasDoc: ed.doc !== null,
      busy: file.busy,
      canSaveInPlace: fsAccessAvailable || tauriHost,
      recents: recentsAvailable ? file.recents : null,
      canUndo: ed.undoDepth > 0,
      canRedo: ed.redoDepth > 0,
      hasSelection: ed.selection !== null,
      canMergeDown,
      layerCount: layerCountNow,
      frameCount: ed.frameCount,
      currentFrame: ed.currentFrame,
      playing: play.playing,
      probeOn: probe.on,
      showPixelGrid: view.showPixelGrid,
      showGrid: view.showGrid,
      tiled: view.tiled,
      setTiled: (mode) => {
        view.tiled = mode;
        live.needsRecompose = true;
      },
      newDoc: openNewDialog,
      open: () => void openDoc(),
      openRecent: (r) => void openRecent(r),
      save: (asNew) => void save(asNew),
      close: () => requestCloseTab(),
      saving: runningSave() !== null,
      importPng: () => void importPngLayer(),
      exportPng: openPngDialog,
      exportSheet: openSheetDialog,
      exportAnimation: openAnimationDialog,
      exportSvg: openSvgDialog,
      undo,
      redo,
      deleteSelection: deleteSelectionPixels,
      copy: runCopy,
      paste: () => void pasteFromClipboard(),
      pasteNewImage: () => void pasteAsNewImage(),
      swapColors,
      resetColors,
      selectAll: () => selectAction('all'),
      deselect: () => selectAction('none'),
      invertSelection: () => selectAction('invert'),
      growSelection: (mode) => (dialogs.selectionRadius = mode),
      transformCanvas: (op) => imageAction(`canvas:${op}`),
      transformLayer: (op) => imageAction(`layer:${op}`),
      resizeCanvas: () => imageAction('canvas:resize'),
      scaleImage: () => imageAction('canvas:scale'),
      crop: () => imageAction('canvas:crop'),
      addLayer,
      addGroup,
      duplicateLayer: () => {
        const id = targetLayer();
        if (id !== null) duplicateLayer(id);
      },
      removeLayer: () => {
        const id = targetLayer();
        if (id !== null) removeLayerById(id);
      },
      mergeDown: () => {
        const id = targetLayer();
        if (id !== null) mergeDownLayer(id);
      },
      flatten: flattenLayers,
      renameLayer: () => {
        const id = targetLayer();
        if (id !== null && ed.doc) dialogs.renaming = { id, name: ed.doc.layerName(id) };
      },
      togglePlay,
      stepFrame: (d) => {
        stopPlayback();
        setFrame(ed.currentFrame + d);
      },
      addFrame,
      removeFrame: () => removeFrame(ed.currentFrame),
      openEffect,
      zoomIn,
      zoomOut,
      zoomFit: resetView,
      zoomActual,
      toggleProbe,
      togglePixelGrid,
      toggleGrid,
      openGridSettings,
      openShortcuts: openOverview,
      toggleUi,
    }),
  );

  function onKeyDown(e: KeyboardEvent) {
    // A hidden editor (another mode's tab is active) takes no keys (AC-21).
    // A key the other editor already handled is not handled again: a tab
    // switch it made shows this editor before the event reaches it.
    if (!active || e.defaultPrevented) return;
    if (e.code === 'Space' && !e.repeat && !isEditableTarget(e.target)) {
      // Prevent the browser from page-scrolling on space; a focused button
      // or checkbox keeps its Space activation (a Space-drag still pans).
      spacePan = { x: view.panX, y: view.panY };
      if (!isSpaceActivated(e.target)) e.preventDefault();
      view.spaceDown = true;
      return;
    }
    // Dialogs own the keyboard while open — the shell's About dialog too,
    // which is mounted by App.svelte rather than by this editor.
    if (
      dialogs.effect ||
      toolState.textAnchor ||
      dialogs.resize ||
      dialogs.sheet ||
      dialogs.png ||
      dialogs.grid ||
      dialogs.animation ||
      dialogs.svg
    )
      return;
    if (dialogs.selectionRadius || dialogs.paste || helpDialogOpen()) return;
    // `ed.selection` otherwise catches up on the next frame; a key pressed
    // right after a marquee release must already see that selection.
    syncSelection();
    if (e.key === 'Escape') {
      if (toolState.polyPoints || live.lassoPoints) {
        cancelPolygon();
        return;
      }
      // A marquee drag drops without touching the selection; Escape during
      // any other drag leaves both alone (pixel-selection-move AC-12, AC-23).
      if (cancelMarqueeDrag() || gestureInProgress()) return;
      if (ed.selection && !isEditableTarget(e.target)) {
        deselect();
        return;
      }
    }
    if (isEditableTarget(e.target)) return;
    if (e.key === 'Enter' && toolState.polyPoints) {
      e.preventDefault();
      commitPolygon();
      return;
    }
    // F2 toggles the frame-time probe (M12.6).
    if (e.key === 'F2') {
      e.preventDefault();
      toggleProbe();
      return;
    }
    // The keyboard map both modes share (lib/shell/shortcuts.ts). Where the
    // OS draws the menu bar (macOS desktop app) that menu owns its
    // accelerators (New / Open / Save / Export / Undo / Redo) — handling
    // them here too would double-fire. Everywhere else they are ours.
    const action = actionFor(e);
    if (action !== null) {
      if (nativeMenuHost && nativeMenuOwns(action)) return;
      // Tab toggles the UI only on the canvas stage; elsewhere it moves focus.
      if (action === 'toggleUi' && !isStageTarget(e.target)) return;
      if (runShortcut(action)) e.preventDefault();
      return;
    }
    // Single-key tool selection; repeated presses cycle a group's variants.
    const group = toolGroupForKey(e);
    const next = group ? cycleGroup(railGroups, group, toolState.tool) : null;
    if (next) {
      e.preventDefault();
      toolState.tool = next;
    }
  }

  /** Runs a shared shortcut; false when it does not apply right now. */
  function runShortcut(action: ShortcutAction): boolean {
    switch (action) {
      case 'open':
        void openDoc();
        return true;
      case 'save':
      case 'saveAs':
        void save(action === 'saveAs');
        return true;
      case 'nextTab':
      case 'prevTab':
        onStepTab(action === 'nextTab' ? 1 : -1);
        return true;
      case 'closeTab':
        requestCloseTab();
        return true;
      case 'export':
        openPngDialog();
        return true;
      case 'exportSheet':
        openSheetDialog();
        return true;
      case 'undo':
        undo();
        return true;
      case 'redo':
        redo();
        return true;
      case 'swapColors':
        swapColors();
        return true;
      case 'resetColors':
        resetColors();
        return true;
      case 'pasteNewImage':
        void pasteAsNewImage();
        return true;
      case 'shortcuts':
        openOverview();
        return true;
      case 'toggleUi':
        toggleUi();
        return true;
      case 'commandPalette':
        openCommandPalette();
        return true;
    }
    if (!ed.doc) return false;
    switch (action) {
      case 'copy':
      case 'cut':
        runCopy(action);
        return true;
      case 'selectAll':
        selectAll();
        return true;
      case 'deselect':
        deselect();
        return true;
      case 'invertSelection':
        selectAction('invert');
        return true;
      case 'deleteSelection':
        if (!ed.selection) return false;
        deleteSelectionPixels();
        return true;
      case 'zoomIn':
        zoomIn();
        return true;
      case 'zoomOut':
        zoomOut();
        return true;
      case 'zoomFit':
        resetView();
        return true;
      case 'zoomActual':
        zoomActual();
        return true;
      case 'toggleGrid':
        toggleGrid();
        return true;
      case 'togglePixelGrid':
        togglePixelGrid();
        return true;
      case 'prevFrame':
      case 'nextFrame':
        stopPlayback();
        setFrame(ed.currentFrame + (action === 'nextFrame' ? 1 : -1));
        return true;
      case 'togglePlay':
        togglePlay();
        return true;
      case 'brushSmaller':
      case 'brushLarger':
        if (!isBrushTool(toolState.tool)) return false;
        stepBrushSize(action === 'brushLarger' ? 1 : -1);
        return true;
      // Layer, frame and transform keys run what their menu items run;
      // where the item is disabled the key does nothing (AC-16).
      case 'newLayer':
        addLayer();
        return true;
      case 'duplicateLayer': {
        const id = targetLayer();
        if (id !== null) duplicateLayer(id);
        return true;
      }
      case 'deleteLayer': {
        const id = targetLayer();
        if (id !== null && layerCountNow > 1) removeLayerById(id);
        return true;
      }
      case 'mergeDown': {
        const id = targetLayer();
        if (id !== null && canMergeDown) mergeDownLayer(id);
        return true;
      }
      case 'layerAbove':
      case 'layerBelow':
        stepActiveLayer(action === 'layerAbove' ? -1 : 1);
        return true;
      case 'newFrame':
        addFrame();
        return true;
      case 'flipLayerH':
      case 'flipLayerV':
        imageAction(action === 'flipLayerH' ? 'layer:flip_horizontal' : 'layer:flip_vertical');
        return true;
      case 'rotateCw':
      case 'rotateCcw':
        imageAction(action === 'rotateCw' ? 'canvas:rotate_90_cw' : 'canvas:rotate_90_ccw');
        return true;
      case 'paletteNext':
      case 'palettePrev':
        stepPaletteColor(action === 'paletteNext' ? 1 : -1);
        return true;
      default:
        return false;
    }
  }

  // True while any editor dialog (or the tile editor) owns the input.
  function dialogOpen(): boolean {
    return Boolean(
      dialogs.effect ||
      dialogs.resize ||
      dialogs.sheet ||
      dialogs.png ||
      dialogs.grid ||
      dialogs.animation ||
      dialogs.svg ||
      dialogs.renaming ||
      dialogs.selectionRadius ||
      dialogs.editingTile ||
      dialogs.paste ||
      toolState.textAnchor,
    );
  }

  // Ctrl+V / ⌘V arrives as the browser's `paste` event (spec §5.4); not
  // while an editor dialog or the About dialog is open.
  function onWindowPaste(e: ClipboardEvent) {
    if (!active || e.defaultPrevented) return;
    const blocked = Boolean(
      dialogs.effect ||
      dialogs.resize ||
      dialogs.sheet ||
      dialogs.png ||
      dialogs.grid ||
      dialogs.svg ||
      dialogs.selectionRadius ||
      dialogs.paste,
    );
    onPaste(e, blocked || helpDialogOpen() || toolState.textAnchor !== null);
  }

  // The view offset when Space went down: a hold that panned must not
  // also activate the focused button on release.
  let spacePan = { x: 0, y: 0 };

  function onKeyUp(e: KeyboardEvent) {
    if (active && e.code === 'Space') {
      if (view.panX !== spacePan.x || view.panY !== spacePan.y) e.preventDefault();
      view.spaceDown = false;
    }
  }

  function onVisibilityChange() {
    if (document.hidden) onWindowBlur();
  }

  onMount(() => {
    let cancelled = false;
    // The core and the base-layer renderer, before the first document.
    // The base canvas is bound by the time onMount runs (spec §4.4, M12.5).
    const ready = loadCore().then(async () => {
      if (cancelled) return;
      if (view.canvas && !live.renderer) {
        const r = await createRenderer(view.canvas);
        if (cancelled) {
          r.destroy();
          return;
        }
        live.renderer = r;
        ed.backend = r.backend;
      }
      live.rafHandle = requestAnimationFrame(tick);
    });
    ready.catch((err: unknown) => {
      if (cancelled) return;
      const msg = err instanceof Error ? err.message : String(err);
      console.error('loadCore failed', err);
      ed.status = t('status.failed', { action: t('act.wasmInit'), error: msg });
    });
    // The canvas reports ready once the first document has composed: the
    // frame is queued behind the `tick` that draws it.
    function markReady(): void {
      if (ed.ready) return;
      requestAnimationFrame(() => {
        if (!cancelled) ed.ready = true;
      });
    }
    /** Runs `open` once the core is loaded; resolves `null` once the tab
     *  is open, else the error (the core failing to load, AC-23). */
    async function whenReady(open: () => Promise<string | null>): Promise<string | null> {
      try {
        await ready;
      } catch (err) {
        return err instanceof Error ? err.message : String(err);
      }
      if (cancelled) return t('px.noDocument');
      const error = await open();
      if (error === null) markReady();
      return error;
    }
    // The documents the shell routes here open as tabs (document-tabs
    // AC-02, AC-15).
    const unregisterSession = registerSession({
      open: (opened) =>
        whenReady(async () => {
          file.busy = true;
          try {
            return await openOpened(opened);
          } finally {
            file.busy = false;
          }
        }),
      create: (params) =>
        whenReady(async () => {
          createTab(params);
          return null;
        }),
      recover: async (bytes, meta) => {
        const error = await whenReady(async () => {
          openRecovered(bytes, meta.name, meta.docId);
          return null;
        });
        if (error !== null) throw new Error(error);
      },
    });
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('paste', onWindowPaste);
    window.addEventListener('blur', onWindowBlur);
    document.addEventListener('visibilitychange', onVisibilityChange);
    // Wheel zoom is registered imperatively (not via `onwheel`) so the
    // listener is non-passive and `preventDefault()` actually suppresses
    // page scroll. `canvas` is bound by the time onMount runs.
    view.canvas?.addEventListener('wheel', onWheel, { passive: false });
    // Restore the last-used foreground color (best-effort; the default
    // stands when the pref is missing or IDB is unavailable). The
    // persist effect below only starts writing once this load settled,
    // so a slow IDB can't overwrite the stored value with the default.
    if (idbAvailable) {
      getPref(COLOR_PREF)
        .then((v) => {
          if (cancelled) return;
          if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) colors.color = v;
          return getPref(BG_COLOR_PREF);
        })
        .then((v) => {
          if (cancelled) return;
          if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) colors.bgColor = v;
          live.colorPrefLoaded = true;
        })
        .catch(() => {
          live.colorPrefLoaded = true;
        });
    }
    // Restore the shared Pencil / Eraser brush (defaults stand when the
    // pref is missing or IDB is unavailable).
    if (idbAvailable) void loadBrushPref().catch(() => undefined);
    // Restore the grid view settings (pixel-grid AC-11, AC-15).
    void loadGridPref();
    // The last used export scale (scaled-export AC-10).
    void loadExportScale();
    // Best-effort recents load — failures are silent (the dropdown
    // just stays empty / hidden).
    if (recentsAvailable) {
      listRecents()
        .then((rows) => {
          if (!cancelled) file.recents = rows;
        })
        .catch((err: unknown) => {
          console.error('listRecents failed', err);
        });
    }
    // Files dropped onto the editor (docs/specs/file-access.md AC-20 –
    // AC-28): the shell asks and opens documents; Pixel mode adds PNGs.
    registerDropTarget({
      accepts: () => ed.ready && ed.doc !== null && !file.busy && !dialogOpen(),
      layerFormats: ['png'],
      addLayer: (opened) => pasteFile(opened.bytes),
    });
    // Native-menu items this editor serves (Tauri only; the shell owns
    // New / Open / Open Recent and the open-file events).
    registerMenuHandlers({
      'menu:close': () => requestCloseTab(),
      'menu:save': () => void save(),
      'menu:saveAs': () => void save(true),
      'menu:exportPng': openPngDialog,
      'menu:exportSheet': openSheetDialog,
      'menu:exportAnimation': openAnimationDialog,
      'menu:exportSvg': openSvgDialog,
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoomIn': zoomIn,
      'menu:zoomOut': zoomOut,
      'menu:resetZoom': resetView,
      'menu:shortcuts': openOverview,
    });
    return () => {
      cancelled = true;
      if (live.rafHandle !== null) cancelAnimationFrame(live.rafHandle);
      stopPlayback();
      unregisterSession();
      // A drop waiting on the larger-image dialog ends with the editor.
      answerPaste(null);
      registerDropTarget(null);
      registerMenuHandlers(null);
      closeShortcuts();
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('paste', onWindowPaste);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onWindowBlur);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      view.canvas?.removeEventListener('wheel', onWheel);
      live.renderer?.destroy();
      live.renderer = null;
      detachAllTabs();
    };
  });

  // Choosing another tool ends the Magic Wand link (pixel-selection-move
  // AC-26), even if the wand is chosen again later.
  $effect(() => {
    void toolState.tool;
    dropWandLink();
  });

  // The shell cancels the open effect before its unsaved-changes prompt
  // or a document switch (effect-panel AC-17).
  $effect(() => {
    if (!dialogs.effect) return;
    activeEffect.cancel = cancelEffect;
    return () => {
      if (activeEffect.cancel === cancelEffect) activeEffect.cancel = null;
    };
  });

  // Persist the foreground / background colors once the initial load
  // settled.
  $effect(() => {
    const fg = colors.color;
    const bg = colors.bgColor;
    if (!live.colorPrefLoaded || !idbAvailable) return;
    Promise.all([setPref(COLOR_PREF, fg), setPref(BG_COLOR_PREF, bg)]).catch((err: unknown) => {
      console.error('setPref colors failed', err);
    });
  });

  // Persist the brush once its initial load settled.
  $effect(() => {
    const pref = {
      size: toolState.brushSize,
      shape: toolState.brushShape,
      pixelPerfect: toolState.pixelPerfect,
    };
    if (idbAvailable) saveBrushPref(pref);
  });

  // Reflect the document identity + dirty state in the window title while
  // this editor's tab is active.
  $effect(() => {
    if (active) document.title = `${isDirty ? '● ' : ''}${file.saveTarget.name} – Pincel`;
  });

  // Leaving for another mode's tab (or a shell dialog) drops the gestures,
  // playback and sub-modes in flight, as switching tabs does (AC-21).
  $effect(() => {
    if (active) return;
    stopPlayback();
    onWindowBlur();
    cancelPolygon();
    view.spaceDown = false;
  });


  // Push the current recents list to the Rust-side `Open Recent`
  // submenu so the native menu stays in sync with the in-memory list.
  // Only entries with a path can be re-opened by the menu (FSA handles
  // need an in-page permission gesture). Failures are logged but don't
  // surface in the UI — the toolbar dropdown remains the authoritative
  // recents UI on the web.
  $effect(() => {
    if (!tauriHost || !active) return;
    const items = file.recents
      .filter((r) => r.path !== null)
      .map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });
</script>

{#snippet colorsBody()}
  <Panel>
    <ColorPair
      fg={colors.color}
      bg={colors.bgColor}
      onFg={(hex) => (colors.color = hex)}
      onBg={(hex) => (colors.bgColor = hex)}
      onSwap={swapColors}
      onReset={resetColors}
      alpha={colors.alpha}
      onAlpha={(a) => (colors.alpha = a)}
    />
  </Panel>
{/snippet}
{#snippet paletteBody()}
  <PalettePanel
    doc={ed.doc}
    rev={ed.docRev}
    activeColor={colors.color}
    addColor={packColor(colors.color, colors.alpha)}
    onChange={() => {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    }}
    onError={(msg) => (ed.status = msg)}
    onStatus={(msg) => (ed.status = msg)}
    fileStem={file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '')}
    onPick={(packed) => {
      // Set both the RGB (via the picker) and the alpha slider from
      // the palette entry so a swatch's transparency carries over.
      colors.color = unpackColor(packed);
      colors.alpha = packed & 0xff;
    }}
    onPickBg={(packed) => {
      colors.bgColor = unpackColor(packed);
      colors.bgAlpha = packed & 0xff;
    }}
  />
{/snippet}
{#snippet previewBody()}
  <PreviewPanel />
{/snippet}

{#snippet layersBody()}
  <LayersPanel
    doc={ed.doc}
    rev={ed.docRev}
    frame={ed.currentFrame}
    activeLayerId={ed.activeLayerId}
    onActivate={(layerId) => {
      ed.activeLayerId = layerId;
      // Route pixel tools to the selected layer (M13.3b). A fresh
      // document resets the wasm-side target to None on its own, so
      // the fallback stays correct after New / Open.
      ed.doc?.setActiveLayer(layerId);
    }}
    onToggleVisible={(layerId, visible) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerVisible(layerId, visible);
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerVisibility'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onToggleLocked={(layerId, editable) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerEditable(layerId, editable);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerLock'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onReorder={(layerId, targetId) => {
      if (!ed.doc) return;
      try {
        ed.doc.reorderLayer(layerId, targetId);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerReorder'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onRename={(layerId, name) => {
      if (!ed.doc) return;
      try {
        ed.doc.renameLayer(layerId, name);
        // The rename is an undo step: the Undo button and the unsaved
        // marker read the mirrored depths.
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerRename'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onAddLayer={addLayer}
    onAddGroup={addGroup}
    onMove={moveLayerTo}
    onSetOpacity={(layerId, opacity, commit) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerOpacity(layerId, opacity);
        if (commit) ed.doc.endStroke();
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerOpacity'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onSetBlendMode={(layerId, mode) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerBlendMode(layerId, mode);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerBlendMode'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
    onDuplicate={duplicateLayer}
    onMergeDown={mergeDownLayer}
    onFlatten={flattenLayers}
    onRemoveLayer={removeLayerById}
  />
{/snippet}
{#snippet tilesetsBody()}
  <TilesetPanel
    doc={ed.doc}
    rev={ed.docRev}
    selectedTile={toolState.stampTile}
    onChange={() => {
      // Tileset edits join the undo bus: refresh its depth so Undo / Redo
      // and the status bar follow.
      syncMeta();
      ed.docRev += 1;
    }}
    onSelectStampTile={(tilesetId, tileId) => {
      toolState.stampTile = { tilesetId, tileId };
      // Auto-switch to the Stamp tool so a single click on a tile
      // is enough to start placing. The user can switch back via
      // the toolbar if they wanted to keep editing pixels.
      toolState.tool = 'tilemap-stamp';
    }}
    onEditTile={(tilesetId, tileId) => {
      dialogs.editingTile = { tilesetId, tileId };
    }}
  />
{/snippet}
{#snippet slicesBody()}
  <SlicesPanel
    doc={ed.doc}
    rev={ed.docRev}
    activeSliceId={ed.activeSliceId}
    onChange={() => (ed.docRev += 1)}
    onActivate={(sliceId) => {
      ed.activeSliceId = sliceId;
      if (sliceId !== null) toolState.tool = 'slice';
      live.needsRecompose = true;
    }}
  />
{/snippet}
{#snippet timelineBody()}
  <TimelinePanel
    doc={ed.doc}
    rev={ed.docRev}
    currentFrame={ed.currentFrame}
    playing={play.playing}
    loopTag={play.loopTag}
    onion={play.onion}
    onSelectFrame={setFrame}
    onAddFrame={addFrame}
    onRemoveFrame={removeFrame}
    onTogglePlay={togglePlay}
    onLoopTagChange={(next) => (play.loopTag = next)}
    onOnionChange={applyOnion}
    onChange={() => {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    }}
    onError={(msg) => (ed.status = msg)}
  />
{/snippet}

{#snippet effectBody()}
  {#if dialogs.effect}
    {#key dialogs.effect.token}
      <EffectPanel
        session={dialogs.effect}
        hasSelection={ed.selection !== null}
        onPreview={previewEffectParams}
        onApply={applyEffectParams}
        onCancel={cancelEffect}
      />
    {/key}
  {/if}
{/snippet}

<EditorFrame
  mode="pixel"
  {menus}
  title={ed.doc ? file.saveTarget.name : t('px.noDocument')}
  dirty={isDirty}
  effectPanel={dialogs.effect ? { title: effectLabel(dialogs.effect.def), body: effectBody } : null}
  tabs={tabBar}
  panels={[
    { id: 'colors', title: t('panel.colors'), icon: 'colors', body: colorsBody },
    { id: 'palette', title: t('panel.palette'), icon: 'palette', body: paletteBody },
    {
      id: 'preview',
      title: t('panel.preview'),
      icon: 'play',
      body: previewBody,
      defaultCollapsed: true,
    },
    { id: 'layers', title: t('panel.layers'), icon: 'layers', body: layersBody, grow: true },
    {
      id: 'tilesets',
      title: t('panel.tilesets'),
      icon: 'grid',
      body: tilesetsBody,
      defaultCollapsed: true,
    },
    {
      id: 'slices',
      title: t('panel.slices'),
      icon: 'slice',
      body: slicesBody,
      defaultCollapsed: true,
    },
    {
      id: 'timeline',
      title: t('panel.timeline'),
      icon: 'film',
      body: timelineBody,
      placement: 'bottom',
    },
  ]}
>
  {#snippet quick()}
    <IconButton
      icon="undo"
      label={t('menu.undo')}
      hint={hint('undo')}
      disabled={ed.undoDepth === 0}
      onclick={undo}
    />
    <IconButton
      icon="redo"
      label={t('menu.redo')}
      hint={hint('redo')}
      disabled={ed.redoDepth === 0}
      onclick={redo}
    />
  {/snippet}
  {#snippet tools(orientation)}
    <ToolRail
      groups={railGroups}
      active={toolState.tool}
      onSelect={(next) => (toolState.tool = next)}
      {orientation}
      resetKey={active ? ed.doc : null}
    />
  {/snippet}
  {#snippet options()}
    <ToolVariants
      groups={railGroups}
      active={toolState.tool}
      onSelect={(next) => (toolState.tool = next)}
    />
    {#if toolState.tool === 'selection-wand'}
      <label class="opt">
        {t('opt.tolerance')}
        <input
          type="number"
          min="0"
          max="255"
          bind:value={toolState.wandTolerance}
          onchange={relinkWand}
          class="opt-field w-16 text-right tabular-nums"
          aria-label={t('px.wandTolerance')}
        />
      </label>
      <label class="opt">
        <input
          type="checkbox"
          bind:checked={toolState.wandContiguous}
          onchange={relinkWand}
          aria-label={t('opt.contiguous')}
        />
        {t('opt.contiguous')}
      </label>
      <label class="opt">
        {t('opt.sample')}
        <select
          bind:value={toolState.wandSample}
          onchange={relinkWand}
          class="opt-field"
          aria-label={t('px.wandSample')}
        >
          <option value="layer">{t('px.layer')}</option>
          <option value="composite">{t('opt.composite')}</option>
        </select>
      </label>
    {:else if toolState.tool === 'gradient'}
      <label class="opt">
        {t('opt.kind')}
        <select bind:value={toolState.gradientKind} class="opt-field" aria-label={t('px.gradientKind')}>
          <option value="linear">{t('opt.gradientLinear')}</option>
          <option value="radial">{t('opt.gradientRadial')}</option>
        </select>
      </label>
      <label class="opt">
        {t('opt.dither')}
        <select bind:value={toolState.gradientDither} class="opt-field" aria-label={t('px.dither')}>
          <option value={0}>{t('opt.ditherNone')}</option>
          <option value={2}>{t('opt.bayer2')}</option>
          <option value={4}>{t('opt.bayer4')}</option>
          <option value={8}>{t('opt.bayer8')}</option>
        </select>
      </label>
      <span class="opt opt-hint">{t('px.gradientHint')}</span>
    {:else if isShapeTool(toolState.tool)}
      <label class="opt">
        {t('opt.stroke')}
        <input
          type="number"
          min="1"
          max="64"
          bind:value={toolState.strokeWidth}
          class="opt-field w-14 text-right tabular-nums"
          aria-label={t('px.strokeWidth')}
        />
      </label>
      <label class="opt">
        {t('opt.mode')}
        <select bind:value={toolState.shapeMode} class="opt-field" aria-label={t('px.shapeMode')}>
          <option value="outline">{t('opt.outline')}</option>
          <option value="fill">{t('opt.fillShape')}</option>
          <option value="fill_outline">{t('opt.fillOutline')}</option>
        </select>
      </label>
      {#if toolState.tool === 'rounded-rect'}
        <label class="opt">
          {t('opt.radius')}
          <input
            type="number"
            min="0"
            max="256"
            bind:value={toolState.cornerRadius}
            class="opt-field w-14 text-right tabular-nums"
            aria-label={t('px.cornerRadius')}
          />
        </label>
      {/if}
      {#if toolState.tool === 'polygon-shape'}
        <label class="opt">
          {t('opt.sides')}
          <input
            type="number"
            min="3"
            max="64"
            bind:value={toolState.polygonSides}
            class="opt-field w-14 text-right tabular-nums"
            aria-label={t('px.polygonSides')}
          />
        </label>
      {/if}
      {#if toolState.shapeMode !== 'outline'}
        <span class="opt opt-hint">{t('px.fillsBg')}</span>
      {/if}
    {:else if isBrushTool(toolState.tool)}
      <BrushOptions />
      <label class="opt opt-divided">
        {t('opt.symmetry')}
        <select bind:value={toolState.symmetry} class="opt-field" aria-label={t('opt.symmetry')}>
          <option value="none">{t('opt.symmetryNone')}</option>
          <option value="x">{t('opt.symmetryX')}</option>
          <option value="y">{t('opt.symmetryY')}</option>
          <option value="xy">{t('opt.symmetryXY')}</option>
        </select>
      </label>
      {#if toolState.tool === 'pencil'}
        <label class="opt opt-divided">
          {t('opt.ink')}
          <select bind:value={toolState.pencilInk} class="opt-field" aria-label={t('px.pencilInk')}>
            <option value="simple">{t('opt.inkSimple')}</option>
            <option value="shading">{t('opt.inkShading')}</option>
          </select>
        </label>
        {#if toolState.pencilInk === 'shading'}
          <span class="opt opt-hint" data-testid="shading-ramp">
            {colors.rampRange
              ? t('px.shadingRange', { from: colors.rampRange.from, to: colors.rampRange.to })
              : t('px.shadingWholePalette')}
          </span>
        {/if}
      {/if}
      {#if BG_TOOLS.has(toolState.tool)}
        <span class="opt opt-hint">{t('px.rightBg')}</span>
      {/if}
    {:else}
      <span class="opt opt-hint">{toolLabel}</span>
      {#if BG_TOOLS.has(toolState.tool)}
        <span class="opt opt-hint">{t('px.rightBg')}</span>
      {/if}
    {/if}
  {/snippet}
  {#snippet viewport()}
    <!-- The stage takes the pointer, not just the canvas, so a selection
             gesture can start beside the canvas (spec §9.4); `onPointerDown`
             lets only selection and view gestures start there. -->
    <div
      class="absolute inset-0 flex items-center justify-center overflow-hidden select-none"
      bind:this={view.stage}
      bind:clientWidth={view.stageW}
      bind:clientHeight={view.stageH}
      role="presentation"
      oncontextmenu={(e) => e.preventDefault()}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerCancel}
      onpointerleave={onPointerLeave}
      ondblclick={onDoubleClick}
    >
      <!--
            Stacked render surfaces (spec §4.4, M12.5): the base canvas
            shows the composed sprite (driven by a `CanvasRenderer` — WebGPU
            or Canvas2D) and the overlay canvas, sitting exactly on top with
            `pointer-events: none`, carries the transient Canvas2D furniture
            (selection marquee, drag previews, tile grid, slice accents).
            The wrapper owns the display size / pan transform / frame chrome
            so both layers scale and translate as one; the canvases fill it
            via `h-full w-full` and CSS upscales their sprite-sized backing
            stores (`image-rendering: pixelated`).
          -->
      <div
        class="relative shrink-0 border border-pc-border bg-pc-panel shadow-lg"
        style:width="{ed.canvasW * view.zoom}px"
        style:height="{ed.canvasH * view.zoom}px"
        style:transform="translate({view.panX}px, {view.panY}px)"
      >
        <canvas
          bind:this={view.canvas}
          class="canvas-pixelated absolute inset-0 h-full w-full touch-none"
          style:cursor={view.panning
            ? 'grabbing'
            : toolState.moveSelStart
              ? 'move'
              : toolState.tool === 'move' && ed.selection && !view.spaceDown
                ? 'move'
                : toolState.tool === 'move' || view.spaceDown
                  ? 'grab'
                  : 'crosshair'}
          aria-label="Pincel canvas"
          data-ready={ed.ready}
        ></canvas>
        <canvas
          bind:this={view.overlay}
          class="canvas-pixelated pointer-events-none absolute inset-0 h-full w-full"
          width={ed.canvasW}
          height={ed.canvasH}
          aria-hidden="true"
        ></canvas>
        <!-- Tiled mode (symmetry-tiled-mode AC-13, AC-14): the composed
             frame repeated around the canvas, dimmed so the editable
             canvas stands out. Presses fall through to the stage. -->
        {#each tileOffsets(view.tiled) as o (`${o.dx},${o.dy}`)}
          <canvas
            class="canvas-pixelated pointer-events-none absolute h-full w-full opacity-50"
            style:left="{o.dx * 100}%"
            style:top="{o.dy * 100}%"
            width={ed.canvasW}
            height={ed.canvasH}
            data-tile-copy
            data-testid="tile-copy"
            data-offset="{o.dx},{o.dy}"
            aria-hidden="true"
          ></canvas>
        {/each}
        <!-- Symmetry (AC-08, AC-09): axis lines while the Pencil or
             Eraser is active, handles in the margin with any tool. -->
        {#if coversX(toolState.symmetry)}
          {#if isBrushTool(toolState.tool)}
            <div
              class="symmetry-axis pointer-events-none absolute inset-y-0 w-px"
              style:left="{(axisX2() / 2 / ed.canvasW) * 100}%"
              data-testid="symmetry-axis-x"
            ></div>
          {/if}
          <button
            type="button"
            class="symmetry-handle absolute -top-3.5 h-3 w-3 -translate-x-1/2 touch-none"
            style:left="{(axisX2() / 2 / ed.canvasW) * 100}%"
            aria-label={t('px.symmetryAxisX')}
            title={t('px.symmetryHandleHint')}
            data-axis={axisX2()}
            onpointerdown={(e) => startAxisDrag('x', e)}
            onpointermove={(e) => moveAxisDrag('x', e)}
            onpointerup={endAxisDrag}
            onpointercancel={endAxisDrag}
            ondblclick={() => resetAxis('x')}
          >
            <svg viewBox="0 0 12 12" class="h-full w-full" aria-hidden="true">
              <polygon points="0,0 12,0 6,11" />
            </svg>
          </button>
        {/if}
        {#if coversY(toolState.symmetry)}
          {#if isBrushTool(toolState.tool)}
            <div
              class="symmetry-axis pointer-events-none absolute inset-x-0 h-px"
              style:top="{(axisY2() / 2 / ed.canvasH) * 100}%"
              data-testid="symmetry-axis-y"
            ></div>
          {/if}
          <button
            type="button"
            class="symmetry-handle absolute -left-3.5 h-3 w-3 -translate-y-1/2 touch-none"
            style:top="{(axisY2() / 2 / ed.canvasH) * 100}%"
            aria-label={t('px.symmetryAxisY')}
            title={t('px.symmetryHandleHint')}
            data-axis={axisY2()}
            onpointerdown={(e) => startAxisDrag('y', e)}
            onpointermove={(e) => moveAxisDrag('y', e)}
            onpointerup={endAxisDrag}
            onpointercancel={endAxisDrag}
            ondblclick={() => resetAxis('y')}
          >
            <svg viewBox="0 0 12 12" class="h-full w-full" aria-hidden="true">
              <polygon points="0,0 0,12 11,6" />
            </svg>
          </button>
        {/if}
      </div>
      <!-- Pixel grid and custom grid (docs/specs/pixel-grid.md): a
               stage-sized canvas over the sprite, so lines stay one device
               pixel at every zoom (grid.ts). -->
      <canvas
        bind:this={view.gridCanvas}
        class="pointer-events-none absolute inset-0 h-full w-full"
        data-testid="pixel-grid"
        aria-hidden="true"
      ></canvas>
      {#if ed.doc && dialogs.editingTile}
        <TileEditor
          doc={ed.doc}
          tilesetId={dialogs.editingTile.tilesetId}
          tileId={dialogs.editingTile.tileId}
          color={colors.color}
          rev={ed.docRev}
          onClose={() => (dialogs.editingTile = null)}
          onChange={() => {
            ed.docRev += 1;
            live.needsRecompose = true;
            syncMeta();
          }}
        />
      {/if}
      {#if ed.doc}
        <ZoomControl
          label={`${view.zoom}×`}
          onZoomIn={zoomIn}
          onZoomOut={zoomOut}
          onFit={resetView}
          onActual={zoomActual}
          canZoomIn={view.zoom < MAX_ZOOM}
          canZoomOut={view.zoom > MIN_ZOOM}
        />
      {/if}
    </div>
  {/snippet}
  {#snippet statusbar()}
    {#if ed.doc}
      <span>{ed.canvasW}×{ed.canvasH}</span>
      <span>·</span>
      <span>{t('px.frameOf', { n: ed.currentFrame + 1, total: ed.frameCount })}</span>
      <span class="hidden md:inline">·</span>
      <span class="hidden md:inline">{t('px.undoRedo', { u: ed.undoDepth, r: ed.redoDepth })}</span>
    {/if}
    <span class="min-w-0 flex-1 truncate text-center text-pc-muted">{ed.status}</span>
    {#if probe.on}
      <span class="text-emerald-400">
        {probe.fpsEma.toFixed(0)} fps · compose {probe.composeMs.toFixed(2)}ms (max
        {probe.composeMaxMs.toFixed(2)}ms)
      </span>
    {/if}
    {#if ed.backend !== 'none'}
      <span>{ed.backend}</span>
      <span>·</span>
    {/if}
    <span title={t('px.version')}>{appVersion}</span>
  {/snippet}
</EditorFrame>

{#if dialogs.selectionRadius}
  {@const op = dialogs.selectionRadius}
  <RadiusDialog
    title={t(op === 'expand' ? 'menu.expand' : 'menu.contract')}
    value={1}
    onCancel={() => (dialogs.selectionRadius = null)}
    onApply={(radius) => {
      // Read the operation before closing: `op` follows `selectionRadius`.
      const action = op;
      dialogs.selectionRadius = null;
      selectAction(action, radius);
    }}
  />
{/if}

{#if dialogs.renaming && ed.doc}
  {@const target = dialogs.renaming}
  <RenameDialog
    name={target.name}
    onCancel={() => (dialogs.renaming = null)}
    onRename={(name) => {
      // Read the target before closing: `target` follows `renaming`.
      const id = target.id;
      dialogs.renaming = null;
      try {
        ed.doc?.renameLayer(id, name);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', {
          action: t('act.layerRename'),
          error: err instanceof Error ? err.message : String(err),
        });
      }
    }}
  />
{/if}

{#if dialogs.paste}
  {@const paste = dialogs.paste}
  <PasteLargerDialog
    width={paste.width}
    height={paste.height}
    canExpand={paste.canExpand}
    onExpand={() => answerPaste('expand')}
    onKeep={() => answerPaste('keep')}
    onCancel={() => answerPaste(null)}
  />
{/if}

{#if dialogs.sheet && ed.doc}
  <SpriteSheetDialog
    frameCount={ed.frameCount}
    tags={sheetTags}
    frameWidth={ed.canvasW}
    frameHeight={ed.canvasH}
    scale={file.exportScale}
    onSubmit={(o) => void exportSheet(o)}
    onCancel={() => (dialogs.sheet = false)}
  />
{/if}

{#if dialogs.png && ed.doc}
  <PngExportDialog
    frameWidth={ed.canvasW}
    frameHeight={ed.canvasH}
    scale={file.exportScale}
    onSubmit={(scale) => void exportPng(scale)}
    onCancel={() => (dialogs.png = false)}
  />
{/if}

{#if dialogs.grid && ed.doc}
  <GridSettingsDialog
    initial={gridSettingsInitial()}
    tileSize={activeTileSize()}
    onSubmit={applyGridSettings}
    onCancel={() => (dialogs.grid = false)}
  />
{/if}

{#if dialogs.svg && ed.doc}
  <SvgExportDialog
    defaultMode="exact"
    trace={traceSvg}
    onExport={(svg) => void exportSvg(svg)}
    onCancel={closeSvgDialog}
  />
{/if}

{#if dialogs.animation && ed.doc}
  <AnimationExportDialog
    frameCount={ed.frameCount}
    tags={sheetTags}
    frameWidth={ed.canvasW}
    frameHeight={ed.canvasH}
    describe={describeAnimation}
    onSubmit={(o) => void exportAnimation(o)}
    onCancel={() => (dialogs.animation = false)}
  />
{/if}

{#if dialogs.resize}
  {#key dialogs.resize}
    <ResizeDialog
      mode={dialogs.resize}
      width={ed.canvasW}
      height={ed.canvasH}
      maxSize={MAX_DOC_SIZE}
      onSubmit={submitResize}
      onCancel={() => (dialogs.resize = null)}
    />
  {/key}
{/if}

{#if toolState.textAnchor}
  <TextDialog
    x={toolState.textAnchor.x}
    y={toolState.textAnchor.y}
    onPreview={previewTextParams}
    onApply={applyTextParams}
    onCancel={cancelText}
  />
{/if}



<style>
  .canvas-pixelated {
    image-rendering: pixelated;
  }
  .symmetry-axis {
    background: var(--color-pc-accent);
  }
  .symmetry-handle {
    color: var(--color-pc-accent);
    fill: currentColor;
  }
</style>
