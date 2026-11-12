<script lang="ts">
  // App shell: start screen, New-document dialog, file → mode routing, the
  // tab bar of both modes and the editors behind it (docs/specs/pincel.md
  // §1.1 "Two modes", §9.2; docs/specs/document-tabs.md). At most one editor
  // per mode is mounted — a lazily imported chunk that brings its own wasm
  // module, fetched with the first document of its mode — and only the
  // active tab's editor is shown and receives input.
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import AboutDialog from './lib/components/AboutDialog.svelte';
  import ShortcutsDialog from './lib/components/ShortcutsDialog.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import ComfyUiDialog from './lib/comfyui/ComfyUiDialog.svelte';
  import { closeImageGeneration, comfy } from './lib/comfyui/state.svelte';
  import {
    closeAbout,
    closePalette,
    closeShortcuts,
    help,
    helpDialogOpen,
    loadUpdaterState,
  } from './lib/help/state.svelte';
  import FileAssocDialog from './lib/components/FileAssocDialog.svelte';
  import RecoveryDialog from './lib/components/RecoveryDialog.svelte';
  import NewDocumentDialog from './lib/shell/NewDocumentDialog.svelte';
  import StartScreen from './lib/shell/StartScreen.svelte';
  import ClipboardOffer from './lib/shell/ClipboardOffer.svelte';
  import DropDialog from './lib/shell/DropDialog.svelte';
  import TooltipLayer from './lib/shell/TooltipLayer.svelte';
  import { dropChoice, loadDropChoice, setDropChoice } from './lib/shell/drop-choice.svelte';
  import { loadTouchGestures } from './lib/shell/touch-gestures.svelte';
  import { planDrop, type DroppedImage } from './lib/shell/editor-drop';
  import { watchFileDrops, type DroppedFile } from './lib/fs/drop';
  import {
    checkClipboard,
    dismissOffer,
    offer,
    takeOffer,
  } from './lib/shell/clipboard-offer.svelte';
  import { encodeRgbaPng } from './lib/encode-png';
  import UnsavedPrompt from './lib/shell/UnsavedPrompt.svelte';
  import type { TabBarProps } from './lib/components/layout/TabBar.svelte';
  import { requestClose, wireCloseGuard } from './lib/shell/close-guard';
  import {
    answerUnsaved,
    guardDocument,
    hasUnsaved,
    promptBusy,
    unsaved,
  } from './lib/shell/unsaved.svelte';
  import { loadAutoNew } from './lib/shell/auto-new.svelte';
  import Tour from './lib/tour/Tour.svelte';
  import { loadTourPrefs, startTour } from './lib/tour/tour-state.svelte';
  import {
    activateDocument,
    activateTab,
    addTab,
    detectMode,
    openDocuments,
    session,
    shownMode,
    stepTab,
    tabByKey,
    tabScroll,
  } from './lib/shell/session.svelte';
  import {
    editors,
    menuHandlers,
    openIn,
    registerSession,
    releaseIfEmpty,
    serial,
    setEditorLoaders,
    setMenuHandlers,
  } from './lib/shell/editors.svelte';
  import type { DropTarget, EditorDocument, EditorMode, NewDocParams } from './lib/shell/types';
  import {
    ensureReadPermission,
    formatFromName,
    hasFsAccess,
    mimeFromName,
    pickAndOpen,
    sameFile,
    sniffFormat,
    takeLaunchFiles,
    type OpenedFile,
  } from './lib/fs';
  import type { AutosaveSnapshotMeta } from './lib/idb/autosave';
  import { isIdbAvailable } from './lib/idb/db';
  import { RecoveryOffer } from './lib/idb/autosave-session.svelte';
  import { getPref, setPref } from './lib/idb/prefs';
  import { listRecents, onRecentsChanged, type RecentFile } from './lib/idb/recent-files';
  import { forgetRecent, listPresentRecents, recentIsGone } from './lib/fs/recents';
  import { syncMenuAccelerators, syncRecentMenu, wireNativeMenu } from './lib/menu';
  import { isTauri } from './lib/platform';
  import { startLiveHost } from './lib/agent/live-host';
  import AgentsDialog from './lib/agent/AgentsDialog.svelte';
  import { agentAccess, startAgentAccess } from './lib/agent/access.svelte';
  import { t } from './lib/i18n/index.svelte';
  import {
    clipboardImage,
    imageSize,
    newImageBytes,
    pasteEventImage,
    pastedImageFile,
  } from './lib/clipboard';
  import { actionFor, hint, isTypingTarget, nativeAccelerators } from './lib/shell/shortcuts';
  import { initLocale } from './lib/i18n/locale';
  import { initMotion } from './lib/motion.svelte';
  import { loadKeymap } from './lib/shell/keymap';
  import { supportsWasmSimd } from './lib/platform/wasm-simd';

  // Dynamic imports keep each editor (and its wasm package) in its own
  // chunk; Vite only fetches the ones that get mounted (AC-22). Both cores
  // need wasm SIMD: without it no editor loads, whichever way a document
  // arrives (performance-build-simd AC-07).
  const wasmSimd = supportsWasmSimd();
  const unsupported = (): Promise<never> =>
    Promise.reject(new Error(t('start.unsupportedBrowser')));
  setEditorLoaders({
    pixel: () => (wasmSimd ? import('./modes/pixel/PixelEditor.svelte') : unsupported()),
    image: () => (wasmSimd ? import('./modes/image/ImageEditor.svelte') : unsupported()),
  });

  const MODES: readonly EditorMode[] = ['pixel', 'image'];
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
  // True while the file picker is open.
  let picking = $state(false);
  let shellError = $state<string | null>(null);
  let fileAssocOpen = $state(false);
  // True until the "open like this on startup" pref is read, so the start
  // screen doesn't flash before an autostarted document replaces it.
  let booting = $state(true);
  // How each mounted editor takes dropped files (file-access AC-20 -
  // AC-28); a drop goes to the shown one.
  const dropTargets: Record<EditorMode, DropTarget | null> = { pixel: null, image: null };
  // A file drag is over the window and a drop would be taken now.
  let dropHover = $state(false);
  // Dropped images awaiting "Add as layer" / "Open as document".
  let dropDialog = $state<{
    images: DroppedImage[];
    notices: string[];
    canAddLayers: boolean;
  } | null>(null);
  // Dropped images are being added as layers, one after another.
  let addingLayers = false;

  // Autosave recovery (document-tabs AC-15, autosave-recovery AC-04): the
  // snapshots of both modes, offered when the first editor starts; Recover
  // opens one as a tab of its mode, loading that mode's editor if needed.
  const recovery = new RecoveryOffer({
    busy: () => false,
    open: (bytes, meta) => recoverSnapshot(bytes, meta),
    report: (action, error) => {
      shellError = t('status.failed', {
        action: t(action === 'recover' ? 'act.recover' : 'act.discard'),
        error,
      });
    },
  });
  const recoveryOpen = $derived(recovery.snapshots.length > 0);

  const activeMode = $derived(shownMode());
  const showStart = $derived(session.tabs.length === 0 && editors.loading === null);
  // A modal dialog over the start screen hides the clipboard card (AC-32).
  const modalOpen = $derived(
    newDialog !== null ||
      recoveryOpen ||
      unsaved.prompt !== null ||
      fileAssocOpen ||
      dropDialog !== null ||
      helpDialogOpen(),
  );

  /** The tab bar both editors draw (document-tabs AC-01). */
  const tabBar: TabBarProps = $derived({
    tabs: session.tabs.map((tab) => ({
      id: tab.key,
      mode: tab.mode,
      name: tab.doc.name(),
      dirty: tab.doc.dirty(),
    })),
    active: session.active?.key ?? null,
    closeHint: hint('closeTab'),
    scroll: tabScroll,
    onSelect: (key) => {
      const tab = tabByKey(key);
      if (tab && tab !== session.active) activateTab(tab);
    },
    onClose: (key) => tabByKey(key)?.doc.close(),
  });

  function errText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function requestNew(mode: EditorMode) {
    if (promptBusy()) return;
    newDialog = mode;
  }

  // The first tab of a page load starts the recovery offer.
  $effect(() => {
    if (session.tabs.length > 0) recovery.probe();
  });

  // Show the error of a document that could not be opened, unless the
  // editor it was meant for is shown and reports it itself.
  function reportOpenFailure(mode: EditorMode, name: string, error: string): void {
    if (activeMode === mode) return;
    shellError = t('shell.couldNotOpen', { name, error });
  }

  // Create opens a new tab of the chosen mode (document-tabs AC-02,
  // app-shell AC-08).
  function createNew(params: NewDocParams) {
    newDialog = null;
    void serial(async () => {
      const failed = await openIn(params.mode, (s) => s.create(params));
      if (failed !== null) {
        const name = t('shell.newDocument', { mode: t(`mode.${params.mode}`) });
        reportOpenFailure(params.mode, name, failed);
      }
    });
  }

  // The mode an already-read file belongs to, or its error when it is not
  // a supported format.
  function modeOf(opened: OpenedFile): EditorMode | { error: string } {
    const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
    if (format === null) return { error: t('shell.unsupported', { name: opened.name }) };
    return detectMode(format);
  }

  /** The open tab whose file `opened` is (document-tabs AC-03). */
  async function tabOfFile(opened: OpenedFile): Promise<EditorDocument | null> {
    for (const doc of openDocuments()) {
      if (await sameFile(doc.location(), opened)) return doc;
    }
    return null;
  }

  // Route an already-read file (inside a `serial` job): the tab holding the
  // same file becomes active (AC-03), else it opens as a new tab of `mode`,
  // or of the mode its format belongs to (AC-02). Resolves `true` once it
  // is open; a failure is shown unless `quiet` (a drop of several files
  // counts them instead, AC-19). Ignored with a notice while the
  // unsaved-changes prompt is open (unsaved-changes-prompt AC-08).
  async function routeFile(
    opened: OpenedFile,
    opts: { mode?: EditorMode; quiet?: boolean } = {},
  ): Promise<boolean> {
    if (promptBusy()) {
      shellError = t('shell.ignoredPrompt', { name: opened.name });
      return false;
    }
    const same = await tabOfFile(opened);
    if (same) {
      activateDocument(same);
      return true;
    }
    const mode = opts.mode ?? modeOf(opened);
    if (typeof mode !== 'string') {
      if (!opts.quiet) shellError = mode.error;
      return false;
    }
    if (!(await psdReadable(opened, opts.quiet === true))) return false;
    const failed = await openIn(mode, (s) => s.open(opened));
    if (failed === null) return true;
    if (!opts.quiet) reportOpenFailure(mode, opened.name, failed);
    return false;
  }

  // A PSD is checked before its tab opens, so a file the Image editor
  // cannot open (PSB, 16-bit, CMYK, too large, broken) leaves the tabs as
  // they were (docs/specs/psd-import.md AC-08 – AC-10). Other formats pass.
  async function psdReadable(opened: OpenedFile, quiet: boolean): Promise<boolean> {
    if (sniffFormat(opened.bytes) !== 'psd') return true;
    try {
      const { preparePsd } = await import('./modes/image/core/psd-open');
      await preparePsd(opened.bytes);
      return true;
    } catch (err) {
      if (!quiet) shellError = t('shell.openFailed', { error: errText(err) });
      return false;
    }
  }

  /** Opens a bundled sample through the normal open path; it is precached
   *  with the app, so it opens offline too (first-run-tutorial AC-13 – AC-15). */
  async function openSample(sample: 'sprite' | 'image'): Promise<void> {
    const name = sample === 'sprite' ? 'slime.aseprite' : 'postcard.ora';
    try {
      const res = await fetch(`${import.meta.env.BASE_URL}samples/${name}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const bytes = new Uint8Array(await res.arrayBuffer());
      const mime = sample === 'sprite' ? 'application/x-aseprite' : 'image/openraster';
      openFile({ name, bytes, mime, handle: null, path: null });
    } catch (err) {
      shellError = t('shell.openFailed', { error: errText(err) });
    }
  }

  function openFile(opened: OpenedFile, mode?: EditorMode): void {
    void serial(async () => {
      await routeFile(opened, mode === undefined ? {} : { mode });
    });
  }

  // A recovered snapshot always opens as a new tab (AC-03, AC-15).
  function recoverSnapshot(bytes: Uint8Array, meta: AutosaveSnapshotMeta): Promise<void> {
    const mode = meta.mode ?? 'pixel';
    return new Promise<void>((resolve, reject) => {
      void serial(async () => {
        const failed = await openIn(mode, async (s) => {
          await s.recover(bytes, meta);
          return null;
        });
        if (failed === null) resolve();
        else reject(new Error(failed));
      });
    });
  }

  async function openFromPicker() {
    if (picking || promptBusy()) return;
    picking = true;
    let opened: OpenedFile | null = null;
    try {
      opened = await pickAndOpen();
    } catch (err) {
      shellError = t('shell.openFailed', { error: errText(err) });
    } finally {
      picking = false;
    }
    if (opened) openFile(opened);
  }

  async function readPath(path: string): Promise<Uint8Array> {
    const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', { path });
    return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
  }

  // Tauri: file-association double-click, CLI arg, single-instance forward.
  // Handed-over files open in the order they arrive (AC-24).
  function openByPath(path: string) {
    void serial(async () => {
      try {
        const bytes = await readPath(path);
        const name = path.replace(/^.*[/\\]/, '');
        await routeFile({ name, bytes, mime: mimeFromName(name), handle: null, path });
      } catch (err) {
        shellError = t('shell.openFailed', { error: errText(err) });
      }
    });
  }

  function openRecent(r: RecentFile) {
    void serial(async () => {
      try {
        if (tauriHost && r.path) {
          const bytes = await readPath(r.path);
          await routeFile({
            name: r.name,
            bytes,
            mime: mimeFromName(r.name),
            handle: null,
            path: r.path,
          });
        } else if (r.handle) {
          if (!(await ensureReadPermission(r.handle))) {
            shellError = t('shell.permissionDenied', { name: r.name });
            return;
          }
          const file = await r.handle.getFile();
          const bytes = new Uint8Array(await file.arrayBuffer());
          await routeFile({ name: file.name, bytes, mime: file.type, handle: r.handle, path: null });
        } else {
          shellError = t('shell.noHandle', { name: r.name });
        }
      } catch (err) {
        if (await recentIsGone(r, err)) {
          await forgetRecent(r);
          shellError = t('shell.recentMissing', { name: r.name });
        } else {
          shellError = t('shell.recentFailed', { name: r.name, error: errText(err) });
        }
      }
    });
  }

  // Ctrl/⌘+V (or Ctrl/⌘+Alt+V) on the start screen opens the clipboard
  // image as a new document (copy-cut-paste AC-22, AC-26). An empty paste
  // stays silent: it is often a stray key with text on the clipboard.
  function startScreenIdle(target: EventTarget | null): boolean {
    if (
      !showStart ||
      editors.pending > 0 ||
      offer.opening ||
      clipOpening ||
      newDialog !== null ||
      helpDialogOpen()
    ) {
      return false;
    }
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
    const action = actionFor(e);
    if (action === null || !startScreenIdle(e.target)) return;
    // Ctrl/⌘+N where the page gets it (power-user-shortcuts-coverage AC-20).
    if (action === 'newDocument') {
      e.preventDefault();
      requestNew('pixel');
      return;
    }
    if (action !== 'pasteNewImage') return;
    e.preventDefault();
    void clipboardImage().then(openPasted);
  }

  // The desktop clipboard offer (copy-cut-paste AC-27 – AC-32): read when
  // the start screen appears and on window focus while it shows; a tab
  // opening hides the card and drops the reads in flight.
  const offerEligible = (): boolean => tauriHost && showStart && !booting;

  $effect(() => {
    if (!tauriHost) return;
    if (showStart && !booting) untrack(() => void checkClipboard(offerEligible));
    else dismissOffer();
  });

  // "Open from clipboard" › "As pixel sprite" / "As image" (copy-cut-paste
  // AC-34 – AC-38): the image Paste as New Image would read opens in
  // `mode`; no image — none, refused, unsupported or undecodable, and no
  // in-app copy — is a notice. While one runs (the read may wait on the
  // browser's prompt, then the document is created) the buttons, Ctrl/⌘+V
  // and the card's Open open nothing more.
  let clipOpening = $state(false);

  async function openClipboardAs(mode: EditorMode): Promise<void> {
    if (clipOpening || offer.opening) return;
    clipOpening = true;
    try {
      let system = await clipboardImage();
      if (system) {
        const size = await imageSize(system);
        if (!size || size.width === 0 || size.height === 0) system = null;
      }
      const bytes = await newImageBytes(system);
      if (!bytes) {
        shellError = t('paste.noImage');
        return;
      }
      await serial(async () => {
        await routeFile(pastedImageFile(bytes, system?.type ?? 'image/png'), { mode });
      });
    } finally {
      clipOpening = false;
    }
  }

  // Open takes the card's image, not a new read (AC-29), and opens it as
  // Ctrl/⌘+V on the start screen does (AC-22).
  async function openOffer(): Promise<void> {
    if (offer.opening || clipOpening) return;
    const image = takeOffer();
    if (!image) return;
    offer.opening = true;
    try {
      const png = await encodeRgbaPng(image.rgba, image.width, image.height);
      await serial(async () => {
        await routeFile(pastedImageFile(png, 'image/png'));
      });
    } catch (err) {
      shellError = t('shell.couldNotOpen', { name: t('paste.newImageName'), error: errText(err) });
    } finally {
      offer.opening = false;
    }
  }

  // Shows `lines` (one per file or event) in the shell's notice.
  function notify(lines: readonly string[]): void {
    if (lines.length > 0) shellError = lines.join('\n');
  }

  // Whether a file drop would be taken now: on the start screen and while
  // the first editor loads (the files queue up, document-tabs AC-24); on
  // an editor only while its document is loaded and no dialog is open
  // (file-access AC-27). Under the unsaved-changes prompt a drop is taken
  // only to be refused with a notice (unsaved-changes-prompt AC-08).
  function dropTaken(): boolean {
    if (promptBusy()) return true;
    if (picking || modalOpen) return false;
    if (activeMode === null) return true;
    return !addingLayers && (dropTargets[activeMode]?.accepts() ?? false);
  }

  function onDropHover(active: boolean): boolean {
    const taken = dropTaken();
    dropHover = active && taken && !promptBusy();
    return taken;
  }

  // Dropped files on the start screen, or a layered file (Aseprite,
  // OpenRaster, PSD) among them on an editor: each opens as a tab of its
  // own mode, in drop order (document-tabs AC-19, file-access AC-24).
  // Raster images dropped on an editor ask first (file-access AC-20).
  function onDropFiles(files: DroppedFile[]): void {
    const target = activeMode === null ? null : dropTargets[activeMode];
    // `routeFile` names each file the open prompt keeps from opening.
    if (target === null || promptBusy()) {
      openDropped(files);
      return;
    }
    const { plan, notices } = planDrop(files);
    if (plan.kind === 'project') {
      openDropped(files);
      return;
    }
    notify(notices);
    if (plan.kind !== 'images') return;
    const canAddLayers = plan.images.every((i) => target.layerFormats.includes(i.format));
    if (dropChoice.value === 'layer' && canAddLayers) {
      void addLayers(target, plan.images, notices);
    } else if (dropChoice.value === 'document') {
      openDropped(files);
    } else {
      dropDialog = { images: plan.images, notices, canAddLayers };
    }
  }

  // Every dropped file opens as a tab, in drop order (document-tabs AC-19,
  // file-access AC-23); no unsaved-changes prompt, since no tab is
  // replaced. Of several files, the unreadable or unsupported ones are
  // skipped with one notice.
  function openDropped(files: readonly DroppedFile[]): void {
    void serial(async () => {
      const several = files.length > 1;
      let skipped = 0;
      for (const d of files) {
        if (!d.file) {
          skipped += 1;
          if (!several) shellError = t('shell.dropFailed', { error: d.error ?? '' });
          continue;
        }
        if (!(await routeFile(d.file, { quiet: several }))) skipped += 1;
      }
      if (several && skipped > 0) shellError = t('shell.dropSkipped', { n: skipped });
    });
  }

  // "Add as layer" (file-access AC-21, AC-26): one file after another into
  // the editor the drop landed on, each asking like a paste when larger
  // than the canvas; a failure names the file and the rest go on.
  async function addLayers(
    target: DropTarget,
    images: readonly DroppedImage[],
    notices: readonly string[],
  ): Promise<void> {
    const mode = activeMode;
    const lines = [...notices];
    addingLayers = true;
    try {
      for (const { file } of images) {
        if (mode === null || dropTargets[mode] !== target) break;
        try {
          await target.addLayer(file);
        } catch (err) {
          lines.push(t('drop.layerFailed', { name: file.name, error: errText(err) }));
          notify(lines);
        }
      }
    } finally {
      addingLayers = false;
    }
  }

  function chooseDrop(action: 'layer' | 'document', dontAsk: boolean): void {
    const pending = dropDialog;
    dropDialog = null;
    if (!pending) return;
    if (dontAsk) setDropChoice(action);
    const target = activeMode === null ? null : dropTargets[activeMode];
    if (action === 'layer' && target) {
      void addLayers(target, pending.images, pending.notices);
    } else {
      openDropped(pending.images.map(({ file }) => ({ name: file.name, file, error: null })));
    }
  }

  // Warn before the page closes or reloads while any tab has unsaved
  // changes (app-shell AC-15, document-tabs AC-13).
  function onBeforeUnload(e: BeforeUnloadEvent) {
    if (!hasUnsaved()) return;
    e.preventDefault();
    // Chrome still requires returnValue to be set.
    e.returnValue = '';
  }

  async function refreshRecents() {
    if (!recentsAvailable) return;
    try {
      recents = await listPresentRecents();
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

  // A save of a tab in the background failed: name it at once (AC-25).
  function reportError(doc: EditorDocument, error: string) {
    if (session.active?.doc === doc) return;
    shellError = t('shell.saveFailed', { name: doc.name(), error });
  }

  // Re-read the recents whenever the start screen shows (an editor may
  // have recorded new ones meanwhile).
  $effect(() => {
    if (showStart) void refreshRecents();
  });

  $effect(() => {
    if (showStart) document.title = 'Pincel';
  });

  // The native macOS menu draws and fires its own accelerators: give it
  // the user's rebound chords (power-user-shortcuts-keymap AC-14).
  $effect(() => {
    if (!tauriHost) return;
    const map = nativeAccelerators();
    syncMenuAccelerators(map).catch((err: unknown) => {
      console.error('syncMenuAccelerators failed', err);
    });
  });

  // While the start screen shows, the shell keeps the native Open Recent
  // submenu in sync; a shown Pixel editor does the same from its list.
  $effect(() => {
    if (!tauriHost || !showStart) return;
    const items = recents.filter((r) => r.path !== null).map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });

  onMount(() => {
    void initLocale();
    void initMotion();
    void loadKeymap();
    void loadDropChoice();
    void loadTouchGestures();
    const stopDrops = watchFileDrops({ hover: onDropHover, drop: onDropFiles });
    // An editor recorded or removed a recent: the start screen's list and
    // the native menu follow (file-access AC-30).
    const stopRecents = onRecentsChanged(() => {
      if (showStart) void refreshRecents();
    });
    let cancelled = false;
    let unlistenOpenFile: UnlistenFn | null = null;
    // Files the OS hands over: each as an `open-file` event once the
    // listener is armed; those from before (a cold start by double-click)
    // wait on the Rust side until asked for (desktop-app AC-04).
    const launchOpened: Promise<void> = tauriHost
      ? listen<string>('open-file', (e) => {
          if (typeof e.payload === 'string') openByPath(e.payload);
        })
          .then((fn) => {
            if (cancelled) {
              fn();
              return;
            }
            unlistenOpenFile = fn;
            return takeLaunchFiles().then((paths) => {
              if (!cancelled) for (const path of paths) openByPath(path);
            });
          })
          .catch((err: unknown) => {
            console.error('open-file listen failed', err);
          })
      : Promise.resolve();
    void loadTourPrefs();
    void Promise.all([loadAutoNew(), launchOpened])
      .then(([params]) => {
        // An OS-handed file that arrived meanwhile wins over the empty document.
        if (!cancelled && params && session.tabs.length === 0 && editors.pending === 0) {
          createNew(params);
        }
      })
      .finally(() => (booting = false));
    let unlistenMenu: UnlistenFn | null = null;
    let unlistenClose: UnlistenFn | null = null;
    let unlistenFocus: UnlistenFn | null = null;
    // MCP clients' tool calls on the open documents (mcp-live-desktop).
    const stopAgentAccess = startAgentAccess();
    const stopLiveHost = tauriHost
      ? startLiveHost({
          openFile: (opened, mode) =>
            new Promise<boolean>((resolve) => {
              void serial(async () => resolve(await routeFile(opened, mode ? { mode } : {})));
            }),
        })
      : null;
    if (tauriHost) {
      // `getCurrentWindow()` throws at once without the window's metadata
      // (a Tauri 1 or partial bridge); starting it inside a promise turns
      // that into a logged rejection instead of aborting the shell's start
      // (desktop-app AC-20).
      Promise.resolve()
        .then(() =>
          getCurrentWindow().onFocusChanged(({ payload: focused }) => {
            if (focused && offerEligible()) void checkClipboard(offerEligible);
          }),
        )
        .then((fn) => {
          if (cancelled) fn();
          else unlistenFocus = fn;
        })
        .catch((err: unknown) => {
          console.error('focus listen failed', err);
        });
      void loadUpdaterState();
      Promise.resolve()
        .then(wireCloseGuard)
        .then((fn) => {
          if (cancelled) fn();
          else unlistenClose = fn;
        })
        .catch((err: unknown) => {
          console.error('close guard failed', err);
        });
      wireNativeMenu({
        'menu:new': () => requestNew(activeMode ?? 'pixel'),
        'menu:open': () => void openFromPicker(),
        'menu:quit': () => requestClose(),
        'menu:tour': () => startTour(activeMode ?? 'start'),
        recent: async (id) => {
          // The native menu only knows the id; look the row up fresh so
          // entries recorded by an editor are found too.
          const rows = recentsAvailable ? await listRecents() : [];
          const r = rows.find((row) => row.id === id);
          if (r) openRecent(r);
        },
        // Only the active tab's editor serves the native menu (AC-21); while
        // the unsaved-changes prompt or the Recovery dialog is open its
        // accelerators (macOS Cmd+Z, Cmd+S, …) reach no editor.
        editor: () => (promptBusy() || recoveryOpen ? null : menuHandlers(activeMode)),
      })
        .then((fn) => {
          if (cancelled) fn();
          else unlistenMenu = fn;
        })
        .catch((err: unknown) => {
          console.error('wireNativeMenu failed', err);
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
      stopRecents();
      stopLiveHost?.();
      stopAgentAccess();
      recovery.stop();
      stopDrops();
      if (unlistenMenu) unlistenMenu();
      if (unlistenOpenFile) unlistenOpenFile();
      if (unlistenClose) unlistenClose();
      if (unlistenFocus) unlistenFocus();
    };
  });
</script>

<svelte:window
  onpaste={onShellPaste}
  onkeydown={onShellKeydown}
  onbeforeunload={onBeforeUnload}
/>

{#if booting && showStart}
  <div class="h-full bg-pc-bg"></div>
{:else if showStart}
  <StartScreen
    {recents}
    busy={picking || editors.pending > 0}
    unsupported={!wasmSimd}
    onCreate={createNew}
    onOpen={() => void openFromPicker()}
    onOpenRecent={(r) => openRecent(r)}
    onOpenSample={(s) => void openSample(s)}
    onOpenClipboard={(m) => void openClipboardAs(m)}
    {dropHover}
  >
    {#snippet clipboardCard()}
      {#if offer.image && !modalOpen && wasmSimd}
        <ClipboardOffer
          image={offer.image}
          busy={picking || editors.pending > 0}
          onOpen={() => void openOffer()}
          onIgnore={dismissOffer}
        />
      {/if}
    {/snippet}
  </StartScreen>
{/if}

<!-- The editors: at most one per mode; only the active tab's is shown and
     takes input (document-tabs AC-21). A hidden editor keeps its layout
     (`invisible`, not `display: none`), so its tabs keep their view. -->
<div class="relative h-full" class:hidden={showStart}>
  {#each MODES as mode (mode)}
    {@const Editor = editors.component[mode]}
    {#if Editor}
      {@const shown = activeMode === mode}
      <div
        class="absolute inset-0"
        class:invisible={!shown}
        class:pointer-events-none={!shown}
        inert={!shown}
        data-editor={mode}
      >
        <Editor
          active={shown && !recoveryOpen}
          {tabBar}
          onStepTab={stepTab}
          onRequestNew={() => requestNew(mode)}
          onRequestOpen={() => void openFromPicker()}
          onOpenForeign={(f) => openFile(f)}
          onOpenFile={(f, m) => openFile(f, m)}
          onOpenRecent={(r) => openRecent(r)}
          confirmClose={guardDocument}
          registerDocument={(doc) => addTab(mode, doc)}
          {activateDocument}
          {reportError}
          registerSession={(s) => registerSession(mode, s)}
          onEmpty={() => releaseIfEmpty(mode)}
          onNotice={(message) => (shellError = message)}
          registerDropTarget={(d) => (dropTargets[mode] = d)}
          registerMenuHandlers={(h) => setMenuHandlers(mode, h)}
        />
      </div>
    {/if}
  {/each}
  {#if activeMode === null && editors.loading !== null}
    <div
      class="absolute inset-0 flex items-center justify-center bg-pc-bg text-sm text-pc-muted"
    >
      {t('shell.loadingEditor', { mode: t(`mode.${editors.loading}`) })}
    </div>
  {/if}
</div>

{#if dropHover && !showStart}
  <div
    class="pointer-events-none fixed inset-0 z-40 flex items-center justify-center border-4 border-dashed border-pc-accent bg-black/40"
  >
    <p class="rounded bg-pc-panel px-4 py-2 text-sm text-pc-text shadow-xl">{t('drop.overlay')}</p>
  </div>
{/if}

{#if dropDialog}
  <DropDialog
    names={dropDialog.images.map((i) => i.file.name)}
    canAddLayers={dropDialog.canAddLayers}
    onChoose={chooseDrop}
    onCancel={() => (dropDialog = null)}
  />
{/if}

{#if agentAccess.dialogOpen}
  <AgentsDialog onClose={() => (agentAccess.dialogOpen = false)} />
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

{#if recoveryOpen}
  <RecoveryDialog
    snapshots={recovery.snapshots}
    errors={recovery.errors}
    onRecover={(snap) => void recovery.recover(snap)}
    onDiscard={(id) => void recovery.discard(id)}
    onDismiss={() => recovery.dismiss()}
  />
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

<Tour />

{#if help.shortcuts}
  <ShortcutsDialog source={help.shortcuts} onClose={closeShortcuts} />
{/if}

{#if help.palette}
  <CommandPalette source={help.palette} onClose={closePalette} />
{/if}

{#if comfy.open}
  <ComfyUiDialog onClose={closeImageGeneration} />
{/if}

{#if shellError}
  <div
    class="fixed bottom-3 left-3 z-30 flex items-center gap-3 rounded border border-red-900 bg-pc-panel px-3 py-2 text-xs text-red-200 shadow-xl"
    role="alert"
  >
    <span class="whitespace-pre-line">{shellError}</span>
    <button class="panel-btn" onclick={() => (shellError = null)}>{t('shell.dismiss')}</button>
  </div>
{/if}

<TooltipLayer />
