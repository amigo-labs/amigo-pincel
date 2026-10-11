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

  // Export formats (spec §13.2; WebP is lossless per ADR-007).
  const exportFormats: Array<{ format: ExportFormat; label: string }> = [
    { format: 'png', label: 'PNG' },
    { format: 'jpeg', label: 'JPEG…' },
    { format: 'webp', label: 'WebP (lossless)' },
  ];

  function runExport(format: ExportFormat, quality?: number): void {
    // JPEG is lossy: ask for the quality first (spec §13.2).
    if (format === 'jpeg' && quality === undefined) {
      jpegDialogOpen = true;
      return;
    }
    exportImage(format, quality).catch((e) => (loadError = `Export failed: ${String(e)}`));
  }

  // Tool rail (spec §9.2): same groups, order and keys as Pixel mode
  // (lib/shell/shortcuts.ts), with the tools Image mode has.
  const toolGroups: ToolGroup<ToolKind>[] = [
    { id: 'move', label: 'Move', tools: [{ id: 'move', label: 'Move', icon: 'move' }] },
    {
      id: 'select',
      label: 'Marquee',
      tools: [
        { id: 'rect_select', label: 'Rectangle Select', icon: 'select-rect' },
        { id: 'ellipse_select', label: 'Ellipse Select', icon: 'select-ellipse' },
      ],
    },
    {
      id: 'lasso',
      label: 'Lasso',
      tools: [
        { id: 'lasso', label: 'Lasso', icon: 'lasso' },
        { id: 'polygon_lasso', label: 'Polygonal Lasso', icon: 'polygon-lasso' },
      ],
    },
    { id: 'wand', label: 'Magic Wand', tools: [{ id: 'magic_wand', label: 'Magic Wand', icon: 'wand' }] },
    { id: 'pencil', label: 'Brush', tools: [{ id: 'pencil', label: 'Brush', icon: 'brush' }] },
    { id: 'eraser', label: 'Eraser', tools: [{ id: 'eraser', label: 'Eraser', icon: 'eraser' }] },
    { id: 'fill', label: 'Fill', tools: [{ id: 'fill', label: 'Fill', icon: 'bucket' }] },
    {
      id: 'eyedropper',
      label: 'Eyedropper',
      tools: [{ id: 'eyedropper', label: 'Eyedropper', icon: 'eyedropper' }],
    },
    { id: 'shapes', label: 'Shapes', tools: [{ id: 'shapes', label: 'Shapes', icon: 'shapes' }] },
    { id: 'text', label: 'Text', tools: [{ id: 'text', label: 'Text', icon: 'text' }] },
  ].map((g) => ({ ...g, shortcut: toolHint(g.id) }) as ToolGroup<ToolKind>);

  const toolLabel = $derived(
    groupOf(toolGroups, tool.kind)?.tools.find((t) => t.id === tool.kind)?.label ?? '',
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
      loadError = `Save failed: ${String(e)}`;
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
      loadError = `Autosave failed: ${String(err)}`;
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
    start.catch((e) => (loadError = `Could not open image: ${String(e)}`));
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
      label: 'File',
      items: tidy([
        { label: 'New…', run: onRequestNew },
        { label: 'Open…', shortcut: hint('open'), run: onRequestOpen },
        { kind: 'separator' },
        { label: 'Save', shortcut: hint('save'), disabled: !hasDoc, run: () => void save() },
        {
          label: 'Save As…',
          shortcut: hint('saveAs'),
          disabled: !hasDoc,
          run: () => void save(true),
        },
        { kind: 'separator' },
        {
          kind: 'submenu',
          label: 'Export',
          disabled: !hasDoc,
          items: [
            { label: 'PNG', shortcut: hint('export'), run: () => runExport('png') },
            { label: 'JPEG…', run: () => runExport('jpeg') },
            { label: 'WebP (lossless)', run: () => runExport('webp') },
          ],
        },
      ]),
    },
    {
      id: 'edit',
      label: 'Edit',
      items: [
        { label: 'Undo', shortcut: hint('undo'), disabled: !editor.canUndo, run: undo },
        { label: 'Redo', shortcut: hint('redo'), disabled: !editor.canRedo, run: redo },
        { kind: 'separator' },
        {
          label: 'Delete Selection',
          shortcut: hint('deleteSelection'),
          disabled: !editor.hasSelection,
          run: deleteSelection,
        },
        { kind: 'separator' },
        { label: 'Swap Colors', shortcut: hint('swapColors'), run: swapColors },
        { label: 'Default Colors', shortcut: hint('resetColors'), run: resetColors },
      ],
    },
    {
      id: 'select',
      label: 'Select',
      items: [
        { label: 'Select All', shortcut: hint('selectAll'), disabled: !hasDoc, run: selectAll },
        { label: 'Deselect', shortcut: hint('deselect'), disabled: !editor.hasSelection, run: deselect },
        {
          label: 'Invert Selection',
          shortcut: hint('invertSelection'),
          disabled: !hasDoc,
          run: invertSelection,
        },
      ],
    },
    {
      id: 'image',
      label: 'Image',
      items: [
        { label: 'Flip Horizontal', disabled: !hasDoc, run: () => flipCanvas(true) },
        { label: 'Flip Vertical', disabled: !hasDoc, run: () => flipCanvas(false) },
        { kind: 'separator' },
        { label: 'Rotate 90° CW', disabled: !hasDoc, run: () => rotateCanvas('cw90') },
        { label: 'Rotate 90° CCW', disabled: !hasDoc, run: () => rotateCanvas('ccw90') },
        { label: 'Rotate 180°', disabled: !hasDoc, run: () => rotateCanvas('rotate_180') },
        { kind: 'separator' },
        { label: 'Resize Canvas…', disabled: !hasDoc, run: () => (transformDialog = 'resize') },
        { label: 'Scale Image…', disabled: !hasDoc, run: () => (transformDialog = 'scale') },
        { label: 'Crop to Selection', disabled: !editor.hasSelection, run: cropToSelection },
      ],
    },
    {
      id: 'layer',
      label: 'Layer',
      items: [
        { label: 'New Layer', disabled: !hasDoc, run: addLayer },
        {
          label: 'Duplicate Layer',
          disabled: !hasDoc,
          run: () => duplicateLayer(editor.activeLayer),
        },
        {
          label: 'Delete Layer',
          disabled: editor.layers.length <= 1,
          run: () => deleteLayer(editor.activeLayer),
        },
        { kind: 'separator' },
        {
          label: 'Merge Down',
          disabled: editor.activeLayer === 0,
          run: () => mergeDown(editor.activeLayer),
        },
        { label: 'Merge Visible', disabled: !hasDoc, run: mergeVisible },
        { label: 'Flatten Image', disabled: !hasDoc, run: flattenImage },
        { kind: 'separator' },
        { label: 'Flip Horizontal', disabled: !hasDoc, run: () => transformLayer('flip_h') },
        { label: 'Flip Vertical', disabled: !hasDoc, run: () => transformLayer('flip_v') },
        { label: 'Rotate 90° CW', disabled: !hasDoc, run: () => rotateLayer90(false) },
        { label: 'Rotate 90° CCW', disabled: !hasDoc, run: () => rotateLayer90(true) },
        { label: 'Rotate 180°', disabled: !hasDoc, run: () => transformLayer('rotate_180') },
      ],
    },
    ...effectMenus((def) => (effectOpen = def), !hasDoc),
    {
      id: 'view',
      label: 'View',
      items: [
        { label: 'Zoom In', shortcut: hint('zoomIn'), disabled: !hasDoc, run: () => viewCommands.zoomIn() },
        { label: 'Zoom Out', shortcut: hint('zoomOut'), disabled: !hasDoc, run: () => viewCommands.zoomOut() },
        { label: 'Fit to Window', shortcut: hint('zoomFit'), disabled: !hasDoc, run: () => viewCommands.fit() },
        { label: 'Actual Size', shortcut: hint('zoomActual'), disabled: !hasDoc, run: () => viewCommands.actual() },
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
  title={editor.handle === null ? 'No document' : projectName()}
  {dirty}
  panels={[
    { id: 'colors', title: 'Colors', icon: 'colors', body: colorsBody },
    { id: 'layers', title: 'Layers', icon: 'layers', body: layersBody, grow: true },
  ]}
>
  {#snippet quick()}
    <IconButton icon="undo" label="Undo" hint={hint('undo')} disabled={!editor.canUndo} onclick={undo} />
    <IconButton icon="redo" label="Redo" hint={hint('redo')} disabled={!editor.canRedo} onclick={redo} />
  {/snippet}
  {#snippet tools(orientation)}
    <ToolRail groups={toolGroups} active={tool.kind} onSelect={(t) => (tool.kind = t)} {orientation} />
  {/snippet}
  {#snippet options()}
    <ToolVariants groups={toolGroups} active={tool.kind} onSelect={(t) => (tool.kind = t)} />
    <ToolOptions />
  {/snippet}
  {#snippet canvas()}
    <MainCanvas />
  {/snippet}
  {#snippet status()}
    <span>{toolLabel}</span>
    <span>·</span>
    <span>{editor.width} × {editor.height} · sRGB</span>
    <span class="flex-1"></span>
    <span class:text-amber-400={loadError !== null}>{loadError ?? 'Ready'}</span>
  {/snippet}
</EditorFrame>

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
