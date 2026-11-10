<script lang="ts">
  // "New document" dialog of the shell (File ▸ New… in an editor): choose
  // the editor mode and the canvas size. The start screen offers the same
  // inline (StartScreen.svelte); limits and defaults are in `new-doc.ts`.
  // The dialog says what Create will make before it makes it: a result
  // line from `newDocParams` (the clamp Create applies) and a note under a
  // field whose value that clamp would change.
  import type { EditorMode, NewDocParams } from './types';
  import type { MessageKey } from '../i18n/en';
  import { tick } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import {
    NEW_DOC_LIMITS as LIMITS,
    clipboardDocSize,
    newDocParams,
    prefillKeepingSelection,
  } from './new-doc';
  import { autoNew, setAutoNew } from './auto-new.svelte';
  import { MAX_CANVAS_AREA } from '../../modes/image/canvas-limits';

  let {
    initialMode,
    onCreate,
    onCancel,
  }: {
    initialMode: EditorMode;
    onCreate: (params: NewDocParams) => void;
    onCancel: () => void;
  } = $props();

  const uid = $props.id();
  const MODES: readonly EditorMode[] = ['pixel', 'image'];

  // The shell remounts the dialog per open, so the initial props are the
  // only ones this instance sees.
  // svelte-ignore state_referenced_locally
  let mode = $state<EditorMode>(initialMode);
  // A cleared number field binds `null`.
  // svelte-ignore state_referenced_locally
  let width = $state<number | null>(LIMITS[initialMode].width);
  // svelte-ignore state_referenced_locally
  let height = $state<number | null>(LIMITS[initialMode].height);
  // `validity.badInput`: the field holds text that is no number ("1e").
  let widthBad = $state(false);
  let heightBad = $state(false);

  let form: HTMLFormElement | undefined = $state();
  const radios: Partial<Record<EditorMode, HTMLButtonElement>> = {};
  let widthInput: HTMLInputElement | undefined = $state();

  /** What Create makes now — the same clamp as on submit. */
  const result = $derived(newDocParams(mode, width ?? NaN, height ?? NaN));

  type Note = { key: MessageKey; params: Record<string, string | number> };

  /** The edge `newDocParams` uses for `v`, before the image area cap. */
  function edgeOf(m: EditorMode, v: number | null, fallback: number): number {
    if (v === null || !Number.isFinite(v)) return fallback;
    const r = m === 'image' ? Math.round(v) : Math.floor(v);
    return Math.max(1, Math.min(LIMITS[m].max, r));
  }

  /** Why the clamp changes this field's value; `n` is the edge Create uses. */
  function noteFor(m: EditorMode, v: number | null, bad: boolean, n: number): Note | null {
    if (bad) return { key: 'newDoc.notNumber', params: { n } };
    if (v === null || !Number.isFinite(v)) return { key: 'newDoc.empty', params: { n } };
    if (v < 1) return { key: 'newDoc.tooSmall', params: { n } };
    if (v > LIMITS[m].max) return { key: 'newDoc.tooLarge', params: { max: LIMITS[m].max, n } };
    if (!Number.isInteger(v)) return { key: 'newDoc.notWhole', params: { n } };
    return null;
  }

  const widthNote = $derived(noteFor(mode, width, widthBad, result.width));
  const heightNote = $derived(noteFor(mode, height, heightBad, result.height));
  // Both edges fit, their product does not: the image core scales both.
  const areaNote = $derived.by((): Note | null => {
    const w = edgeOf(mode, width, LIMITS[mode].width);
    const h = edgeOf(mode, height, LIMITS[mode].height);
    if (result.width === w && result.height === h) return null;
    const side = Math.round(Math.sqrt(MAX_CANVAS_AREA));
    return {
      key: 'newDoc.tooManyPixels',
      params: { limit: `${side} × ${side}`, w: result.width, h: result.height },
    };
  });

  const startup = $derived(autoNew.params?.mode === mode);

  function pickMode(next: EditorMode) {
    if (next === mode) return;
    mode = next;
    width = LIMITS[next].width;
    height = LIMITS[next].height;
    widthBad = false;
    heightBad = false;
    prefillFromClipboard();
  }

  // A clipboard image sets the size, unless the user typed one meanwhile.
  function prefillFromClipboard() {
    const [m, w, h] = [mode, width, height];
    void clipboardDocSize(m).then((p) => {
      if (p && mode === m && width === w && height === h) {
        prefillKeepingSelection(() => {
          width = p.width;
          height = p.height;
        });
      }
    });
  }
  prefillFromClipboard();

  function submit() {
    const params = newDocParams(mode, width ?? NaN, height ?? NaN);
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
      return;
    }
    // Enter creates from any field, the type radios and the checkbox;
    // number fields already submit, buttons keep their own Enter.
    const el = e.target;
    if (
      e.key === 'Enter' &&
      ((el instanceof HTMLInputElement && el.type === 'checkbox') ||
        (el instanceof HTMLElement && el.getAttribute('role') === 'radio'))
    ) {
      e.preventDefault();
      form?.requestSubmit();
      return;
    }
    // aria-modal: Tab wraps inside the dialog instead of reaching the editor.
    if (e.key === 'Tab' && form) {
      const stops = Array.from(
        form.querySelectorAll<HTMLElement>('input, button:not([tabindex="-1"])'),
      );
      const first = stops[0];
      const last = stops[stops.length - 1];
      // Focus on the backdrop (a click there) wraps too.
      const at = document.activeElement;
      const outside = !form.contains(at);
      if (outside || (e.shiftKey ? at === first : at === last)) {
        e.preventDefault();
        (e.shiftKey ? last : first)?.focus();
      }
    }
  }

  // One tab stop for the type: arrows move and select (WAI-ARIA radio group).
  function onRadioKeydown(e: KeyboardEvent) {
    const i = MODES.indexOf(mode);
    let next: EditorMode | undefined;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = MODES[(i + 1) % MODES.length];
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp')
      next = MODES[(i + MODES.length - 1) % MODES.length];
    else if (e.key === 'Home') next = MODES[0];
    else if (e.key === 'End') next = MODES[MODES.length - 1];
    if (!next) return;
    e.preventDefault();
    pickMode(next);
    radios[next]?.focus();
  }

  // Focus starts in Width with its value selected: the first digit
  // replaces the default instead of appending to it.
  function focusFirst(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  // The first click or tap into a size field selects its value, so the
  // digits that follow replace it; a later click places the caret.
  function replaceOnFirstClick(el: HTMLInputElement) {
    let fresh = true;
    const onFocus = () => (fresh = true);
    const onInput = () => (fresh = false);
    const onClick = () => {
      if (!fresh) return;
      fresh = false;
      el.select();
    };
    el.addEventListener('focus', onFocus);
    el.addEventListener('input', onInput);
    el.addEventListener('click', onClick);
    return () => {
      el.removeEventListener('focus', onFocus);
      el.removeEventListener('input', onInput);
      el.removeEventListener('click', onClick);
    };
  }
</script>

{#snippet modeRadio(m: EditorMode, label: string, hint: string)}
  <button
    bind:this={radios[m]}
    type="button"
    class="mode-btn"
    class:mode-btn-active={mode === m}
    role="radio"
    aria-checked={mode === m}
    tabindex={mode === m ? 0 : -1}
    onclick={(e) => {
      pickMode(m);
      // A click or tap hands over to the size; a keyboard Space stays here.
      // After the flush, or the selection covers the old mode's value.
      if (e.detail > 0) void tick().then(() => widthInput && focusFirst(widthInput));
    }}
    onkeydown={onRadioKeydown}
  >
    <span class="font-semibold">{label}</span>
    <span class="text-xs text-pc-muted">{hint}</span>
  </button>
{/snippet}

{#snippet note(id: string, n: Note | null)}
  {#if n}
    <p {id} class="mt-1 text-xs leading-snug text-amber-400">{t(n.key, n.params)}</p>
  {/if}
{/snippet}

<div
  class="fixed inset-0 z-30 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('newDoc.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <!-- novalidate: an out-of-range size is clamped by `newDocParams`, not
       refused by the browser's min / max check (app-shell AC-06); the
       notes below say what the clamp will use. -->
  <form
    bind:this={form}
    class="max-h-full w-80 max-w-full overflow-y-auto rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    novalidate
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <p class="font-semibold">{t('newDoc.title')}</p>

    <div class="mt-3 grid grid-cols-2 gap-2" role="radiogroup" aria-label={t('newDoc.type')}>
      {@render modeRadio('pixel', t('newDoc.pixel'), '.aseprite')}
      {@render modeRadio('image', t('newDoc.image'), 'PNG / JPEG / WebP')}
    </div>

    <div class="mt-3 grid grid-cols-[1fr_auto_1fr] items-start gap-x-2">
      <div>
        <label class="flex items-center gap-1 text-xs text-pc-muted">
          {t('newDoc.w')}
          <input
            {@attach focusFirst}
            bind:this={widthInput}
            {@attach replaceOnFirstClick}
            type="number"
            min="1"
            max={LIMITS[mode].max}
            step="1"
            inputmode="numeric"
            bind:value={width}
            oninput={(e) => (widthBad = e.currentTarget.validity.badInput)}
            class="h-8 w-full min-w-0 rounded border border-pc-border bg-pc-bg px-1.5 text-pc-text"
            aria-label={t('common.width')}
            aria-describedby={widthNote ? `${uid}-wnote` : undefined}
          />
        </label>
        {@render note(`${uid}-wnote`, widthNote)}
      </div>
      <span class="flex h-8 items-center text-pc-faint" aria-hidden="true">×</span>
      <div>
        <label class="flex items-center gap-1 text-xs text-pc-muted">
          {t('newDoc.h')}
          <input
            {@attach replaceOnFirstClick}
            type="number"
            min="1"
            max={LIMITS[mode].max}
            step="1"
            inputmode="numeric"
            bind:value={height}
            oninput={(e) => (heightBad = e.currentTarget.validity.badInput)}
            class="h-8 w-full min-w-0 rounded border border-pc-border bg-pc-bg px-1.5 text-pc-text"
            aria-label={t('common.height')}
            aria-describedby={heightNote ? `${uid}-hnote` : undefined}
          />
          <span class="text-pc-faint">px</span>
        </label>
        {@render note(`${uid}-hnote`, heightNote)}
      </div>
    </div>
    {@render note(`${uid}-anote`, areaNote)}

    <label class="mt-3 flex min-h-8 items-center gap-2 text-xs text-pc-muted">
      <input
        type="checkbox"
        checked={startup}
        onchange={(e) =>
          setAutoNew(
            e.currentTarget.checked ? newDocParams(mode, width ?? NaN, height ?? NaN) : null,
          )}
      />
      {t('newDoc.autoOpen')}
    </label>

    <div id="{uid}-result" class="mt-3 border-t border-pc-border pt-3 text-xs" aria-live="polite">
      <p class="text-pc-text">
        {t(mode === 'pixel' ? 'newDoc.resultPixel' : 'newDoc.resultImage', {
          w: result.width,
          h: result.height,
        })}
      </p>
      {#if startup}
        <p class="mt-0.5 text-pc-muted">{t('newDoc.resultStartup')}</p>
      {/if}
    </div>

    <div class="mt-3 flex justify-end gap-2">
      <button type="button" class="panel-btn min-h-8" onclick={onCancel}
        >{t('common.cancel')}</button
      >
      <button
        type="submit"
        class="panel-btn panel-btn-primary min-h-8"
        aria-describedby="{uid}-result">{t('common.create')}</button
      >
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
    border: 1px solid var(--color-pc-border);
    padding: 0.5rem 0.625rem;
    text-align: left;
  }
  .mode-btn:hover {
    background-color: var(--color-pc-hover);
  }
  .mode-btn-active {
    border-color: var(--color-pc-accent);
    background-color: var(--color-pc-selected);
  }
</style>
