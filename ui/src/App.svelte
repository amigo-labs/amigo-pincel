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
    guardDocument,
    guardUnsaved,
    hasUnsaved,
    promptBusy,
    registerDocument,
    unsaved,
  } from './lib/shell/unsaved.svelte';
  import { loadAutoNew } from './lib/shell/auto-new.svelte';
  import { detectMode, launch, session } from './lib/shell/session.svelte';
  import type { EditorMode, EditorProps, EditorSession, NewDocParams } from './lib/shell/types';
  import {
    droppedHandles,
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
  // The mounted editor's tabs: same-mode documents open there.
  let editorSession: EditorSession | null = null;

  const currentMode = $derived<EditorMode>(session.mode === 'image' ? 'image' : 'pixel');

  function errText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function requestNew(mode: EditorMode) {
    if (promptBusy()) return;
    newDialog = mode;
  }

  // The mounted editor's mode, or `null` on the start screen.
  function mountedMode(): EditorMode | null {
    return session.mode === 'start' || editorSession === null ? null : session.mode;
  }

  // Create in the mounted editor's mode adds a tab; another mode replaces
  // the session after the unsaved-changes prompt (document-tabs AC-02,
  // AC-10 – AC-12, app-shell AC-08).
  function createNew(params: NewDocParams) {
    newDialog = null;
    if (params.mode === mountedMode()) {
      editorSession?.create(params);
      return;
    }
    void guardUnsaved(() => launch(params.mode, { file: null, create: params }));
  }

  // The mode an already-read file belongs to, or `null` (with its error
  // shown) when it is not a supported format.
  function modeOf(opened: OpenedFile): EditorMode | null {
    const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
    if (format === null) {
      shellError = t('shell.unsupported', { name: opened.name });
      return null;
    }
    return detectMode(format);
  }

  // Route an already-read file: a document of the mounted editor's mode
  // opens as a tab without asking (document-tabs AC-02, AC-03); another
  // mode's document replaces the session once the unsaved-changes prompt
  // allows it. Its format is recognized first, so only a file that can be
  // opened asks (unsaved-changes-prompt AC-01, AC-11; document-tabs AC-12).
  // Ignored while the prompt is busy (unsaved-changes-prompt AC-08).
  async function openFile(opened: OpenedFile): Promise<void> {
    if (promptBusy()) return;
    shellError = null;
    const mode = modeOf(opened);
    if (mode === null) return;
    if (mode === mountedMode() && editorSession) {
      await editorSession.open(opened);
      return;
    }
    await guardUnsaved(() => launch(mode, { file: opened, create: null }));
  }

  function registerSession(s: EditorSession): () => void {
    editorSession = s;
    return () => {
      if (editorSession === s) editorSession = null;
    };
  }

  // The last tab closed, or the editor could not open its first document:
  // back to the start screen (document-tabs AC-09, edge cases).
  function showStart(error?: string) {
    if (error) shellError = error;
    editorSession = null;
    session.pending = null;
    session.mode = 'start';
  }

  async function openFromPicker() {
    if (busy || promptBusy()) return;
    busy = true;
    try {
      const opened = await pickAndOpen();
      if (opened) await openFile(opened);
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
      await openFile({ name, bytes, mime: mimeFromName(name), handle: null, path });
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
        await openFile({ name: r.name, bytes, mime: mimeFromName(r.name), handle: null, path: r.path });
      } else if (r.handle) {
        if (!(await ensureReadPermission(r.handle))) {
          shellError = t('shell.permissionDenied', { name: r.name });
          return;
        }
        const file = await r.handle.getFile();
        const bytes = new Uint8Array(await file.arrayBuffer());
        await openFile({ name: file.name, bytes, mime: file.type, handle: r.handle, path: null });
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
    if (bytes) await openFile(pastedImageFile(bytes, system?.type ?? 'image/png'));
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
    const files = Array.from(e.dataTransfer?.files ?? []);
    if (files.length === 0) return;
    e.preventDefault();
    // Read the handles now: the drop's items are gone after this event.
    void onDropFiles(files, droppedHandles(e.dataTransfer));
  }

  // A dropped file with its File System Access handle, if the browser
  // gave one: that handle makes it the same file as an open tab's
  // (document-tabs AC-03).
  async function readDropped(
    file: File,
    handle: FileSystemFileHandle | null = null,
  ): Promise<OpenedFile> {
    const bytes = new Uint8Array(await file.arrayBuffer());
    return { name: file.name, bytes, mime: file.type, handle, path: null };
  }

  // A file dropped on the start screen.
  async function onDropFile(file: File) {
    if (busy || promptBusy()) return;
    busy = true;
    try {
      await openFile(await readDropped(file));
    } catch (err) {
      shellError = t('shell.dropFailed', { error: errText(err) });
    } finally {
      busy = false;
    }
  }

  // Files dropped onto an open editor (document-tabs AC-19): each one of
  // the mounted editor's mode opens as a tab, in drop order; the other
  // mode's are skipped with a notice, unless every file is of the other
  // mode — then the first one replaces the session.
  async function onDropFiles(files: File[], handles: Promise<(FileSystemFileHandle | null)[]>) {
    if (busy || promptBusy()) return;
    busy = true;
    try {
      const found = await handles;
      const read = await Promise.all(files.map((f, i) => readDropped(f, found[i] ?? null)));
      shellError = null;
      const own: OpenedFile[] = [];
      const other: OpenedFile[] = [];
      for (const opened of read) {
        const mode = modeOf(opened);
        if (mode !== null) (mode === mountedMode() ? own : other).push(opened);
      }
      const unsupported = shellError;
      for (const opened of own) await openFile(opened);
      const first = other[0];
      if (own.length === 0 && first) {
        other.shift();
        await openFile(first);
      }
      if (other.length > 0) shellError = t('shell.dropSkipped', { n: other.length });
      else if (unsupported !== null) shellError = unsupported;
    } catch (err) {
      shellError = t('shell.dropFailed', { error: errText(err) });
    } finally {
      busy = false;
    }
  }

  // Warn before the page closes or reloads while any tab has unsaved
  // changes (app-shell AC-15, document-tabs AC-13).
  function onBeforeUnload(e: BeforeUnloadEvent) {
    if (session.mode === 'start' || !hasUnsaved()) return;
    e.preventDefault();
    // Chrome still requires returnValue to be set.
    e.returnValue = '';
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
        'menu:new': () => requestNew(currentMode),
        'menu:open': () => void openFromPicker(),
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
  onbeforeunload={onBeforeUnload}
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
        onRequestNew={() => requestNew(currentMode)}
        onRequestOpen={() => void openFromPicker()}
        onOpenForeign={(f) => void openFile(f)}
        onOpenFile={(f) => void openFile(f)}
        onOpenRecent={(r) => void openRecent(r)}
        confirmClose={guardDocument}
        {registerDocument}
        registerSession={registerSession}
        onEmpty={showStart}
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
  <UnsavedPrompt names={unsaved.prompt.names} onAnswer={(a) => void answerUnsaved(a)} />
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
