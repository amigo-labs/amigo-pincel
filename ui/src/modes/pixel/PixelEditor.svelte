<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from '../../lib/core';
  import RecoveryDialog from '../../lib/components/RecoveryDialog.svelte';
  import TileEditor from './components/TileEditor.svelte';
  import TilesetPanel from './components/TilesetPanel.svelte';
  import SlicesPanel from './components/SlicesPanel.svelte';
  import LayersPanel from './components/LayersPanel.svelte';
  import TimelinePanel from './components/TimelinePanel.svelte';
  import { isShapeTool, toolGroups } from './tools';
  import SpriteSheetDialog from './components/SpriteSheetDialog.svelte';
  import AnimationExportDialog from './components/AnimationExportDialog.svelte';
  import PalettePanel from './components/PalettePanel.svelte';
  import EffectDialog from '../../lib/components/EffectDialog.svelte';
  import ColorPair from '../../lib/components/ColorPair.svelte';
  import EditorFrame from '../../lib/components/layout/EditorFrame.svelte';
  import Panel from '../../lib/components/layout/Panel.svelte';
  import IconButton from '../../lib/components/layout/IconButton.svelte';
  import ToolRail from '../../lib/components/layout/ToolRail.svelte';
  import ToolVariants from '../../lib/components/layout/ToolVariants.svelte';
  import ZoomControl from '../../lib/components/layout/ZoomControl.svelte';
  import { cycleGroup, groupOf } from '../../lib/components/layout/tools';
  import {
    NATIVE_MENU_ACTIONS,
    actionFor,
    hint,
    isTypingTarget,
    toolGroupForKey,
    type ShortcutAction,
  } from '../../lib/shell/shortcuts';
  import { pixelMenus } from './menus';
  import TextDialog from './components/TextDialog.svelte';
  import BrushOptions from './components/BrushOptions.svelte';
  import { isBrushTool, loadBrushPref, saveBrushPref, stepBrushSize } from './brush';
  import ResizeDialog from './components/ResizeDialog.svelte';
  import type { EditorProps } from '../../lib/shell/types';
  import { getPref, setPref } from '../../lib/idb/prefs';
  import { syncRecentMenu } from '../../lib/menu';
  import { listRecents } from '../../lib/idb/recent-files';
  import { packColor, unpackColor } from '../../lib/color';
  import { t } from '../../lib/i18n/index.svelte';
  import { help } from '../../lib/help/state.svelte';
  import RadiusDialog from '../../lib/components/RadiusDialog.svelte';
  import RenameDialog from '../../lib/components/RenameDialog.svelte';
  import PasteLargerDialog from '../../lib/components/PasteLargerDialog.svelte';
  import { onPaste, pasteFromClipboard, runCopy, runPaste } from './clipboard-ops';
  import {
    addFrame,
    addGroup,
    addLayer,
    applyOnion,
    deleteSelectionPixels,
    deselect,
    disposeDoc,
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
    stopPlayback,
    swapColors,
    targetLayer,
    togglePlay,
    undo,
  } from './doc-ops';
  import {
    autosave,
    bindShell,
    confirmDiscard,
    describeAnimation,
    exportAnimation,
    exportPng,
    exportSheet,
    fsAccessAvailable,
    guardUnsaved,
    idbAvailable,
    importPngLayer,
    initAutosave,
    isDocDirty,
    openDoc,
    openNewDialog,
    openOpened,
    openRecent,
    nativeMenuHost,
    openAnimationDialog,
    openSheetDialog,
    recentsAvailable,
    save,
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
    fitView,
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
    toolState,
    view,
  } from './stores/editor.svelte';

  // Shell contract (src/App.svelte mounts one editor per session): the
  // document to open / create on mount, and the callbacks the shell
  // serves — New dialog, foreign-format re-routing, dirty mirror, and
  // the native-menu items this editor handles.
  let {
    initialFile,
    initialNew,
    onRequestNew,
    onOpenForeign,
    onDirtyChange,
    registerMenuHandlers,
  }: EditorProps = $props();

  // Module-level store: start every mount from a fresh state.
  resetPixelStore();
  bindShell({ requestNew: () => onRequestNew(), openForeign: (opened) => onOpenForeign(opened) });
  initAutosave();

  // Release version stamped in by the release workflow; `dev` locally.
  const appVersion = import.meta.env.VITE_APP_VERSION ?? 'dev';

  // "Modified since last save" — drives the footer dot, the tab title,
  // the beforeunload warning, and the New / Open confirm guard.
  const isDirty = $derived(isDocDirty());
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
      return ed.doc.layerKind(id) === 'image' && ed.doc.layerKind(ed.doc.layerIdAt(i - 1)) === 'image';
    }
    return false;
  });

  const layerCountNow = $derived.by(() => {
    void ed.docRev;
    return ed.doc?.layerCount ?? 0;
  });

  const railGroups = $derived(toolGroups(toolState.stampTile));
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
      newDoc: openNewDialog,
      open: () => guardUnsaved(() => void openDoc()),
      openRecent: (r) => guardUnsaved(() => void openRecent(r)),
      save: (asNew) => void save({ forceAs: asNew }),
      importPng: () => void importPngLayer(),
      exportPng: () => void exportPng(),
      exportSheet: openSheetDialog,
      exportAnimation: openAnimationDialog,
      undo,
      redo,
      deleteSelection: deleteSelectionPixels,
      copy: runCopy,
      paste: () => void pasteFromClipboard(),
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
    }),
  );

  function onKeyDown(e: KeyboardEvent) {
    if (e.code === 'Space' && !e.repeat && !isEditableTarget(e.target)) {
      // Prevent the browser from page-scrolling on space.
      e.preventDefault();
      view.spaceDown = true;
      return;
    }
    // Dialogs own the keyboard while open — the shell's About dialog too,
    // which is mounted by App.svelte rather than by this editor.
    if (dialogs.effect || toolState.textAnchor || dialogs.resize || dialogs.sheet || dialogs.animation || autosave.snapshots.length > 0) return;
    if (dialogs.selectionRadius || dialogs.paste || help.about) return;
    if (e.key === 'Escape') {
      if (dialogs.confirm) {
        dialogs.confirm = null;
        return;
      }
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
      if (nativeMenuHost && NATIVE_MENU_ACTIONS.has(action)) return;
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
        guardUnsaved(() => void openDoc());
        return true;
      case 'save':
      case 'saveAs':
        void save({ forceAs: action === 'saveAs' });
        return true;
      case 'export':
        void exportPng();
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
      default:
        return false;
    }
  }

  // Ctrl+V / ⌘V arrives as the browser's `paste` event (spec §5.4); not
  // while an editor dialog or the About dialog is open.
  function onWindowPaste(e: ClipboardEvent) {
    const blocked = Boolean(
      dialogs.effect || dialogs.resize || dialogs.sheet || dialogs.selectionRadius || dialogs.paste,
    );
    onPaste(e, blocked || help.about || toolState.textAnchor !== null);
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.code === 'Space') {
      view.spaceDown = false;
    }
  }

  function onVisibilityChange() {
    if (document.hidden) onWindowBlur();
  }

  onMount(() => {
    let cancelled = false;
    loadCore()
      .then(async () => {
        if (cancelled) return;
        // Pick the base-layer renderer before the first frame. The base
        // canvas is bound by the time onMount runs (spec §4.4, M12.5).
        if (view.canvas && !live.renderer) {
          const r = await createRenderer(view.canvas);
          if (cancelled) {
            r.destroy();
            return;
          }
          live.renderer = r;
          ed.backend = r.backend;
        }
        const w = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(initialNew?.width ?? 64)));
        const h = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(initialNew?.height ?? 64)));
        ed.doc = new Document(w, h);
        syncMeta();
        fitView();
        syncSelection();
        live.needsRecompose = true;
        ed.lastSavedUndoDepth = ed.doc.undoDepth;
        ed.status = t('px.ready');
        live.rafHandle = requestAnimationFrame(tick);
        // The shell hands over a file it already read (start screen,
        // drag-and-drop, file association, Open Recent).
        if (initialFile) {
          file.busy = true;
          try {
            await openOpened(initialFile);
          } finally {
            file.busy = false;
          }
        }
        // Queued behind the `tick` above, so the document has composed
        // by the time the canvas reports ready.
        requestAnimationFrame(() => {
          if (!cancelled) ed.ready = true;
        });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const msg = err instanceof Error ? err.message : String(err);
        console.error('loadCore failed', err);
        ed.status = t('status.failed', { action: t('act.wasmInit'), error: msg });
      });
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('paste', onWindowPaste);
    window.addEventListener('blur', onWindowBlur);
    window.addEventListener('beforeunload', onBeforeUnload);
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
    // Recovery probe and the autosave timer. A tick before `loadCore`
    // resolves is a no-op (no document yet).
    autosave.start();
    // Native-menu items this editor serves (Tauri only; the shell owns
    // New / Open / Open Recent and the open-file events).
    registerMenuHandlers({
      'menu:save': () => save(),
      'menu:saveAs': () => save({ forceAs: true }),
      'menu:exportPng': () => void exportPng(),
      'menu:exportSheet': openSheetDialog,
      'menu:exportAnimation': openAnimationDialog,
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoomIn': zoomIn,
      'menu:zoomOut': zoomOut,
      'menu:resetZoom': resetView,
    });
    return () => {
      cancelled = true;
      if (live.rafHandle !== null) cancelAnimationFrame(live.rafHandle);
      stopPlayback();
      autosave.close();
      registerMenuHandlers(null);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('paste', onWindowPaste);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onWindowBlur);
      window.removeEventListener('beforeunload', onBeforeUnload);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      view.canvas?.removeEventListener('wheel', onWheel);
      live.renderer?.destroy();
      live.renderer = null;
      disposeDoc();
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

  // Reflect the document identity + dirty state in the tab title.
  $effect(() => {
    document.title = `${isDirty ? '● ' : ''}${file.saveTarget.name} – Pincel`;
  });

  // Mirror the dirty state to the shell (its guards for mode switches,
  // native New / Open and open-file events).
  $effect(() => {
    onDirtyChange(isDirty);
  });

  // Warn before the tab closes with unsaved edits. Autosave (30 s
  // cadence) softens the loss, but the user should still get the
  // browser's "leave site?" prompt.
  function onBeforeUnload(e: BeforeUnloadEvent) {
    if (!isDirty) return;
    e.preventDefault();
    // Chrome still requires returnValue to be set.
    e.returnValue = '';
  }

  // Push the current recents list to the Rust-side `Open Recent`
  // submenu so the native menu stays in sync with the in-memory list.
  // Only entries with a path can be re-opened by the menu (FSA handles
  // need an in-page permission gesture). Failures are logged but don't
  // surface in the UI — the toolbar dropdown remains the authoritative
  // recents UI on the web.
  $effect(() => {
    if (!tauriHost) return;
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
        ed.status = t('status.failed', { action: t('act.layerVisibility'), error: err instanceof Error ? err.message : String(err) });
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
        ed.status = t('status.failed', { action: t('act.layerLock'), error: err instanceof Error ? err.message : String(err) });
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
        ed.status = t('status.failed', { action: t('act.layerReorder'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onRename={(layerId, name) => {
      if (!ed.doc) return;
      try {
        ed.doc.renameLayer(layerId, name);
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerRename'), error: err instanceof Error ? err.message : String(err) });
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
        ed.status = t('status.failed', { action: t('act.layerOpacity'), error: err instanceof Error ? err.message : String(err) });
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
        ed.status = t('status.failed', { action: t('act.layerBlendMode'), error: err instanceof Error ? err.message : String(err) });
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
    onChange={() => (ed.docRev += 1)}
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

<EditorFrame
  mode="pixel"
  {menus}
  title={ed.doc ? file.saveTarget.name : t('px.noDocument')}
  dirty={isDirty}
  panels={[
    { id: 'colors', title: t('panel.colors'), icon: 'colors', body: colorsBody },
    { id: 'palette', title: t('panel.palette'), icon: 'palette', body: paletteBody },
    { id: 'layers', title: t('panel.layers'), icon: 'layers', body: layersBody, grow: true },
    {
      id: 'tilesets',
      title: t('panel.tilesets'),
      icon: 'grid',
      body: tilesetsBody,
      defaultCollapsed: true,
    },
    { id: 'slices', title: t('panel.slices'), icon: 'slice', body: slicesBody, defaultCollapsed: true },
    { id: 'timeline', title: t('panel.timeline'), icon: 'film', body: timelineBody, placement: 'bottom' },
  ]}
>
  {#snippet quick()}
    <IconButton icon="undo" label={t('menu.undo')} hint={hint('undo')} disabled={ed.undoDepth === 0} onclick={undo} />
    <IconButton icon="redo" label={t('menu.redo')} hint={hint('redo')} disabled={ed.redoDepth === 0} onclick={redo} />
  {/snippet}
  {#snippet tools(orientation)}
    <ToolRail groups={railGroups} active={toolState.tool} onSelect={(next) => (toolState.tool = next)} {orientation} />
  {/snippet}
  {#snippet options()}
    <ToolVariants groups={railGroups} active={toolState.tool} onSelect={(next) => (toolState.tool = next)} />
    {#if toolState.tool === 'selection-wand'}
      <label class="opt">
        {t('opt.tolerance')}
        <input
          type="number"
          min="0"
          max="255"
          bind:value={toolState.wandTolerance}
          class="opt-field w-16 text-right tabular-nums"
          aria-label={t('px.wandTolerance')}
        />
      </label>
      <label class="opt">
        <input type="checkbox" bind:checked={toolState.wandContiguous} aria-label={t('opt.contiguous')} />
        {t('opt.contiguous')}
      </label>
      <label class="opt">
        {t('opt.sample')}
        <select bind:value={toolState.wandSample} class="opt-field" aria-label={t('px.wandSample')}>
          <option value="layer">{t('px.layer')}</option>
          <option value="composite">{t('opt.composite')}</option>
        </select>
      </label>
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
        <span class="opt text-pc-faint">{t('px.fillsBg')}</span>
      {/if}
    {:else if isBrushTool(toolState.tool)}
      <BrushOptions />
      {#if BG_TOOLS.has(toolState.tool)}
        <span class="opt text-pc-faint">{t('px.rightBg')}</span>
      {/if}
    {:else}
      <span class="opt text-pc-faint">{toolLabel}</span>
      {#if BG_TOOLS.has(toolState.tool)}
        <span class="opt text-pc-faint">{t('px.rightBg')}</span>
      {/if}
    {/if}
  {/snippet}
  {#snippet viewport()}
        <!-- The stage takes the pointer, not just the canvas, so a selection
             gesture can start beside the canvas (spec §9.4); `onPointerDown`
             lets only selection and view gestures start there. -->
        <div
          class="absolute inset-0 flex items-center justify-center overflow-hidden select-none"
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
          </div>
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

{#if autosave.snapshots.length > 0}
  <RecoveryDialog
    snapshots={autosave.snapshots}
    errors={autosave.errors}
    onRecover={(snap) => void autosave.recover(snap)}
    onDiscard={(id) => void autosave.discard(id)}
    onDismiss={() => autosave.dismiss()}
  />
{/if}

{#if dialogs.selectionRadius}
  {@const op = dialogs.selectionRadius}
  <RadiusDialog
    title={t(op === 'expand' ? 'menu.expand' : 'menu.contract')}
    value={1}
    onCancel={() => (dialogs.selectionRadius = null)}
    onApply={(radius) => {
      dialogs.selectionRadius = null;
      selectAction(op, radius);
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
        ed.status = t('status.failed', { action: t('act.layerRename'), error: err instanceof Error ? err.message : String(err) });
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
    onExpand={() => runPaste(paste.doc, paste.bytes, paste.x, paste.y, true)}
    onKeep={() => runPaste(paste.doc, paste.bytes, paste.x, paste.y, false)}
    onCancel={() => (dialogs.paste = null)}
  />
{/if}

{#if dialogs.sheet && ed.doc}
  <SpriteSheetDialog
    frameCount={ed.frameCount}
    tags={sheetTags}
    frameWidth={ed.canvasW}
    frameHeight={ed.canvasH}
    onSubmit={(o) => void exportSheet(o)}
    onCancel={() => (dialogs.sheet = false)}
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

{#if dialogs.effect}
  {#key dialogs.effect.id}
    <EffectDialog
      def={dialogs.effect}
      hasSelection={ed.selection !== null}
      onPreview={previewEffectParams}
      onApply={applyEffectParams}
      onCancel={cancelEffect}
    />
  {/key}
{/if}

{#if dialogs.confirm}
  <div
    class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
    role="alertdialog"
    aria-modal="true"
    aria-label={t('shell.unsavedChanges')}
  >
    <div class="w-80 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl">
      <p>{dialogs.confirm.message}</p>
      <div class="mt-4 flex justify-end gap-2">
        <button class="panel-btn" onclick={() => (dialogs.confirm = null)}>{t('common.cancel')}</button>
        <button
          class="panel-btn border-red-800 text-red-300"
          onclick={confirmDiscard}
        >
          {t('shell.discard')}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .canvas-pixelated {
    image-rendering: pixelated;
  }
</style>
