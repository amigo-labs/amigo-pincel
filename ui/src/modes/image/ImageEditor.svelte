<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorProps } from '../../lib/shell/types';
  import { editor, tool, ui, resetColors, swapColors, type ToolKind } from './stores/editor.svelte';
  import {
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
  import ToolBar from './components/toolbar/ToolBar.svelte';
  import ToolOptions from './components/toolbar/ToolOptions.svelte';
  import ColorsPanel from './components/panels/ColorsPanel.svelte';
  import LayersPanel from './components/panels/LayersPanel.svelte';
  import TransformMenu from './components/menus/TransformMenu.svelte';
  import EffectsMenu from './components/menus/EffectsMenu.svelte';
  import ExportDialog from './components/dialogs/ExportDialog.svelte';
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
  let exportOpen = $state(false);
  let jpegDialogOpen = $state(false);

  // Export formats (spec §13.2; WebP is lossless per ADR-007).
  const exportFormats: Array<{ format: ExportFormat; label: string }> = [
    { format: 'png', label: 'PNG' },
    { format: 'jpeg', label: 'JPEG…' },
    { format: 'webp', label: 'WebP (lossless)' },
  ];

  function runExport(format: ExportFormat, quality?: number): void {
    exportOpen = false;
    // JPEG is lossy: ask for the quality first (spec §13.2).
    if (format === 'jpeg' && quality === undefined) {
      jpegDialogOpen = true;
      return;
    }
    exportImage(format, quality).catch((e) => (loadError = `Export failed: ${String(e)}`));
  }

  // Single-key tool shortcuts (spec §9.2, §16.2). M and L cycle their pair.
  const toolShortcuts: Record<string, ToolKind> = {
    b: 'pencil',
    e: 'eraser',
    g: 'fill',
    i: 'eyedropper',
    v: 'move',
    w: 'magic_wand',
    u: 'shapes',
    t: 'text',
  };
  const toolLabels: Record<ToolKind, string> = {
    pencil: 'Pencil',
    eraser: 'Eraser',
    fill: 'Fill',
    eyedropper: 'Eyedropper',
    move: 'Move',
    rect_select: 'Rectangle Select',
    ellipse_select: 'Ellipse Select',
    lasso: 'Lasso',
    polygon_lasso: 'Polygonal Lasso',
    magic_wand: 'Magic Wand',
    shapes: 'Shapes',
    text: 'Text',
  };

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

  function onKeydown(e: KeyboardEvent): void {
    // Ignore shortcuts while typing in a field (text-entry overlay included)
    // or while a modal dialog owns the keyboard.
    if (
      ui.modalOpen ||
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement
    ) {
      return;
    }
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (ctrl && key === 'z' && !e.shiftKey) {
      e.preventDefault();
      undo();
    } else if (ctrl && (key === 'y' || (key === 'z' && e.shiftKey))) {
      e.preventDefault();
      redo();
    } else if (ctrl && key === 'a') {
      e.preventDefault();
      selectAll();
    } else if (ctrl && key === 'd') {
      e.preventDefault();
      deselect();
    } else if (ctrl && e.shiftKey && key === 'i') {
      e.preventDefault();
      invertSelection();
    } else if (ctrl && key === 's') {
      e.preventDefault();
      void save(e.shiftKey);
    } else if (ctrl && key === 'e') {
      e.preventDefault();
      exportOpen = !exportOpen;
    } else if (key === 'escape') {
      gestureControl.cancel();
      exportOpen = false;
    } else if (key === 'delete' || key === 'backspace') {
      e.preventDefault();
      deleteSelection();
    } else if (key === '[') {
      tool.size = Math.max(1, tool.size - (tool.size > 10 ? 5 : 1));
    } else if (key === ']') {
      tool.size = Math.min(500, tool.size + (tool.size >= 10 ? 5 : 1));
    } else if (!ctrl && key === 'x') {
      swapColors();
    } else if (!ctrl && key === 'd') {
      resetColors();
    } else if (!ctrl && key === 'm') {
      tool.kind = tool.kind === 'rect_select' ? 'ellipse_select' : 'rect_select';
    } else if (!ctrl && key === 'l') {
      tool.kind = tool.kind === 'lasso' ? 'polygon_lasso' : 'lasso';
    } else if (!ctrl && toolShortcuts[key]) {
      tool.kind = toolShortcuts[key];
    }
  }

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

<div class="flex h-full flex-col bg-pc-bg text-pc-text">
  <!-- Top action bar -->
  <header
    class="flex items-center gap-2 border-b border-pc-border bg-pc-panel px-3 py-1.5 text-sm"
  >
    <span class="mr-3 font-semibold tracking-wide">Pincel</span>
    <span
      class="mr-3 rounded border border-pc-border px-1.5 text-xs text-pc-accent"
    >
      Image
    </span>
    <button class="rounded px-2 py-1 hover:bg-pc-hover" onclick={onRequestNew}>New</button>
    <button class="rounded px-2 py-1 hover:bg-pc-hover" onclick={onRequestOpen}>Open…</button>
    <button
      class="rounded px-2 py-1 hover:bg-pc-hover"
      onclick={() => void save()}
      title="Save as OpenRaster (.ora) — keeps layers (Ctrl+S)">Save</button
    >
    <button
      class="rounded px-2 py-1 hover:bg-pc-hover"
      onclick={() => void save(true)}
      title="Save to a new .ora file (Ctrl+Shift+S)">Save As…</button
    >
    <div class="relative">
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover"
        class:bg-pc-active={exportOpen}
        onclick={() => (exportOpen = !exportOpen)}
      >
        Export…
      </button>
      {#if exportOpen}
        <!-- Backdrop closes the menu on an outside click. -->
        <button
          class="fixed inset-0 z-40 cursor-default"
          aria-label="Close menu"
          onclick={() => (exportOpen = false)}
        ></button>
        <div
          class="absolute left-0 top-8 z-50 w-40 rounded border border-pc-border bg-pc-panel py-1 text-sm shadow-xl"
        >
          {#each exportFormats as f (f.format)}
            <button
              class="block w-full px-3 py-1 text-left hover:bg-pc-hover"
              onclick={() => runExport(f.format)}
            >
              {f.label}
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <div class="mx-2 h-5 w-px bg-pc-border"></div>
    <button
      class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
      onclick={undo}
      disabled={!editor.canUndo}
    >
      Undo
    </button>
    <button
      class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
      onclick={redo}
      disabled={!editor.canRedo}
    >
      Redo
    </button>
    <div class="mx-2 h-5 w-px bg-pc-border"></div>
    <TransformMenu />
    <EffectsMenu />
    <EffectsMenu label="Adjustments" groups={['adjust']} />
  </header>

  <div class="flex min-h-0 flex-1">
    <ToolBar />

    <div class="flex min-w-0 flex-1 flex-col">
      <ToolOptions />
      <main class="min-h-0 flex-1">
        <MainCanvas />
      </main>
      <!-- Status bar (spec §16.8) -->
      <footer
        class="flex items-center justify-between border-t border-pc-border bg-pc-panel px-3 py-1 text-xs text-pc-muted"
      >
        <span>{toolLabels[tool.kind]}</span>
        <span
          >{projectName()}{#if dirty}<span class="text-amber-400" title="unsaved changes">
              ●</span
            >{/if}
          · {editor.width} × {editor.height} · sRGB</span
        >
        <span>{loadError ?? 'Ready'}</span>
      </footer>
    </div>

    <aside
      class="flex w-56 flex-col border-l border-pc-border bg-pc-bg"
    >
      <ColorsPanel />
      <LayersPanel />
    </aside>
  </div>
</div>

{#if recoverySnapshots.length > 0}
  <RecoveryDialog
    snapshots={recoverySnapshots}
    errors={recoveryErrors}
    onRecover={(snap) => void recover(snap)}
    onDiscard={(id) => void discardSnapshot(id)}
    onDismiss={() => (recoverySnapshots = [])}
  />
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
