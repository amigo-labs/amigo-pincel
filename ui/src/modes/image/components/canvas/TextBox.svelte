<script lang="ts">
  // The live text's box, caret, selection and move handle
  // (docs/specs/image-live-text.md AC-01, AC-05, AC-07), drawn in viewport
  // space over the layer the core draws the text into, and the hidden
  // field that takes the text's keyboard input (AC-23). Pointer input goes
  // to the stage (`tools/pointer.ts`); this layer takes no pointer events.
  import { onMount } from 'svelte';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { isTypingTarget } from '../../../../lib/shell/shortcuts';
  import { view } from '../../stores/editor.svelte';
  import {
    cancelText,
    commitText,
    deleteChar,
    deleteSelection,
    moveCaret,
    selectAllText,
    selectedText,
    textEdit,
    textObject,
    textView,
    typeText,
  } from '../../core/live-text.svelte';
  import { caretRect, charsOf, selectionRects, type CaretMove } from '../../tools/text-caret';

  /** Edge of the move handle square, CSS px. */
  const SIZE = 9;

  let input = $state<HTMLTextAreaElement | null>(null);
  let pointerDown = false;

  const v = $derived(textView());
  const hasObject = $derived(textObject() !== null);
  const sx = (x: number): number => view.panX + x * view.zoom;
  const sy = (y: number): number => view.panY + y * view.zoom;
  const caret = $derived.by(() => {
    if (v === null) return null;
    const c = caretRect(charsOf(v.text), v.layout, textEdit.caret);
    return { x: sx(c.x), y: sy(c.top), h: c.height * view.zoom };
  });
  const marks = $derived(
    v === null
      ? []
      : selectionRects(charsOf(v.text), v.layout, textEdit).map((r) => ({
          x: sx(r.x),
          y: sy(r.y),
          w: r.w * view.zoom,
          h: r.h * view.zoom,
        })),
  );

  /** Fields the user types into keep the focus a request would take. */
  function holdsFocus(el: Element | null): boolean {
    if (!(el instanceof HTMLElement) || el === input) return false;
    if (el instanceof HTMLSelectElement) return false;
    return isTypingTarget(el);
  }

  /** Moves focus into the text when it is wanted and nothing holds it. */
  function tryFocus(): void {
    if (!textEdit.focusWanted || input === null || pointerDown) return;
    if (holdsFocus(document.activeElement)) return;
    textEdit.focusWanted = false;
    input.focus({ preventScroll: true });
  }

  $effect(() => {
    void textEdit.focusSeq;
    if (input !== null) tryFocus();
  });

  onMount(() => {
    const down = (): void => {
      pointerDown = true;
    };
    const up = (): void => {
      pointerDown = false;
      queueMicrotask(tryFocus);
    };
    const focusChange = (): void => queueMicrotask(tryFocus);
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('pointerup', up, true);
    window.addEventListener('pointercancel', up, true);
    window.addEventListener('focusin', focusChange);
    window.addEventListener('focusout', focusChange);
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('pointerup', up, true);
      window.removeEventListener('pointercancel', up, true);
      window.removeEventListener('focusin', focusChange);
      window.removeEventListener('focusout', focusChange);
    };
  });

  const MOVES: Record<string, CaretMove> = {
    ArrowLeft: 'left',
    ArrowRight: 'right',
    ArrowUp: 'up',
    ArrowDown: 'down',
    Home: 'home',
    End: 'end',
  };

  /**
   * Keys in the text (AC-23): text keys act on the text only and stop here;
   * the other Ctrl / ⌘ / Alt chords (Undo, Redo, Save, tab keys, …) go on
   * to the editor's shortcuts.
   */
  function onKeydown(e: KeyboardEvent): void {
    if (e.isComposing) return;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const own = (): void => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (e.key === 'Escape') {
      own();
      cancelText();
      return;
    }
    if (e.key === 'Enter' && !e.altKey) {
      own();
      if (mod) commitText();
      else typeText('\n');
      return;
    }
    if (e.key === 'Tab') {
      own();
      return;
    }
    const move = MOVES[e.key];
    if (move && !e.altKey) {
      own();
      moveCaret(move, e.shiftKey);
      return;
    }
    if ((e.key === 'Backspace' || e.key === 'Delete') && !e.altKey) {
      own();
      deleteChar(e.key === 'Delete');
      return;
    }
    if (mod && !e.altKey && !e.shiftKey) {
      if (key === 'a') {
        own();
        selectAllText();
        return;
      }
      if (key === 'c' || key === 'x') {
        own();
        const text = selectedText();
        if (text.length === 0) return;
        void navigator.clipboard?.writeText(text).catch(() => {});
        if (key === 'x') deleteSelection();
        return;
      }
      if (key === 'v') {
        // The browser's paste event brings the clipboard's text (onPaste).
        e.stopPropagation();
        return;
      }
    }
    // Plain keys type (through the input event) and never reach the
    // editor's tool keys or the Space pan.
    if (!mod && !e.altKey) e.stopPropagation();
  }

  function onInput(e: Event): void {
    if (input === null || (e instanceof InputEvent && e.isComposing)) return;
    const text = input.value;
    input.value = '';
    typeText(text);
  }

  function onPaste(e: ClipboardEvent): void {
    e.preventDefault();
    typeText(e.clipboardData?.getData('text/plain') ?? '');
  }
</script>

{#if v !== null}
  <div data-text-overlay class="pointer-events-none absolute inset-0 overflow-hidden">
    <div
      data-text-box
      class="absolute border border-dashed border-pc-accent"
      style="left: {sx(v.box[0]) - 1}px; top: {sy(v.box[1]) - 1}px; width: {(v.box[2] - v.box[0]) *
        view.zoom +
        2}px; height: {(v.box[3] - v.box[1]) * view.zoom + 2}px;"
    ></div>
    {#each marks as m, i (i)}
      <div
        data-text-selection
        class="absolute bg-pc-accent/35"
        style="left: {m.x}px; top: {m.y}px; width: {m.w}px; height: {m.h}px;"
      ></div>
    {/each}
    {#if caret}
      <div
        data-text-caret
        class="absolute w-[2px] bg-pc-accent shadow-[0_0_0_1px_rgba(255,255,255,0.7)]"
        style="left: {caret.x - 1}px; top: {caret.y}px; height: {caret.h}px;"
      ></div>
    {/if}
    {#if hasObject}
      <div
        data-text-handle
        class="absolute rounded-full border border-pc-bg bg-pc-accent shadow-[0_0_0_1px_rgba(0,0,0,0.6)]"
        style="left: {sx(v.box[2]) - SIZE / 2}px; top: {sy(v.box[3]) -
          SIZE / 2}px; width: {SIZE}px; height: {SIZE}px;"
      ></div>
    {/if}
    <textarea
      bind:this={input}
      data-live-text
      aria-label={t('img.liveText')}
      class="absolute h-4 w-px resize-none overflow-hidden border-0 bg-transparent p-0 opacity-0 outline-none"
      style="left: {caret?.x ?? 0}px; top: {caret?.y ?? 0}px;"
      spellcheck="false"
      autocomplete="off"
      onkeydown={onKeydown}
      oninput={onInput}
      oncompositionend={onInput}
      onpaste={onPaste}
    ></textarea>
  </div>
{/if}
