<script lang="ts">
  // App shell: start screen, New-document dialog, file → mode routing,
  // and exactly one mounted editor per session (Pixel or Image). Each
  // editor is a lazily imported chunk that brings its own wasm module,
  // so a session only downloads the core it uses. See
  // docs/specs/pincel.md §1.1 "Two modes" and §9.2.
  import { onMount, type Component } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import AboutDialog from './lib/components/AboutDialog.svelte';
  import { closeAbout, help, loadUpdaterState } from './lib/help/state.svelte';
  import FileAssocDialog from './lib/components/FileAssocDialog.svelte';
  import NewDocumentDialog from './lib/shell/NewDocumentDialog.svelte';
  import StartScreen from './lib/shell/StartScreen.svelte';
  import UnsavedPrompt from './lib/shell/UnsavedPrompt.svelte';
  import { requestClose, wireCloseGuard } from './lib/shell/close-guard';
  import {
    answerUnsaved,
    guardUnsaved,
    promptBusy,
    registerDocument,
    unsaved,
  } from './lib/shell/unsaved.svelte';
  import { loadAutoNew } from './lib/shell/auto-new.svelte';
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
  import { t } from './lib/i18n/index.svelte';
  import { clipboardImage, newImageBytes, pasteEventImage, pastedImageFile } from './lib/clipboard';
  import { actionFor, isTypingTarget } from './lib/shell/shortcuts';
  import { initLocale } from './lib/i18n/locale';

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
  let recents = $state<RecentFile[]>([]);
  // True while the shell reads a file (picker, recent, drop, open-file).
  let busy = $state(false);
  let shellError = $state<string | null>(null);
  let fileAssocOpen = $state(false);
  // True until the "open like this on startup" pref is read, so the start
  // screen doesn't flash before an autostarted document replaces it.
  let booting = $state(true);
  // Native-menu items the mounted editor serves (Tauri only).
  let editorMenu: EditorMenuHandlers | null = null;

  const currentMode = $derived<EditorMode>(session.mode === 'image' ? 'image' : 'pixel');

  function errText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function requestNew(mode: EditorMode) {
    newDialog = mode;
  }

  function createNew(params: NewDocParams) {
    newDialog = null;
    launch(params.mode, { file: null, create: params });
  }

  // Route an already-read file to the editor its format belongs to. Its
  // format is recognized first, so only a file that can be opened asks
  // about unsaved changes (unsaved-changes-prompt AC-01, AC-11); `guarded`
  // is false where the prompt already ran (Open…, before its picker).
  function openFile(opened: OpenedFile, guarded = true) {
    shellError = null;
    const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
    if (format === null) {
      shellError = t('shell.unsupported', { name: opened.name });
      return;
    }
    guardIf(guarded, () => launch(detectMode(format), { file: opened, create: null }));
  }

  function guardIf(guarded: boolean, action: () => void) {
    if (guarded) guardUnsaved(action);
    else action();
  }

  async function openFromPicker() {
    if (busy) return;
    busy = true;
    try {
      const opened = await pickAndOpen();
      if (opened) openFile(opened, false);
    } catch (err) {
      shellError = t('shell.openFailed', { error: errText(err) });
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
      shellError = t('shell.openFailed', { error: errText(err) });
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
          shellError = t('shell.permissionDenied', { name: r.name });
          return;
        }
        const file = await r.handle.getFile();
        const bytes = new Uint8Array(await file.arrayBuffer());
        openFile({ name: file.name, bytes, mime: file.type, handle: r.handle, path: null });
      } else {
        shellError = t('shell.noHandle', { name: r.name });
      }
    } catch (err) {
      shellError = t('shell.recentFailed', { name: r.name, error: errText(err) });
    } finally {
      busy = false;
    }
  }

  // Ctrl/⌘+V (or Ctrl/⌘+Alt+V) on the start screen opens the clipboard
  // image as a new document (copy-cut-paste AC-22, AC-26). An empty paste
  // stays silent: it is often a stray key with text on the clipboard.
  function startScreenIdle(target: EventTarget | null): boolean {
    if (session.mode !== 'start' || busy || newDialog !== null || help.about) return false;
    if (isTypingTarget(target)) return false;
    return !(target instanceof Element && target.closest('[role="dialog"], [role="alertdialog"]'));
  }

  async function openPasted(system: Blob | null): Promise<void> {
    const bytes = await newImageBytes(system);
    if (bytes) openFile(pastedImageFile(bytes, system?.type ?? 'image/png'));
  }

  function onShellPaste(e: ClipboardEvent) {
    if (!startScreenIdle(e.target)) return;
    e.preventDefault();
    void openPasted(pasteEventImage(e));
  }

  function onShellKeydown(e: KeyboardEvent) {
    if (actionFor(e) !== 'pasteNewImage' || !startScreenIdle(e.target)) return;
    e.preventDefault();
    void clipboardImage().then(openPasted);
  }

  // A file dropped onto an open editor (the start screen takes its own).
  function onWindowDragOver(e: DragEvent) {
    if (session.mode === 'start' || !e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
  }

  function onWindowDrop(e: DragEvent) {
    if (session.mode === 'start' || e.defaultPrevented) return;
    const file = e.dataTransfer?.files[0];
    if (!file) return;
    e.preventDefault();
    void onDropFile(file);
  }

  async function onDropFile(file: File) {
    if (busy) return;
    busy = true;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      openFile({ name: file.name, bytes, mime: file.type, handle: null, path: null });
    } catch (err) {
      shellError = t('shell.dropFailed', { error: errText(err) });
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
    void initLocale();
    let cancelled = false;
    void loadAutoNew()
      .then((params) => {
        // An OS-handed file that arrived meanwhile wins over the empty document.
        if (!cancelled && params && session.mode === 'start') {
          launch(params.mode, { file: null, create: params });
        }
      })
      .finally(() => (booting = false));
    let unlistenMenu: UnlistenFn | null = null;
    let unlistenOpenFile: UnlistenFn | null = null;
    let unlistenClose: UnlistenFn | null = null;
    if (tauriHost) {
      void loadUpdaterState();
      wireCloseGuard()
        .then((fn) => {
          if (cancelled) fn();
          else unlistenClose = fn;
        })
        .catch((err: unknown) => {
          console.error('close guard failed', err);
        });
      wireNativeMenu({
        'menu:new': () => guardUnsaved(() => requestNew(currentMode)),
        'menu:open': () => guardUnsaved(() => void openFromPicker()),
        'menu:quit': () => requestClose(),
        recent: async (id) => {
          // The native menu only knows the id; look the row up fresh so
          // entries recorded by the mounted editor are found too.
          const rows = recentsAvailable ? await listRecents() : [];
          const r = rows.find((row) => row.id === id);
          if (r) void openRecent(r);
        },
        // While the unsaved-changes prompt is open the native menu's
        // accelerators (macOS Cmd+Z, Cmd+S, …) reach no editor (AC-02).
        editor: () => (promptBusy() ? null : editorMenu),
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
          void openByPath(e.payload);
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
      if (unlistenClose) unlistenClose();
    };
  });
</script>

<svelte:window
  onpaste={onShellPaste}
  onkeydown={onShellKeydown}
  ondragover={onWindowDragOver}
  ondrop={onWindowDrop}
/>

{#if session.mode === 'start' && booting}
  <div class="h-full bg-pc-bg"></div>
{:else if session.mode === 'start'}
  <StartScreen
    {recents}
    {busy}
    onCreate={createNew}
    onOpen={() => void openFromPicker()}
    onOpenRecent={(r) => void openRecent(r)}
    onDropFile={(f) => void onDropFile(f)}
  />
{:else}
  {#key session.seq}
    {#await loaders[currentMode]()}
      <div class="flex h-full items-center justify-center bg-pc-bg text-sm text-pc-muted">
        {t('shell.loadingEditor', { mode: t(`mode.${currentMode}`) })}
      </div>
    {:then mod}
      {@const Editor = mod.default}
      <Editor
        initialFile={session.pending?.file ?? null}
        initialNew={session.pending?.create ?? null}
        onRequestNew={() => guardUnsaved(() => requestNew(currentMode))}
        onRequestOpen={() => guardUnsaved(() => void openFromPicker())}
        onOpenForeign={(f) => openFile(f, false)}
        onOpenFile={(f) => openFile(f)}
        onOpenRecent={(r) => void openRecent(r)}
        confirmUnsaved={guardUnsaved}
        {registerDocument}
        registerMenuHandlers={(h) => (editorMenu = h)}
      />
    {:catch err}
      <div class="flex h-full items-center justify-center bg-pc-bg text-sm text-red-300">
        {t('shell.editorFailed', { mode: t(`mode.${currentMode}`), error: errText(err) })}
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

{#if unsaved.prompt}
  <UnsavedPrompt name={unsaved.prompt.name} onAnswer={(a) => void answerUnsaved(a)} />
{/if}

{#if fileAssocOpen}
  <FileAssocDialog {platform} onDismiss={dismissFileAssoc} />
{/if}

{#if help.about}
  <AboutDialog onClose={closeAbout} />
{/if}

{#if shellError}
  <div
    class="fixed bottom-3 left-3 z-30 flex items-center gap-3 rounded border border-red-900 bg-pc-panel px-3 py-2 text-xs text-red-200 shadow-xl"
    role="alert"
  >
    <span>{shellError}</span>
    <button class="panel-btn" onclick={() => (shellError = null)}>{t('shell.dismiss')}</button>
  </div>
{/if}
