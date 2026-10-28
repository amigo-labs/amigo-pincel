<script lang="ts">
  // "New document" dialog of the shell (File ▸ New… in an editor): choose
  // the editor mode and the canvas size. The start screen offers the same
  // inline (StartScreen.svelte); limits and defaults are in `new-doc.ts`.
  import type { EditorMode, NewDocParams } from './types';
  import { t } from '../i18n/index.svelte';
  import { NEW_DOC_LIMITS as LIMITS, clipboardDocSize, newDocParams } from './new-doc';
  import { autoNew, setAutoNew } from './auto-new.svelte';

  let {
    initialMode,
    onCreate,
    onCancel,
  }: {
    initialMode: EditorMode;
    onCreate: (params: NewDocParams) => void;
    onCancel: () => void;
  } = $props();

  // The shell remounts the dialog per open, so the initial props are the
  // only ones this instance sees.
  // svelte-ignore state_referenced_locally
  let mode = $state<EditorMode>(initialMode);
  // svelte-ignore state_referenced_locally
  let width = $state(LIMITS[initialMode].width);
  // svelte-ignore state_referenced_locally
  let height = $state(LIMITS[initialMode].height);

  function pickMode(next: EditorMode) {
    if (next === mode) return;
    mode = next;
    width = LIMITS[next].width;
    height = LIMITS[next].height;
    prefillFromClipboard();
  }

  // A clipboard image sets the size, unless the user typed one meanwhile.
  function prefillFromClipboard() {
    const [m, w, h] = [mode, width, height];
    void clipboardDocSize(m).then((p) => {
      if (p && mode === m && width === w && height === h) {
        width = p.width;
        height = p.height;
      }
    });
  }
  prefillFromClipboard();

  function submit() {
    const params = newDocParams(mode, width, height);
    if (autoNew.params?.mode === mode) setAutoNew(params);
    onCreate(params);
  }

  // Keys typed in the dialog must not reach the mounted editor's
  // window-level shortcut map (Space pans, B/E pick tools, …).
  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  class="fixed inset-0 z-30 flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={t('newDoc.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form
    class="w-80 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <p class="font-semibold">{t('newDoc.title')}</p>

    <div class="mt-3 grid grid-cols-2 gap-2" role="radiogroup" aria-label={t('newDoc.type')}>
      <button
        type="button"
        class="mode-btn"
        class:mode-btn-active={mode === 'pixel'}
        role="radio"
        aria-checked={mode === 'pixel'}
        onclick={() => pickMode('pixel')}
      >
        <span class="font-semibold">{t('newDoc.pixel')}</span>
        <span class="text-xs text-pc-muted">.aseprite</span>
      </button>
      <button
        type="button"
        class="mode-btn"
        class:mode-btn-active={mode === 'image'}
        role="radio"
        aria-checked={mode === 'image'}
        onclick={() => pickMode('image')}
      >
        <span class="font-semibold">{t('newDoc.image')}</span>
        <span class="text-xs text-pc-muted">PNG / JPEG / WebP</span>
      </button>
    </div>

    <div class="mt-3 flex items-center gap-2">
      <label class="flex items-center gap-1 text-xs text-pc-muted">
        {t('newDoc.w')}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="number"
          min="1"
          max={LIMITS[mode].max}
          step="1"
          inputmode="numeric"
          bind:value={width}
          autofocus
          class="w-20 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-pc-text"
          aria-label={t('common.width')}
        />
      </label>
      <span class="text-pc-faint">×</span>
      <label class="flex items-center gap-1 text-xs text-pc-muted">
        {t('newDoc.h')}
        <input
          type="number"
          min="1"
          max={LIMITS[mode].max}
          step="1"
          inputmode="numeric"
          bind:value={height}
          class="w-20 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-pc-text"
          aria-label={t('common.height')}
        />
      </label>
      <span class="text-xs text-pc-faint">px</span>
    </div>

    <label class="mt-3 flex items-center gap-2 text-xs text-pc-muted">
      <input
        type="checkbox"
        checked={autoNew.params?.mode === mode}
        onchange={(e) =>
          setAutoNew(e.currentTarget.checked ? newDocParams(mode, width, height) : null)}
      />
      {t('newDoc.autoOpen')}
    </label>

    <div class="mt-4 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary">{t('common.create')}</button>
    </div>
  </form>
</div>

<style>
  .mode-btn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.125rem;
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.5rem 0.625rem;
    text-align: left;
  }
  .mode-btn:hover {
    background-color: rgb(38 38 38);
  }
  .mode-btn-active {
    border-color: rgb(163 163 163);
    background-color: rgb(38 38 38);
  }
</style>
