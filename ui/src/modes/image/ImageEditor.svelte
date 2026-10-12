<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorProps } from '../../lib/shell/types';
  import {
    editor,
    tool,
    ui,
    resetColors,
    swapColors,
    viewCommands,
    type ToolKind,
  } from './stores/editor.svelte';
  import {
    addLayer,
    applyEffect,
    clearEffectPreview,
    cropToSelection,
    deleteLayer,
    duplicateLayer,
    flattenImage,
    flipCanvas,
    mergeDown,
    renameLayer,
    mergeVisible,
    previewEffect,
    resizeCanvas,
    rotateCanvas,
    rotateLayer90,
    scaleImage,
    transformLayer,
    newDocument,
    openBytes,
    exportImage,
    saveProject,
    projectBytes,
    projectName,
    markUnsaved,
    undo,
    redo,
    selectAll,
    deselect,
    deleteSelection,
    invertSelection,
    type ExportFormat,
  } from './core/controller';
  import { gestureControl } from './tools/pointer';
  import MainCanvas from './components/canvas/MainCanvas.svelte';
  import ToolOptions from './components/toolbar/ToolOptions.svelte';
  import ColorsPanel from './components/panels/ColorsPanel.svelte';
  import LayersPanel from './components/panels/LayersPanel.svelte';
  import ExportDialog from './components/dialogs/ExportDialog.svelte';
  import ResizeDialog from './components/dialogs/ResizeDialog.svelte';
  import ScaleDialog from './components/dialogs/ScaleDialog.svelte';
  import EffectDialog from '../../lib/components/EffectDialog.svelte';
  import EditorFrame from '../../lib/components/layout/EditorFrame.svelte';
  import IconButton from '../../lib/components/layout/IconButton.svelte';
  import ToolRail from '../../lib/components/layout/ToolRail.svelte';
  import ToolVariants from '../../lib/components/layout/ToolVariants.svelte';
  import { tidy, type MenuDef } from '../../lib/components/layout/menu';
  import { cycleGroup, groupOf, type ToolGroup } from '../../lib/components/layout/tools';
  import { effectMenus } from '../../lib/effects/menu';
  import type { EffectDef } from '../../lib/effects/catalog';
  import {
    NATIVE_MENU_ACTIONS,
    actionFor,
    hint,
    isTypingTarget,
    toolGroupForKey,
    toolHint,
  } from '../../lib/shell/shortcuts';
  import { hasFsAccess } from '../../lib/fs';
  import { isIdbAvailable } from '../../lib/idb/db';
  import { upsertRecent } from '../../lib/idb/recent-files';
  import {
    latestSnapshot,
    listLatestSnapshots,
    removeSnapshots,
    writeSnapshot,
    type AutosaveSnapshotMeta,
  } from '../../lib/idb/autosave';
  import RecoveryDialog from '../../lib/components/RecoveryDialog.svelte';
  import type { SaveTarget } from '../../lib/fs';
  import { isTauri } from '../../lib/platform';
  import { languageMenu } from '../../lib/i18n/locale';
  import { t } from '../../lib/i18n/index.svelte';
  import RenameDialog from '../../lib/components/RenameDialog.svelte';

  // Shell contract (src/App.svelte mounts one editor per session): the
  // document to open / create on mount, and the callbacks the shell
  // serves — New / Open dialogs, dirty mirror, native-menu items.
  let {
    initialFile,
    initialNew,
    onRequestNew,
    onRequestOpen,
    onDirtyChange,
    registerMenuHandlers,
  }: EditorProps = $props();

  let loadError = $state<string | null>(null);
  let jpegDialogOpen = $state(false);
  let transformDialog = $state<'resize' | 'scale' | null>(null);
  let effectOpen = $state<EffectDef | null>(null);
  // Layer ▸ Rename… target while its dialog is open.
  let renaming = $state<{ index: number; name: string } | null>(null);

  function runExport(format: ExportFormat, quality?: number): void {
    // JPEG is lossy: ask for the quality first (spec §13.2).
    if (format === 'jpeg' && quality === undefined) {
      jpegDialogOpen = true;
      return;
    }
    exportImage(format, quality).catch((e) => (loadError = t('img.exportFailed', { error: String(e) })));
  }

  // Tool rail (spec §9.2): same groups, order and keys as Pixel mode
  // (lib/shell/shortcuts.ts), with the tools Image mode has.
  const toolGroups: ToolGroup<ToolKind>[] = $derived(
    [
      {
        id: 'move',
        label: t('tool.move'),
        tools: [{ id: 'move', label: t('tool.move'), icon: 'move' }],
      },
      {
        id: 'select',
        label: t('tool.marquee'),
        tools: [
          { id: 'rect_select', label: t('tool.rectSelect'), icon: 'select-rect' },
          { id: 'ellipse_select', label: t('tool.ellipseSelect'), icon: 'select-ellipse' },
        ],
      },
      {
        id: 'lasso',
        label: t('tool.lasso'),
        tools: [
          { id: 'lasso', label: t('tool.lasso'), icon: 'lasso' },
          { id: 'polygon_lasso', label: t('tool.polygonLasso'), icon: 'polygon-lasso' },
        ],
      },
      {
        id: 'wand',
        label: t('tool.wand'),
        tools: [{ id: 'magic_wand', label: t('tool.wand'), icon: 'wand' }],
      },
      {
        id: 'pencil',
        label: t('tool.brush'),
        tools: [{ id: 'pencil', label: t('tool.brush'), icon: 'brush' }],
      },
      {
        id: 'eraser',
        label: t('tool.eraser'),
        tools: [{ id: 'eraser', label: t('tool.eraser'), icon: 'eraser' }],
      },
      {
        id: 'fill',
        label: t('tool.fill'),
        tools: [{ id: 'fill', label: t('tool.fill'), icon: 'bucket' }],
      },
      {
        id: 'eyedropper',
        label: t('tool.eyedropper'),
        tools: [{ id: 'eyedropper', label: t('tool.eyedropper'), icon: 'eyedropper' }],
      },
      {
        id: 'shapes',
        label: t('tool.shapes'),
        tools: [{ id: 'shapes', label: t('tool.shapes'), icon: 'shapes' }],
      },
      {
        id: 'text',
        label: t('tool.text'),
        tools: [{ id: 'text', label: t('tool.text'), icon: 'text' }],
      },
    ].map((g) => ({ ...g, shortcut: toolHint(g.id) }) as ToolGroup<ToolKind>),
  );

  const toolLabel = $derived(
    groupOf(toolGroups, tool.kind)?.tools.find((x) => x.id === tool.kind)?.label ?? '',
  );

  // Recents need something a re-open can use: an FSA handle (web) or a
  // path (Tauri). Same gate as the Pixel editor / the shell.
  const recentsAvailable = (hasFsAccess() || isTauri()) && isIdbAvailable();

  async function recordRecent(
    file: Pick<SaveTarget, 'name' | 'handle' | 'path'> | null,
  ): Promise<void> {
    if (!recentsAvailable || !file || (!file.handle && !file.path)) return;
    try {
      await upsertRecent({
        id: docId,
        name: file.name,
        handle: file.handle,
        path: file.path,
        mode: 'image',
      });
    } catch (err) {
      console.error('upsertRecent failed', err);
    }
  }

  // Save / Save As write the OpenRaster project (spec §15, 2026-10-11);
  // Export stays the flat PNG / JPEG / WebP path.
  let saving = false;
  async function save(forceAs = false): Promise<void> {
    if (saving || editor.handle === null) return;
    saving = true;
    try {
      const target = await saveProject(forceAs);
      lastWriteDepth = editor.undoDepth;
      await clearAutosave();
      await recordRecent(target);
    } catch (e) {
      loadError = t('img.saveFailed', { error: String(e) });
    } finally {
      saving = false;
    }
  }

  // Autosave (Pixel-mode parity, M10.3): every 30 s, an OpenRaster
  // snapshot of an edited document goes to the shared IDB store, tagged
  // `mode: 'image'`; a save or a fresh open drops it. On mount, snapshots
  // left by an earlier session are offered for recovery.
  const AUTOSAVE_INTERVAL_MS = 30_000;
  const autosaveAvailable = isIdbAvailable();
  let docId: string = crypto.randomUUID();
  let lastWriteDepth = 0;
  let recoverySnapshots = $state<AutosaveSnapshotMeta[]>([]);
  let recoveryErrors = $state<Record<string, string>>({});

  async function clearAutosave(): Promise<void> {
    if (!autosaveAvailable) return;
    try {
      await removeSnapshots(docId);
    } catch (err) {
      console.error('autosave clear failed', err);
    }
  }

  async function autosaveTick(): Promise<void> {
    if (!autosaveAvailable || saving || editor.handle === null) return;
    if (editor.undoDepth === lastWriteDepth || editor.undoDepth === editor.savedUndoDepth) return;
    const depth = editor.undoDepth;
    try {
      const bytes = projectBytes(true);
      if (bytes === null) return;
      await writeSnapshot(docId, projectName(), bytes, 'image');
      lastWriteDepth = depth;
    } catch (err) {
      console.error('autosave failed', err);
      loadError = t('img.autosaveFailed', { error: String(err) });
    }
  }

  async function recover(meta: AutosaveSnapshotMeta): Promise<void> {
    try {
      const full = await latestSnapshot(meta.docId);
      if (!full) throw new Error('snapshot bytes not found');
      await openBytes(full.bytes, meta.name, 'image/openraster');
      docId = meta.docId;
      markUnsaved();
      lastWriteDepth = editor.undoDepth;
      recoverySnapshots = [];
    } catch (err) {
      recoveryErrors = { ...recoveryErrors, [meta.docId]: String(err) };
    }
  }

  async function discardSnapshot(id: string): Promise<void> {
    try {
      await removeSnapshots(id);
      recoverySnapshots = recoverySnapshots.filter((s) => s.docId !== id);
    } catch (err) {
      recoveryErrors = { ...recoveryErrors, [id]: String(err) };
    }
  }

  onMount(() => {
    // Open what the shell handed over: an already-read file, or a blank
    // canvas of the requested size.
    const start = initialFile
      ? openBytes(initialFile.bytes, initialFile.name, initialFile.mime, initialFile).then(() =>
          recordRecent(initialFile),
        )
      : newDocument(initialNew?.width ?? 800, initialNew?.height ?? 600);
    start.catch((e) => (loadError = t('img.openFailed', { error: String(e) })));
    let timer: ReturnType<typeof setInterval> | null = null;
    if (autosaveAvailable) {
      listLatestSnapshots('image')
        .then((snaps) => (recoverySnapshots = snaps))
        .catch((err: unknown) => console.error('autosave probe failed', err));
      timer = setInterval(() => void autosaveTick(), AUTOSAVE_INTERVAL_MS);
    }
    registerMenuHandlers({
      'menu:save': () => void save(),
      'menu:saveAs': () => void save(true),
      'menu:exportPng': () => runExport('png'),
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoomIn': () => viewCommands.zoomIn(),
      'menu:zoomOut': () => viewCommands.zoomOut(),
      'menu:resetZoom': () => viewCommands.fit(),
    });
    return () => {
      if (timer !== null) clearInterval(timer);
      registerMenuHandlers(null);
    };
  });

  const dirty = $derived(editor.undoDepth !== editor.savedUndoDepth);

  // Mirror the dirty state to the shell (its guards for mode switches,
  // native New / Open and open-file events): edits since the last save.
  $effect(() => {
    onDirtyChange(dirty);
  });

  // Keyboard map shared with Pixel mode (lib/shell/shortcuts.ts).
  function onKeydown(e: KeyboardEvent): void {
    // Ignore shortcuts while typing in a field (text-entry overlay included)
    // or while a modal dialog owns the keyboard.
    if (ui.modalOpen || isTypingTarget(e.target)) {
      return;
    }
    const action = actionFor(e);
    if (action !== null && isTauri() && NATIVE_MENU_ACTIONS.has(action)) return;
    if (action !== null && runShortcut(action)) {
      e.preventDefault();
      return;
    }
    if (e.key === 'Escape') {
      gestureControl.cancel();
      return;
    }
    const group = toolGroupForKey(e);
    const next = group ? cycleGroup(toolGroups, group, tool.kind) : null;
    if (next) {
      e.preventDefault();
      tool.kind = next;
    }
  }

  /** Runs a shared shortcut; false when Image mode has no such action. */
  function runShortcut(action: ReturnType<typeof actionFor>): boolean {
    switch (action) {
      case 'open':
        onRequestOpen();
        return true;
      case 'save':
        void save();
        return true;
      case 'saveAs':
        void save(true);
        return true;
      case 'export':
        runExport('png');
        return true;
      case 'undo':
        undo();
        return true;
      case 'redo':
        redo();
        return true;
      case 'selectAll':
        selectAll();
        return true;
      case 'deselect':
        deselect();
        return true;
      case 'invertSelection':
        invertSelection();
        return true;
      case 'deleteSelection':
        deleteSelection();
        return true;
      case 'zoomIn':
        viewCommands.zoomIn();
        return true;
      case 'zoomOut':
        viewCommands.zoomOut();
        return true;
      case 'zoomFit':
        viewCommands.fit();
        return true;
      case 'zoomActual':
        viewCommands.actual();
        return true;
      case 'swapColors':
        swapColors();
        return true;
      case 'resetColors':
        resetColors();
        return true;
      case 'brushSmaller':
        tool.size = Math.max(1, tool.size - (tool.size > 10 ? 5 : 1));
        return true;
      case 'brushLarger':
        tool.size = Math.min(500, tool.size + (tool.size >= 10 ? 5 : 1));
        return true;
      default:
        return false;
    }
  }

  // The dialogs own Enter / Escape; keep the editor's shortcuts out.
  $effect(() => {
    ui.modalOpen = effectOpen !== null || transformDialog !== null || jpegDialogOpen;
  });

  const hasDoc = $derived(editor.handle !== null);

  // Menu bar (spec §9.2): the same menus, order and labels as Pixel mode;
  // entries Image mode lacks are simply absent.
  const menus: MenuDef[] = $derived([
    {
      id: 'file',
      label: t('menu.file'),
      items: tidy([
        { label: t('menu.new'), run: onRequestNew },
        { label: t('menu.open'), shortcut: hint('open'), run: onRequestOpen },
        { kind: 'separator' },
        {
          label: t('menu.save'),
          shortcut: hint('save'),
          disabled: !hasDoc,
          run: () => void save(),
        },
        {
          label: t('menu.saveAs'),
          shortcut: hint('saveAs'),
          disabled: !hasDoc,
          run: () => void save(true),
        },
        { kind: 'separator' },
        {
          kind: 'submenu',
          label: t('menu.export'),
          disabled: !hasDoc,
          items: [
            { label: t('menu.exportPng'), shortcut: hint('export'), run: () => runExport('png') },
            { label: t('menu.exportJpeg'), run: () => runExport('jpeg') },
            // Lossless only: no pure-Rust lossy encoder (ADR-007).
            { label: t('menu.exportWebp'), run: () => runExport('webp') },
          ],
        },
      ]),
    },
    {
      id: 'edit',
      label: t('menu.edit'),
      items: [
        { label: t('menu.undo'), shortcut: hint('undo'), disabled: !editor.canUndo, run: undo },
        { label: t('menu.redo'), shortcut: hint('redo'), disabled: !editor.canRedo, run: redo },
        { kind: 'separator' },
        {
          label: t('menu.deleteSelection'),
          shortcut: hint('deleteSelection'),
          disabled: !editor.hasSelection,
          run: deleteSelection,
        },
        { kind: 'separator' },
        { label: t('menu.swapColors'), shortcut: hint('swapColors'), run: swapColors },
        { label: t('menu.defaultColors'), shortcut: hint('resetColors'), run: resetColors },
      ],
    },
    {
      id: 'select',
      label: t('menu.select'),
      items: [
        {
          label: t('menu.selectAll'),
          shortcut: hint('selectAll'),
          disabled: !hasDoc,
          run: selectAll,
        },
        {
          label: t('menu.deselect'),
          shortcut: hint('deselect'),
          disabled: !editor.hasSelection,
          run: deselect,
        },
        {
          label: t('menu.invertSelection'),
          shortcut: hint('invertSelection'),
          disabled: !hasDoc,
          run: invertSelection,
        },
      ],
    },
    {
      id: 'image',
      label: t('menu.image'),
      items: [
        { label: t('menu.flipH'), disabled: !hasDoc, run: () => flipCanvas(true) },
        { label: t('menu.flipV'), disabled: !hasDoc, run: () => flipCanvas(false) },
        { kind: 'separator' },
        { label: t('menu.rotateCw'), disabled: !hasDoc, run: () => rotateCanvas('cw90') },
        { label: t('menu.rotateCcw'), disabled: !hasDoc, run: () => rotateCanvas('ccw90') },
        { label: t('menu.rotate180'), disabled: !hasDoc, run: () => rotateCanvas('rotate_180') },
        { kind: 'separator' },
        {
          label: t('menu.resizeCanvas'),
          disabled: !hasDoc,
          run: () => (transformDialog = 'resize'),
        },
        { label: t('menu.scaleImage'), disabled: !hasDoc, run: () => (transformDialog = 'scale') },
        { label: t('menu.crop'), disabled: !editor.hasSelection, run: cropToSelection },
      ],
    },
    {
      id: 'layer',
      label: t('menu.layer'),
      items: [
        { label: t('menu.newLayer'), disabled: !hasDoc, run: addLayer },
        {
          label: t('menu.duplicateLayer'),
          disabled: !hasDoc,
          run: () => duplicateLayer(editor.activeLayer),
        },
        {
          label: t('menu.renameLayer'),
          disabled: !hasDoc,
          run: () => {
            const layer = editor.layers[editor.activeLayer];
            if (layer) renaming = { index: editor.activeLayer, name: layer.name };
          },
        },
        {
          label: t('menu.deleteLayer'),
          disabled: editor.layers.length <= 1,
          run: () => deleteLayer(editor.activeLayer),
        },
        { kind: 'separator' },
        {
          label: t('menu.mergeDown'),
          disabled: editor.activeLayer === 0,
          run: () => mergeDown(editor.activeLayer),
        },
        { label: t('menu.mergeVisible'), disabled: !hasDoc, run: mergeVisible },
        { label: t('menu.flatten'), disabled: !hasDoc, run: flattenImage },
        { kind: 'separator' },
        { label: t('menu.flipH'), disabled: !hasDoc, run: () => transformLayer('flip_h') },
        { label: t('menu.flipV'), disabled: !hasDoc, run: () => transformLayer('flip_v') },
        { label: t('menu.rotateCw'), disabled: !hasDoc, run: () => rotateLayer90(false) },
        { label: t('menu.rotateCcw'), disabled: !hasDoc, run: () => rotateLayer90(true) },
        { label: t('menu.rotate180'), disabled: !hasDoc, run: () => transformLayer('rotate_180') },
      ],
    },
    ...effectMenus((def) => (effectOpen = def), !hasDoc),
    {
      id: 'view',
      label: t('menu.view'),
      items: [
        {
          label: t('menu.zoomIn'),
          shortcut: hint('zoomIn'),
          disabled: !hasDoc,
          run: () => viewCommands.zoomIn(),
        },
        {
          label: t('menu.zoomOut'),
          shortcut: hint('zoomOut'),
          disabled: !hasDoc,
          run: () => viewCommands.zoomOut(),
        },
        {
          label: t('menu.zoomFit'),
          shortcut: hint('zoomFit'),
          disabled: !hasDoc,
          run: () => viewCommands.fit(),
        },
        {
          label: t('menu.zoomActual'),
          shortcut: hint('zoomActual'),
          disabled: !hasDoc,
          run: () => viewCommands.actual(),
        },
        { kind: 'separator' },
        languageMenu(),
      ],
    },
  ]);

  // Warn before discarding edits on reload/close.
  function onBeforeUnload(e: BeforeUnloadEvent): void {
    if (dirty) {
      e.preventDefault();
      // Legacy browsers gate the confirmation prompt on a set returnValue.
      e.returnValue = '';
    }
  }
</script>

<svelte:window onkeydown={onKeydown} onbeforeunload={onBeforeUnload} />

{#snippet colorsBody()}<ColorsPanel />{/snippet}
{#snippet layersBody()}<LayersPanel />{/snippet}

<EditorFrame
  mode="image"
  {menus}
  title={editor.handle === null ? t('px.noDocument') : projectName()}
  {dirty}
  panels={[
    { id: 'colors', title: t('panel.colors'), icon: 'colors', body: colorsBody },
    { id: 'layers', title: t('panel.layers'), icon: 'layers', body: layersBody, grow: true },
  ]}
>
  {#snippet quick()}
    <IconButton
      icon="undo"
      label={t('menu.undo')}
      hint={hint('undo')}
      disabled={!editor.canUndo}
      onclick={undo}
    />
    <IconButton
      icon="redo"
      label={t('menu.redo')}
      hint={hint('redo')}
      disabled={!editor.canRedo}
      onclick={redo}
    />
  {/snippet}
  {#snippet tools(orientation)}
    <ToolRail
      groups={toolGroups}
      active={tool.kind}
      onSelect={(next) => (tool.kind = next)}
      {orientation}
    />
  {/snippet}
  {#snippet options()}
    <ToolVariants groups={toolGroups} active={tool.kind} onSelect={(next) => (tool.kind = next)} />
    <ToolOptions />
  {/snippet}
  {#snippet viewport()}
    <MainCanvas />
  {/snippet}
  {#snippet statusbar()}
    <span>{toolLabel}</span>
    <span>·</span>
    <span>{editor.width} × {editor.height} · sRGB</span>
    <span
      class="min-w-0 flex-1 truncate text-center"
      class:text-amber-400={loadError !== null || editor.notice !== null}
      class:text-pc-muted={loadError === null && editor.notice === null}
      >{loadError ?? editor.notice ?? ''}</span
    >
  {/snippet}
</EditorFrame>

{#if renaming}
  {@const target = renaming}
  <RenameDialog
    name={target.name}
    onCancel={() => (renaming = null)}
    onRename={(name) => {
      // Read the target before closing: `target` follows `renaming`.
      const index = target.index;
      renaming = null;
      renameLayer(index, name);
    }}
  />
{/if}

{#if recoverySnapshots.length > 0}
  <RecoveryDialog
    snapshots={recoverySnapshots}
    errors={recoveryErrors}
    onRecover={(snap) => void recover(snap)}
    onDiscard={(id) => void discardSnapshot(id)}
    onDismiss={() => (recoverySnapshots = [])}
  />
{/if}

{#if transformDialog === 'resize'}
  <ResizeDialog
    onApply={(w, h, anchor) => {
      resizeCanvas(w, h, anchor);
      transformDialog = null;
    }}
    onClose={() => (transformDialog = null)}
  />
{:else if transformDialog === 'scale'}
  <ScaleDialog
    onApply={(w, h, interp) => {
      scaleImage(w, h, interp);
      transformDialog = null;
    }}
    onClose={() => (transformDialog = null)}
  />
{/if}

{#if effectOpen}
  {#key effectOpen.id}
    <EffectDialog
      def={effectOpen}
      onPreview={(params) => {
        if (!effectOpen) return;
        // An empty vector means "preview off": show the real composite.
        if (params.length === 0 && effectOpen.params.length > 0) clearEffectPreview();
        else previewEffect(effectOpen.id, params);
      }}
      onApply={(params) => {
        if (effectOpen) applyEffect(effectOpen.id, params);
        effectOpen = null;
      }}
      onCancel={() => {
        clearEffectPreview();
        effectOpen = null;
      }}
    />
  {/key}
{/if}

{#if jpegDialogOpen}
  <ExportDialog
    onApply={(quality) => {
      jpegDialogOpen = false;
      runExport('jpeg', quality);
    }}
    onClose={() => (jpegDialogOpen = false)}
  />
{/if}
