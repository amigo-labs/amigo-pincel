<script lang="ts">
  import TourHint from '../../lib/tour/TourHint.svelte';
  import { hexToRgb, toHex } from '../../lib/color';
  import {
    colorDraft,
    loadColorPref,
    loadRecent,
    saveColorPref,
  } from '../../lib/colors/recent.svelte';
  import { onMount, untrack } from 'svelte';
  import { dropWandLink, relinkWand } from './tools/wand-link';
  import type { EditorProps } from '../../lib/shell/types';
  import {
    clampCanvasSize,
    editor,
    isShapeTool,
    live,
    shapeOf,
    tabs,
    tool,
    ui,
    resetColors,
    savePrompt,
    swapColors,
    view,
    viewCommands,
    type ToolKind,
  } from './stores/editor.svelte';
  import { cancelLive, commitLive, editLive, liveToolOf } from './core/live';
  import { printComposite, type PrintPlacement } from './core/print';
  import PrintDialog from './components/dialogs/PrintDialog.svelte';
  import { touchGesturesItem } from '../../lib/shell/touch-gestures.svelte';
  import { imageGenerationItems } from '../../lib/comfyui/state.svelte';
  import { installLiveSync } from './core/live-sync.svelte';
  import { imageFont } from './core/text-font';
  import { initFontSource } from '../../lib/fonts/source.svelte';
  import { closeTextBox, installLiveText } from './core/live-text.svelte';
  import {
    addGroup,
    addLayer,
    applyEffectRegion,
    clearEffectPreview,
    autoCrop,
    cropToSelection,
    deleteLayer,
    documentDpi,
    duplicateLayer,
    flattenImage,
    flipCanvas,
    mergeDown,
    mergeGroup,
    renameLayer,
    mergeVisible,
    effectJobFor,
    showEffectPreview,
    resizeCanvas,
    rotateCanvas,
    rotateLayer90,
    scaleImage,
    selectLayer,
    transformLayer,
    newDocument,
    openBytes,
    pasteImage,
    copyImage,
    exportImage,
    markUnsaved,
    undo,
    redo,
    jumpHistory,
    selectAll,
    deselect,
    deleteSelection,
    fillSelection,
    invertSelection,
    expandSelection,
    contractSelection,
    featherSelection,
    closeSvgExport,
    exportSvg,
    traceSvg,
    type ExportFormat,
  } from './core/controller';
  import { docFormat, fileTarget, parseDocFormat } from './core/document-file';
  import {
    attachTab,
    bindTabHooks,
    detachAllTabs,
    docIdOf,
    focusTab,
    isDirty,
    requestCloseTab,
    runningSave,
    saveTab,
  } from './core/tab-session';
  import SaveDialogs from './components/dialogs/SaveDialogs.svelte';
  import { gestureControl } from './tools/pointer';
  import ImageViewport from './components/canvas/ImageViewport.svelte';
  import { loadViewAids, pointerPixel, setViewAids, viewAids } from './stores/view-aids.svelte';
  import { formatPixels, type ViewUnit } from './view-units';
  import ToolOptions from './components/toolbar/ToolOptions.svelte';
  import ColorsPanel from './components/panels/ColorsPanel.svelte';
  import { loadImagePalette } from './stores/palette.svelte';
  import LayersPanel from './components/panels/LayersPanel.svelte';
  import ExportDialog from './components/dialogs/ExportDialog.svelte';
  import SvgExportDialog from '../../lib/components/SvgExportDialog.svelte';
  import Modal from './components/dialogs/Modal.svelte';
  import {
    clipboardImage,
    imageSize,
    newImageBytes,
    pasteEventImage,
    pastePosition,
    pastedImageFile,
    resolvePaste,
    storeCopy,
  } from '../../lib/clipboard';
  import { core } from './core/wasm';
  import { canDeleteLayer, canMergeDown, editRefused, treeOrder } from './core/layer-tree';
  import ResizeDialog from './components/dialogs/ResizeDialog.svelte';
  import ScaleDialog from './components/dialogs/ScaleDialog.svelte';
  import EffectPanel from '../../lib/components/EffectPanel.svelte';
  import EditorFrame from '../../lib/components/layout/EditorFrame.svelte';
  import HistoryPanel from '../../lib/components/history/HistoryPanel.svelte';
  import IconButton from '../../lib/components/layout/IconButton.svelte';
  import ToolRail from '../../lib/components/layout/ToolRail.svelte';
  import ToolVariants from '../../lib/components/layout/ToolVariants.svelte';
  import { tidy, type MenuDef } from '../../lib/components/layout/menu';
  import { cycleGroup, groupOf, type ToolGroup } from '../../lib/components/layout/tools';
  import { effectMenus } from '../../lib/effects/menu';
  import { effectById, forMode, rgbaOf, type EffectDef } from '../../lib/effects/catalog';
  import { EffectSession, activeEffect } from '../../lib/effects/session.svelte';
  import { effectRun, type EffectRun } from './jobs';
  import {
    OPACITY_ACTIONS,
    actionFor,
    hint,
    isStageTarget,
    isTypingTarget,
    NEW_DOCUMENT_KEY,
    nativeMenuOwns,
    opacityPercent,
    toolGroupForKey,
    toolHint,
    type OpacityAction,
    type ShortcutAction,
  } from '../../lib/shell/shortcuts';
  import { toggleUi } from '../../lib/components/layout/chrome.svelte';
  import {
    formatFromName,
    hasFsAccess,
    mimeFor,
    sameFile,
    sniffFormat,
    type OpenedFile,
  } from '../../lib/fs';
  import { isIdbAvailable } from '../../lib/idb/db';
  import { listRecents, upsertRecent, type RecentFile } from '../../lib/idb/recent-files';
  import type { AutosaveSnapshotMeta } from '../../lib/idb/autosave';
  import type { NewDocParams } from '../../lib/shell/types';
  import type { SaveTarget } from '../../lib/fs';
  import { helpMenus } from '../../lib/help/menu';
  import {
    closeShortcuts,
    helpDialogOpen,
    openPalette,
    openShortcuts,
  } from '../../lib/help/state.svelte';
  import { recentMenuItems } from '../../lib/components/layout/recent-menu';
  import { droppedImagesMenu } from '../../lib/shell/drop-choice.svelte';
  import { syncRecentMenu } from '../../lib/menu';
  import { hasNativeMenu, isTauri } from '../../lib/platform';
  import { languageMenu } from '../../lib/i18n/locale';
  import { reduceMotionMenuItem } from '../../lib/motion.svelte';
  import { locale, t, type MessageKey } from '../../lib/i18n/index.svelte';
  import { effectLabel } from '../../lib/effects/labels';
  import RadiusDialog from '../../lib/components/RadiusDialog.svelte';
  import RenameDialog from '../../lib/components/RenameDialog.svelte';

  // Shell contract (src/App.svelte mounts at most one editor per mode):
  // whether the active tab is this editor's, the shared tab bar, and the
  // callbacks the shell serves — New / Open dialogs, the unsaved-changes
  // prompt, native-menu items.
  let {
    active,
    tabBar,
    onStepTab,
    onRequestNew,
    onRequestOpen,
    onOpenRecent,
    onOpenFile,
    confirmClose,
    registerDocument,
    activateDocument,
    reportError,
    registerSession,
    onEmpty,
    registerDropTarget,
    registerMenuHandlers,
  }: EditorProps = $props();

  let loadError = $state<string | null>(null);
  let jpegDialogOpen = $state(false);
  /** File ▸ Export ▸ SVG… is open (vectorize-dialog). */
  let svgOpen = $state(false);
  let transformDialog = $state<'resize' | 'scale' | null>(null);
  // The open effect (effect-panel spec), shown in the frame's sidebar.
  let effectOpen = $state<EffectSession | null>(null);
  // File ▸ Print… (docs/specs/image-formats-more-print.md).
  let printOpen = $state(false);

  /** Print needs a document and no dialog over the editor (AC-09); the
   *  shell's dialogs are found by their role. */
  function canPrint(): boolean {
    return (
      editor.handle !== null &&
      !ui.modalOpen &&
      !helpDialogOpen() &&
      savePrompt.value === null &&
      document.querySelector('[role="dialog"], [role="alertdialog"]') === null
    );
  }

  /** Opens the Print dialog; `false` when Print is not available, so
   *  Ctrl+P stays the browser's (AC-02, AC-09). */
  function openPrint(): boolean {
    if (!canPrint()) return false;
    printOpen = true;
    return true;
  }

  async function runPrint(placement: PrintPlacement): Promise<void> {
    printOpen = false;
    const handle = editor.handle;
    if (handle === null) return;
    try {
      await printComposite(handle, editor.width, editor.height, placement);
    } catch (e) {
      editor.notice = t('print.failed', { error: e instanceof Error ? e.message : String(e) });
    }
  }
  // Layer ▸ Rename… target while its dialog is open.
  let renaming = $state<{ index: number; name: string } | null>(null);
  // A pasted image larger than the canvas, awaiting Expand / Keep (spec §5.4).
  // `resolve` is set for a dropped file, whose caller places the layer.
  let pasteDialog = $state<{
    handle: number;
    bytes: Uint8Array;
    x: number;
    y: number;
    width: number;
    height: number;
    canExpand: boolean;
    resolve?: (answer: PasteAnswer) => void;
  } | null>(null);

  /** The larger-image dialog's answer; `null` is Cancel. */
  type PasteAnswer = 'expand' | 'keep' | null;

  /** Source tag of the in-app copy record; handles are never reused. */
  const docSource = (handle: number): string => `image:${handle}`;

  /** Dropped formats the browser cannot size: decoded by the core first. */
  const CORE_DECODED: ReadonlySet<string> = new Set([
    mimeFor('tga'),
    mimeFor('dds'),
    mimeFor('ico'),
  ]);

  /** Runs a pixel edit on the active layer unless it is refused — the layer
   *  or a group it sits in is locked, or it is a group — and shows the
   *  locked-layer notice instead (docs/specs/image-layer-groups.md AC-11,
   *  AC-12). `movesContent`: Move / flips / rotations, which a group allows. */
  function layerEdit(movesContent: boolean, run: () => void): void {
    const layer = editor.layers[editor.activeLayer];
    if (layer && editRefused(editor.layers, editor.activeLayer, movesContent)) {
      editor.notice = t('layers.locked', { name: layer.name });
      return;
    }
    editor.notice = null;
    run();
  }

  // Edit ▸ Copy / Copy Merged / Cut (spec §5.4): the PNG goes to the system
  // clipboard from this click / keydown (user activation) and into the
  // in-app record, which also keeps the canvas position for paste-in-place.
  function runCopy(kind: 'copy' | 'merged' | 'cut'): void {
    const handle = editor.handle;
    if (handle === null) return;
    // Copy reads the final pixels: commit a live object first (AC-09).
    commitLive();
    let copied: { png: Uint8Array; x: number; y: number } | null;
    try {
      copied = copyImage(kind);
    } catch (e) {
      editor.notice = t('copy.failed', { error: String(e) });
      return;
    }
    if (!copied) return;
    editor.notice = null;
    void storeCopy({ ...copied, source: docSource(handle), key: core.imageKey(copied.png) });
  }

  // Edit ▸ Paste / Ctrl+V (spec §5.4): a new layer at the copy origin when
  // Pincel's own copy goes back into its document, else at the top-left of
  // the visible viewport; an image larger than the canvas asks first whether
  // to grow the canvas, like paint.net. `handle` is the document the paste
  // was asked for: reading the clipboard and decoding are async, and a
  // document opened meanwhile (or another editor mounted) must not receive
  // it. Handles are never reused.
  async function beginPaste(system: Blob | null, handle: number): Promise<void> {
    const src = await resolvePaste(system, docSource(handle), core.imageKey);
    if (editor.handle !== handle) return;
    if (!src) {
      editor.notice = t('paste.noImage');
      return;
    }
    const { bytes } = src;
    const size = await imageSize(new Blob([bytes.slice()], { type: system?.type || 'image/png' }));
    if (editor.handle !== handle) return;
    const canvas = { width: editor.width, height: editor.height };
    const at =
      src.origin ??
      (size
        ? pastePosition({ x: -view.panX / view.zoom, y: -view.panY / view.zoom }, size, canvas)
        : { x: 0, y: 0 });
    // A format the browser can't measure still goes to the core's decoder.
    if (!size || (size.width <= editor.width && size.height <= editor.height)) {
      runPaste(handle, bytes, at.x, at.y, false);
      return;
    }
    const w = Math.max(editor.width, at.x + size.width);
    const h = Math.max(editor.height, at.y + size.height);
    const [cw, ch] = clampCanvasSize(w, h);
    pasteDialog = {
      handle,
      bytes,
      x: at.x,
      y: at.y,
      width: size.width,
      height: size.height,
      canExpand: cw === w && ch === h,
    };
  }

  /** Closes the larger-image dialog with the user's answer. */
  function answerPaste(answer: PasteAnswer): void {
    const p = pasteDialog;
    if (!p) return;
    pasteDialog = null;
    if (p.resolve) p.resolve(answer);
    else if (answer !== null) runPaste(p.handle, p.bytes, p.x, p.y, answer === 'expand');
  }

  // A raster file dropped onto the editor (docs/specs/file-access.md
  // AC-21): a new layer placed like a paste of another image, one undo
  // step. Resolves once added or skipped (the larger-image dialog's
  // Cancel); throws when the core cannot decode it.
  async function pasteFile(bytes: Uint8Array, mime: string): Promise<void> {
    const handle = editor.handle;
    if (handle === null) return;
    // TGA / DDS / ICO go through the core's decoder as PNG, so sizing and
    // the expand prompt work as for the browser's own formats
    // (docs/specs/image-formats-more.md AC-13).
    if (CORE_DECODED.has(mime)) {
      bytes = core.imageToPng(bytes, mime);
      mime = 'image/png';
    }
    const size = await imageSize(new Blob([bytes.slice()], { type: mime || 'image/png' }));
    if (editor.handle !== handle) return;
    const canvas = { width: editor.width, height: editor.height };
    const at = size
      ? pastePosition({ x: -view.panX / view.zoom, y: -view.panY / view.zoom }, size, canvas)
      : { x: 0, y: 0 };
    let expand = false;
    if (size && (size.width > canvas.width || size.height > canvas.height)) {
      const w = Math.max(canvas.width, at.x + size.width);
      const h = Math.max(canvas.height, at.y + size.height);
      const [cw, ch] = clampCanvasSize(w, h);
      const answer = await new Promise<PasteAnswer>((resolve) => {
        pasteDialog = {
          handle,
          bytes,
          x: at.x,
          y: at.y,
          width: size.width,
          height: size.height,
          canExpand: cw === w && ch === h,
          resolve,
        };
      });
      if (answer === null || editor.handle !== handle) return;
      expand = answer === 'expand';
    }
    pasteImage(bytes, at.x, at.y, expand);
    editor.notice = null;
  }

  function runPaste(
    handle: number,
    bytes: Uint8Array,
    x: number,
    y: number,
    expand: boolean,
  ): void {
    pasteDialog = null;
    if (editor.handle !== handle) return;
    try {
      pasteImage(bytes, x, y, expand);
      editor.notice = null;
    } catch (e) {
      editor.notice = t('paste.failed', { error: String(e) });
    }
  }

  async function pasteFromClipboard(): Promise<void> {
    const handle = editor.handle;
    if (handle === null) return;
    const blob = await clipboardImage();
    if (editor.handle !== handle) return;
    await beginPaste(blob, handle);
  }

  // Edit ▸ Paste as New Image (copy-cut-paste AC-23, AC-25).
  async function pasteAsNewImage(): Promise<void> {
    const system = await clipboardImage();
    const bytes = await newImageBytes(system);
    if (!bytes) {
      editor.notice = t('paste.noImage');
      return;
    }
    onOpenFile(pastedImageFile(bytes, system?.type ?? 'image/png'));
  }

  function onPaste(e: ClipboardEvent): void {
    const handle = editor.handle;
    if (handle === null || !active || e.defaultPrevented || ui.modalOpen || helpDialogOpen())
      return;
    if (isTypingTarget(e.target)) return;
    // The shell's dialogs (New document, unsaved changes, …) don't set
    // `ui.modalOpen`; a paste from inside any dialog is not for the canvas.
    if (e.target instanceof Element && e.target.closest('[role="dialog"], [role="alertdialog"]')) {
      return;
    }
    e.preventDefault();
    void beginPaste(pasteEventImage(e), handle);
  }

  /** Image ▸ Auto Crop; a notice when nothing is cropped (image-resize-autocrop AC-15). */
  function runAutoCrop(): void {
    editor.notice = autoCrop() ? null : t('image.nothingToCrop');
  }

  function openSvgExport(): void {
    if (editor.handle === null) return;
    commitLive();
    closeSvgExport();
    svgOpen = true;
  }

  function closeSvg(): void {
    svgOpen = false;
    closeSvgExport();
  }

  function runExport(format: ExportFormat, quality?: number): void {
    commitLive();
    // JPEG is lossy: ask for the quality first (spec §13.2).
    if (format === 'jpeg' && quality === undefined) {
      jpegDialogOpen = true;
      return;
    }
    exportImage(format, quality).catch(
      (e) => (loadError = t('img.exportFailed', { error: String(e) })),
    );
  }

  // Tool rail (spec §9.2): same groups, order and keys as Pixel mode
  // (lib/shell/shortcuts.ts), with the tools Image mode has.
  const toolGroups: ToolGroup<ToolKind>[] = $derived(
    [
      {
        id: 'move',
        label: t('tool.move'),
        tools: [
          { id: 'move', label: t('tool.moveLayer'), icon: 'move' },
          { id: 'move_selection', label: t('tool.moveSelection'), icon: 'move-selection' },
          { id: 'move_mask', label: t('tool.moveMask'), icon: 'move-mask' },
        ],
      },
      {
        id: 'hand',
        label: t('tool.hand'),
        tools: [{ id: 'hand', label: t('tool.hand'), icon: 'hand' }],
      },
      {
        id: 'zoom',
        label: t('tool.zoom'),
        tools: [{ id: 'zoom', label: t('tool.zoom'), icon: 'zoom-in' }],
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
        id: 'recolor',
        label: t('tool.recolor'),
        tools: [{ id: 'recolor', label: t('tool.recolor'), icon: 'recolor' }],
      },
      {
        id: 'smudge',
        label: t('tool.smudge'),
        tools: [{ id: 'smudge', label: t('tool.smudge'), icon: 'smudge' }],
      },
      {
        id: 'clone',
        label: t('tool.clone'),
        tools: [{ id: 'clone', label: t('tool.clone'), icon: 'clone-stamp' }],
      },
      {
        id: 'fill',
        label: t('tool.fill'),
        tools: [{ id: 'fill', label: t('tool.fill'), icon: 'bucket' }],
      },
      {
        id: 'gradient',
        label: t('tool.gradient'),
        tools: [{ id: 'gradient', label: t('tool.gradient'), icon: 'gradient' }],
      },
      {
        id: 'eyedropper',
        label: t('tool.eyedropper'),
        tools: [{ id: 'eyedropper', label: t('tool.eyedropper'), icon: 'eyedropper' }],
      },
      {
        id: 'line',
        label: t('tool.lineCurve'),
        tools: [{ id: 'line', label: t('tool.lineCurve'), icon: 'line' }],
      },
      {
        id: 'shapes',
        label: t('tool.shapes'),
        tools: [
          { id: 'rectangle', label: t('tool.rectangle'), icon: 'rect' },
          { id: 'rounded_rect', label: t('tool.roundedRect'), icon: 'rounded-rect' },
          { id: 'ellipse', label: t('tool.ellipse'), icon: 'ellipse' },
          { id: 'polygon', label: t('tool.polygon'), icon: 'polygon' },
          { id: 'triangle', label: t('tool.triangle'), icon: 'triangle' },
          { id: 'diamond', label: t('tool.diamond'), icon: 'diamond' },
          { id: 'star', label: t('tool.star'), icon: 'star' },
          { id: 'block_arrow', label: t('tool.blockArrow'), icon: 'block-arrow' },
          { id: 'heart', label: t('tool.heart'), icon: 'heart' },
          { id: 'callout', label: t('tool.callout'), icon: 'callout' },
        ],
      },
      {
        id: 'text',
        label: t('tool.text'),
        tools: [{ id: 'text', label: t('tool.text'), icon: 'text' }],
      },
    ].map((g) => ({ ...g, shortcut: toolHint(g.id) }) as ToolGroup<ToolKind>),
  );

  /**
   * Every shared action `runShortcut` (or, for `paste`, the paste event)
   * serves. No frames in Image mode, so no frame keys and no sheet export.
   */
  const IMAGE_ACTIONS: readonly ShortcutAction[] = [
    ...(NEW_DOCUMENT_KEY ? (['newDocument'] as const) : []),
    'open',
    'save',
    'saveAs',
    'export',
    'undo',
    'redo',
    'cut',
    'copy',
    'paste',
    'pasteNewImage',
    'deleteSelection',
    'fillSelection',
    'selectAll',
    'deselect',
    'invertSelection',
    'zoomIn',
    'zoomOut',
    'zoomFit',
    'zoomActual',
    'shortcuts',
    'swapColors',
    'resetColors',
    'brushSmaller',
    'brushLarger',
    'newLayer',
    'duplicateLayer',
    'deleteLayer',
    'mergeDown',
    'layerAbove',
    'layerBelow',
    'flipLayerH',
    'flipLayerV',
    'rotateCw',
    'rotateCcw',
    'toggleUi',
    'togglePixelGrid',
    'toggleRulers',
    'commandPalette',
    ...OPACITY_ACTIONS,
  ];

  // View ▸ Units and the status bar (docs/specs/image-view-aids.md AC-11, AC-12).
  const UNIT_ITEMS: readonly (readonly [ViewUnit, MessageKey])[] = [
    ['px', 'menu.unitPixels'],
    ['in', 'menu.unitInches'],
    ['cm', 'menu.unitCentimeters'],
  ];
  const UNIT_SUFFIX: Record<ViewUnit, MessageKey> = {
    px: 'units.px',
    in: 'units.in',
    cm: 'units.cm',
  };
  // The active document's resolution, read again on every tab switch and
  // document change (AC-19, AC-23).
  const statusDpi = $derived.by(() => {
    void editor.handle;
    void editor.revision;
    return documentDpi();
  });
  function unitValue(p: number): string {
    return formatPixels(p, viewAids.units, statusDpi, locale());
  }
  // The pixel under the pointer through the current view (AC-10, AC-19).
  const statusPointer = $derived(pointerPixel());

  /** Help ▸ Keyboard Shortcuts, listing what this mode binds. */
  function openOverview(): void {
    openShortcuts({ actions: IMAGE_ACTIONS, groups: toolGroups });
  }

  const toolLabel = $derived(
    groupOf(toolGroups, tool.kind)?.tools.find((x) => x.id === tool.kind)?.label ?? '',
  );

  // Select ▸ Expand / Contract / Feather ask for a radius; the core takes
  // one, the options bar used to pin it to 1 / 1 / 2 px.
  type RadiusOp = 'expand' | 'contract' | 'feather';
  let radiusOp = $state<RadiusOp | null>(null);
  const RADIUS_TITLES: Record<RadiusOp, 'menu.expand' | 'menu.contract' | 'menu.feather'> = {
    expand: 'menu.expand',
    contract: 'menu.contract',
    feather: 'menu.feather',
  };

  function openRadius(op: RadiusOp): void {
    radiusOp = op;
  }

  function applyRadius(radius: number): void {
    const op = radiusOp;
    radiusOp = null;
    if (op === 'expand') expandSelection(radius);
    else if (op === 'contract') contractSelection(radius);
    else if (op === 'feather') featherSelection(radius);
  }

  // Recents need something a re-open can use: an FSA handle (web) or a
  // path (Tauri). Same gate as the Pixel editor / the shell.
  const recentsAvailable = (hasFsAccess() || isTauri()) && isIdbAvailable();

  // File ▸ Open Recent. `null` keeps the entry out of the menu where the
  // registry cannot work at all; an empty array shows it disabled.
  let recents = $state<RecentFile[]>([]);

  // The Magic Wand link (docs/specs/image-editing.md AC-33, AC-34): every
  // value the settings take recomputes the linked click; choosing another
  // tool ends the link, even if the wand is chosen again later.
  $effect(() => {
    void tool.tolerance;
    void tool.contiguous;
    void tool.wandSample;
    untrack(relinkWand);
  });
  $effect(() => {
    void tool.kind;
    untrack(dropWandLink);
  });

  // Keep the native Open Recent submenu in step with the recents, as the
  // shell and the Pixel editor do (desktop-app.md AC-19). Only entries
  // with a path can be reopened from the native menu.
  $effect(() => {
    if (!isTauri()) return;
    const items = recents.filter((r) => r.path !== null).map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });

  async function refreshRecents(): Promise<void> {
    if (!recentsAvailable) return;
    try {
      recents = await listRecents();
    } catch (err) {
      console.error('listRecents failed', err);
    }
  }

  async function recordRecent(
    file: Pick<SaveTarget, 'name' | 'handle' | 'path'> | null,
    id: string | null,
  ): Promise<void> {
    if (!recentsAvailable || !file || id === null || (!file.handle && !file.path)) return;
    try {
      await upsertRecent({
        id,
        name: file.name,
        handle: file.handle,
        path: file.path,
        mode: 'image',
      });
      await refreshRecents();
    } catch (err) {
      console.error('upsertRecent failed', err);
    }
  }

  // Save / Save As write back to the opened file in its own format, or the
  // OpenRaster project (save-original-format, core/save-flow.ts); Export
  // stays the flat PNG / JPEG / WebP path. Each tab saves on its own
  // (core/tab-session.ts).
  function save(forceAs = false): Promise<boolean> {
    return saveTab(tabs.active, forceAs);
  }

  // Tabs (docs/specs/document-tabs.md): the shell routes same-mode
  // documents here; each opens as a tab after the last one.

  /** Opens an already-read file as a new tab, or activates the tab of the
   *  same file (AC-03). Resolves `null`, else the error. */
  async function openTabFile(opened: OpenedFile): Promise<string | null> {
    for (const tab of tabs.list) {
      if (await sameFile(fileTarget(tab), opened)) {
        focusTab(tab);
        return null;
      }
    }
    try {
      const tab = await openBytes(opened.bytes, opened.name, opened.mime, opened);
      attachTab(tab);
      loadError = null;
      await recordRecent(opened, docIdOf(tab));
      return null;
    } catch (e) {
      loadError = t('img.openFailed', { error: String(e) });
      return e instanceof Error ? e.message : String(e);
    }
  }

  /** A blank document as a new tab; resolves `null`, else the error. */
  async function createTab(params: NewDocParams): Promise<string | null> {
    try {
      attachTab(await newDocument(params.width, params.height));
      loadError = null;
      return null;
    } catch (e) {
      loadError = t('img.openFailed', { error: String(e) });
      return e instanceof Error ? e.message : String(e);
    }
  }

  /** Open a recovered autosave snapshot as an unsaved tab (AC-15). The
   *  snapshot is OpenRaster; Save keeps writing the format stored with it
   *  (a recovered PNG saves back as PNG). Older snapshots carry none: the
   *  file name's extension stands in. */
  async function recoverTab(bytes: Uint8Array, meta: AutosaveSnapshotMeta): Promise<void> {
    const tab = await openBytes(bytes, meta.name, 'image/openraster', {
      handle: null,
      path: null,
      format: parseDocFormat(meta.format) ?? docFormat(formatFromName(meta.name)) ?? 'ora',
    });
    markUnsaved();
    tab.doc.origin = 'recovered';
    attachTab(tab, { docId: meta.docId, recovered: true });
  }

  const FG_PREF = 'lastColor:image';
  const BG_PREF = 'lastBgColor:image';
  // The last foreground / background colors and the recent colors, per
  // mode in IndexedDB (color-picker AC-15); saving starts once they loaded,
  // so a slow store cannot overwrite them with the defaults, and happens
  // once per picker interaction: a drag's live colors wait for its end.
  let colorPrefsLoaded = $state(false);
  $effect(() => {
    const fg = { ...hexToRgb(tool.foreground), a: tool.foregroundAlpha };
    const bg = { ...hexToRgb(tool.background), a: tool.backgroundAlpha };
    const draft = colorDraft.image;
    if (!colorPrefsLoaded || draft) return;
    saveColorPref(FG_PREF, fg);
    saveColorPref(BG_PREF, bg);
  });

  onMount(() => {
    // The Text tool's fonts (docs/specs/system-fonts.md).
    void initFontSource();
    void imageFont.load();
    void loadRecent('image');
    void loadImagePalette();
    void loadViewAids();
    void Promise.all([loadColorPref(FG_PREF), loadColorPref(BG_PREF)]).then(([fg, bg]) => {
      if (fg) [tool.foreground, tool.foregroundAlpha] = [toHex(fg), fg.a];
      if (bg) [tool.background, tool.backgroundAlpha] = [toHex(bg), bg.a];
      colorPrefsLoaded = true;
    });
    bindTabHooks({
      registerDocument,
      confirmClose,
      activateDocument,
      reportError,
      onEmpty: () => onEmpty(),
      recordRecent: (target, id) => recordRecent(target, id),
      report: (message) => (loadError = message),
    });
    void refreshRecents();
    // The documents the shell routes here open as tabs (document-tabs
    // AC-02, AC-15); the core loads with the first of them.
    const unregisterSession = registerSession({
      open: openTabFile,
      create: createTab,
      recover: recoverTab,
    });
    registerMenuHandlers({
      'menu:close': () => requestCloseTab(),
      'menu:save': () => void save(),
      'menu:saveAs': () => void save(true),
      'menu:exportPng': () => runExport('png'),
      'menu:exportSvg': openSvgExport,
      'menu:print': () => void openPrint(),
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoomIn': () => viewCommands.zoomIn(),
      'menu:zoomOut': () => viewCommands.zoomOut(),
      'menu:resetZoom': () => viewCommands.fit(),
      'menu:shortcuts': openOverview,
    });
    return () => {
      unregisterSession();
      detachAllTabs();
      // A drop waiting on the larger-image dialog ends with the editor.
      answerPaste(null);
      registerDropTarget(null);
      registerMenuHandlers(null);
      closeShortcuts();
    };
  });

  // Files dropped onto the editor (docs/specs/file-access.md AC-20 –
  // AC-28): the shell asks and opens documents; Image mode adds every
  // raster format its core decodes. The shell keeps one target per mode
  // and hands a drop to the shown editor's; a hidden one refuses it.
  $effect(() => {
    registerDropTarget({
      accepts: () =>
        active &&
        editor.handle !== null &&
        !ui.modalOpen &&
        renaming === null &&
        savePrompt.value === null,
      layerFormats: ['png', 'jpeg', 'webp', 'bmp', 'gif', 'tiff', 'tga', 'dds', 'ico'],
      addLayer: (opened) => {
        // A dropped file often has no MIME type: the content, else the
        // name, tells the format (TGA needs it as the decoder's hint).
        const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
        return pasteFile(opened.bytes, format ? mimeFor(format) : opened.mime);
      },
    });
  });

  // A hidden editor takes no input (AC-21): the canvas' Space pan reads
  // this too. Leaving for another mode's tab drops a gesture in flight.
  $effect.pre(() => {
    ui.active = active;
    if (!active) {
      gestureControl.cancel();
      untrack(commitLive);
    }
  });

  // Option and color changes reach the live object (AC-06, AC-07).
  installLiveSync();
  // The live text's box follows the core (docs/specs/image-live-text.md).
  installLiveText();

  // Another tool commits a live object; another box shape converts a live
  // box shape (image-live-objects-ui AC-08, AC-09).
  $effect(() => {
    const kind = tool.kind;
    untrack(() => {
      const own = liveToolOf();
      if (own === null || own === kind) return;
      const shape = shapeOf(kind);
      if (shape !== null && shape !== 'line' && isShapeTool(own) && own !== 'line') {
        editLive({ shape });
      } else {
        commitLive();
      }
    });
  });

  const dirty = $derived(isDirty(tabs.active));

  // The window title follows the active tab (document-tabs AC-04), as in
  // Pixel mode.
  $effect(() => {
    if (active && editor.handle !== null) {
      document.title = `${dirty ? '● ' : ''}${editor.fileName} – Pincel`;
    }
  });

  // Keyboard map shared with Pixel mode (lib/shell/shortcuts.ts).
  function onKeydown(e: KeyboardEvent): void {
    // Ignore shortcuts while another mode's tab is active (AC-21), while
    // typing in a field (text-entry overlay included) or while a modal
    // dialog owns the keyboard.
    // A key the other editor already handled is not handled again: a tab
    // switch it made shows this editor before the event reaches it.
    // The live text's field passes on only the chords it does not own
    // (docs/specs/image-live-text.md AC-23): they run as on the canvas.
    const inLiveText = e.target instanceof HTMLElement && e.target.dataset.liveText !== undefined;
    if (
      !active ||
      e.defaultPrevented ||
      ui.modalOpen ||
      helpDialogOpen() ||
      (isTypingTarget(e.target) && !inLiveText)
    ) {
      return;
    }
    // Enter commits a live object (image-live-objects-ui AC-09); Image
    // mode has no Play for the key to mean otherwise.
    if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && !e.altKey && live.object !== null) {
      e.preventDefault();
      commitLive();
      return;
    }
    // Enter closes an open polygonal lasso (spec §5.4).
    if (
      e.key === 'Enter' &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey &&
      gestureControl.commitPolygon()
    ) {
      e.preventDefault();
      return;
    }
    const action = actionFor(e);
    // Only a native menu bar fires these as accelerators (spec §9.4);
    // without one the editor has to handle them itself.
    if (action !== null && hasNativeMenu() && nativeMenuOwns(action)) return;
    // An action chord wins over a tool key, so Shift+H flips instead of
    // picking the Hand (power-user-shortcuts-coverage AC-04, AC-07). Tab
    // toggles the UI only on the canvas stage; elsewhere it moves focus.
    if (action !== null) {
      if (action === 'toggleUi' && !isStageTarget(e.target)) return;
      if (runShortcut(action)) e.preventDefault();
      return;
    }
    // Escape drops a gesture in progress; with none it cancels a live
    // object (image-live-objects-ui AC-11), else it deselects
    // (image-editing AC-31).
    if (e.key === 'Escape') {
      if (!gestureControl.cancel() && !cancelLive() && !closeTextBox() && editor.hasSelection) {
        deselect();
      }
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
      case 'newDocument':
        onRequestNew();
        return true;
      case 'open':
        onRequestOpen();
        return true;
      case 'nextTab':
      case 'prevTab':
        onStepTab(action === 'nextTab' ? 1 : -1);
        return true;
      case 'closeTab':
        requestCloseTab();
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
      case 'print':
        return openPrint();
      case 'undo':
        undo();
        return true;
      case 'redo':
        redo();
        return true;
      case 'pasteNewImage':
        void pasteAsNewImage();
        return true;
      case 'copy':
      case 'cut':
        if (editor.handle === null) return false;
        runCopy(action);
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
        layerEdit(false, deleteSelection);
        return true;
      case 'fillSelection':
        layerEdit(false, fillSelection);
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
      case 'shortcuts':
        openOverview();
        return true;
      case 'toggleUi':
        toggleUi();
        return true;
      case 'togglePixelGrid':
        setViewAids({ showPixelGrid: !viewAids.showPixelGrid });
        return true;
      case 'toggleRulers':
        setViewAids({ showRulers: !viewAids.showRulers });
        return true;
      case 'commandPalette':
        // Ctrl+K: the command palette over these menus and tools.
        openPalette({
          menus: () => menus,
          groups: () => toolGroups,
          selectTool: (id) => {
            if (groupOf(toolGroups, id as ToolKind)) tool.kind = id as ToolKind;
          },
        });
        return true;
      default:
        return runLayerShortcut(action);
    }
  }

  /**
   * Layer, transform and opacity keys: they run what their menu items run,
   * and where the item is disabled they do nothing (AC-16).
   */
  function runLayerShortcut(action: ReturnType<typeof actionFor>): boolean {
    if (action === null || editor.handle === null) return false;
    const active = editor.activeLayer;
    if (OPACITY_ACTIONS.includes(action as OpacityAction)) {
      tool.opacity = opacityPercent(action as OpacityAction);
      return true;
    }
    switch (action) {
      case 'newLayer':
        addLayer();
        return true;
      case 'duplicateLayer':
        duplicateLayer(active);
        return true;
      case 'deleteLayer':
        if (canDeleteLayer(editor.layers, active)) deleteLayer(active);
        return true;
      case 'mergeDown':
        if (canMergeDown(editor.layers, active)) mergeDown(active);
        return true;
      case 'layerAbove':
      case 'layerBelow': {
        const order = treeOrder(editor.layers);
        const at = order.indexOf(active);
        const next = at < 0 ? undefined : order[at + (action === 'layerAbove' ? -1 : 1)];
        if (next !== undefined) selectLayer(next);
        return true;
      }
      case 'flipLayerH':
      case 'flipLayerV':
        layerEdit(true, () => transformLayer(action === 'flipLayerH' ? 'flip_h' : 'flip_v'));
        return true;
      case 'rotateCw':
      case 'rotateCcw':
        rotateCanvas(action === 'rotateCw' ? 'cw90' : 'ccw90');
        return true;
      default:
        return false;
    }
  }

  // The dialogs own Enter / Escape; keep the editor's shortcuts out.
  $effect(() => {
    ui.modalOpen =
      effectOpen !== null ||
      transformDialog !== null ||
      jpegDialogOpen ||
      svgOpen ||
      printOpen ||
      radiusOp !== null ||
      pasteDialog !== null;
  });

  const hasDoc = $derived(editor.handle !== null);

  /** Opens `def` at its defaults; an effect already open is cancelled
   *  first, even the same one (effect-panel AC-05). */
  function openEffect(def: EffectDef): void {
    // The effect previews and applies to the committed pixels (AC-09).
    commitLive();
    if (effectOpen) {
      endEffectRun();
      clearEffectPreview();
    }
    // Colour parameters start at the current colours (outline-effect AC-01).
    effectOpen = new EffectSession(def, {
      foreground: rgbaOf(tool.foreground),
      background: rgbaOf(tool.background),
    });
  }

  /** Layer ▸ Rotate / Zoom… (image-transform-handles-rotate-zoom): the
   *  effect panel, unless the active layer refuses pixel edits (AC-08). */
  function openRotateZoom(): void {
    const def = effectById('rotate_zoom');
    if (def) layerEdit(false, () => openEffect(forMode(def, 'image')));
  }

  function closeEffect(): void {
    endEffectRun();
    clearEffectPreview();
    effectOpen = null;
  }

  // The open panel's jobs in the job worker
  // (docs/specs/performance-background-jobs.md); started at its first
  // preview or apply.
  let fxRun: EffectRun | null = null;

  function effectRunFor(): EffectRun | null {
    if (editor.handle === null) return null;
    fxRun ??= effectRun(
      editor.handle,
      (composite) => {
        effectSucceeded();
        showEffectPreview(composite);
      },
      (err) => effectFailed(err, t('act.preview')),
    );
    return fxRun;
  }

  function endEffectRun(): void {
    fxRun?.close();
    fxRun = null;
  }

  // The notice of the last failed effect run, cleared by the next success.
  let effectFailure: string | null = null;

  function effectSucceeded(): void {
    if (effectFailure !== null && editor.notice === effectFailure) editor.notice = null;
    effectFailure = null;
  }

  /** Shows a failed preview or apply in the notice line; the document is
   *  unchanged (effect-panel AC-16). */
  function effectFailed(err: unknown, action: string): void {
    effectFailure = t('status.failed', {
      action,
      error: err instanceof Error ? err.message : String(err),
    });
    editor.notice = effectFailure;
  }

  /** Applies the open effect once the worker has computed it; a panel
   *  cancelled or replaced meanwhile changes nothing (AC-08). */
  async function applyOpenEffect(session: EffectSession, params: number[]): Promise<void> {
    const run = effectRunFor();
    if (!run) return;
    const label = effectLabel(session.def);
    let region: Uint8Array;
    try {
      region = await run.region(effectJobFor(session.def.id, params));
    } catch (err) {
      // A failed effect keeps the panel open with its values (AC-16).
      if (effectOpen === session) effectFailed(err, label);
      return;
    }
    if (effectOpen !== session) return;
    let changed: boolean;
    try {
      changed = applyEffectRegion(session.def.id, params, region);
    } catch (err) {
      effectFailed(err, label);
      return;
    }
    effectSucceeded();
    // An empty region is an empty selection; a region that changes no
    // pixel adds no step and needs no notice.
    if (!changed && region.length === 0) editor.notice = t('fx.selectionEmpty', { effect: label });
    endEffectRun();
    effectOpen = null;
  }

  // The preview is not part of the document (AC-17): the shell cancels the
  // open effect before its unsaved-changes prompt or a document switch
  // (`cancelActiveEffect`), and a document replaced in place ends it too.
  $effect(() => {
    if (!effectOpen) return;
    activeEffect.cancel = closeEffect;
    return () => {
      if (activeEffect.cancel === closeEffect) activeEffect.cancel = null;
    };
  });
  let effectHandle: number | null = null;
  $effect(() => {
    const handle = editor.handle;
    if (handle !== effectHandle && effectOpen) closeEffect();
    effectHandle = handle;
  });

  // Menu bar (spec §9.2): the same menus, order and labels as Pixel mode;
  // entries Image mode lacks are simply absent.
  const menus: MenuDef[] = $derived([
    {
      id: 'file',
      label: t('menu.file'),
      items: tidy([
        { label: t('menu.new'), shortcut: hint('newDocument'), run: onRequestNew },
        { label: t('menu.open'), shortcut: hint('open'), run: onRequestOpen },
        ...recentMenuItems(recentsAvailable ? recents : null, onOpenRecent),
        droppedImagesMenu(),
        { kind: 'separator' },
        {
          label: t('menu.close'),
          shortcut: hint('closeTab'),
          disabled: !hasDoc || runningSave() !== null,
          run: () => requestCloseTab(),
        },
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
            { label: t('menu.exportSvg'), run: openSvgExport },
          ],
        },
        {
          label: t('menu.print'),
          shortcut: hint('print'),
          disabled: !hasDoc || ui.modalOpen,
          run: () => void openPrint(),
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
          label: t('menu.cut'),
          shortcut: hint('cut'),
          disabled: !hasDoc,
          run: () => runCopy('cut'),
        },
        {
          label: t('menu.copy'),
          shortcut: hint('copy'),
          disabled: !hasDoc,
          run: () => runCopy('copy'),
        },
        { label: t('menu.copyMerged'), disabled: !hasDoc, run: () => runCopy('merged') },
        {
          label: t('menu.paste'),
          shortcut: hint('paste'),
          disabled: !hasDoc,
          run: () => void pasteFromClipboard(),
        },
        {
          label: t('menu.pasteNewImage'),
          shortcut: hint('pasteNewImage'),
          run: () => void pasteAsNewImage(),
        },
        {
          label: t('menu.deleteSelection'),
          shortcut: hint('deleteSelection'),
          disabled: !editor.hasSelection,
          run: () => layerEdit(false, deleteSelection),
        },
        {
          label: t('menu.fillSelection'),
          shortcut: hint('fillSelection'),
          disabled: !hasDoc,
          run: () => layerEdit(false, fillSelection),
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
          label: t('menu.expand'),
          disabled: !editor.hasSelection,
          run: () => openRadius('expand'),
        },
        {
          label: t('menu.contract'),
          disabled: !editor.hasSelection,
          run: () => openRadius('contract'),
        },
        {
          label: t('menu.feather'),
          disabled: !editor.hasSelection,
          run: () => openRadius('feather'),
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
        {
          label: t('menu.rotateCw'),
          shortcut: hint('rotateCw'),
          disabled: !hasDoc,
          run: () => rotateCanvas('cw90'),
        },
        {
          label: t('menu.rotateCcw'),
          shortcut: hint('rotateCcw'),
          disabled: !hasDoc,
          run: () => rotateCanvas('ccw90'),
        },
        { label: t('menu.rotate180'), disabled: !hasDoc, run: () => rotateCanvas('rotate_180') },
        { kind: 'separator' },
        {
          label: t('menu.resizeCanvas'),
          disabled: !hasDoc,
          run: () => (transformDialog = 'resize'),
        },
        { label: t('menu.scaleImage'), disabled: !hasDoc, run: () => (transformDialog = 'scale') },
        { label: t('menu.crop'), disabled: !editor.hasSelection, run: cropToSelection },
        { label: t('menu.autoCrop'), disabled: !hasDoc, run: runAutoCrop },
      ],
    },
    {
      id: 'layer',
      label: t('menu.layer'),
      items: [
        { label: t('menu.newLayer'), shortcut: hint('newLayer'), disabled: !hasDoc, run: addLayer },
        { label: t('menu.newGroup'), disabled: !hasDoc, run: addGroup },
        {
          label: t('menu.duplicateLayer'),
          shortcut: hint('duplicateLayer'),
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
          shortcut: hint('deleteLayer'),
          disabled: !canDeleteLayer(editor.layers, editor.activeLayer),
          run: () => deleteLayer(editor.activeLayer),
        },
        { kind: 'separator' },
        {
          label: t('menu.mergeDown'),
          shortcut: hint('mergeDown'),
          disabled: !canMergeDown(editor.layers, editor.activeLayer),
          run: () => mergeDown(editor.activeLayer),
        },
        {
          label: t('menu.mergeGroup'),
          disabled: editor.layers[editor.activeLayer]?.kind !== 'group',
          run: () => mergeGroup(editor.activeLayer),
        },
        { label: t('menu.mergeVisible'), disabled: !hasDoc, run: mergeVisible },
        { label: t('menu.flatten'), disabled: !hasDoc, run: flattenImage },
        { kind: 'separator' },
        {
          label: t('menu.flipH'),
          shortcut: hint('flipLayerH'),
          disabled: !hasDoc,
          run: () => layerEdit(true, () => transformLayer('flip_h')),
        },
        {
          label: t('menu.flipV'),
          shortcut: hint('flipLayerV'),
          disabled: !hasDoc,
          run: () => layerEdit(true, () => transformLayer('flip_v')),
        },
        {
          label: t('menu.rotateCw'),
          disabled: !hasDoc,
          run: () => layerEdit(true, () => rotateLayer90(false)),
        },
        {
          label: t('menu.rotateCcw'),
          disabled: !hasDoc,
          run: () => layerEdit(true, () => rotateLayer90(true)),
        },
        {
          label: t('menu.rotate180'),
          disabled: !hasDoc,
          run: () => layerEdit(true, () => transformLayer('rotate_180')),
        },
        { label: t('menu.rotateZoom'), disabled: !hasDoc, run: openRotateZoom },
      ],
    },
    ...effectMenus(openEffect, !hasDoc, 'image'),
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
        {
          label: t('menu.rulers'),
          shortcut: hint('toggleRulers'),
          checked: viewAids.showRulers,
          run: () => setViewAids({ showRulers: !viewAids.showRulers }),
        },
        {
          label: t('menu.pixelGrid'),
          shortcut: hint('togglePixelGrid'),
          checked: viewAids.showPixelGrid,
          run: () => setViewAids({ showPixelGrid: !viewAids.showPixelGrid }),
        },
        {
          kind: 'submenu',
          label: t('menu.units'),
          items: UNIT_ITEMS.map(([units, label]) => ({
            label: t(label),
            checked: viewAids.units === units,
            run: () => setViewAids({ units }),
          })),
        },
        { kind: 'separator' },
        { label: t('menu.toggleUi'), shortcut: hint('toggleUi'), run: toggleUi },
        reduceMotionMenuItem(),
        touchGesturesItem(),
        ...imageGenerationItems(),
        { kind: 'separator' },
        languageMenu(),
      ],
    },
    ...helpMenus(openOverview, 'image'),
  ]);
</script>

<svelte:window onkeydown={onKeydown} onpaste={onPaste} />

{#snippet colorsBody()}<ColorsPanel />{/snippet}
{#snippet effectBody()}
  {#if effectOpen}
    {@const session = effectOpen}
    {#key session.token}
      <EffectPanel
        {session}
        hasSelection={editor.hasSelection}
        onPreview={(params) => {
          // `null` means "preview off": show the real composite.
          if (params === null) {
            fxRun?.stopPreview();
            clearEffectPreview();
          } else {
            effectRunFor()?.preview(effectJobFor(session.def.id, params));
          }
        }}
        onApply={(params) => void applyOpenEffect(session, params)}
        onCancel={closeEffect}
      />
    {/key}
  {/if}
{/snippet}
{#snippet layersBody()}<LayersPanel />{/snippet}
{#snippet historyBody()}
  <HistoryPanel
    history={editor.history}
    origin={editor.origin}
    busy={() => gestureControl.inProgress()}
    onJump={jumpHistory}
  />
{/snippet}

<EditorFrame
  mode="image"
  {menus}
  title={editor.handle === null ? t('px.noDocument') : editor.fileName}
  {dirty}
  effectPanel={effectOpen ? { title: effectLabel(effectOpen.def), body: effectBody } : null}
  tabs={tabBar}
  panels={[
    { id: 'colors', title: t('panel.colors'), icon: 'colors', body: colorsBody },
    { id: 'layers', title: t('panel.layers'), icon: 'layers', body: layersBody, grow: true },
    {
      id: 'history',
      title: t('historyPanel.title'),
      icon: 'history',
      body: historyBody,
      defaultCollapsed: true,
    },
  ]}
>
  {#snippet headerHint()}<TourHint mode="image" {active} />{/snippet}
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
      resetKey={active ? editor.handle : null}
    />
  {/snippet}
  {#snippet options()}
    <ToolVariants groups={toolGroups} active={tool.kind} onSelect={(next) => (tool.kind = next)} />
    <ToolOptions />
  {/snippet}
  {#snippet viewport()}
    <ImageViewport />
  {/snippet}
  {#snippet statusbar()}
    <span>{toolLabel}</span>
    <span>·</span>
    {#if statusPointer}
      <span data-testid="status-pointer"
        >{t('status.position', {
          x: unitValue(statusPointer.x),
          y: unitValue(statusPointer.y),
          unit: t(UNIT_SUFFIX[viewAids.units]),
        })}</span
      >
      <span>·</span>
    {/if}
    <span
      >{t('status.size', {
        width: unitValue(editor.width),
        height: unitValue(editor.height),
        unit: t(UNIT_SUFFIX[viewAids.units]),
      })} · sRGB</span
    >
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

{#if radiusOp}
  <RadiusDialog
    title={t(RADIUS_TITLES[radiusOp])}
    value={radiusOp === 'feather' ? 2 : 1}
    onCancel={() => (radiusOp = null)}
    onApply={applyRadius}
  />
{/if}

{#if transformDialog === 'resize'}
  <ResizeDialog
    doc={{ width: editor.width, height: editor.height, dpi: documentDpi() }}
    onApply={(w, h, anchor) => {
      resizeCanvas(w, h, anchor);
      transformDialog = null;
    }}
    onClose={() => (transformDialog = null)}
  />
{:else if transformDialog === 'scale'}
  <ScaleDialog
    doc={{ width: editor.width, height: editor.height, dpi: documentDpi() }}
    onApply={(w, h, interp) => {
      scaleImage(w, h, interp);
      transformDialog = null;
    }}
    onClose={() => (transformDialog = null)}
  />
{/if}

{#if pasteDialog}
  {@const paste = pasteDialog}
  <Modal
    title={t('paste.largerTitle')}
    applyLabel={paste.canExpand ? t('paste.expand') : t('paste.keep')}
    onApply={() => answerPaste(paste.canExpand ? 'expand' : 'keep')}
    secondary={paste.canExpand
      ? { label: t('paste.keep'), onClick: () => answerPaste('keep') }
      : undefined}
    onClose={() => answerPaste(null)}
  >
    <p class="mb-4 text-pc-muted">
      {t(paste.canExpand ? 'paste.larger' : 'paste.tooLarge', { w: paste.width, h: paste.height })}
    </p>
  </Modal>
{/if}

<SaveDialogs />
{#if printOpen}
  <PrintDialog onPrint={(p) => void runPrint(p)} onClose={() => (printOpen = false)} />
{/if}

{#if svgOpen}
  <SvgExportDialog
    defaultMode="color"
    trace={traceSvg}
    onExport={(svg) => {
      closeSvg();
      exportSvg(svg).catch((e) => (loadError = t('img.exportFailed', { error: String(e) })));
    }}
    onCancel={closeSvg}
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
