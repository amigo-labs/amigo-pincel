<script lang="ts">
  // App shell: start screen, New-document dialog, file → mode routing,
  // and exactly one mounted editor per session (Pixel or Image). Each
  // editor is a lazily imported chunk that brings its own wasm module,
  // so a session only downloads the core it uses. See
  // docs/specs/pincel.md §1.1 "Two modes" and §9.2.
  import { onMount, type Component } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import FileAssocDialog from './lib/components/FileAssocDialog.svelte';
  import NewDocumentDialog from './lib/shell/NewDocumentDialog.svelte';
  import StartScreen from './lib/shell/StartScreen.svelte';
  import { detectMode, launch, session } from './lib/shell/session.svelte';
  import type { EditorMode, EditorProps, NewDocParams } from './lib/shell/types';
  import {
    ensureReadPermission,
    formatFromName,
    hasFsAccess,
    mimeFromName,
    pickAndOpen,
    sniffFormat,
    type OpenedFile,
  } from './lib/fs';
  import { isIdbAvailable } from './lib/idb/db';
  import { getPref, setPref } from './lib/idb/prefs';
  import { listRecents, type RecentFile } from './lib/idb/recent-files';
  import { syncRecentMenu, wireNativeMenu, type EditorMenuHandlers } from './lib/menu';
  import { isTauri } from './lib/platform';

  type EditorModule = { default: Component<EditorProps> };
  // Dynamic imports keep each editor (and its wasm package) in its own
  // chunk; Vite only fetches the one that gets mounted.
  const loaders: Record<EditorMode, () => Promise<EditorModule>> = {
    pixel: () => import('./modes/pixel/PixelEditor.svelte'),
    image: () => import('./modes/image/ImageEditor.svelte'),
  };

  const tauriHost = isTauri();
  const recentsAvailable = (hasFsAccess() || tauriHost) && isIdbAvailable();
  const FILE_ASSOC_PREF = 'fileAssocPromptShown';
  const platform: 'macos' | 'windows' | 'linux' | 'unknown' = (() => {
    if (typeof navigator === 'undefined') return 'unknown';
    const ua = navigator.userAgent;
    if (/Mac/i.test(ua)) return 'macos';
    if (/Win/i.test(ua)) return 'windows';
    if (/Linux/i.test(ua)) return 'linux';
    return 'unknown';
  })();

  // Mode preselected in the New dialog; `null` while it is closed.
  let newDialog = $state<EditorMode | null>(null);
  // Pending destructive action awaiting the unsaved-changes confirmation.
  let confirmState = $state<{ message: string; run: () => void } | null>(null);
  let recents = $state<RecentFile[]>([]);
  // True while the shell reads a file (picker, recent, drop, open-file).
  let busy = $state(false);
  let shellError = $state<string | null>(null);
  let fileAssocOpen = $state(false);
  // Native-menu items the mounted editor serves (Tauri only).
  let editorMenu: EditorMenuHandlers | null = null;

  const currentMode = $derived<EditorMode>(session.mode === 'image' ? 'image' : 'pixel');

  // Run `action` now when nothing is unsaved, else ask first. Editors
  // guard their own in-editor flows; this covers the shell's entry
  // points (native New / Open / Open Recent, open-file events).
  function guard(action: () => void) {
    if (!session.dirty) {
      action();
      return;
    }
    confirmState = { message: 'Discard unsaved changes to the open document?', run: action };
  }

  function confirmDiscard() {
    const pending = confirmState;
    confirmState = null;
    pending?.run();
  }

  function requestNew(mode: EditorMode) {
    newDialog = mode;
  }

  function createNew(params: NewDocParams) {
    newDialog = null;
    launch(params.mode, { file: null, create: params });
  }

  // Route an already-read file to the editor its format belongs to.
  function openFile(opened: OpenedFile) {
    shellError = null;
    const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
    if (format === null) {
      shellError = `${opened.name}: not a supported file (sprite or PNG / JPEG / WebP / BMP / GIF / TIFF)`;
      return;
    }
    launch(detectMode(format), { file: opened, create: null });
  }

  async function openFromPicker() {
    if (busy) return;
    busy = true;
    try {
      const opened = await pickAndOpen();
      if (opened) openFile(opened);
    } catch (err) {
      shellError = `open failed: ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      busy = false;
    }
  }

  async function readPath(path: string): Promise<Uint8Array> {
    const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', { path });
    return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
  }

  // Tauri: file-association double-click, CLI arg, single-instance forward.
  async function openByPath(path: string) {
    if (busy) return;
    busy = true;
    try {
      const bytes = await readPath(path);
      const name = path.replace(/^.*[/\\]/, '');
      openFile({ name, bytes, mime: mimeFromName(name), handle: null, path });
    } catch (err) {
      shellError = `open-file failed: ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      busy = false;
    }
  }

  async function openRecent(r: RecentFile) {
    if (busy) return;
    busy = true;
    try {
      if (tauriHost && r.path) {
        const bytes = await readPath(r.path);
        openFile({ name: r.name, bytes, mime: mimeFromName(r.name), handle: null, path: r.path });
      } else if (r.handle) {
        if (!(await ensureReadPermission(r.handle))) {
          shellError = `${r.name}: permission denied`;
          return;
        }
        const file = await r.handle.getFile();
        const bytes = new Uint8Array(await file.arrayBuffer());
        openFile({ name: file.name, bytes, mime: file.type, handle: r.handle, path: null });
      } else {
        shellError = `${r.name}: no handle / path`;
      }
    } catch (err) {
      shellError = `recent ${r.name} open failed: ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      busy = false;
    }
  }

  async function onDropFile(file: File) {
    if (busy) return;
    busy = true;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      openFile({ name: file.name, bytes, mime: file.type, handle: null, path: null });
    } catch (err) {
      shellError = `drop failed: ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      busy = false;
    }
  }

  async function refreshRecents() {
    if (!recentsAvailable) return;
    try {
      recents = await listRecents();
    } catch (err) {
      console.error('listRecents failed', err);
    }
  }

  function dismissFileAssoc(dontShowAgain: boolean) {
    fileAssocOpen = false;
    if (dontShowAgain) {
      setPref(FILE_ASSOC_PREF, true).catch((err: unknown) => {
        console.error('setPref fileAssoc failed', err);
      });
    }
  }

  // Keys typed into the confirm dialog must not reach the editor's
  // window-level shortcuts.
  function onConfirmKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      confirmState = null;
    }
  }

  // Re-read the recents whenever the start screen shows (an editor may
  // have recorded new ones meanwhile).
  $effect(() => {
    if (session.mode === 'start') void refreshRecents();
  });

  $effect(() => {
    if (session.mode === 'start') document.title = 'Pincel';
  });

  // While the start screen shows, the shell keeps the native Open Recent
  // submenu in sync; a mounted Pixel editor does the same from its list.
  $effect(() => {
    if (!tauriHost || session.mode !== 'start') return;
    const items = recents.filter((r) => r.path !== null).map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });

  onMount(() => {
    let cancelled = false;
    let unlistenMenu: UnlistenFn | null = null;
    let unlistenOpenFile: UnlistenFn | null = null;
    if (tauriHost) {
      wireNativeMenu({
        'menu:new': () => guard(() => requestNew(currentMode)),
        'menu:open': () => guard(() => void openFromPicker()),
        recent: async (id) => {
          // The native menu only knows the id; look the row up fresh so
          // entries recorded by the mounted editor are found too.
          const rows = recentsAvailable ? await listRecents() : [];
          const r = rows.find((row) => row.id === id);
          if (r) guard(() => void openRecent(r));
        },
        editor: () => editorMenu,
      })
        .then((fn) => {
          if (cancelled) fn();
          else unlistenMenu = fn;
        })
        .catch((err: unknown) => {
          console.error('wireNativeMenu failed', err);
        });
      listen<string>('open-file', (e) => {
        if (typeof e.payload === 'string') {
          const path = e.payload;
          guard(() => void openByPath(path));
        }
      })
        .then((fn) => {
          if (cancelled) fn();
          else unlistenOpenFile = fn;
        })
        .catch((err: unknown) => {
          console.error('open-file listen failed', err);
        });
      // First-launch file-association advisory. Best-effort: a missing
      // IDB or a getPref failure silently skips the dialog.
      if (isIdbAvailable()) {
        getPref(FILE_ASSOC_PREF)
          .then((shown) => {
            if (!cancelled && !shown) fileAssocOpen = true;
          })
          .catch((err: unknown) => {
            console.error('getPref fileAssoc failed', err);
          });
      }
    }
    return () => {
      cancelled = true;
      if (unlistenMenu) unlistenMenu();
      if (unlistenOpenFile) unlistenOpenFile();
    };
  });
</script>

{#if session.mode === 'start'}
  <StartScreen
    {recents}
    {busy}
    onNew={requestNew}
    onOpen={() => void openFromPicker()}
    onOpenRecent={(r) => void openRecent(r)}
    onDropFile={(f) => void onDropFile(f)}
  />
{:else}
  {#key session.seq}
    {#await loaders[currentMode]()}
      <div class="flex h-full items-center justify-center bg-neutral-950 text-sm text-neutral-400">
        loading {currentMode} editor…
      </div>
    {:then mod}
      {@const Editor = mod.default}
      <Editor
        initialFile={session.pending?.file ?? null}
        initialNew={session.pending?.create ?? null}
        onRequestNew={() => guard(() => requestNew(currentMode))}
        onRequestOpen={() => guard(() => void openFromPicker())}
        onOpenForeign={openFile}
        onDirtyChange={(dirty) => (session.dirty = dirty)}
        registerMenuHandlers={(h) => (editorMenu = h)}
      />
    {:catch err}
      <div class="flex h-full items-center justify-center bg-neutral-950 text-sm text-red-300">
        failed to load the {currentMode} editor: {err instanceof Error ? err.message : String(err)}
      </div>
    {/await}
  {/key}
{/if}

{#if newDialog}
  {#key newDialog}
    <NewDocumentDialog
      initialMode={newDialog}
      onCreate={createNew}
      onCancel={() => (newDialog = null)}
    />
  {/key}
{/if}

{#if confirmState}
  <div
    class="fixed inset-0 z-30 flex items-center justify-center bg-black/50"
    role="alertdialog"
    aria-modal="true"
    aria-label="Unsaved changes"
    tabindex="-1"
    onkeydown={onConfirmKeydown}
  >
    <div
      class="w-80 rounded border border-neutral-700 bg-neutral-900 p-4 text-sm text-neutral-100 shadow-xl"
    >
      <p>{confirmState.message}</p>
      <div class="mt-4 flex justify-end gap-2">
        <button class="panel-btn" onclick={() => (confirmState = null)}>Cancel</button>
        <button class="panel-btn border-red-800 text-red-300" onclick={confirmDiscard}
          >Discard changes</button
        >
      </div>
    </div>
  </div>
{/if}

{#if fileAssocOpen}
  <FileAssocDialog {platform} onDismiss={dismissFileAssoc} />
{/if}

{#if shellError}
  <div
    class="fixed bottom-3 left-3 z-30 flex items-center gap-3 rounded border border-red-900 bg-neutral-900 px-3 py-2 text-xs text-red-200 shadow-xl"
    role="alert"
  >
    <span>{shellError}</span>
    <button class="panel-btn" onclick={() => (shellError = null)}>Dismiss</button>
  </div>
{/if}
