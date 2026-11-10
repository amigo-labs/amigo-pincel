<script lang="ts">
  // One tooltip for the whole app (spec §9.2). Every element with a
  // `title` gets it: after a short rest of the mouse or pen, or on
  // keyboard focus, the layer shows the text in the theme, with the
  // shortcut as a keycap (`data-shortcut`) and an optional second line
  // (`data-tip-hint`). While it shows, the element's `title` is parked so
  // the OS tooltip does not come up as well; a press, a key, a scroll or
  // leaving the element puts it back. Touch gets no tooltip: a long press
  // there opens menus.
  import { onMount, tick } from 'svelte';
  import { placeTip, splitShortcut, type TipSide } from './tooltip';

  /** Rest before the tooltip shows; shorter than the OS tooltip's delay. */
  const SHOW_MS = 400;

  interface Tip {
    label: string;
    key: string | null;
    hint: string | null;
  }
  let tip = $state<Tip | null>(null);
  let pos = $state<{ left: number; top: number } | null>(null);
  let tipEl = $state<HTMLElement | null>(null);

  /** Element the pointer or focus is on; stays set after a dismissal so it does not re-show. */
  let current: HTMLElement | null = null;
  /** Element whose `title` is parked while its tooltip shows. */
  let host: HTMLElement | null = null;
  let parked = '';
  let lentLabel = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function hide(): void {
    if (timer) clearTimeout(timer);
    timer = null;
    if (host) {
      // A re-render may have written a new title meanwhile: keep that one.
      if (!host.hasAttribute('title')) host.setAttribute('title', parked);
      if (lentLabel) host.removeAttribute('aria-label');
      host = null;
    }
    tip = null;
    pos = null;
  }

  async function show(el: HTMLElement): Promise<void> {
    timer = null;
    const text = el.getAttribute('title');
    if (!text || !el.isConnected) return;
    const { label, key } = splitShortcut(text, el.dataset.shortcut);
    host = el;
    parked = text;
    // The title may be the element's only accessible name: lend it one.
    lentLabel = !el.hasAttribute('aria-label') && !el.hasAttribute('aria-labelledby');
    if (lentLabel) el.setAttribute('aria-label', text);
    el.removeAttribute('title');
    tip = { label, key, hint: el.dataset.tipHint ?? null };
    await tick();
    if (host !== el || !tipEl) return;
    const side: TipSide = el.dataset.tipSide === 'right' ? 'right' : 'below';
    pos = placeTip(
      el.getBoundingClientRect(),
      tipEl.offsetWidth,
      tipEl.offsetHeight,
      { width: window.innerWidth, height: window.innerHeight },
      side,
    );
  }

  function enter(el: HTMLElement | null): void {
    if (el === current) return;
    hide();
    current = el;
    if (el) timer = setTimeout(() => void show(el), SHOW_MS);
  }

  function titled(target: EventTarget | null): HTMLElement | null {
    if (!(target instanceof Element)) return null;
    if (host?.contains(target)) return host;
    return target.closest<HTMLElement>('[title]');
  }

  onMount(() => {
    const onOver = (e: PointerEvent): void => {
      if (e.pointerType === 'touch') return;
      enter(titled(e.target));
    };
    const onLeave = (): void => enter(null);
    const onFocusIn = (e: FocusEvent): void => {
      const el = titled(e.target);
      if (el && el === e.target && el.matches(':focus-visible')) enter(el);
    };
    // Dismissed, not left: `current` stays, so it does not show again
    // until the pointer or the focus has been somewhere else.
    const dismiss = (): void => hide();
    const opts = { capture: true, passive: true } as const;
    document.addEventListener('pointerover', onOver, opts);
    document.documentElement.addEventListener('pointerleave', onLeave);
    document.addEventListener('focusin', onFocusIn, opts);
    document.addEventListener('focusout', onLeave, opts);
    document.addEventListener('pointerdown', dismiss, opts);
    document.addEventListener('keydown', dismiss, opts);
    document.addEventListener('wheel', dismiss, opts);
    document.addEventListener('scroll', dismiss, opts);
    window.addEventListener('blur', onLeave);
    return () => {
      hide();
      document.removeEventListener('pointerover', onOver, opts);
      document.documentElement.removeEventListener('pointerleave', onLeave);
      document.removeEventListener('focusin', onFocusIn, opts);
      document.removeEventListener('focusout', onLeave, opts);
      document.removeEventListener('pointerdown', dismiss, opts);
      document.removeEventListener('keydown', dismiss, opts);
      document.removeEventListener('wheel', dismiss, opts);
      document.removeEventListener('scroll', dismiss, opts);
      window.removeEventListener('blur', onLeave);
    };
  });
</script>

{#if tip}
  <div
    bind:this={tipEl}
    class="pointer-events-none fixed z-[100] max-w-64 rounded-md border border-pc-border bg-pc-bg px-2 py-1.5 text-xs text-pc-text shadow-lg"
    style:left="{pos?.left ?? 0}px"
    style:top="{pos?.top ?? 0}px"
    style:visibility={pos ? 'visible' : 'hidden'}
    data-tooltip
    aria-hidden="true"
  >
    <div class="flex items-center gap-2">
      <span class="whitespace-pre-line">{tip.label}</span>
      {#if tip.key}
        <kbd class="keycap">{tip.key}</kbd>
      {/if}
    </div>
    {#if tip.hint}
      <div class="mt-0.5 text-[11px] text-pc-muted">{tip.hint}</div>
    {/if}
  </div>
{/if}
